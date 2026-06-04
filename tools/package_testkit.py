"""Reusable validation helpers for first-party Manifold package fixtures."""

from __future__ import annotations

import json
import math
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
    "hostess",
    "jni",
    "cdylib",
    "java_io",
    "system.loadlibrary",
    "gonzo",
    "blimp",
    "gargoyle",
    "kiosk",
    "viscereality",
    "s:\\",
    "c:\\",
]
BOUNDARY_SKIP = {"tools/check_packages.py", "tools/package_testkit.py"}
BOUNDARY_TERM_EXCEPTIONS = {
    "packages/polar-h10/manifests/provenance.manifold.json": {"ble", "gatt"}
}


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
    pmd_handoffs: list[dict[str, Any]]
    completion_evidence: list[dict[str, Any]]
    processing_goldens: list[dict[str, Any]]
    source_adapter_descriptors: list[dict[str, Any]]
    shell_handoffs: list[dict[str, Any]]
    provenance_docs: list[dict[str, Any]]
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
                    pmd_handoffs=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="handoff-*.json"
                    ),
                    completion_evidence=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="completion-*.json"
                    ),
                    processing_goldens=read_json_dir(
                        package_root / "fixtures/valid", glob_pattern="processor-*.json"
                    ),
                    source_adapter_descriptors=read_json_dir(
                        package_root / "fixtures/valid",
                        glob_pattern="source-adapter-*.json",
                    ),
                    shell_handoffs=read_json_dir(
                        package_root / "fixtures/valid",
                        glob_pattern="shell-handoff-*.json",
                    ),
                    provenance_docs=read_json_dir(
                        package_root / "manifests", glob_pattern="provenance*.json"
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
        if path.is_dir() or ".git" in path.parts or "__pycache__" in path.parts or "target" in path.parts:
            continue
        if relative in BOUNDARY_SKIP:
            continue
        if path.suffix.lower() not in {".json", ".md", ".py", ".rs", ".toml", ".txt"}:
            continue
        lower = path.read_text(encoding="utf-8").lower()
        ignored_terms = BOUNDARY_TERM_EXCEPTIONS.get(relative, set())
        for term in FORBIDDEN_TERMS:
            if term in ignored_terms:
                continue
            if contains_forbidden_term(lower, term):
                offenders.append(f"{relative} contains {term}")
    if offenders:
        checks.append(fail("validation.public_boundary_terms", "; ".join(offenders)))
    else:
        checks.append(pass_check("validation.public_boundary_terms", "no forbidden terms found"))


def contains_forbidden_term(text: str, term: str) -> bool:
    if "\\" in term or ":" in term:
        return term in text
    if term == "windows":
        pattern = rf"(?<![a-z0-9]){re.escape(term)}(?![a-z0-9])"
        for match in re.finditer(pattern, text):
            before = text[match.start() - 1] if match.start() > 0 else ""
            after = text[match.end()] if match.end() < len(text) else ""
            if before in "._" or after in "._":
                continue
            return True
        return False
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
    validate_provenance(prefix, package, checks)
    validate_processor_goldens(prefix, package, checks)
    validate_shell_handoffs(prefix, package, ids, checks)
    validate_polar_readiness(prefix, package, modules_by_id, checks)
    validate_polar_completion_evidence(prefix, package, checks)
    validate_projected_motion_breath(prefix, package, checks)


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
            "rejection.handoff_release_timeout",
            "rejection.handoff_advertisement_timeout",
            "rejection.handoff_connect_timeout",
            "rejection.handoff_first_frame_timeout",
            "rejection.settings_mismatch",
            "rejection.previous_owner_not_stopped",
            "rejection.stop_command_failed",
            "rejection.service_discovery_failed",
            "rejection.service_cache_failed",
            "rejection.control_write_failed",
            "rejection.sample_rate_below_tolerance",
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
        "package.projected_motion_breath": "scorecard.projected_motion_breath_synthetic_contract",
    }
    required_scorecard = required_scorecards.get(str(package.manifest.get("package_id")))
    present_scorecards = {scorecard.get("scorecard_id") for scorecard in package.scorecards}
    if required_scorecard and required_scorecard not in present_scorecards:
        invalid.append(required_scorecard)
    if str(package.manifest.get("package_id")) == "package.projected_motion_breath":
        scorecard = find_one(
            package.scorecards,
            "scorecard_id",
            "scorecard.projected_motion_breath_synthetic_contract",
        )
        required_checks = {
            "validation.check.pose_and_vector_inputs",
            "validation.check.provider_processor_sink_split",
            "validation.check.synthetic_replay_ready",
            "validation.check.profile_boundary",
            "validation.check.profile_command_validation",
            "validation.check.source_adapter_descriptors",
            "validation.check.source_binding_validation",
            "validation.check.adapter_normalization_validation",
        }
        if scorecard is None:
            invalid.append("scorecard.projected_motion_breath_synthetic_contract")
        else:
            present_checks = {check.get("check_id") for check in scorecard.get("checks", [])}
            invalid += sorted(required_checks - present_checks)
    append_check(
        checks,
        f"{prefix}.scorecards",
        not invalid,
        "scorecard fixtures are present and use dotted check ids",
        f"invalid scorecard rows: {invalid}",
    )


def validate_provenance(prefix: str, package: PackageBundle, checks: list[Check]) -> None:
    package_id = str(package.manifest.get("package_id"))
    if package_id != "package.polar_h10":
        return

    errors: list[str] = []
    provenance_refs = set(package.manifest.get("provenance_refs", []))
    notice_refs = set(package.manifest.get("notice_refs", []))
    if "provenance.polar_h10.source_manifest" not in provenance_refs:
        errors.append("manifest:provenance.polar_h10.source_manifest")
    for notice_id in {
        "notice.polar_h10.not_medical_device",
        "notice.polar_h10.not_affiliated",
    }:
        if notice_id not in notice_refs:
            errors.append(f"manifest:{notice_id}")

    provenance = find_one(
        package.provenance_docs,
        "provenance_id",
        "provenance.polar_h10.source_manifest",
    )
    if provenance is None:
        errors.append("provenance.polar_h10.source_manifest")
    else:
        source_ids = {source.get("source_id") for source in provenance.get("sources", [])}
        required_sources = {
            "source.polar_h10.vendor_technical_docs",
            "source.polar_h10.measurement_spec_snapshot",
            "source.polar_h10.security_context",
            "source.method.hrv_metrics",
            "source.method.hrv_transform_context",
            "source.method.rmssd_gain",
            "source.method.coherence_ratio",
            "source.method.hrvb_protocol_guidelines",
            "source.method.hrvb_resonance_mechanism",
            "source.method.hrvb_resonance_amplitude",
            "source.method.breathing_dynamics",
            "source.method.sample_entropy",
            "source.method.multiscale_entropy",
            "source.method.lempel_ziv_complexity",
            "source.method.acc_breath_proxy",
        }
        errors += [f"source:{source_id}" for source_id in sorted(required_sources - source_ids)]
        for source in provenance.get("sources", []):
            source_id = str(source.get("source_id", ""))
            if not ID_RE.match(source_id):
                errors.append(f"{source_id}:source_id")
            if not source.get("claim_scope"):
                errors.append(f"{source_id}:claim_scope")
            if not source.get("copy_policy"):
                errors.append(f"{source_id}:copy_policy")
            citation = source.get("citation", {})
            doi = citation.get("doi")
            url = citation.get("url")
            if doi == "10.3390/s25072005":
                errors.append(f"{source_id}:stale_doi")
            if not doi and not url:
                errors.append(f"{source_id}:citation")
            if url and not (
                str(url).startswith("https://")
                or str(url).startswith("http://")
                or str(url).startswith("package://")
            ):
                errors.append(f"{source_id}:url")
            snapshot = source.get("snapshot", {})
            if not isinstance(snapshot, dict):
                errors.append(f"{source_id}:snapshot")
            else:
                snapshot_id = str(snapshot.get("snapshot_id", ""))
                if not ID_RE.match(snapshot_id):
                    errors.append(f"{source_id}:snapshot_id")
                retrieved_at = str(snapshot.get("retrieved_at", ""))
                if not re.match(r"^\d{4}-\d{2}-\d{2}$", retrieved_at):
                    errors.append(f"{source_id}:retrieved_at")
                if not str(snapshot.get("digest", "")):
                    errors.append(f"{source_id}:digest")

        modules = {module["module_id"] for module in package.modules}
        bindings = provenance.get("module_bindings", [])
        bound_modules = {binding.get("module_id") for binding in bindings}
        errors += [f"binding:{module_id}" for module_id in sorted(modules - bound_modules)]
        for binding in bindings:
            module_id = str(binding.get("module_id", ""))
            if module_id not in modules:
                errors.append(f"{module_id}:module_id")
            if not set(binding.get("source_ids", [])).issubset(source_ids):
                errors.append(f"{module_id}:source_ids")
            if not binding.get("claim_boundary"):
                errors.append(f"{module_id}:claim_boundary")

        notice_ids = {notice.get("notice_id") for notice in provenance.get("notice_requirements", [])}
        errors += [f"notice:{notice_id}" for notice_id in sorted(notice_refs - notice_ids)]
        rejected_citations = {
            item.get("citation_id") for item in provenance.get("rejected_citations", [])
        }
        if "citation.stale_security_paper" not in rejected_citations:
            errors.append("rejected_citation:citation.stale_security_paper")
        if package_contains_text(package.root, "10.3390/s25072005"):
            errors.append("stale_doi_present")

    append_check(
        checks,
        f"{prefix}.provenance",
        not errors,
        "Polar source ids, module bindings, notices, and stale DOI rejection are explicit",
        f"provenance issues: {errors}",
    )


