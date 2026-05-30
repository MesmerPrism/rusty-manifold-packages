#!/usr/bin/env python3
"""Validate first-party Manifold package manifests."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?)*$")
FORBIDDEN_TERMS = [
    "polar",
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


def contains_forbidden_term(text: str, term: str) -> bool:
    if "\\" in term or ":" in term:
        return term in text
    pattern = rf"(?<![a-z0-9]){re.escape(term)}(?![a-z0-9])"
    return re.search(pattern, text) is not None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    report = validate_repo(repo_root)
    print(json.dumps(report.to_json(), indent=2))
    return 0 if report.status == "pass" else 1


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
        if path.is_dir() or ".git" in path.parts or "__pycache__" in path.parts:
            continue
        if path.relative_to(repo_root).as_posix() == "tools/check_packages.py":
            continue
        if path.suffix.lower() not in {".json", ".md", ".py", ".toml", ".txt"}:
            continue
        text = path.read_text(encoding="utf-8")
        lower = text.lower()
        for term in FORBIDDEN_TERMS:
            if contains_forbidden_term(lower, term):
                offenders.append(f"{path.relative_to(repo_root)} contains {term}")
    if offenders:
        checks.append(fail("validation.public_boundary_terms", "; ".join(offenders)))
    else:
        checks.append(pass_check("validation.public_boundary_terms", "no forbidden terms found"))


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

    validate_dotted_ids(prefix, ids, checks)
    validate_exports(prefix, package, ids, checks)
    validate_module_links(prefix, package, ids, checks)
    validate_stream_links(prefix, package, ids, checks)
    validate_graph_links(prefix, package, ids, checks)
    validate_deployment_links(prefix, package, ids, checks)


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
    if invalid:
        checks.append(fail(f"{prefix}.dotted_ids", f"invalid ids: {invalid}"))
    else:
        checks.append(pass_check(f"{prefix}.dotted_ids", "all ids match dotted-id grammar"))


def validate_exports(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    exports = package.manifest.get("exports", {})
    missing_modules = sorted(set(exports.get("modules", [])) - ids["modules"])
    missing_streams = sorted(set(exports.get("streams", [])) - ids["streams"])
    missing_commands = sorted(set(exports.get("commands", [])) - ids["commands"])
    missing = missing_modules + missing_streams + missing_commands
    if missing:
        checks.append(fail(f"{prefix}.exports", f"exports missing manifests: {missing}"))
    else:
        checks.append(pass_check(f"{prefix}.exports", "package exports resolve to manifests"))


def validate_module_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing: list[str] = []
    for module in package.modules:
        missing += sorted(set(module.get("provides_streams", [])) - ids["streams"])
        missing += sorted(set(module.get("consumes_streams", [])) - ids["streams"])
        missing += sorted(set(module.get("accepted_commands", [])) - ids["commands"])
    if missing:
        checks.append(fail(f"{prefix}.module_links", f"module links missing: {missing}"))
    else:
        checks.append(pass_check(f"{prefix}.module_links", "module stream and command links resolve"))


def validate_stream_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing = sorted(
        stream["source_module_id"]
        for stream in package.streams
        if stream["source_module_id"] not in ids["modules"]
    )
    if missing:
        checks.append(fail(f"{prefix}.stream_links", f"stream source modules missing: {missing}"))
    else:
        checks.append(pass_check(f"{prefix}.stream_links", "stream source modules resolve"))


def validate_graph_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing: list[str] = []
    for graph in package.graphs:
        node_ids = {node["node_id"] for node in graph.get("nodes", [])}
        missing += sorted(
            node["module_id"] for node in graph.get("nodes", []) if node["module_id"] not in ids["modules"]
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
    if missing:
        checks.append(fail(f"{prefix}.graph_links", f"graph links missing: {sorted(set(missing))}"))
    else:
        checks.append(pass_check(f"{prefix}.graph_links", "graph links resolve"))


def validate_deployment_links(
    prefix: str, package: PackageBundle, ids: dict[str, set[str]], checks: list[Check]
) -> None:
    missing: list[str] = []
    for deployment in package.deployments:
        if deployment.get("package_id") != package.manifest.get("package_id"):
            missing.append(str(deployment.get("package_id")))
        missing += sorted(set(deployment.get("selected_modules", [])) - ids["modules"])
    if missing:
        checks.append(fail(f"{prefix}.deployment_links", f"deployment links missing: {missing}"))
    else:
        checks.append(pass_check(f"{prefix}.deployment_links", "deployment links resolve"))


def pass_check(check_id: str, evidence: str) -> Check:
    return Check(check_id=check_id, status="pass", evidence=evidence)


def fail(check_id: str, evidence: str) -> Check:
    return Check(check_id=check_id, status="fail", evidence=evidence)


if __name__ == "__main__":
    sys.exit(main())
