# Validation

Run the repo-local check before committing changes:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\check_all.ps1
```

The check covers:

- package validation through `tools\check_packages.py`;
- Python syntax checks for validator and bridge tools;
- `cargo fmt --all --check`;
- `cargo test --workspace`;
- Polar H10 processor golden validation;
- projected-motion-breath processor golden validation;
- projected-motion-breath live-route self-test;
- desktop, mobile, and headset readiness checks.

For package-manifest-only edits, run the package validator first:

```powershell
python tools\check_packages.py --repo-root .
```

For processor-core edits, run the relevant core directly before the full check:

```powershell
cargo run -p polar-h10-core -- validate-goldens --package-root packages\polar-h10
cargo run -p projected-motion-breath-core -- validate-goldens --package-root packages\projected-motion-breath
cargo run -p projected-motion-breath-core -- live-route-self-test --package-root packages\projected-motion-breath
```

For host-profile readiness changes, run all profiles:

```powershell
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

Validation must prove package contracts without turning this repo into a host
runtime. If a test needs device bridges, wireless sensor acquisition, renderer
imports, dynamic loading, or app lifecycle behavior, move that proof to a
host-validation repo or platform adapter and keep a package-level fixture or
scorecard here.
