# Rusty Manifold Packages Agent Notes

This is the first-party package workspace for public-safe Manifold package
manifests, fixtures, and package validation.

Rusty Morphospace is the top-level project/platform umbrella. This workspace is
the Manifold package lane inside that umbrella: package manifests, fixtures,
scorecards, and package validation that may reference Matter, Lattice, Optics,
or Manifold artifacts without becoming their authority.

Project-owned source in this repo is licensed `AGPL-3.0-or-later`. Keep
third-party dependencies, package evidence, generated reports, fixtures
imported from other projects, binary releases, and external tools under their
own provenance and notice requirements; see `docs/LICENSING.md`.

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

## Sustainable Design Guardrails

- Treat monolithic file pressure as an ownership problem, not a line-count
  problem. Split only by durable authority, schema, route, validation, adapter,
  or test-family boundaries; preserve facades, schema IDs, serde fields,
  fixture outputs, CLI behavior, validation outcomes, and dependency boundaries.
- After a split, update the nearest distributed file map: this `AGENTS.md`,
  `README.md`, `docs/ARCHITECTURE.md`, fixture docs, validation docs, or the
  planning `agent-state\iteration-events.jsonl`.
- Keep `AGENTS.md`, README, and skill files as concise routing indexes. Move
  lane-specific recipes, device/build detail, compatibility ledgers, and long
  validation flows into named docs or runbooks.
- Keep legacy Rusty-XR names as explicit compatibility surfaces only. New
  schemas, routes, and types use the owning lane (`rusty.manifold.*`,
  `rusty.lattice.*`, `rusty.matter.*`, `rusty.optics.*`, `rusty.quest.*`, or
  repo-local names); do not introduce `rusty.morphospace.*` schemas or
  `Morphospace*` core types by default.
## Validation

```powershell
python tools\check_packages.py --repo-root .
python -m py_compile tools\check_packages.py tools\hand_animation_matter_bridge.py tools\package_testkit.py tools\package_testkit_common.py tools\projected_motion_breath_testkit.py tools\check_device_readiness.py
python tools\check_device_readiness.py --repo-root . --host-profile desktop
python tools\check_device_readiness.py --repo-root . --host-profile mobile
python tools\check_device_readiness.py --repo-root . --host-profile headset
```

Keep package ids behavior-oriented and generic. If a device-specific backend or
internal-only source note is needed, record it outside the public package repo
first.

## File Organization

- Keep `tools\check_packages.py` as a dispatch-only CLI wrapper.
- Keep `tools\package_testkit.py` focused on generic package validation
  orchestration: loading, public-boundary scans, package exports,
  graph/deployment/runtime links, generic scorecards, handoffs, Polar checks,
  and existing package check orchestration. Shared dataclasses, JSON readers,
  and numeric/check helpers live in `tools\package_testkit_common.py`.
- Keep projected-motion-breath package fixture validation in
  `tools\projected_motion_breath_testkit.py`; do not rebuild PMB profile,
  command, source-adapter, source-binding, adapter-normalization, or golden
  fixture checks in the generic facade.
- Put package-specific bridge validation in focused helper modules. The
  hand-animation Matter mesh/SDF/collider/particle bridge lives in
  `tools\hand_animation_matter_bridge.py`.
- Keep package-local Rust processor cores as facades plus focused helper
  modules. In polar-h10-core, `goldens.rs` owns processor golden fixture
  validation while `lib.rs` remains graph/runtime computation and public
  facade. In projected-motion-breath-core, `documents.rs` owns private serde
  document models and fixture readers, and `math.rs` owns private scalar,
  vector, projection, and deadband helpers. `live_route.rs` owns live-route
  reports, route execution, transport-event conversion, incremental transport
  processor state, and live estimator state. `validation.rs` owns PMB fixture
  validators, golden-check helpers, and the `validate_package_goldens` report
  boundary. Do not rebuild those schemas, helper families, live transport
  routines, or validation routines inside `lib.rs`.
- Do not add Matter SDF, particle simulation, mesh sampling, collider, or
  coordinate-map algorithms to package validators or Manifold descriptors.
  Package files may reference `rusty.matter.*` schema IDs and artifact URIs;
  Matter remains the computational owner.