def validate_processor_goldens(prefix: str, package: PackageBundle, checks: list[Check]) -> None:
    package_id = str(package.manifest.get("package_id"))
    if package_id != "package.polar_h10":
        return

    errors: list[str] = []
    validators = {
        "golden.polar_h10.hrv_window.rr_metrics": validate_hrv_window_golden,
        "golden.polar_h10.rmssd_gain.log_delta": validate_rmssd_gain_golden,
        "golden.polar_h10.coherence.spectral_ratio": validate_coherence_golden,
        "golden.polar_h10.breath_volume.acc_projection": validate_breath_volume_golden,
        "golden.polar_h10.breath_dynamics.cycle_stats": validate_breath_dynamics_golden,
        "golden.polar_h10.hrvb_resonance_amplitude.sine_fit": validate_hrvb_amplitude_golden,
    }
    for golden_id, validator in validators.items():
        golden = find_one(package.processing_goldens, "golden_id", golden_id)
        if golden is None:
            errors.append(golden_id)
        else:
            errors += validator(package, golden)

    append_check(
        checks,
        f"{prefix}.processor_goldens",
        not errors,
        "Polar processor golden fixtures recompute expected non-live outputs",
        f"processor golden issues: {errors}",
    )


def validate_shell_handoffs(
    prefix: str,
    package: PackageBundle,
    ids: dict[str, set[str]],
    checks: list[Check],
) -> None:
    errors: list[str] = []
    for handoff in package.shell_handoffs:
        handoff_id = str(handoff.get("handoff_id", ""))
        if not ID_RE.match(handoff_id):
            errors.append(f"{handoff_id}:handoff_id")
        for key in (
            "target_host_profile",
            "shell_app_id",
            "validation_slot_id",
            "expected_scorecard_id",
        ):
            value = str(handoff.get(key, ""))
            if not ID_RE.match(value):
                errors.append(f"{handoff_id}:{key}")

        for binding in handoff.get("stream_bindings", []):
            stream_id = str(binding.get("stream_id", ""))
            role = str(binding.get("role", ""))
            direction = str(binding.get("direction", ""))
            if stream_id not in ids["streams"]:
                errors.append(f"{handoff_id}:stream:{stream_id}")
            if not ID_RE.match(role):
                errors.append(f"{handoff_id}:role:{role}")
            if direction not in {"publish", "subscribe"}:
                errors.append(f"{handoff_id}:direction:{direction}")
            if not isinstance(binding.get("required"), bool):
                errors.append(f"{handoff_id}:required:{stream_id}")

        for command_id in handoff.get("command_ids", []):
            if command_id not in ids["commands"]:
                errors.append(f"{handoff_id}:command:{command_id}")

        for offer in handoff.get("transport_offers", []):
            transport_id = str(offer.get("transport_id", ""))
            endpoint_id = offer.get("endpoint_id")
            if not ID_RE.match(transport_id):
                errors.append(f"{handoff_id}:transport_id:{transport_id}")
            if endpoint_id is not None and not ID_RE.match(str(endpoint_id)):
                errors.append(f"{handoff_id}:endpoint_id:{endpoint_id}")

    if str(package.manifest.get("package_id")) == "package.projected_motion_breath":
        bindings = {
            (str(binding.get("stream_id", "")), str(binding.get("direction", "")))
            for handoff in package.shell_handoffs
            for binding in handoff.get("stream_bindings", [])
        }
        required = {
            ("stream.motion.object_pose", "publish"),
            ("stream.breath.feedback_state", "subscribe"),
            ("stream.breath.feedback_receipt", "publish"),
        }
        errors += [
            f"missing_binding:{stream_id}:{direction}"
            for stream_id, direction in sorted(required - bindings)
        ]

    append_check(
        checks,
        f"{prefix}.shell_handoffs",
        not errors,
        "shell handoff fixtures resolve package streams, commands, and receipt bindings",
        f"shell handoff issues: {errors}",
    )


def validate_golden_links(
    package: PackageBundle, golden: dict[str, Any], required_fields: dict[str, str]
) -> list[str]:
    errors: list[str] = []
    module_ids = {module["module_id"] for module in package.modules}
    stream_ids = {stream["stream_id"] for stream in package.streams}

    for key, expected in required_fields.items():
        if golden.get(key) != expected:
            errors.append(f"{key}:{golden.get(key)}")

    if golden.get("module_id") not in module_ids:
        errors.append("module_id")
    for key in ("input_stream_id", "output_stream_id"):
        if golden.get(key) not in stream_ids:
            errors.append(key)
    return errors


def validate_holding_cases(
    golden: dict[str, Any], required_damaged_issue_codes: set[str]
) -> list[str]:
    errors: list[str] = []
    cases = golden.get("cases", [])
    if not isinstance(cases, list) or not cases:
        errors.append("cases")
    damaged_cases = golden.get("damaged_cases", [])
    if not isinstance(damaged_cases, list) or not damaged_cases:
        errors.append("damaged_cases")
    else:
        present = {
            str(case.get("expected_issue_code", ""))
            for case in damaged_cases
            if isinstance(case, dict)
        }
        errors += [
            f"damaged_issue:{issue_code}"
            for issue_code in sorted(required_damaged_issue_codes - present)
        ]
        for damaged_case in damaged_cases:
            if not isinstance(damaged_case, dict):
                errors.append("damaged_case")
                continue
            case_id = str(damaged_case.get("case_id", ""))
            if not ID_RE.match(case_id):
                errors.append(f"{case_id}:case_id")
            issue_code = str(damaged_case.get("expected_issue_code", ""))
            if not ID_RE.match(issue_code):
                errors.append(f"{case_id}:expected_issue_code")
    return errors


def validate_hrv_window_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors = validate_golden_links(
        package,
        golden,
        {
            "package_id": "package.polar_h10",
            "module_id": "module.polar_h10.hrv_window",
            "input_stream_id": "stream.polar_h10.hr_rr",
            "output_stream_id": "stream.polar_h10.hrv_window",
            "source_id": "source.method.hrv_metrics",
        },
    )
    errors += validate_holding_cases(
        golden, {"issue.window_underfilled", "issue.quality_low"}
    )
    for case in golden.get("cases", []):
        if isinstance(case, dict):
            errors += validate_hrv_window_case(case)
    return errors


