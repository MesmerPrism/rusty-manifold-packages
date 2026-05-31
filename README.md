# Rusty Manifold Packages

This workspace contains first-party Manifold package manifests and fixtures.
It is manifest-only for now: no runtime hosts, platform SDKs, dynamic loading,
device APIs, or transport stacks.

## Packages

- `synthetic`: synthetic provider and processor package for contract tests.
- `biosignal-sensor`: generic biosignal provider plus separate processing
  modules for derived streams.
- `polar-h10`: public sensor package manifests and fixtures for provider,
  processor, ownership, backend, and damaged-input contract readiness.

## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\package_testkit.py tools\check_device_readiness.py
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

The validator checks package exports, module stream/command links, graph links,
deployment selections, completion evidence, dotted ids, public-boundary terms,
and host-profile readiness bundles.
