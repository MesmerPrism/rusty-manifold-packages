"""Shared data structures and helpers for Manifold package validation tools."""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9_-]*[a-z0-9])?)*$")


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


def find_one(items: list[dict[str, Any]], key: str, value: str) -> dict[str, Any] | None:
    for item in items:
        if item.get(key) == value:
            return item
    return None


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
    return (
        finite_number(left)
        and finite_number(right)
        and abs(float(left) - float(right)) <= tolerance
    )


def list_close(left: Any, right: Any, tolerance: float = 0.000_000_001) -> bool:
    return (
        isinstance(left, list)
        and isinstance(right, list)
        and len(left) == len(right)
        and all(
            float_close(left_item, right_item, tolerance)
            for left_item, right_item in zip(left, right)
        )
    )


def within_tolerance(actual: float | str, expected: float, tolerance: float) -> bool:
    if not isinstance(actual, int | float):
        return False
    if not math.isfinite(float(actual)) or not math.isfinite(expected):
        return float(actual) == expected
    return abs(float(actual) - expected) <= tolerance


def prefix_errors(prefix: str, errors: list[str]) -> list[str]:
    return [f"{prefix}:{error}" for error in errors]


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
