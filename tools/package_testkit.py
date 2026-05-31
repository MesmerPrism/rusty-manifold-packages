"""Reusable validation helpers for first-party Manifold package fixtures."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?)*$")
FORBIDDEN_TERMS = [
    "quest",
    "rusty-xr",
    "broker",
    "makepad",
    "openxr",
    "android",
    "windows",
    "ble",
    "gatt",
    "gonzo",
    "blimp",
    "gargoyle",
    "kiosk",
    "viscereality",
    "s:\\",
    "c:\\",
]
BOUNDARY_SKIP = {"tools/check_packages.py", "tools/package_testkit.py"}


@dataclass
class Check:
    check_id: str
    status: str
    evidence: str


@dataclass
class Report:
    status: str
    checks: list[Check]

    def to_json(self) -> dict[str, Any]:
        return {
            "$schema": "rusty.manifold.package.validation_report.v1",
            "status": self.status,
            "checks": [check.__dict__ for check in self.checks],
        }


@dataclass
class PackageBundle:
    root: Path
    manifest: dict[str, Any]
    modules: list[dict[str, Any]]
    streams: list[dict[str, Any]]
    commands: list[dict[str, Any]]
    graphs: list[dict[str, Any]]
    deployments: list[dict[str, Any]]
    runtime_states: list[dict[str, Any]]
    scorecards: list[dict[str, Any]]
    ownership_modes: list[dict[str, Any]]
    rejections: list[dict[str, Any]]


def validate_repo(repo_root: Path) -> Report:
    checks: list[Check] = []
    packages = load_packages(repo_root, checks)

    add_boundary_check(repo_root, checks)
    add_catalog_check(repo_root, packages, checks)
    for package in packages:
        add_package_checks(package, checks)

    status = "fail" if any(check.status == "fail" for check in checks) else "pass"
    return Report(status=status, checks=checks)


def load_packages(repo_root: Path, checks: list[Check]) -> list[PackageBundle]:
    package_roots = sorted((repo_root / "packages").glob("*/manifests/package.manifold.json"))
    packages: list[PackageBundle] = []
    for manifest_path in package_roots:
        package_root = manifest_path.parents[1]
        try:
            packages.append(
                PackageBundle(
                    root=package_root,
                    manifest=read_json(manifest_path),
                    modules=read_json_dir(package_root / "manifests/modules"),
                    streams=read_json_dir(package_root / "manifests/streams"),
                    commands=read_json_dir(package_root / "manifests/commands"),
                    graphs=read_json_dir(package_root / "fixtures/valid", name="graph.json"),
                    deployments=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="deployment-*.json"
                    ),
                    runtime_states=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="runtime-*.json"
                    ),
                    scorecards=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="scorecard-*.json"
                    ),
                    ownership_modes=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="ownership-*.json"
                    ),
                    rejections=read_json_dir(
                        package_root / "fixtures/damaged", glob_pattern="rejection-*.json"
                    ),
                )
            )
        except ValueError as error:
            checks.append(fail("validation.load_package", str(error)))
    checks.append(pass_check("validation.load_packages", f"loaded {len(packages)} packages"))
    return packages


def read_json(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(handle)
    except OSError as error:
        raise ValueError(f"{path}: {error}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"{path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected JSON object")
    return value


def read_json_dir(
    directory: Path, *, name: str | None = None, glob_pattern: str = "*.json"
) -> list[dict[str, Any]]:
    if not directory.exists():
        return []
    paths = [directory / name] if name else sorted(directory.glob(glob_pattern))
    return [read_json(path) for path in paths if path.exists()]


def add_boundary_check(repo_root: Path, checks: list[Check]) -> None:
    offenders: list[str] = []
    for path in sorted(repo_root.rglob("*")):
        relative = path.relative_to(repo_root).as_posix()
        if path.is_dir() or ".git" in path.parts or "__pycache__" in path.parts:
            continue
        if relative in BOUNDARY_SKIP:
            continue
        if path.suffix.lower() not in {".json", ".md", ".py", ".toml", ".txt"}:
            continue
        lower = path.read_text(encoding="utf-8").lower()
        for term in FORBIDDEN_TERMS:
            if contains_forbidden_term(lower, term):
                offenders.append(f"{relative} contains {term}")
    if offenders:
        checks.append(fail("validation.public_boundary_terms", "; ".join(offenders)))
    else:
        checks.append(pass_check("validation.public_boundary_terms", "no forbidden terms found"))


def contains_forbidden_term(text: str, term: str) -> bool:
    if "\\" in term or ":" in term:
        return term in text
    pattern = rf"(?<![a-z0-9]){re.escape(term)}(?![a-z0-9])"
    return re.search(pattern, text) is not None


def add_catalog_check(repo_root: Path, packages: list[PackageBundle], checks: list[Check]) -> None:
    catalog = read_json(repo_root / "packages/catalog.manifold.json")
    catalog_ids = sorted(item["package_id"] for item in catalog.get("packages", []))
    package_ids = sorted(package.manifest["package_id"] for package in packages)
    if catalog_ids == package_ids:
        checks.append(pass_check("validation.catalog_packages", "catalog matches package manifests"))
    else:
        checks.append(
            fail(
                "validation.catalog_packages",
                f"catalog ids {catalog_ids} do not match package ids {package_ids}",
            )
        )


def add_package_checks(package: PackageBundle, checks: list[Check]) -> None:
    package_id = package.manifest.get("package_id", "<missing>")
    prefix = f"validation.package.{package_id}"
    ids = collect_ids(package)
    modules_by_id = {item["module_id"]: item for item in package.modules}

    validate_dotted_ids(prefix, ids, checks)
    validate_exports(prefix, package, ids, checks)
    validate_module_links(prefix, package, ids, checks)
    validate_stream_links(prefix, package, ids, checks)
    validate_graph_links(prefix, package, ids, checks)
    validate_deployment_links(prefix, package, ids, modules_by_id, checks)
    validate_runtime_state_links(prefix, package, ids, modules_by_id, checks)
    validate_timestamp_policy(prefix, package, modules_by_id, checks)
    validate_provider_processor_split(prefix, package, modules_by_id, checks)
    validate_rejection_fixtures(prefix, package, checks)
    validate_scorecards(prefix, package, checks)
    validate_polar_readiness(prefix, package, modules_by_id, checks)


def collect_ids(package: PackageBundle) -> dict[str, set[str]]:
    return {
        "modules": {item["module_id"] for item in package.modules},
        "streams": {item["stream_id"] for item in package.streams},
        "commands": {item["command_id"] for item in package.commands},
        "graphs": {item["graph_id"] for item in package.graphs},
        "deployments": {item["deployment_id"] for item in package.deployments},
    }


def validate_dotted_ids(prefix: str, ids: dict[str, set[str]], checks: list[Check]) -> None:
    invalid = sorted(
        identifier
        for group in ids.values()
        for identifier in group
        if not ID_RE.match(identifier)
    )
    append_check(
        checks,
        f"{prefix}.dotted_ids",
        not invalid,
        "all ids match dotted-id grammar",
        f"invalid ids: {invalid}",
    )


def validate_exports(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    exports = package.manifest.get("exports", {})
    missing = sorted(set(exports.get("modules", [])) - ids["modules"])
    missing += sorted(set(exports.get("streams", [])) - ids["streams"])
    missing += sorted(set(exports.get("commands", [])) - ids["commands"])
    append_check(
        checks,
        f"{prefix}.exports",
        not missing,
        "package exports resolve to manifests",
        f"exports missing manifests: {missing}",
    )


def validate_module_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing: list[str] = []
    for module in package.modules:
        missing += sorted(set(module.get("provides_streams", [])) - ids["streams"])
        missing += sorted(set(module.get("consumes_streams", [])) - ids["streams"])
        missing += sorted(set(module.get("accepted_commands", [])) - ids["commands"])
    append_check(
        checks,
        f"{prefix}.module_links",
        not missing,
        "module stream and command links resolve",
        f"module links missing: {sorted(set(missing))}",
    )


def validate_stream_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing = sorted(
        stream["source_module_id"]
        for stream in package.streams
        if stream["source_module_id"] not in ids["modules"]
    )
    append_check(
        checks,
        f"{prefix}.stream_links",
        not missing,
        "stream source modules resolve",
        f"stream source modules missing: {missing}",
    )


def validate_graph_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing: list[str] = []
    for graph in package.graphs:
        node_ids = {node["node_id"] for node in graph.get("nodes", [])}
        missing += sorted(
            node["module_id"]
            for node in graph.get("nodes", [])
            if node["module_id"] not in ids["modules"]
        )
        for edge in graph.get("edges", []):
            if edge["source_node_id"] not in node_ids:
                missing.append(edge["source_node_id"])
            if edge["target_node_id"] not in node_ids:
                missing.append(edge["target_node_id"])
            if edge["source_stream_id"] not in ids["streams"]:
                missing.append(edge["source_stream_id"])
            if edge["target_input_id"] not in ids["streams"]:
                missing.append(edge["target_input_id"])
    append_check(
        checks,
        f"{prefix}.graph_links",
        not missing,
        "graph links resolve",
        f"graph links missing: {sorted(set(missing))}",
    )


def validate_deployment_links(
    prefix: str,
    package: PackageBundle,
    ids: dict[str, set[str]],
    modules_by_id: dict[str, dict[str, Any]],
    checks: list[Check],
) -> None:
    missing: list[str] = []
    for deployment in package.deployments:
        if deployment.get("package_id") != package.manifest.get("package_id"):
            missing.append(str(deployment.get("package_id")))
        missing += sorted(set(deployment.get("selected_modules", [])) - ids["modules"])
        for selection in deployment.get("selected_backends", []):
            module = modules_by_id.get(selection["module_id"])
            if module is None:
                missing.append(selection["module_id"])
            elif selection["backend_id"] not in module.get("platform_support", []):
                missing.append(selection["backend_id"])
    append_check(
        checks,
        f"{prefix}.deployment_links",
        not missing,
        "deployment links and selected backends resolve",
        f"deployment links missing: {sorted(set(missing))}",
    )


def validate_runtime_state_links(
    prefix: str,
    package: PackageBundle,
    ids: dict[str, set[str]],
    modules_by_id: dict[str, dict[str, Any]],
    checks: list[Check],
) -> None:
    missing: list[str] = []
    for state in package.runtime_states:
        module = modules_by_id.get(state.get("module_id"))
        if module is None:
            missing.append(str(state.get("module_id")))
            continue
        backend = state.get("selected_backend")
        if backend and backend not in module.get("platform_support", []):
            missing.append(backend)
        missing += sorted(set(state.get("active_streams", [])) - ids["streams"])
        missing += sorted(set(state.get("active_commands", [])) - ids["commands"])
        missing += sorted(set(state.get("active_streams", [])) - set(module.get("provides_streams", [])))
        missing += sorted(set(state.get("active_commands", [])) - set(module.get("accepted_commands", [])))
    append_check(
        checks,
        f"{prefix}.runtime_state_links",
        not missing,
        "runtime states resolve modules, streams, commands, and backend evidence",
        f"runtime state links missing: {sorted(set(missing))}",
    )


def validate_timestamp_policy(
    prefix: str,
    package: PackageBundle,
    modules_by_id: dict[str, dict[str, Any]],
    checks: list[Check],
) -> None:
    missing: list[str] = []
    for stream in package.streams:
        domains = set(stream.get("timestamp_domains", []))
        if not domains:
            missing.append(f"{stream['stream_id']}:timestamp_domains")
            continue
        source_module = modules_by_id.get(stream["source_module_id"], {})
        semantic = stream.get("semantic_family", "")
        if source_module.get("module_kind") == "provider" and semantic.startswith(
            ("biosignal.", "motion.")
        ):
            required = {"clock.source_device", "clock.host_monotonic"}
            if not required.issubset(domains):
                missing.append(stream["stream_id"])
        elif "clock.host_monotonic" not in domains:
            missing.append(stream["stream_id"])
    append_check(
        checks,
        f"{prefix}.timestamp_policy",
        not missing,
        "stream timestamp domains match direct and derived stream policy",
        f"timestamp policy missing: {sorted(set(missing))}",
    )


def validate_provider_processor_split(
    prefix: str,
    package: PackageBundle,
    modules_by_id: dict[str, dict[str, Any]],
    checks: list[Check],
) -> None:
    misplaced: list[str] = []
    for stream in package.streams:
        module = modules_by_id.get(stream["source_module_id"], {})
        stream_id = stream["stream_id"]
        semantic = stream.get("semantic_family", "")
        if module.get("module_kind") == "provider" and (
            stream_id.startswith(("stream.breath.", "stream.beat."))
            or semantic.startswith(("breath.", "beat."))
        ):
            misplaced.append(stream_id)
        if module.get("module_kind") == "processor" and (
            stream_id.startswith(("stream.biosignal.", "stream.motion."))
            or semantic.startswith(("biosignal.", "motion.", "device.", "backend."))
        ):
            misplaced.append(stream_id)
    append_check(
        checks,
        f"{prefix}.provider_processor_split",
        not misplaced,
        "direct streams stay on providers and derived streams stay on processors",
        f"misplaced streams: {sorted(set(misplaced))}",
    )


def validate_rejection_fixtures(
    prefix: str, package: PackageBundle, checks: list[Check]
) -> None:
    invalid = [
        str(rejection)
        for rejection in package.rejections
        if not ID_RE.match(str(rejection.get("request_id", "")))
        or not ID_RE.match(str(rejection.get("rejection_code", "")))
        or not isinstance(rejection.get("retryable"), bool)
    ]
    required_by_package = {
        "package.biosignal_sensor": {
            "rejection.permission_missing",
            "rejection.source_busy",
            "rejection.unsupported_stream",
            "rejection.backend_missing",
            "rejection.timeout",
            "rejection.malformed_frame",
        },
        "package.polar_h10": {
            "rejection.permission_missing",
            "rejection.raw_stream_owned",
            "rejection.unsupported_stream",
            "rejection.backend_missing",
            "rejection.timeout",
            "rejection.malformed_frame",
        },
    }
    required = required_by_package.get(str(package.manifest.get("package_id")))
    if required:
        present = {rejection.get("rejection_code") for rejection in package.rejections}
        invalid += sorted(required - present)
    append_check(
        checks,
        f"{prefix}.command_rejections",
        not invalid,
        "command rejection fixtures cover expected damaged inputs",
        f"invalid or missing rejection fixtures: {invalid}",
    )


def validate_scorecards(prefix: str, package: PackageBundle, checks: list[Check]) -> None:
    invalid = [
        str(check.get("check_id"))
        for scorecard in package.scorecards
        for check in scorecard.get("checks", [])
        if not ID_RE.match(str(check.get("check_id", "")))
    ]
    required_scorecards = {
        "package.biosignal_sensor": "scorecard.biosignal_synthetic_contract",
        "package.polar_h10": "scorecard.polar_h10_readiness",
    }
    required_scorecard = required_scorecards.get(str(package.manifest.get("package_id")))
    present_scorecards = {scorecard.get("scorecard_id") for scorecard in package.scorecards}
    if required_scorecard and required_scorecard not in present_scorecards:
        invalid.append(required_scorecard)
    append_check(
        checks,
        f"{prefix}.scorecards",
        not invalid,
        "scorecard fixtures are present and use dotted check ids",
        f"invalid scorecard rows: {invalid}",
    )


def validate_polar_readiness(
    prefix: str,
    package: PackageBundle,
    modules_by_id: dict[str, dict[str, Any]],
    checks: list[Check],
) -> None:
    if package.manifest.get("package_id") != "package.polar_h10":
        return

    module_ids = set(modules_by_id)
    stream_by_id = {stream["stream_id"]: stream for stream in package.streams}
    required_modules = {
        "module.polar_h10.provider",
        "module.polar_h10.breath_volume_from_acc",
        "module.polar_h10.hrv_window",
        "module.polar_h10.coherence",
        "module.polar_h10.breath_dynamics",
    }
    append_check(
        checks,
        f"{prefix}.polar_modules",
        required_modules.issubset(module_ids),
        "Polar provider and processor modules are present",
        f"missing modules: {sorted(required_modules - module_ids)}",
    )

    direct_streams = {
        "stream.polar_h10.hr_rr",
        "stream.polar_h10.ecg",
        "stream.polar_h10.acc",
    }
    timestamp_missing: list[str] = []
    for stream_id in sorted(direct_streams):
        stream = stream_by_id.get(stream_id)
        if stream is None:
            timestamp_missing.append(stream_id)
            continue
        if stream.get("source_module_id") != "module.polar_h10.provider":
            timestamp_missing.append(f"{stream_id}:source_module")
        domains = set(stream.get("timestamp_domains", []))
        if not {"clock.source_device", "clock.host_monotonic"}.issubset(domains):
            timestamp_missing.append(f"{stream_id}:timestamp_domains")
    append_check(
        checks,
        f"{prefix}.polar_direct_timestamps",
        not timestamp_missing,
        "Polar direct streams preserve source-device and host timestamp domains",
        f"missing direct stream timestamp evidence: {timestamp_missing}",
    )

    ownership_modes = [
        mode
        for ownership_doc in package.ownership_modes
        for mode in ownership_doc.get("modes", [])
    ]
    ownership_mode_ids = {mode.get("mode_id") for mode in ownership_modes}
    required_ownership_modes = {
        "ownership.raw_stream.single_owner",
        "ownership.hr_rr.dual_receiver",
        "ownership.raw_stream.two_sensor_compare",
    }
    raw_mode_errors: list[str] = []
    for mode in ownership_modes:
        if mode.get("mode_id") == "ownership.raw_stream.single_owner":
            streams = set(mode.get("streams", []))
            if not {"stream.polar_h10.ecg", "stream.polar_h10.acc"}.issubset(streams):
                raw_mode_errors.append("ownership.raw_stream.single_owner:streams")
            if mode.get("rejection_code") != "rejection.raw_stream_owned":
                raw_mode_errors.append("ownership.raw_stream.single_owner:rejection_code")
    ownership_missing = sorted(required_ownership_modes - ownership_mode_ids)
    append_check(
        checks,
        f"{prefix}.polar_ownership_modes",
        not ownership_missing and not raw_mode_errors,
        "Polar raw ownership and HR/RR sharing modes are explicit",
        f"ownership fixture issues: {ownership_missing + raw_mode_errors}",
    )

    provider = modules_by_id.get("module.polar_h10.provider", {})
    required_backend_support = {
        "backend.synthetic",
        "backend.replay",
        "backend.desktop_wireless",
        "backend.mobile_wireless",
        "backend.headset_wireless",
    }
    support = set(provider.get("platform_support", []))
    deployment_backends = {
        selection.get("backend_id")
        for deployment in package.deployments
        for selection in deployment.get("selected_backends", [])
    }
    runtime_backends = {state.get("selected_backend") for state in package.runtime_states}
    fixture_backends = {"backend.synthetic", "backend.replay"}
    backend_errors = sorted(required_backend_support - support)
    backend_errors += [
        f"deployment:{backend}" for backend in sorted(fixture_backends - deployment_backends)
    ]
    backend_errors += [f"runtime:{backend}" for backend in sorted(fixture_backends - runtime_backends)]
    append_check(
        checks,
        f"{prefix}.polar_backend_evidence",
        not backend_errors,
        "Polar backend support plus synthetic and replay fixture evidence are present",
        f"backend evidence issues: {backend_errors}",
    )


def append_check(
    checks: list[Check],
    check_id: str,
    passed: bool,
    pass_evidence: str,
    fail_evidence: str,
) -> None:
    checks.append(pass_check(check_id, pass_evidence) if passed else fail(check_id, fail_evidence))


def pass_check(check_id: str, evidence: str) -> Check:
    return Check(check_id=check_id, status="pass", evidence=evidence)


def fail(check_id: str, evidence: str) -> Check:
    return Check(check_id=check_id, status="fail", evidence=evidence)
