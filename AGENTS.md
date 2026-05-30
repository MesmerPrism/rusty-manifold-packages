# Rusty Manifold Packages Agent Notes

This is the first-party package workspace for public-safe Manifold package
manifests, fixtures, and package validation.

## Scope

- Package catalogs.
- Package, module, stream, command, graph, deployment, runtime-state, and
  scorecard fixtures.
- Synthetic packages before platform packages.
- Generic biosignal package contracts before any device-specific runtime code.

## Non-Scope

- Runtime hosts.
- Dynamic loading.
- Platform SDKs.
- Device APIs.
- Local planning paths.
- Private source history, package identities, product names, or sensor-specific
  implementation details.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\package_testkit.py
```

Keep package ids behavior-oriented and generic. If a device-specific backend or
private source note is needed, record it in the private planning repo first.
