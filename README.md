# Rusty Manifold Packages

This workspace contains first-party Manifold package manifests, fixtures, and
package-local deterministic processor cores. It does not contain runtime hosts,
platform SDKs, dynamic loading, device APIs, or transport stacks.

## Packages

- `synthetic`: synthetic provider and processor package for contract tests.
- `biosignal-sensor`: generic biosignal provider plus separate processing
  modules for derived streams.
- `projected-motion-breath`: source-agnostic pose/vector motion-to-breath
  processor contracts, source-adapter bindings, adapter-normalization fixtures,
  and synthetic/replay fixtures.
- `polar-h10`: public sensor package manifests, fixtures, provenance, and a
  Rust processor core for graph-resolved synthetic/replay validation.
- `hand-animation`: generic hand-rig recording, coordinate-map, validation, and
  animated mesh export contracts over Matter payload schemas, plus package
  bridge descriptors for Matter SDF, dynamic mesh collider, and particle
  simulation artifact flows.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\hand_animation_matter_bridge.py tools\package_testkit.py tools\check_device_readiness.py
cargo fmt --all --check
cargo test --workspace
cargo run -p polar-h10-core -- validate-goldens --package-root packages\polar-h10
cargo run -p projected-motion-breath-core -- validate-goldens --package-root packages\projected-motion-breath
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

The validator checks package exports, module stream/command links, graph links,
deployment selections, completion evidence, dotted ids, public-boundary terms,
processor golden fixtures, provenance metadata, and host-profile readiness
bundles. The Rust cores re-run the Polar and projected-motion processor
goldens, and the Polar core can execute the static package graph from
synthetic/replay input fixtures.
