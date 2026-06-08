# Rusty Manifold Packages Agent Notes

This is the first-party package workspace for public-safe Manifold package
manifests, fixtures, and package validation.

## Scope

- Package catalogs.
- Package, module, stream, command, graph, deployment, runtime-state, and
  scorecard fixtures.
- Synthetic packages before platform packages.
- Generic biosignal package contracts before any device-specific runtime code.
- Renderer-toolkit-free package manifests, fixtures, validators, and
  descriptors.
- Lattice artifact references for spaces, transforms, tracked poses, view sets,
  input roles, frame-state binding, calibration, validity, confidence, or
  runtime capabilities when a package needs situated relation evidence.

## Non-Scope

- Runtime hosts.
- Dynamic loading.
- Platform SDKs.
- Device APIs.
- Local planning paths.
- Private source history, package identities, product names, or sensor-specific
  implementation details.
- Renderer-toolkit dependencies, toolkit-specific generated shells, UI
  frameworks, or renderer assumptions.
- Lattice relation truth or platform runtime ownership. Packages may reference
  `rusty.lattice.*` artifacts, but they do not compute or authorize situated
  relation state.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\package_testkit.py tools\check_device_readiness.py
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

Keep package ids behavior-oriented and generic. If a device-specific backend or
private source note is needed, record it in the private planning repo first.
