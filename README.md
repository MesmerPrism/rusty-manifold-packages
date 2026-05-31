# Rusty Manifold Packages

This workspace contains first-party Manifold package manifests, fixtures, and
package-local deterministic processor cores. It does not contain runtime hosts,
platform SDKs, dynamic loading, device APIs, or transport stacks.

## Packages

- `synthetic`: synthetic provider and processor package for contract tests.
- `biosignal-sensor`: generic biosignal provider plus separate processing
  modules for derived streams.
- `polar-h10`: public sensor package manifests, fixtures, provenance, and a
  Rust processor core for graph-resolved synthetic/replay validation.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\package_testkit.py tools\check_device_readiness.py
cargo fmt --all --check
cargo test --workspace
cargo run -p polar-h10-core -- validate-goldens --package-root packages\polar-h10
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

The validator checks package exports, module stream/command links, graph links,
deployment selections, completion evidence, dotted ids, public-boundary terms,
processor golden fixtures, provenance metadata, and host-profile readiness
bundles. The Rust core re-runs the Polar processor goldens and can execute the
static package graph from synthetic/replay input fixtures.
