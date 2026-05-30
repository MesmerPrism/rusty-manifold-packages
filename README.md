# Rusty Manifold Packages

This workspace contains first-party Manifold package manifests and fixtures.
It is manifest-only for now: no runtime hosts, platform SDKs, dynamic loading,
device APIs, or transport stacks.

## Packages

- `synthetic`: synthetic provider and processor package for contract tests.
- `biosignal-sensor`: generic biosignal provider plus separate processing
  modules for derived streams.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\package_testkit.py
```

The validator checks package exports, module stream/command links, graph links,
deployment selections, dotted ids, and public-boundary terms.
