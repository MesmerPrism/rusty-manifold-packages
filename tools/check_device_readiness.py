#!/usr/bin/env python3
"""Build and optionally exercise host-profile readiness bundles."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from package_testkit import PackageBundle, load_packages, validate_repo


HOST_PROFILES = {
    "desktop": {
        "backend_id": "backend.desktop_wireless",
        "deployment_id": "deployment.polar_h10_desktop_wireless",
    },
    "mobile": {
        "backend_id": "backend.mobile_wireless",
        "deployment_id": "deployment.polar_h10_mobile_wireless",
    },
    "headset": {
        "backend_id": "backend.headset_wireless",
        "deployment_id": "deployment.polar_h10_headset_wireless",
    },
}
REMOTE_ROOT = "/data/local/tmp/rusty-manifold-readiness"


@dataclass
class ReadinessCheck:
    check_id: str
    status: str
    evidence: str


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--package-id", default="package.polar_h10")
    parser.add_argument("--host-profile", required=True, choices=sorted(HOST_PROFILES))
    parser.add_argument("--out", default="artifacts/readiness")
    parser.add_argument("--adb", default=None)
    parser.add_argument("--serial", default=None)
    args = parser.parse_args()

    repo_root = Path(args.repo_root).resolve()
    output_root = Path(args.out).resolve()
    report, bundle_dir = build_readiness_bundle(
        repo_root=repo_root,
        output_root=output_root,
        package_id=args.package_id,
        host_profile=args.host_profile,
    )

    if args.adb or args.serial:
        adb = args.adb or "adb"
        run_device_smoke(
            adb=adb,
            serial=args.serial,
            host_profile=args.host_profile,
            bundle_dir=bundle_dir,
            report=report,
        )
        write_report(bundle_dir / "readiness-report.json", report)

    print(json.dumps(report_to_json(report), indent=2))
    return 0 if all(check.status == "pass" for check in report) else 1


def build_readiness_bundle(
    *,
    repo_root: Path,
    output_root: Path,
    package_id: str,
    host_profile: str,
) -> tuple[list[ReadinessCheck], Path]:
    checks: list[ReadinessCheck] = []
    repo_report = validate_repo(repo_root)
    checks.append(
        ReadinessCheck(
            "readiness.package_workspace",
            repo_report.status,
            f"package workspace validation {repo_report.status}",
        )
    )

    package = load_package(repo_root, package_id)
    profile = HOST_PROFILES[host_profile]
    backend_id = profile["backend_id"]
    deployment_id = profile["deployment_id"]

    deployment = find_by_id(package.deployments, "deployment_id", deployment_id)
    checks.append(
        pass_or_fail(
            "readiness.host_deployment",
            deployment is not None,
            f"{deployment_id} is present",
            f"{deployment_id} is missing",
        )
    )
    if deployment is not None:
        selected_modules = set(deployment.get("selected_modules", []))
        exported_modules = set(package.manifest.get("exports", {}).get("modules", []))
        selected_backends = {
            item.get("module_id"): item.get("backend_id")
            for item in deployment.get("selected_backends", [])
        }
        backend_modules = {
            module_id for module_id, selected_backend in selected_backends.items() if selected_backend == backend_id
        }
        checks.append(
            pass_or_fail(
                "readiness.host_modules",
                exported_modules.issubset(selected_modules),
                f"{host_profile} deployment selects all exported modules",
                f"{host_profile} deployment missing modules: {sorted(exported_modules - selected_modules)}",
            )
        )
        checks.append(
            pass_or_fail(
                "readiness.host_backend",
                exported_modules.issubset(backend_modules),
                f"{host_profile} deployment selects {backend_id} for all modules",
                f"{host_profile} deployment backend coverage mismatch",
            )
        )

    ownership_modes = [
        mode
        for ownership_doc in package.ownership_modes
        for mode in ownership_doc.get("modes", [])
    ]
    checks.append(
        pass_or_fail(
            "readiness.ownership_modes",
            any(mode.get("mode_id") == "ownership.raw_stream.single_owner" for mode in ownership_modes),
            "raw stream single-owner mode is present",
            "raw stream single-owner mode is missing",
        )
    )
    checks.append(
        pass_or_fail(
            "readiness.replay_fallback",
            any(item.get("deployment_id") == "deployment.polar_h10_replay" for item in package.deployments),
            "replay deployment is present for device-free fallback",
            "replay deployment is missing",
        )
    )
    checks.append(
        pass_or_fail(
            "readiness.synthetic_fallback",
            any(item.get("deployment_id") == "deployment.polar_h10_synthetic" for item in package.deployments),
            "synthetic deployment is present for local fallback",
            "synthetic deployment is missing",
        )
    )

    bundle_dir = output_root / host_profile
    if bundle_dir.exists():
        shutil.rmtree(bundle_dir)
    bundle_dir.mkdir(parents=True)
    write_json(bundle_dir / "package.manifold.json", package.manifest)
    if deployment is not None:
        write_json(bundle_dir / "deployment.manifold.json", deployment)
    if package.graphs:
        write_json(bundle_dir / "graph.manifold.json", package.graphs[0])
    if package.ownership_modes:
        write_json(bundle_dir / "ownership-modes.manifold.json", package.ownership_modes[0])
    if package.pmd_handoffs:
        write_json(bundle_dir / "handoff-workflows.manifold.json", package.pmd_handoffs[0])
    write_device_script(bundle_dir / "device-smoke.sh")
    write_report(bundle_dir / "readiness-report.json", checks)
    return checks, bundle_dir


def load_package(repo_root: Path, package_id: str) -> PackageBundle:
    load_checks: list[Any] = []
    packages = load_packages(repo_root, load_checks)
    for package in packages:
        if package.manifest.get("package_id") == package_id:
            return package
    raise SystemExit(f"package not found: {package_id}")


def find_by_id(items: list[dict[str, Any]], key: str, value: str) -> dict[str, Any] | None:
    for item in items:
        if item.get(key) == value:
            return item
    return None


def run_device_smoke(
    *,
    adb: str,
    serial: str | None,
    host_profile: str,
    bundle_dir: Path,
    report: list[ReadinessCheck],
) -> None:
    adb_args = [adb]
    if serial:
        adb_args += ["-s", serial]
    state = run_command(adb_args + ["get-state"])
    report.append(
        pass_or_fail(
            "readiness.device_state",
            state.returncode == 0 and state.stdout.strip() == "device",
            "ADB target reports device state",
            state.stderr.strip() or state.stdout.strip() or "ADB target is not ready",
        )
    )
    if state.returncode != 0 or state.stdout.strip() != "device":
        return

    remote_dir = f"{REMOTE_ROOT}/{sanitize_segment(host_profile)}"
    mkdir = run_command(adb_args + ["shell", "mkdir", "-p", remote_dir])
    report.append(
        pass_or_fail(
            "readiness.device_prepare",
            mkdir.returncode == 0,
            f"prepared {remote_dir}",
            mkdir.stderr.strip() or mkdir.stdout.strip() or "remote directory preparation failed",
        )
    )
    if mkdir.returncode != 0:
        return

    for path in sorted(bundle_dir.iterdir()):
        if path.is_file():
            pushed = run_command(adb_args + ["push", str(path), f"{remote_dir}/{path.name}"])
            report.append(
                pass_or_fail(
                    f"readiness.push.{dotted_file_id(path.name)}",
                    pushed.returncode == 0,
                    f"pushed {path.name}",
                    pushed.stderr.strip() or pushed.stdout.strip() or f"push failed for {path.name}",
                )
            )
            if pushed.returncode != 0:
                return

    smoke = run_command(adb_args + ["shell", "sh", f"{remote_dir}/device-smoke.sh", remote_dir])
    report.append(
        pass_or_fail(
            "readiness.device_bundle_smoke",
            smoke.returncode == 0,
            one_line(smoke.stdout) or "device bundle smoke passed",
            one_line(smoke.stderr) or one_line(smoke.stdout) or "device bundle smoke failed",
        )
    )


def run_command(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, text=True, capture_output=True, timeout=45)


def write_device_script(path: Path) -> None:
    script = "\n".join(
        [
            "#!/system/bin/sh",
            'root="$1"',
            '[ -s "$root/readiness-report.json" ] || { echo "missing readiness-report.json"; exit 2; }',
            '[ -s "$root/package.manifold.json" ] || { echo "missing package.manifold.json"; exit 2; }',
            '[ -s "$root/deployment.manifold.json" ] || { echo "missing deployment.manifold.json"; exit 2; }',
            '[ -s "$root/graph.manifold.json" ] || { echo "missing graph.manifold.json"; exit 2; }',
            '[ -s "$root/ownership-modes.manifold.json" ] || { echo "missing ownership-modes.manifold.json"; exit 2; }',
            '[ -s "$root/handoff-workflows.manifold.json" ] || { echo "missing handoff-workflows.manifold.json"; exit 2; }',
            "grep -q '\"status\": \"pass\"' \"$root/readiness-report.json\" || { echo \"readiness report did not pass\"; exit 3; }",
            "grep -q '\"package_id\": \"package.polar_h10\"' \"$root/package.manifold.json\" || { echo \"package id mismatch\"; exit 3; }",
            "grep -q '\"owner_policy\": \"serial_handoff\"' \"$root/handoff-workflows.manifold.json\" || { echo \"handoff policy mismatch\"; exit 3; }",
            'bytes="$(wc -c < "$root/readiness-report.json" | tr -d " ")"',
            'printf \'{"status":"pass","readinessReportBytes":%s}\\n\' "$bytes"',
            "",
        ]
    )
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(script)


def write_report(path: Path, checks: list[ReadinessCheck]) -> None:
    write_json(path, report_to_json(checks))


def report_to_json(checks: list[ReadinessCheck]) -> dict[str, Any]:
    return {
        "$schema": "rusty.manifold.device_readiness_report.v1",
        "status": "fail" if any(check.status == "fail" for check in checks) else "pass",
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "checks": [check.__dict__ for check in checks],
    }


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def pass_or_fail(
    check_id: str,
    passed: bool,
    pass_evidence: str,
    fail_evidence: str,
) -> ReadinessCheck:
    return ReadinessCheck(
        check_id=check_id,
        status="pass" if passed else "fail",
        evidence=pass_evidence if passed else fail_evidence,
    )


def sanitize_segment(value: str) -> str:
    return re.sub(r"[^a-z0-9_.-]", "_", value.lower())


def dotted_file_id(value: str) -> str:
    return sanitize_segment(value).replace("-", "_").replace(".", "_")


def one_line(value: str) -> str:
    return " ".join(value.split())


if __name__ == "__main__":
    sys.exit(main())
