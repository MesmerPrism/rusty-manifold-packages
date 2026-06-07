"""Hand-animation package checks for the Matter mesh/SDF/particle bridge."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Protocol


ID_RE = re.compile(
    r"^[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?)*$"
)

REQUIRED_MODULES = {
    "module.mesh.sdf_builder",
    "module.mesh.dynamic_collider",
    "module.particle.simulator",
}

REQUIRED_COMMANDS = {
    "command.matter.mesh.sdf.build",
    "command.matter.mesh.collider.update",
    "command.matter.particle.simulate.fixed_step",
    "command.matter.particle.apply_sdf_field",
    "command.matter.particle.export_render_payload",
    "command.matter.particle.diagnostics.snapshot",
}

REQUIRED_STREAM_SCHEMAS = {
    "stream.hand.validation_mesh": "rusty.matter.hand.validation_mesh_frame.v1",
    "stream.mesh.coordinate_map": "rusty.matter.mesh.coordinate_map.v1",
    "stream.matter.sdf_grid": "rusty.matter.sdf.packed_grid.v1",
    "stream.matter.dynamic_collider_update": "rusty.matter.mesh.dynamic_collider_update.v1",
    "stream.matter.dynamic_collider_contact": "rusty.matter.mesh.dynamic_collider_contact.v1",
    "stream.matter.particle_set": "rusty.matter.particle.set.v1",
    "stream.matter.particle_diagnostics": "rusty.matter.particle.simulation_diagnostics.v1",
    "stream.matter.particle_render_payload": "rusty.matter.particle.render_payload.v1",
}

REQUIRED_ARTIFACT_SCHEMAS = set(REQUIRED_STREAM_SCHEMAS.values())

REQUIRED_SCORECARD_CHECKS = {
    "check.hand_animation.matter_bridge",
    "check.hand_animation.artifact_references",
    "check.hand_animation.optics_boundary",
}


class PackageLike(Protocol):
    root: Path
    manifest: dict[str, Any]
    modules: list[dict[str, Any]]
    streams: list[dict[str, Any]]
    commands: list[dict[str, Any]]
    graphs: list[dict[str, Any]]
    deployments: list[dict[str, Any]]
    runtime_states: list[dict[str, Any]]
    scorecards: list[dict[str, Any]]


def check_hand_animation_matter_bridge(
    package: PackageLike, ids: dict[str, set[str]]
) -> tuple[bool, str, str]:
    if package.manifest.get("package_id") != "package.hand_animation":
        return True, "not a hand-animation package", ""

    errors: list[str] = []
    modules_by_id = {module["module_id"]: module for module in package.modules}
    streams_by_id = {stream["stream_id"]: stream for stream in package.streams}

    errors += [f"module:{item}" for item in sorted(REQUIRED_MODULES - ids["modules"])]
    errors += [f"command:{item}" for item in sorted(REQUIRED_COMMANDS - ids["commands"])]
    errors += [
        f"stream:{stream_id}"
        for stream_id in sorted(set(REQUIRED_STREAM_SCHEMAS) - ids["streams"])
    ]

    for stream_id, schema_id in REQUIRED_STREAM_SCHEMAS.items():
        stream = streams_by_id.get(stream_id)
        if stream is not None and stream.get("sample_schema") != schema_id:
            errors.append(f"{stream_id}:sample_schema")

    for module_id in REQUIRED_MODULES:
        module = modules_by_id.get(module_id)
        if module is None:
            continue
        if module.get("module_kind") != "processor":
            errors.append(f"{module_id}:module_kind")
        for backend_id in ("backend.synthetic", "backend.replay", "backend.desktop_host"):
            if backend_id not in module.get("platform_support", []):
                errors.append(f"{module_id}:backend:{backend_id}")

    graph_modules = {
        node.get("module_id")
        for graph in package.graphs
        for node in graph.get("nodes", [])
    }
    errors += [
        f"graph_module:{module_id}"
        for module_id in sorted(REQUIRED_MODULES - graph_modules)
    ]

    runtime_modules = {state.get("module_id") for state in package.runtime_states}
    errors += [
        f"runtime:{module_id}"
        for module_id in sorted(REQUIRED_MODULES - runtime_modules)
    ]

    deployment_modules = {
        module_id
        for deployment in package.deployments
        for module_id in deployment.get("selected_modules", [])
    }
    errors += [
        f"deployment:{module_id}"
        for module_id in sorted(REQUIRED_MODULES - deployment_modules)
    ]

    present_scorecard_checks = {
        check.get("check_id")
        for scorecard in package.scorecards
        for check in scorecard.get("checks", [])
    }
    errors += [
        f"scorecard:{check_id}"
        for check_id in sorted(REQUIRED_SCORECARD_CHECKS - present_scorecard_checks)
    ]

    flows = _read_artifact_flows(package.root)
    if not flows:
        errors.append("artifact_flow:missing")
    for flow in flows:
        errors += _validate_artifact_flow(flow, ids)

    ok = not errors
    evidence = (
        "hand-animation exposes Matter SDF, dynamic-collider, coordinate-map, "
        "particle, diagnostics, and render-neutral payloads through package "
        "descriptors and artifact references"
    )
    return ok, evidence, f"hand-animation Matter bridge issues: {sorted(set(errors))}"


def _read_artifact_flows(package_root: Path) -> list[dict[str, Any]]:
    flow_dir = package_root / "fixtures" / "valid"
    flows: list[dict[str, Any]] = []
    for path in sorted(flow_dir.glob("matter-artifact-flow-*.json")):
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(handle)
        if isinstance(value, dict):
            flows.append(value)
    return flows


def _validate_artifact_flow(flow: dict[str, Any], ids: dict[str, set[str]]) -> list[str]:
    errors: list[str] = []
    if flow.get("$schema") != "rusty.manifold.package.artifact_flow.v1":
        errors.append("artifact_flow:schema")
    if flow.get("package_id") != "package.hand_animation":
        errors.append("artifact_flow:package_id")
    if not ID_RE.match(str(flow.get("flow_id", ""))):
        errors.append("artifact_flow:flow_id")

    artifacts = flow.get("artifacts", [])
    if not isinstance(artifacts, list):
        return ["artifact_flow:artifacts"]

    artifact_ids: set[str] = set()
    artifact_schemas: set[str] = set()
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            errors.append("artifact:not_object")
            continue
        artifact_id = str(artifact.get("artifact_id", ""))
        schema_id = str(artifact.get("schema_id", ""))
        artifact_ids.add(artifact_id)
        artifact_schemas.add(schema_id)
        if not ID_RE.match(artifact_id):
            errors.append(f"{artifact_id}:artifact_id")
        if not schema_id.startswith("rusty.matter."):
            errors.append(f"{artifact_id}:schema_id")
        if not str(artifact.get("artifact_uri", "")).startswith("matter://fixtures/"):
            errors.append(f"{artifact_id}:artifact_uri")
        stream_id = artifact.get("stream_id")
        if stream_id not in ids["streams"]:
            errors.append(f"{artifact_id}:stream_id")
        if "payload" in artifact or "data" in artifact or "samples" in artifact:
            errors.append(f"{artifact_id}:inline_payload")

    errors += [
        f"artifact_schema:{schema_id}"
        for schema_id in sorted(REQUIRED_ARTIFACT_SCHEMAS - artifact_schemas)
    ]

    steps = flow.get("steps", [])
    if not isinstance(steps, list):
        return errors + ["artifact_flow:steps"]

    step_modules: set[str] = set()
    for step in steps:
        if not isinstance(step, dict):
            errors.append("step:not_object")
            continue
        step_id = str(step.get("step_id", ""))
        module_id = str(step.get("module_id", ""))
        command_id = str(step.get("command_id", ""))
        step_modules.add(module_id)
        if not ID_RE.match(step_id):
            errors.append(f"{step_id}:step_id")
        if module_id not in ids["modules"]:
            errors.append(f"{step_id}:module_id")
        if command_id and command_id not in ids["commands"]:
            errors.append(f"{step_id}:command_id")
        for key in ("input_artifact_ids", "output_artifact_ids"):
            values = step.get(key, [])
            if not isinstance(values, list):
                errors.append(f"{step_id}:{key}")
                continue
            for artifact_id in values:
                if artifact_id not in artifact_ids:
                    errors.append(f"{step_id}:{key}:{artifact_id}")
        if "kernel" in step or "formula" in step or "simulation_parameters" in step:
            errors.append(f"{step_id}:algorithm_inline")

    errors += [
        f"artifact_step:{module_id}"
        for module_id in sorted(REQUIRED_MODULES - step_modules)
    ]
    return errors
