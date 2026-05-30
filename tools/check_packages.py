#!/usr/bin/env python3
"""Validate first-party Manifold package manifests."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from package_testkit import validate_repo


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", default=".")
    args = parser.parse_args()

    report = validate_repo(Path(args.repo_root).resolve())
    print(json.dumps(report.to_json(), indent=2))
    return 0 if report.status == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