def validate_hrv_window_case(case: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    rr = case.get("input", {}).get("rr_intervals_ms")
    if not isinstance(rr, list) or len(rr) < 2:
        return [f"{case_id}:rr_intervals_ms"]
    values = [numeric(item) for item in rr]
    mean_nn = sum(values) / len(values)
    diffs = [values[index + 1] - values[index] for index in range(len(values) - 1)]
    rmssd = math.sqrt(sum(diff * diff for diff in diffs) / len(diffs))
    sdnn = math.sqrt(
        sum((value - mean_nn) * (value - mean_nn) for value in values)
        / (len(values) - 1)
    )
    actual = {
        "accepted_count": float(len(values)),
        "rejected_count": 0.0,
        "successive_difference_count": float(len(diffs)),
        "mean_nn_ms": mean_nn,
        "mean_hr_bpm": 60000.0 / mean_nn,
        "sdnn_ms": sdnn,
        "rmssd_ms": rmssd,
        "ln_rmssd": math.log(rmssd),
        "pnn50": sum(1 for diff in diffs if abs(diff) > 50.0) / len(diffs),
        "sd1_ms": rmssd / math.sqrt(2.0),
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_rmssd_gain_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors = validate_golden_links(
        package,
        golden,
        {
            "package_id": "package.polar_h10",
            "module_id": "module.polar_h10.rmssd_gain",
            "input_stream_id": "stream.polar_h10.hrv_window",
            "output_stream_id": "stream.polar_h10.rmssd_gain",
            "source_id": "source.method.rmssd_gain",
        },
    )
    errors += validate_holding_cases(
        golden, {"issue.baseline_missing", "issue.baseline_invalid"}
    )
    for case in golden.get("cases", []):
        if isinstance(case, dict):
            errors += validate_rmssd_gain_case(case)
    return errors


def validate_rmssd_gain_case(case: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    case_input = case.get("input", {})
    live = case_input.get("live", {}) if isinstance(case_input, dict) else {}
    baseline = case_input.get("baseline", {}) if isinstance(case_input, dict) else {}
    live_ln = numeric(live.get("ln_rmssd"))
    baseline_ln = numeric(baseline.get("baseline_ln_rmssd"))
    baseline_mean = numeric(baseline.get("baseline_mean_ln_rmssd"))
    baseline_sd = numeric(baseline.get("baseline_sd_ln_rmssd"))
    if baseline_sd <= 0.0:
        return [f"{case_id}:baseline_sd_ln_rmssd"]
    gain = live_ln - baseline_ln
    actual = {
        "ln_rmssd_gain": gain,
        "rmssd_ratio": math.exp(gain),
        "baseline_z_score": (live_ln - baseline_mean) / baseline_sd,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_coherence_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors: list[str] = []
    module_ids = {module["module_id"] for module in package.modules}
    stream_ids = {stream["stream_id"] for stream in package.streams}

    required_fields = {
        "package_id": "package.polar_h10",
        "module_id": "module.polar_h10.coherence",
        "input_stream_id": "stream.polar_h10.hr_rr",
        "output_stream_id": "stream.polar_h10.coherence",
        "source_id": "source.method.coherence_ratio",
    }
    for key, expected in required_fields.items():
        if golden.get(key) != expected:
            errors.append(f"{key}:{golden.get(key)}")

    if golden.get("module_id") not in module_ids:
        errors.append("module_id")
    for key in ("input_stream_id", "output_stream_id"):
        if golden.get(key) not in stream_ids:
            errors.append(key)

    settings = golden.get("settings", {})
    if not isinstance(settings, dict):
        return errors + ["settings"]

    expected_settings = {
        "rr_interval_units": "ms",
        "detrend": "mean",
        "analysis_window": "rectangular",
        "power_normalization": "magnitude_squared_over_n_squared",
        "paper_ratio_formula": "peak_band_power / (total_band_power - peak_band_power)",
        "coherence_ratio_formula": "peak_band_power / remaining_power",
        "coherence_ratio_squared_formula": "coherence_ratio * coherence_ratio",
        "normalized_peak_power_formula": "peak_band_power / total_band_power",
        "normalized_score_formula": "paper_ratio / (paper_ratio + 1)",
    }
    for key, expected in expected_settings.items():
        if settings.get(key) != expected:
            errors.append(f"settings.{key}")

    if numeric(settings.get("sample_rate_hz")) <= 0.0:
        errors.append("settings.sample_rate_hz")
    if not isinstance(settings.get("fft_length"), int) or settings.get("fft_length") <= 0:
        errors.append("settings.fft_length")
    if numeric(settings.get("window_seconds")) <= 0.0:
        errors.append("settings.window_seconds")

    cases = golden.get("cases", [])
    if not isinstance(cases, list) or not cases:
        errors.append("cases")
    else:
        for case in cases:
            if not isinstance(case, dict):
                errors.append("case")
                continue
            errors += validate_coherence_case(settings, case)

    damaged_cases = golden.get("damaged_cases", [])
    if not isinstance(damaged_cases, list) or not damaged_cases:
        errors.append("damaged_cases")
    else:
        errors += validate_coherence_damaged_cases(settings, damaged_cases)

    return errors


def validate_coherence_case(settings: dict[str, Any], case: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    if not ID_RE.match(case_id):
        errors.append(f"{case_id}:case_id")

    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return errors + [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute"))
    if tolerance <= 0.0:
        errors.append(f"{case_id}:tolerance")
        tolerance = 0.000001

    try:
        actual = compute_coherence_case(settings, case.get("input", {}))
    except ValueError as error:
        return errors + [f"{case_id}:{error}"]

    for key in (
        "peak_frequency_hz",
        "peak_band_power",
        "total_band_power",
        "remaining_power",
        "paper_ratio",
        "coherence_ratio",
        "coherence_ratio_squared",
        "normalized_peak_power",
        "normalized_score",
    ):
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
            continue
        if not within_tolerance(actual[key], numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")

    if actual["quality"] != expected.get("quality"):
        errors.append(f"{case_id}:quality")

    return errors


def validate_coherence_damaged_cases(
    settings: dict[str, Any], damaged_cases: list[Any]
) -> list[str]:
    errors: list[str] = []
    fft_length = settings.get("fft_length")
    if not isinstance(fft_length, int):
        return ["damaged_cases:fft_length"]

    for damaged_case in damaged_cases:
        if not isinstance(damaged_case, dict):
            errors.append("damaged_case")
            continue
        case_id = str(damaged_case.get("case_id", ""))
        if not ID_RE.match(case_id):
            errors.append(f"{case_id}:case_id")
        issue_code = str(damaged_case.get("expected_issue_code", ""))
        if not ID_RE.match(issue_code):
            errors.append(f"{case_id}:expected_issue_code")
        if issue_code == "issue.window_underfilled":
            sample_count = damaged_case.get("input", {}).get("sample_count")
            if not isinstance(sample_count, int) or sample_count >= fft_length:
                errors.append(f"{case_id}:sample_count")
    return errors


def compute_coherence_case(
    settings: dict[str, Any], case_input: Any
) -> dict[str, float | str]:
    if not isinstance(case_input, dict):
        raise ValueError("input")
    fft_length = settings.get("fft_length")
    if not isinstance(fft_length, int) or fft_length <= 0:
        raise ValueError("fft_length")
    sample_rate_hz = numeric(settings.get("sample_rate_hz"))
    if sample_rate_hz <= 0.0:
        raise ValueError("sample_rate_hz")
    if settings.get("detrend") != "mean":
        raise ValueError("detrend")
    if settings.get("analysis_window") != "rectangular":
        raise ValueError("analysis_window")

    total_band = frequency_band(settings.get("total_band_hz"), "total_band_hz")
    peak_search = frequency_band(settings.get("peak_search_hz"), "peak_search_hz")
    peak_half_width = numeric(settings.get("peak_window_half_width_hz"))
    if peak_half_width <= 0.0:
        raise ValueError("peak_window_half_width_hz")

    samples = synthesize_rr_window(fft_length, case_input)
    mean = sum(samples) / float(fft_length)
    detrended = [sample - mean for sample in samples]
    powers = dft_power_by_bin(detrended, sample_rate_hz)

    total_band_power = sum(
        power for _, frequency, power in powers if in_band(frequency, total_band)
    )
    peak_candidates = [
        (bin_index, frequency, power)
        for bin_index, frequency, power in powers
        if in_band(frequency, peak_search)
    ]
    if not peak_candidates:
        raise ValueError("peak_search_hz")
    max_peak_power = max(power for _, _, power in peak_candidates)
    peak_bin, peak_frequency_hz, _ = min(
        (
            (bin_index, frequency, power)
            for bin_index, frequency, power in peak_candidates
            if abs(power - max_peak_power) <= 0.000000000001
        ),
        key=lambda item: item[0],
    )
    if peak_bin <= 0:
        raise ValueError("peak_bin")

    peak_band_power = sum(
        power
        for _, frequency, power in powers
        if in_band(frequency, total_band)
        and abs(frequency - peak_frequency_hz) <= peak_half_width + 0.000000000001
    )
    remaining_power = total_band_power - peak_band_power
    if remaining_power <= 0.0:
        paper_ratio = math.inf
        normalized_peak_power = 1.0
        normalized_score = 1.0
    else:
        paper_ratio = peak_band_power / remaining_power
        normalized_peak_power = peak_band_power / total_band_power
        normalized_score = paper_ratio / (paper_ratio + 1.0)

    return {
        "peak_frequency_hz": peak_frequency_hz,
        "peak_band_power": peak_band_power,
        "total_band_power": total_band_power,
        "remaining_power": remaining_power,
        "paper_ratio": paper_ratio,
        "coherence_ratio": paper_ratio,
        "coherence_ratio_squared": paper_ratio * paper_ratio,
        "normalized_peak_power": normalized_peak_power,
        "normalized_score": normalized_score,
        "quality": "stable" if paper_ratio >= 2.0 else "distributed",
    }


def validate_breath_volume_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors = validate_golden_links(
        package,
        golden,
        {
            "package_id": "package.polar_h10",
            "module_id": "module.polar_h10.breath_volume_from_acc",
            "input_stream_id": "stream.polar_h10.acc",
            "output_stream_id": "stream.polar_h10.breath_volume",
            "source_id": "source.method.acc_breath_proxy",
        },
    )
    errors += validate_holding_cases(golden, {"issue.calibration_invalid"})
    for case in golden.get("cases", []):
        if isinstance(case, dict):
            errors += validate_breath_volume_case(case)
    return errors


def validate_breath_volume_case(case: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    case_input = case.get("input", {})
    calibration = (
        case_input.get("calibration_projection", []) if isinstance(case_input, dict) else []
    )
    if not isinstance(calibration, list) or not calibration:
        return [f"{case_id}:calibration_projection"]
    values = [numeric(item) for item in calibration]
    lower_bound = min(values)
    upper_bound = max(values)
    live_projection = numeric(case_input.get("live_projection"))
    previous_projection = numeric(case_input.get("previous_projection"))
    if upper_bound <= lower_bound:
        return [f"{case_id}:bounds"]
    volume = max(0.0, min(1.0, (live_projection - lower_bound) / (upper_bound - lower_bound)))
    phase = "inhale" if live_projection >= previous_projection else "exhale"
    actual = {
        "lower_bound": lower_bound,
        "upper_bound": upper_bound,
        "breath_volume_01": volume,
        "confidence": 1.0,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("phase") != phase:
        errors.append(f"{case_id}:phase")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_breath_dynamics_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors = validate_golden_links(
        package,
        golden,
        {
            "package_id": "package.polar_h10",
            "module_id": "module.polar_h10.breath_dynamics",
            "input_stream_id": "stream.polar_h10.breath_volume",
            "output_stream_id": "stream.polar_h10.breath_dynamics",
            "source_id": "source.method.breathing_dynamics",
        },
    )
    errors += validate_holding_cases(golden, {"issue.window_underfilled"})
    for case in golden.get("cases", []):
        if isinstance(case, dict):
            errors += validate_breath_dynamics_case(case)
    return errors


def validate_breath_dynamics_case(case: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    case_input = case.get("input", {})
    intervals = case_input.get("breath_intervals_s", []) if isinstance(case_input, dict) else []
    amplitudes = case_input.get("breath_amplitudes_01", []) if isinstance(case_input, dict) else []
    if not isinstance(intervals, list) or not isinstance(amplitudes, list):
        return [f"{case_id}:series"]
    interval_values = [numeric(item) for item in intervals]
    amplitude_values = [numeric(item) for item in amplitudes]
    if len(interval_values) < 2 or len(amplitude_values) < 2:
        return [f"{case_id}:underfilled"]
    interval_mean = sum(interval_values) / len(interval_values)
    amplitude_mean = sum(amplitude_values) / len(amplitude_values)
    interval_sd = sample_sd(interval_values)
    amplitude_sd = sample_sd(amplitude_values)
    actual = {
        "cycle_count": float(len(interval_values)),
        "mean_interval_s": interval_mean,
        "breathing_rate_bpm": 60.0 / interval_mean,
        "interval_sd_s": interval_sd,
        "interval_cv": interval_sd / interval_mean,
        "mean_amplitude_01": amplitude_mean,
        "amplitude_sd_01": amplitude_sd,
        "amplitude_cv": amplitude_sd / amplitude_mean,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("complexity_status") != "underfilled":
        errors.append(f"{case_id}:complexity_status")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_hrvb_amplitude_golden(
    package: PackageBundle, golden: dict[str, Any]
) -> list[str]:
    errors = validate_golden_links(
        package,
        golden,
        {
            "package_id": "package.polar_h10",
            "module_id": "module.polar_h10.hrvb_resonance_amplitude",
            "input_stream_id": "stream.polar_h10.hr_rr",
            "output_stream_id": "stream.polar_h10.hrvb_resonance_amplitude",
            "source_id": "source.method.hrvb_resonance_amplitude",
        },
    )
    errors += validate_holding_cases(
        golden, {"issue.window_underfilled", "issue.frequency_out_of_band"}
    )
    settings = golden.get("settings", {})
    if not isinstance(settings, dict):
        return errors + ["settings"]
    band = frequency_band(settings.get("frequency_band_hz"), "frequency_band_hz")
    if numeric(settings.get("window_seconds")) != 30.0:
        errors.append("settings.window_seconds")
    if numeric(settings.get("source_threshold_bpm")) != 2.0:
        errors.append("settings.source_threshold_bpm")
    for case in golden.get("cases", []):
        if isinstance(case, dict):
            errors += validate_hrvb_amplitude_case(case, band)
    return errors


def validate_hrvb_amplitude_case(
    case: dict[str, Any], band: tuple[float, float]
) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    expected = case.get("expected", {})
    if not isinstance(expected, dict):
        return [f"{case_id}:expected"]
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    generator = case.get("input", {}).get("generator", {})
    if not isinstance(generator, dict):
        return [f"{case_id}:generator"]
    frequency_hz = numeric(generator.get("frequency_hz"))
    if not in_band(frequency_hz, band):
        errors.append(f"{case_id}:frequency_hz")
    amplitude = numeric(generator.get("amplitude_bpm"))
    mean_hr = numeric(generator.get("mean_hr_bpm"))
    phase = numeric(generator.get("phase_rad"))
    omega = 2.0 * math.pi * frequency_hz
    actual = {
        "amplitude_bpm": amplitude,
        "mean_hr_bpm": mean_hr,
        "frequency_hz": frequency_hz,
        "omega_rad_s": omega,
        "phase_rad": phase,
        "median_session_amplitude_bpm": amplitude,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("threshold_status") != "above_source_threshold":
        errors.append(f"{case_id}:threshold_status")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def sample_sd(values: list[float]) -> float:
    mean = sum(values) / len(values)
    return math.sqrt(
        sum((value - mean) * (value - mean) for value in values) / (len(values) - 1)
    )


def synthesize_rr_window(fft_length: int, case_input: dict[str, Any]) -> list[float]:
    base_rr_ms = numeric(case_input.get("base_rr_ms"))
    components = case_input.get("components", [])
    if not isinstance(components, list) or not components:
        raise ValueError("components")

    samples: list[float] = []
    for sample_index in range(fft_length):
        sample = base_rr_ms
        for component in components:
            if not isinstance(component, dict):
                raise ValueError("component")
            bin_index = component.get("bin")
            if not isinstance(bin_index, int) or bin_index <= 0 or bin_index > fft_length // 2:
                raise ValueError("component.bin")
            amplitude_ms = numeric(component.get("amplitude_ms"))
            phase_rad = numeric(component.get("phase_rad"))
            sample += amplitude_ms * math.sin(
                (2.0 * math.pi * float(bin_index) * float(sample_index) / float(fft_length))
                + phase_rad
            )
        samples.append(sample)
    return samples


def dft_power_by_bin(samples: list[float], sample_rate_hz: float) -> list[tuple[int, float, float]]:
    fft_length = len(samples)
    powers: list[tuple[int, float, float]] = []
    for bin_index in range(1, (fft_length // 2) + 1):
        real = 0.0
        imaginary = 0.0
        for sample_index, sample in enumerate(samples):
            angle = -2.0 * math.pi * float(bin_index) * float(sample_index) / float(fft_length)
            real += sample * math.cos(angle)
            imaginary += sample * math.sin(angle)
        frequency = float(bin_index) * sample_rate_hz / float(fft_length)
        power = ((real * real) + (imaginary * imaginary)) / float(fft_length * fft_length)
        powers.append((bin_index, frequency, power))
    return powers


def frequency_band(value: Any, field_name: str) -> tuple[float, float]:
    if (
        not isinstance(value, list)
        or len(value) != 2
        or numeric(value[0]) < 0.0
        or numeric(value[1]) <= numeric(value[0])
    ):
        raise ValueError(field_name)
    return (numeric(value[0]), numeric(value[1]))


def in_band(frequency: float, band: tuple[float, float]) -> bool:
    return band[0] <= frequency <= band[1]


def within_tolerance(actual: float | str, expected: float, tolerance: float) -> bool:
    if not isinstance(actual, int | float):
        return False
    if not math.isfinite(float(actual)) or not math.isfinite(expected):
        return float(actual) == expected
    return abs(float(actual) - expected) <= tolerance


def validate_projected_motion_breath(
    prefix: str, package: PackageBundle, checks: list[Check]
) -> None:
    if package.manifest.get("package_id") != "package.projected_motion_breath":
        return

    module_ids = {module["module_id"] for module in package.modules}
    stream_ids = {stream["stream_id"] for stream in package.streams}
    command_ids = {command["command_id"] for command in package.commands}

    required_modules = {
        "module.motion.object_pose_provider",
        "module.motion.vector_provider",
        "module.breath.projected_motion",
        "module.breath.dynamics",
        "module.breath.feedback_sink",
    }
    required_streams = {
        "stream.motion.object_pose",
        "stream.motion.vector3",
        "stream.breath.volume",
        "stream.breath.dynamics",
        "stream.breath.feedback_state",
    }
    required_commands = {
        "command.breath.configure",
        "command.breath.set_profile",
        "command.breath.begin_calibration",
        "command.breath.reset_calibration",
        "command.breath.status",
    }

    missing_contract = sorted(
        (required_modules - module_ids)
        | (required_streams - stream_ids)
        | (required_commands - command_ids)
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_contract",
        not missing_contract,
        "projected-motion modules, streams, and commands are exported",
        f"missing projected-motion ids: {missing_contract}",
    )

    profile_errors = validate_projected_motion_profile_fixture(package)
    profile_errors += validate_projected_motion_command_fixtures(package, command_ids)
    append_check(
        checks,
        f"{prefix}.projected_motion_profile_commands",
        not profile_errors,
        "projected-motion profile and command payload fixtures validate",
        f"profile or command issues: {profile_errors}",
    )

    golden_errors = validate_projected_motion_golden_fixture(package)
    append_check(
        checks,
        f"{prefix}.projected_motion_goldens",
        not golden_errors,
        "projected-motion processor golden fixture recomputes expected outputs",
        f"projected-motion golden issues: {golden_errors}",
    )

    source_adapter_errors = validate_projected_motion_source_adapters(
        package,
        module_ids,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_source_adapters",
        not source_adapter_errors,
        "projected-motion source adapter descriptors map source shapes to pose/vector streams",
        f"projected-motion source adapter issues: {source_adapter_errors}",
    )

    source_binding_errors = validate_projected_motion_source_bindings(
        package,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_source_bindings",
        not source_binding_errors,
        "projected-motion source binding fixtures map profile intent to selected source streams",
        f"projected-motion source binding issues: {source_binding_errors}",
    )

    adapter_normalization_errors = validate_projected_motion_adapter_normalization(
        package,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_adapter_normalization",
        not adapter_normalization_errors,
        "projected-motion adapter normalization fixtures produce processor input samples",
        f"projected-motion adapter normalization issues: {adapter_normalization_errors}",
    )


def validate_projected_motion_source_adapters(
    package: PackageBundle,
    module_ids: set[str],
    stream_ids: set[str],
) -> list[str]:
    descriptor_set = find_one(
        package.source_adapter_descriptors,
        "descriptor_set_id",
        "descriptor_set.projected_motion_breath.source_adapters.synthetic",
    )
    if descriptor_set is None:
        return ["descriptor_set.projected_motion_breath.source_adapters.synthetic"]

    errors: list[str] = []
    if (
        descriptor_set.get("$schema")
        != "rusty.manifold.projected_motion_breath.source_adapter_descriptors.v1"
    ):
        errors.append("source_adapter_descriptors:schema")
    if descriptor_set.get("package_id") != "package.projected_motion_breath":
        errors.append("source_adapter_descriptors:package_id")
    if descriptor_set.get("target_module_id") != "module.breath.projected_motion":
        errors.append("source_adapter_descriptors:target_module_id")
    if descriptor_set.get("execution_policy") != "not_executed.schema_descriptors_only":
        errors.append("source_adapter_descriptors:execution_policy")
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if descriptor_set.get(flag) is not False:
            errors.append(f"source_adapter_descriptors:{flag}")

    adapters = descriptor_set.get("source_adapters", [])
    if not isinstance(adapters, list):
        return errors + ["source_adapter_descriptors:source_adapters"]

    required = {
        "adapter.projected_motion_breath.object_pose_generic": {
            "source_kind": "object_pose",
            "input_kind": "pose",
            "module_id": "module.motion.object_pose_provider",
            "output_stream_id": "stream.motion.object_pose",
        },
        "adapter.projected_motion_breath.vector_motion_generic": {
            "source_kind": "vector_motion",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
        "adapter.projected_motion_breath.xr_controller_pose_shape": {
            "source_kind": "xr_controller_pose",
            "input_kind": "pose",
            "module_id": "module.motion.object_pose_provider",
            "output_stream_id": "stream.motion.object_pose",
        },
        "adapter.projected_motion_breath.wearable_acceleration_shape": {
            "source_kind": "wearable_acceleration",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
        "adapter.projected_motion_breath.external_patch_stream_bridge_shape": {
            "source_kind": "external_patch_stream_bridge",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
    }
    by_id = {
        adapter.get("adapter_id"): adapter
        for adapter in adapters
        if isinstance(adapter, dict)
    }
    errors += [
        f"source_adapter:{adapter_id}:missing"
        for adapter_id in sorted(set(required) - set(by_id))
    ]
    for adapter_id, adapter in by_id.items():
        if not isinstance(adapter_id, str) or not ID_RE.match(adapter_id):
            errors.append(f"source_adapter:{adapter_id}:adapter_id")
            continue
        expected = required.get(adapter_id)
        if expected is None:
            errors.append(f"source_adapter:{adapter_id}:unexpected")
            continue
        for key, expected_value in expected.items():
            if adapter.get(key) != expected_value:
                errors.append(f"source_adapter:{adapter_id}:{key}")
        if adapter.get("module_id") not in module_ids:
            errors.append(f"source_adapter:{adapter_id}:module_link")
        if adapter.get("output_stream_id") not in stream_ids:
            errors.append(f"source_adapter:{adapter_id}:stream_link")
        if adapter.get("transport_kind") != "descriptor_only":
            errors.append(f"source_adapter:{adapter_id}:transport_kind")
        for flag in (
            "requires_platform_sdk",
            "requires_device_api",
            "runtime_adapter_included",
        ):
            if adapter.get(flag) is not False:
                errors.append(f"source_adapter:{adapter_id}:{flag}")
        sample_shape = adapter.get("sample_value_shape")
        if not isinstance(sample_shape, dict) or not sample_shape:
            errors.append(f"source_adapter:{adapter_id}:sample_value_shape")
        quality_fields = adapter.get("quality_fields", [])
        if not isinstance(quality_fields, list) or "sample_age_s" not in quality_fields:
            errors.append(f"source_adapter:{adapter_id}:quality_fields")
        for field in (
            "source_shape",
            "projection_role",
            "coordinate_frame_policy",
            "timestamp_policy",
        ):
            if not isinstance(adapter.get(field), str) or not adapter.get(field):
                errors.append(f"source_adapter:{adapter_id}:{field}")
    return errors


def validate_projected_motion_source_bindings(
    package: PackageBundle,
    stream_ids: set[str],
) -> list[str]:
    valid_bindings = read_json_dir(
        package.root / "fixtures/valid",
        glob_pattern="source-binding-*.json",
    )
    damaged_bindings = read_json_dir(
        package.root / "fixtures/damaged",
        glob_pattern="source-binding-*.json",
    )
    errors: list[str] = []
    expected_valid = {
        "binding.projected_motion_breath.synthetic.object_pose",
        "binding.projected_motion_breath.synthetic.vector_motion",
        "binding.projected_motion_breath.synthetic.external_patch_stream",
    }
    present_valid = {binding.get("binding_id") for binding in valid_bindings}
    errors += [
        f"valid_source_binding:{binding_id}"
        for binding_id in sorted(expected_valid - present_valid)
    ]
    for binding in valid_bindings:
        issue = projected_motion_source_binding_issue(package, stream_ids, binding)
        if issue is not None:
            errors.append(f"{binding.get('binding_id')}:{issue}")

    required_damaged = {
        "issue.source_adapter_missing",
        "issue.source_binding_stream_mismatch",
    }
    present_damaged = {
        str(binding.get("expected_issue_code", "")) for binding in damaged_bindings
    }
    errors += [
        f"damaged_source_binding:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for binding in damaged_bindings:
        expected = str(binding.get("expected_issue_code", ""))
        actual = projected_motion_source_binding_issue(package, stream_ids, binding) or "ok"
        if expected != actual:
            errors.append(f"{binding.get('binding_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_source_binding_issue(
    package: PackageBundle,
    stream_ids: set[str],
    binding: dict[str, Any],
) -> str | None:
    if binding.get("$schema") != "rusty.manifold.projected_motion_breath.source_binding.v1":
        return "issue.source_binding_invalid"
    if not ID_RE.match(str(binding.get("binding_id", ""))):
        return "issue.source_binding_invalid"
    if binding.get("package_id") != "package.projected_motion_breath":
        return "issue.source_binding_invalid"
    if binding.get("target_module_id") != "module.breath.projected_motion":
        return "issue.source_binding_invalid"
    if binding.get("binding_policy") != "descriptor_only.owner_review_required":
        return "issue.source_binding_invalid"
    if binding.get("execution_policy") != "not_executed.schema_binding_only":
        return "issue.source_binding_invalid"
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if binding.get(flag) is not False:
            return "issue.source_binding_invalid"

    profile_path = binding.get("profile_path")
    descriptor_set_path = binding.get("descriptor_set_path")
    if not isinstance(profile_path, str) or not isinstance(descriptor_set_path, str):
        return "issue.source_binding_invalid"
    try:
        profile = read_json(package.root / profile_path)
        descriptor_set = read_json(package.root / descriptor_set_path)
    except ValueError:
        return "issue.source_binding_invalid"

    if profile.get("profile_id") != binding.get("profile_id"):
        return "issue.source_binding_invalid"
    if validate_projected_motion_profile(profile):
        return "issue.profile_invalid"

    adapters = descriptor_set.get("source_adapters", [])
    if not isinstance(adapters, list):
        return "issue.source_binding_invalid"
    adapter = find_one(adapters, "adapter_id", str(binding.get("selected_adapter_id", "")))
    if adapter is None:
        return "issue.source_adapter_missing"
    if adapter.get("source_kind") != binding.get("selected_source_kind"):
        return "issue.source_binding_stream_mismatch"
    if adapter.get("input_kind") != binding.get("selected_input_kind"):
        return "issue.source_binding_stream_mismatch"
    if adapter.get("output_stream_id") != binding.get("selected_output_stream_id"):
        return "issue.source_binding_stream_mismatch"
    source_stream_supported = binding.get("source_stream_id") == adapter.get(
        "output_stream_id"
    ) or (
        binding.get("selected_source_kind") == "wearable_acceleration"
        and binding.get("source_stream_id") == "bio:polar_acc"
    )
    if not source_stream_supported:
        return "issue.source_binding_stream_mismatch"
    if (
        binding.get("source_stream_id") not in stream_ids
        and binding.get("source_stream_id") != "bio:polar_acc"
    ):
        return "issue.source_binding_stream_mismatch"
    profile_input_kinds = profile.get("input_kinds", [])
    if (
        not isinstance(profile_input_kinds, list)
        or binding.get("selected_input_kind") not in profile_input_kinds
    ):
        return "issue.source_binding_stream_mismatch"
    return None


def validate_projected_motion_adapter_normalization(
    package: PackageBundle,
    stream_ids: set[str],
) -> list[str]:
    valid_cases = read_json_dir(
        package.root / "fixtures/valid",
        glob_pattern="adapter-normalization-*.json",
    )
    damaged_cases = read_json_dir(
        package.root / "fixtures/damaged",
        glob_pattern="adapter-normalization-*.json",
    )
    errors: list[str] = []
    expected_valid = {
        "case.projected_motion_breath.normalize.object_pose_generic",
        "case.projected_motion_breath.normalize.vector_motion",
        "case.projected_motion_breath.normalize.external_patch_vector",
    }
    present_valid = {case.get("case_id") for case in valid_cases}
    errors += [
        f"valid_adapter_normalization:{case_id}"
        for case_id in sorted(expected_valid - present_valid)
    ]
    for case in valid_cases:
        issue = projected_motion_adapter_normalization_issue(package, stream_ids, case)
        if issue is not None:
            errors.append(f"{case.get('case_id')}:{issue}")

    required_damaged = {
        "issue.adapter_payload_invalid",
        "issue.adapter_payload_kind_mismatch",
    }
    present_damaged = {str(case.get("expected_issue_code", "")) for case in damaged_cases}
    errors += [
        f"damaged_adapter_normalization:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for case in damaged_cases:
        expected = str(case.get("expected_issue_code", ""))
        actual = projected_motion_adapter_normalization_issue(package, stream_ids, case) or "ok"
        if expected != actual:
            errors.append(f"{case.get('case_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_adapter_normalization_issue(
    package: PackageBundle,
    stream_ids: set[str],
    case: dict[str, Any],
) -> str | None:
    if (
        case.get("$schema")
        != "rusty.manifold.projected_motion_breath.adapter_normalization_case.v1"
    ):
        return "issue.adapter_normalization_invalid"
    if not ID_RE.match(str(case.get("case_id", ""))):
        return "issue.adapter_normalization_invalid"
    if case.get("package_id") != "package.projected_motion_breath":
        return "issue.adapter_normalization_invalid"
    if case.get("execution_policy") != "not_executed.fixture_normalization_only":
        return "issue.adapter_normalization_invalid"
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if case.get(flag) is not False:
            return "issue.adapter_normalization_invalid"

    binding_path = case.get("binding_path")
    if not isinstance(binding_path, str):
        return "issue.source_binding_invalid"
    try:
        binding = read_json(package.root / binding_path)
    except ValueError:
        return "issue.source_binding_invalid"
    binding_issue = projected_motion_source_binding_issue(package, stream_ids, binding)
    if binding_issue is not None:
        return binding_issue
    source_payload_kind = str(case.get("source_payload_kind", ""))
    if not source_payload_kind_matches(
        str(binding.get("selected_source_kind", "")),
        source_payload_kind,
    ):
        return "issue.adapter_payload_kind_mismatch"
    normalized = normalize_adapter_payload(binding, source_payload_kind, case.get("input"))
    if isinstance(normalized, str):
        return normalized
    sample_kind, sample = normalized
    if not adapter_expected_matches(
        sample_kind,
        sample,
        str(case.get("expected_sample_kind", "")),
        case.get("expected"),
    ):
        return "issue.adapter_normalization_expected_mismatch"
    return None


def source_payload_kind_matches(selected_source_kind: str, source_payload_kind: str) -> bool:
    return (selected_source_kind, source_payload_kind) in {
        ("object_pose", "object_pose"),
        ("vector_motion", "vector_motion"),
        ("wearable_acceleration", "vector_motion"),
        ("external_patch_stream_bridge", "external_patch_channels"),
    }


def normalize_adapter_payload(
    binding: dict[str, Any],
    source_payload_kind: str,
    payload: Any,
) -> tuple[str, dict[str, Any]] | str:
    if not isinstance(payload, dict):
        return "issue.adapter_payload_invalid"
    base = normalize_adapter_base(payload)
    if isinstance(base, str):
        return base
    selected_input_kind = binding.get("selected_input_kind")
    if selected_input_kind == "pose" and source_payload_kind == "object_pose":
        return normalize_object_pose_payload(payload, base)
    if selected_input_kind == "vector3" and source_payload_kind == "vector_motion":
        vector = payload.get("vector3")
        if not finite_list(vector, 3):
            return "issue.adapter_payload_invalid"
        return normalize_vector_payload(payload, base, [float(value) for value in vector])
    if selected_input_kind == "vector3" and source_payload_kind == "external_patch_channels":
        channel_values = payload.get("channel_values")
        channel_map = payload.get("channel_map")
        if not isinstance(channel_values, dict) or not isinstance(channel_map, dict):
            return "issue.adapter_payload_invalid"
        vector: list[float] = []
        for axis in ("x", "y", "z"):
            channel_id = channel_map.get(axis)
            if not isinstance(channel_id, str):
                return "issue.adapter_payload_invalid"
            value = channel_values.get(channel_id)
            if not finite_number(value):
                return "issue.adapter_payload_invalid"
            vector.append(float(value))
        return normalize_vector_payload(payload, base, vector)
    return "issue.adapter_payload_kind_mismatch"


def normalize_adapter_base(payload: dict[str, Any]) -> dict[str, Any] | str:
    source_id = payload.get("source_id")
    frame_id = payload.get("frame_id")
    sample_time_s = payload.get("sample_time_s")
    host_time_s = payload.get("host_time_s")
    if (
        not isinstance(source_id, str)
        or not source_id
        or not isinstance(frame_id, str)
        or not frame_id
        or not finite_number(sample_time_s)
        or not finite_number(host_time_s)
    ):
        return "issue.adapter_payload_invalid"
    return {
        "source_id": source_id,
        "sample_time_s": float(sample_time_s),
        "host_time_s": float(host_time_s),
        "frame_id": frame_id,
    }


def normalize_object_pose_payload(
    payload: dict[str, Any],
    base: dict[str, Any],
) -> tuple[str, dict[str, Any]] | str:
    position = payload.get("position_m")
    orientation = payload.get("orientation_xyzw")
    tracking01 = payload.get("tracking01")
    connected = payload.get("connected")
    tracked = payload.get("tracked")
    if (
        not finite_list(position, 3)
        or not finite_list(orientation, 4)
        or not unit_interval(tracking01)
        or not isinstance(connected, bool)
        or not isinstance(tracked, bool)
    ):
        return "issue.adapter_payload_invalid"
    sample = dict(base)
    sample.update(
        {
            "position_m": [float(value) for value in position],
            "orientation_xyzw": [float(value) for value in orientation],
            "connected": connected,
            "tracked": tracked,
            "quality01": float(tracking01),
        }
    )
    return "rigid_motion", sample


def normalize_vector_payload(
    payload: dict[str, Any],
    base: dict[str, Any],
    vector: list[float],
) -> tuple[str, dict[str, Any]] | str:
    units = payload.get("units")
    quality01 = payload.get("quality01")
    if not isinstance(units, str) or not units or not unit_interval(quality01):
        return "issue.adapter_payload_invalid"
    sample = dict(base)
    sample.update(
        {
            "vector3": vector,
            "units": units,
            "quality01": float(quality01),
        }
    )
    return "vector_motion", sample


def adapter_expected_matches(
    sample_kind: str,
    sample: dict[str, Any],
    expected_sample_kind: str,
    expected: Any,
) -> bool:
    if sample_kind != expected_sample_kind or not isinstance(expected, dict):
        return False
    for field in ("source_id", "frame_id"):
        if sample.get(field) != expected.get(field):
            return False
    for field in ("sample_time_s", "host_time_s", "quality01"):
        if not float_close(sample.get(field), expected.get(field)):
            return False
    if sample_kind == "rigid_motion":
        return (
            list_close(sample.get("position_m"), expected.get("position_m"))
            and list_close(sample.get("orientation_xyzw"), expected.get("orientation_xyzw"))
            and sample.get("connected") == expected.get("connected")
            and sample.get("tracked") == expected.get("tracked")
        )
    if sample_kind == "vector_motion":
        return (
            list_close(sample.get("vector3"), expected.get("vector3"))
            and sample.get("units") == expected.get("units")
        )
    return False


def validate_projected_motion_profile_fixture(package: PackageBundle) -> list[str]:
    profile_path = package.root / "fixtures/valid/profile-synthetic.json"
    profile = read_json(profile_path)
    return prefix_errors("profile.synthetic", validate_projected_motion_profile(profile))


def validate_projected_motion_profile(profile: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if profile.get("$schema") != "rusty.motion_breath_profile.v1":
        errors.append("issue.profile_invalid:schema")
    if profile.get("target_module_id") != "module.breath.projected_motion":
        errors.append("issue.profile_invalid:target_module_id")
    input_kinds = profile.get("input_kinds", [])
    if not isinstance(input_kinds, list) or {"pose", "vector3"} - set(input_kinds):
        errors.append("issue.profile_invalid:input_kinds")

    projection = profile.get("projection", {})
    if not isinstance(projection, dict):
        errors.append("issue.profile_invalid:projection")
    else:
        errors += validate_projected_motion_projection(projection)

    calibration = profile.get("calibration", {})
    if not isinstance(calibration, dict):
        errors.append("issue.profile_invalid:calibration")
    else:
        errors += validate_projected_motion_calibration(calibration)

    normalization = profile.get("normalization", {})
    if not isinstance(normalization, dict):
        errors.append("issue.profile_invalid:normalization")
    else:
        if numeric(normalization.get("soft_margin")) < 0.0:
            errors.append("issue.profile_invalid:soft_margin")
        if numeric(normalization.get("edge_ease")) < 0.0:
            errors.append("issue.profile_invalid:edge_ease")
        if numeric(normalization.get("progress_gamma")) <= 0.0:
            errors.append("issue.profile_invalid:progress_gamma")

    smoothing = profile.get("smoothing", {})
    if not isinstance(smoothing, dict):
        errors.append("issue.profile_invalid:smoothing")
    else:
        if numeric(smoothing.get("analysis_rate_hz")) <= 0.0:
            errors.append("issue.profile_invalid:analysis_rate_hz")
        if not isinstance(smoothing.get("median_window"), int) or smoothing.get("median_window") <= 0:
            errors.append("issue.profile_invalid:median_window")
        ema_alpha = numeric(smoothing.get("ema_alpha"))
        if ema_alpha <= 0.0 or ema_alpha > 1.0:
            errors.append("issue.profile_invalid:ema_alpha")

    classifier = profile.get("classifier", {})
    if not isinstance(classifier, dict):
        errors.append("issue.profile_invalid:classifier")
    else:
        errors += validate_projected_motion_classifier(classifier)

    quality = profile.get("quality", {})
    if not isinstance(quality, dict):
        errors.append("issue.profile_invalid:quality")
    else:
        min_quality = numeric(quality.get("min_quality01"))
        if min_quality < 0.0 or min_quality > 1.0:
            errors.append("issue.profile_invalid:min_quality01")
    return errors


def validate_projected_motion_projection(projection: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    allowed_modes = {
        "principal_motion_axis",
        "fixed_axis",
        "orientation_axis",
        "vector_component",
        "gravity_relative_vector",
    }
    mode = projection.get("mode")
    fallback = projection.get("fallback_mode")
    if mode not in allowed_modes:
        errors.append("issue.projection_unsupported:mode")
    if fallback is not None and fallback not in allowed_modes:
        errors.append("issue.projection_unsupported:fallback_mode")
    if mode == "fixed_axis" or fallback == "fixed_axis":
        axis = projection.get("fixed_axis")
        if not finite_nonzero_axis(axis):
            errors.append("issue.profile_invalid:fixed_axis")
    return errors


def validate_projected_motion_calibration(calibration: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if not isinstance(calibration.get("accepted_sample_count"), int) or calibration.get(
        "accepted_sample_count"
    ) <= 0:
        errors.append("issue.profile_invalid:accepted_sample_count")
    if numeric(calibration.get("min_accepted_delta")) < 0.0:
        errors.append("issue.profile_invalid:min_accepted_delta")
    if numeric(calibration.get("min_span")) <= 0.0:
        errors.append("issue.profile_invalid:min_span")
    if not valid_quantile_pair(
        numeric(calibration.get("lower_quantile")),
        numeric(calibration.get("upper_quantile")),
    ):
        errors.append("issue.profile_invalid:quantiles")
    return errors


def validate_projected_motion_classifier(classifier: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if numeric(classifier.get("delta_threshold")) < 0.0:
        errors.append("issue.profile_invalid:delta_threshold")
    if numeric(classifier.get("stale_timeout_s")) <= 0.0:
        errors.append("issue.profile_invalid:stale_timeout_s")
    return errors


def validate_projected_motion_command_fixtures(
    package: PackageBundle, command_ids: set[str]
) -> list[str]:
    errors: list[str] = []
    valid_payloads = read_json_dir(package.root / "fixtures/valid", glob_pattern="command-*.json")
    damaged_payloads = read_json_dir(
        package.root / "fixtures/damaged", glob_pattern="command-*.json"
    )
    expected_valid = {
        "command.breath.configure",
        "command.breath.set_profile",
        "command.breath.begin_calibration",
        "command.breath.reset_calibration",
        "command.breath.status",
    }
    present_valid = {payload.get("command_id") for payload in valid_payloads}
    errors += [f"valid_command:{command_id}" for command_id in sorted(expected_valid - present_valid)]

    for payload in valid_payloads:
        command_id = str(payload.get("command_id", ""))
        if command_id not in command_ids:
            errors.append(f"{payload.get('request_id')}:command_id")
            continue
        issue = projected_motion_command_issue(package, payload)
        if issue is not None:
            errors.append(f"{payload.get('request_id')}:{issue}")

    required_damaged = {
        "issue.profile_invalid",
        "issue.projection_unsupported",
        "issue.calibration_invalid",
        "issue.source_stale",
        "issue.motion_quality_low",
    }
    present_damaged = {
        str(payload.get("expected_issue_code", "")) for payload in damaged_payloads
    }
    errors += [
        f"damaged_command:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for payload in damaged_payloads:
        expected = str(payload.get("expected_issue_code", ""))
        actual = projected_motion_command_issue(package, payload) or "ok"
        if expected != actual:
            errors.append(f"{payload.get('request_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_command_issue(package: PackageBundle, payload: dict[str, Any]) -> str | None:
    if not ID_RE.match(str(payload.get("request_id", ""))):
        return "issue.profile_invalid"
    if payload.get("target_module_id") != "module.breath.projected_motion":
        return "issue.profile_invalid"
    command_id = payload.get("command_id")
    if command_id == "command.breath.set_profile":
        profile_path = payload.get("profile_path")
        if not isinstance(profile_path, str):
            return "issue.profile_invalid"
        profile_errors = validate_projected_motion_profile(read_json(package.root / profile_path))
        return first_projected_motion_issue(profile_errors)
    if command_id == "command.breath.configure":
        patch = payload.get("profile_patch")
        if not isinstance(patch, dict):
            return "issue.profile_invalid"
        return first_projected_motion_issue(validate_projected_motion_profile_patch(patch))
    if command_id == "command.breath.begin_calibration":
        streams = payload.get("source_stream_ids", [])
        if not isinstance(streams, list) or not set(streams).intersection(
            {"stream.motion.object_pose", "stream.motion.vector3"}
        ):
            return "issue.profile_invalid"
        projection = payload.get("calibration_projection", [])
        if not isinstance(projection, list) or len(set(projection)) <= 1:
            return "issue.calibration_invalid"
        source_status = payload.get("source_status")
        if isinstance(source_status, dict):
            if numeric(source_status.get("sample_age_s")) > numeric(source_status.get("stale_timeout_s")):
                return "issue.source_stale"
            if numeric(source_status.get("quality01")) < numeric(source_status.get("min_quality01")):
                return "issue.motion_quality_low"
        return None
    if command_id in {"command.breath.reset_calibration", "command.breath.status"}:
        return None
    return "issue.profile_invalid"


def validate_projected_motion_profile_patch(patch: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    projection = patch.get("projection")
    if isinstance(projection, dict):
        errors += validate_projected_motion_projection(projection)
    calibration = patch.get("calibration")
    if isinstance(calibration, dict):
        lower = numeric(calibration.get("lower_quantile", 0.05))
        upper = numeric(calibration.get("upper_quantile", 0.95))
        if not valid_quantile_pair(lower, upper):
            errors.append("issue.profile_invalid:quantiles")
        if "min_span" in calibration and numeric(calibration.get("min_span")) <= 0.0:
            errors.append("issue.profile_invalid:min_span")
    classifier = patch.get("classifier")
    if isinstance(classifier, dict):
        errors += validate_projected_motion_classifier(classifier)
    quality = patch.get("quality")
    if isinstance(quality, dict) and "min_quality01" in quality:
        min_quality = numeric(quality.get("min_quality01"))
        if min_quality < 0.0 or min_quality > 1.0:
            errors.append("issue.profile_invalid:min_quality01")
    return errors


def validate_projected_motion_golden_fixture(package: PackageBundle) -> list[str]:
    golden = find_one(
        package.processing_goldens,
        "golden_id",
        "golden.projected_motion_breath.pose_and_vector_projection",
    )
    if golden is None:
        return ["golden.projected_motion_breath.pose_and_vector_projection"]

    errors: list[str] = []
    if golden.get("package_id") != "package.projected_motion_breath":
        errors.append(f"package_id:{golden.get('package_id')}")
    if golden.get("module_id") != "module.breath.projected_motion":
        errors.append(f"module_id:{golden.get('module_id')}")
    if golden.get("output_stream_id") != "stream.breath.volume":
        errors.append(f"output_stream_id:{golden.get('output_stream_id')}")
    input_stream_ids = set(golden.get("input_stream_ids", []))
    for stream_id in {"stream.motion.object_pose", "stream.motion.vector3"}:
        if stream_id not in input_stream_ids:
            errors.append(f"input_stream_id:{stream_id}")

    settings = golden.get("settings", {})
    quantiles = settings.get("calibration_quantiles", []) if isinstance(settings, dict) else []
    if not isinstance(quantiles, list) or len(quantiles) != 2 or not valid_quantile_pair(
        numeric(quantiles[0] if len(quantiles) > 0 else None),
        numeric(quantiles[1] if len(quantiles) > 1 else None),
    ):
        errors.append("settings.calibration_quantiles")
        quantiles = [0.0, 1.0]

    cases = golden.get("cases", [])
    if not isinstance(cases, list) or len(cases) < 2:
        errors.append("cases")
    else:
        for case in cases:
            if isinstance(case, dict):
                errors += validate_projected_motion_case(case, quantiles)
            else:
                errors.append("case")

    damaged_cases = golden.get("damaged_cases", [])
    if not isinstance(damaged_cases, list) or not damaged_cases:
        errors.append("damaged_cases")
    else:
        present = {
            str(case.get("expected_issue_code", "")) for case in damaged_cases if isinstance(case, dict)
        }
        for issue_code in {"issue.calibration_invalid", "issue.source_stale"} - present:
            errors.append(f"damaged_issue:{issue_code}")
        for damaged_case in damaged_cases:
            if isinstance(damaged_case, dict):
                errors += validate_projected_motion_damaged_case(damaged_case)
            else:
                errors.append("damaged_case")
    return errors


def validate_projected_motion_case(case: dict[str, Any], quantiles: list[Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    case_input = case.get("input", {})
    expected = case.get("expected", {})
    if not isinstance(case_input, dict) or not isinstance(expected, dict):
        return [f"{case_id}:shape"]
    calibration = case_input.get("calibration_projection", [])
    if not isinstance(calibration, list) or not calibration:
        return [f"{case_id}:calibration_projection"]
    values = [numeric(item) for item in calibration]
    lower_bound = nearest_quantile_value(values, numeric(quantiles[0]))
    upper_bound = nearest_quantile_value(values, numeric(quantiles[1]))
    if upper_bound <= lower_bound:
        return [f"{case_id}:bounds"]
    live_projection = numeric(case_input.get("live_projection"))
    previous_projection = numeric(case_input.get("previous_projection"))
    volume = max(0.0, min(1.0, (live_projection - lower_bound) / (upper_bound - lower_bound)))
    phase = "inhale" if live_projection > previous_projection else "exhale"
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    actual = {
        "lower_bound": lower_bound,
        "upper_bound": upper_bound,
        "volume01": volume,
        "tracking01": 1.0,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("phase") != phase:
        errors.append(f"{case_id}:phase")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_projected_motion_damaged_case(case: dict[str, Any]) -> list[str]:
    case_id = str(case.get("case_id", ""))
    expected = str(case.get("expected_issue_code", ""))
    case_input = case.get("input", {})
    actual = "ok"
    if isinstance(case_input, dict):
        calibration = case_input.get("calibration_projection", [])
        if isinstance(calibration, list) and calibration and len(set(calibration)) <= 1:
            actual = "issue.calibration_invalid"
        elif numeric(case_input.get("sample_age_s")) > numeric(case_input.get("stale_timeout_s")):
            actual = "issue.source_stale"
    if actual != expected:
        return [f"{case_id}:expected:{expected}:actual:{actual}"]
    return []


def finite_nonzero_axis(value: Any) -> bool:
    if not isinstance(value, list) or len(value) != 3:
        return False
    values = [numeric(item) for item in value]
    return all(math.isfinite(item) for item in values) and sum(item * item for item in values) > 0.0


def valid_quantile_pair(lower: float, upper: float) -> bool:
    return (
        math.isfinite(lower)
        and math.isfinite(upper)
        and 0.0 <= lower <= 1.0
        and 0.0 <= upper <= 1.0
        and lower < upper
    )


def nearest_quantile_value(values: list[float], quantile: float) -> float:
    sorted_values = sorted(values)
    index = int(math.floor((max(0.0, min(1.0, quantile)) * (len(sorted_values) - 1)) + 0.5))
    return sorted_values[index]


def first_projected_motion_issue(errors: list[str]) -> str | None:
    if not errors:
        return None
    issue = errors[0].split(":", 1)[0]
    if issue in {
        "issue.calibration_invalid",
        "issue.motion_quality_low",
        "issue.profile_invalid",
        "issue.projection_unsupported",
        "issue.source_stale",
    }:
        return issue
    return "issue.profile_invalid"


def prefix_errors(prefix: str, errors: list[str]) -> list[str]:
    return [f"{prefix}:{error}" for error in errors]


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
        "module.polar_h10.rmssd_gain",
        "module.polar_h10.coherence",
        "module.polar_h10.breath_dynamics",
        "module.polar_h10.hrvb_resonance_amplitude",
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
    deployment_fixture_backends = {
        "backend.synthetic",
        "backend.replay",
        "backend.desktop_wireless",
        "backend.mobile_wireless",
        "backend.headset_wireless",
    }
    runtime_fixture_backends = {"backend.synthetic", "backend.replay"}
    backend_errors = sorted(required_backend_support - support)
    backend_errors += [
        f"deployment:{backend}"
        for backend in sorted(deployment_fixture_backends - deployment_backends)
    ]
    backend_errors += [
        f"runtime:{backend}" for backend in sorted(runtime_fixture_backends - runtime_backends)
    ]
    append_check(
        checks,
        f"{prefix}.polar_backend_evidence",
        not backend_errors,
        "Polar backend support plus synthetic and replay fixture evidence are present",
        f"backend evidence issues: {backend_errors}",
    )

    handoffs = [
        handoff
        for handoff_doc in package.pmd_handoffs
        for handoff in handoff_doc.get("handoffs", [])
    ]
    required_handoff_ids = {
        "handoff.polar_h10.desktop_to_headset_raw_pmd",
        "handoff.polar_h10.headset_to_desktop_raw_pmd",
        "handoff.polar_h10.mobile_to_headset_raw_pmd",
        "handoff.polar_h10.headset_to_mobile_raw_pmd",
    }
    present_handoff_ids = {handoff.get("handoff_id") for handoff in handoffs}
    required_evidence_fields = {
        "handoff_id",
        "previous_owner_backend",
        "next_owner_backend",
        "source_device_id",
        "handoff_phase",
        "release_elapsed_ms",
        "first_frame_elapsed_ms",
        "settings_fingerprint",
        "source_timestamp_anchor",
        "host_timestamp_anchor",
    }
    required_phases = {
        "phase.stop_previous_pmd",
        "phase.release_previous_owner",
        "phase.observe_source_advertisement",
        "phase.connect_next_owner",
        "phase.match_settings",
        "phase.start_next_pmd",
        "phase.observe_first_frame",
    }
    handoff_errors = sorted(required_handoff_ids - present_handoff_ids)
    for handoff in handoffs:
        handoff_id = str(handoff.get("handoff_id", ""))
        if not ID_RE.match(handoff_id):
            handoff_errors.append(f"{handoff_id}:handoff_id")
        if handoff.get("owner_policy") != "serial_handoff":
            handoff_errors.append(f"{handoff_id}:owner_policy")
        if handoff.get("previous_owner_backend") == handoff.get("next_owner_backend"):
            handoff_errors.append(f"{handoff_id}:owner_transition")
        streams = set(handoff.get("streams", []))
        if not {"stream.polar_h10.ecg", "stream.polar_h10.acc"}.issubset(streams):
            handoff_errors.append(f"{handoff_id}:streams")
        if not required_phases.issubset(set(handoff.get("required_phases", []))):
            handoff_errors.append(f"{handoff_id}:required_phases")
        if not required_evidence_fields.issubset(set(handoff.get("evidence_fields", []))):
            handoff_errors.append(f"{handoff_id}:evidence_fields")
    append_check(
        checks,
        f"{prefix}.polar_pmd_handoffs",
        not handoff_errors,
        "Polar raw PMD host handoff workflows are explicit and evidence-scoped",
        f"PMD handoff fixture issues: {handoff_errors}",
    )


def validate_polar_completion_evidence(
    prefix: str, package: PackageBundle, checks: list[Check]
) -> None:
    if package.manifest.get("package_id") != "package.polar_h10":
        return

    completion = find_one(
        package.completion_evidence,
        "completion_id",
        "completion.polar_h10.pmd_on_device",
    )
    errors: list[str] = []
    if completion is None:
        errors.append("completion.polar_h10.pmd_on_device")
    else:
        if completion.get("completion_status") != "complete":
            errors.append("completion_status")
        if completion.get("status") != "pass":
            errors.append("status")

        summary = completion.get("evidence_summary", {})
        required_summary_flags = {
            "raw_pmd_single_owner",
            "hr_rr_dual_receiver_is_observer_only",
            "desktop_control_failures_are_not_sample_rate_failures",
            "separates_notification_cadence_from_sensor_sample_rate",
        }
        errors += sorted(flag for flag in required_summary_flags if summary.get(flag) is not True)

        required_fields = set(completion.get("required_evidence_fields", []))
        expected_fields = {
            "handoff_sequence_id",
            "leg_order",
            "previous_owner_backend",
            "next_owner_backend",
            "stream_id",
            "requested_settings_fingerprint",
            "applied_settings_fingerprint",
            "owner_release_at",
            "advertisement_seen_at",
            "connect_started_at",
            "services_discovered_at",
            "settings_read_at",
            "pmd_start_ack_at",
            "first_pmd_frame_at",
            "notification_cadence_hz",
            "sensor_sample_rate_hz",
            "frame_sample_count",
            "decoded_sample_count",
            "payload_size_bytes",
            "max_pdu_size",
            "connection_mode",
            "connection_priority",
            "hr_subscription_state",
            "backend_id",
            "control_write_status",
            "service_cache_status",
            "stop_command_status",
            "rejection_code",
        }
        errors += [f"required_field:{field}" for field in sorted(expected_fields - required_fields)]

        legs = {leg.get("leg_id"): leg for leg in completion.get("legs", [])}
        required_leg_ids = {
            "leg.polar_h10.headset_acc_200_initial",
            "leg.polar_h10.headset_ecg_initial",
            "leg.polar_h10.desktop_acc_200_success",
            "leg.polar_h10.desktop_control_session_fragility",
            "leg.polar_h10.hr_rr_dual_observer",
            "leg.polar_h10.headset_acc_200_reacquire",
            "leg.polar_h10.headset_ecg_reacquire",
        }
        errors += [f"leg:{leg_id}" for leg_id in sorted(required_leg_ids - set(legs))]

        require_rate_leg(
            legs,
            errors,
            "leg.polar_h10.headset_acc_200_initial",
            stream_id="stream.polar_h10.acc",
            backend_id="backend.headset_wireless",
            min_rate_hz=190.0,
            min_samples=3000,
        )
        require_rate_leg(
            legs,
            errors,
            "leg.polar_h10.headset_acc_200_reacquire",
            stream_id="stream.polar_h10.acc",
            backend_id="backend.headset_wireless",
            min_rate_hz=190.0,
            min_samples=3000,
        )
        require_rate_leg(
            legs,
            errors,
            "leg.polar_h10.headset_ecg_initial",
            stream_id="stream.polar_h10.ecg",
            backend_id="backend.headset_wireless",
            min_rate_hz=120.0,
            min_samples=1500,
        )
        require_rate_leg(
            legs,
            errors,
            "leg.polar_h10.headset_ecg_reacquire",
            stream_id="stream.polar_h10.ecg",
            backend_id="backend.headset_wireless",
            min_rate_hz=120.0,
            min_samples=1500,
        )

        desktop_success = legs.get("leg.polar_h10.desktop_acc_200_success", {})
        if desktop_success:
            require_rate_leg(
                legs,
                errors,
                "leg.polar_h10.desktop_acc_200_success",
                stream_id="stream.polar_h10.acc",
                backend_id="backend.desktop_wireless",
                min_rate_hz=180.0,
                min_samples=3000,
            )
            observer = desktop_success.get("observer", {})
            if observer.get("backend_id") != "backend.headset_wireless":
                errors.append("desktop_success:observer_backend")
            if numeric(observer.get("sensor_sample_rate_hz")) < 190.0:
                errors.append("desktop_success:observer_sample_rate")
            if numeric(observer.get("decoded_sample_count")) < 3000.0:
                errors.append("desktop_success:observer_samples")
            if numeric(desktop_success.get("mean_source_to_forward_ms")) > 5.0:
                errors.append("desktop_success:source_to_forward_delay")

        control_fragility = legs.get("leg.polar_h10.desktop_control_session_fragility", {})
        if control_fragility:
            if control_fragility.get("outcome") != "control_session_failure":
                errors.append("control_fragility:outcome")
            if control_fragility.get("data_rate_verdict") != "not_evaluated":
                errors.append("control_fragility:data_rate_verdict")

        hr_rr = legs.get("leg.polar_h10.hr_rr_dual_observer", {})
        if hr_rr:
            if hr_rr.get("raw_pmd_enabled") is not False:
                errors.append("hr_rr_dual:raw_pmd_enabled")
            if hr_rr.get("outcome") != "observer_only":
                errors.append("hr_rr_dual:outcome")

    append_check(
        checks,
        f"{prefix}.polar_completion_evidence",
        not errors,
        "Polar on-device completion evidence covers PMD handoff, reacquire, and observer-only HR/RR cases",
        f"completion evidence issues: {errors}",
    )


def find_one(items: list[dict[str, Any]], key: str, value: str) -> dict[str, Any] | None:
    for item in items:
        if item.get(key) == value:
            return item
    return None


def require_rate_leg(
    legs: dict[Any, dict[str, Any]],
    errors: list[str],
    leg_id: str,
    *,
    stream_id: str,
    backend_id: str,
    min_rate_hz: float,
    min_samples: int,
) -> None:
    leg = legs.get(leg_id)
    if not leg:
        return
    if leg.get("outcome") != "pass":
        errors.append(f"{leg_id}:outcome")
    if leg.get("stream_id") != stream_id:
        errors.append(f"{leg_id}:stream_id")
    if leg.get("backend_id") != backend_id:
        errors.append(f"{leg_id}:backend_id")
    if numeric(leg.get("sensor_sample_rate_hz")) < min_rate_hz:
        errors.append(f"{leg_id}:sensor_sample_rate_hz")
    if numeric(leg.get("decoded_sample_count")) < float(min_samples):
        errors.append(f"{leg_id}:decoded_sample_count")
    if numeric(leg.get("frame_sample_count")) <= 0.0:
        errors.append(f"{leg_id}:frame_sample_count")


def numeric(value: Any) -> float:
    if isinstance(value, int | float):
        return float(value)
    return 0.0


def finite_number(value: Any) -> bool:
    return isinstance(value, int | float) and math.isfinite(float(value))


def unit_interval(value: Any) -> bool:
    return finite_number(value) and 0.0 <= float(value) <= 1.0


def finite_list(value: Any, length: int) -> bool:
    return (
        isinstance(value, list)
        and len(value) == length
        and all(finite_number(item) for item in value)
    )


def float_close(left: Any, right: Any, tolerance: float = 0.000_000_001) -> bool:
    return finite_number(left) and finite_number(right) and abs(float(left) - float(right)) <= tolerance


def list_close(left: Any, right: Any, tolerance: float = 0.000_000_001) -> bool:
    return (
        isinstance(left, list)
        and isinstance(right, list)
        and len(left) == len(right)
        and all(float_close(left_item, right_item, tolerance) for left_item, right_item in zip(left, right))
    )


def package_contains_text(package_root: Path, needle: str) -> bool:
    for path in sorted(package_root.rglob("*")):
        if path.is_dir() or path.suffix.lower() not in {".json", ".md", ".txt"}:
            continue
        if needle in path.read_text(encoding="utf-8"):
            return True
    return False


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
