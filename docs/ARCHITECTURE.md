# Architecture

Rusty Manifold Packages is the first-party package catalog and package
validation workspace inside Rusty Morphospace. It is not a runtime host,
platform adapter, UI framework, or device API layer.

## Authority

This repo owns:

- package manifests and package-local provenance;
- package fixture families and damaged-input fixtures;
- package exports, graph links, deployment selections, runtime-state examples,
  scorecards, and handoff evidence shapes;
- deterministic package-local processor cores when a package needs reference
  behavior that can run in tests;
- validators that prove package files are public-safe, internally consistent,
  and host-profile ready.

This repo may reference Matter, Lattice, Optics, and Manifold schema IDs or
artifact URIs, but it does not compute their truth. Matter keeps mesh, SDF,
ADF, particle, field, and bioelectric algorithms. Lattice keeps situated
relation contracts. Manifold keeps command, stream, lease, host, and audit
authority. External host-validation repos execute package evidence routes.

## Current Package Families

- `synthetic`: minimal provider and processor fixtures for contract tests.
- `biosignal-sensor`: generic biosignal package contracts.
- `polar-h10`: public Polar H10 manifests, fixtures, provenance, and processor
  goldens.
- `projected-motion-breath`: source-agnostic motion-to-breath processor
  contracts, adapter-normalization fixtures, live-route self-tests, and
  deterministic core validation.
- `hand-animation`: hand-rig recording and bridge descriptors over Matter
  payload schemas without duplicating Matter mesh, SDF, collider, or particle
  behavior.

## Module Map

- `tools/check_packages.py`: dispatch-only package validator entrypoint.
- `tools/package_testkit.py`: generic package validation orchestration for
  package loading, public-boundary scans, exports, graph/deployment/runtime
  links, scorecards, handoffs, Polar checks, and package orchestration.
- `tools/package_testkit_common.py`: shared validation dataclasses, JSON
  readers, dotted-id grammar, numeric helpers, and check record helpers.
- `tools/projected_motion_breath_testkit.py`: projected-motion-breath package
  validation for profile/command fixtures, processor goldens, source adapter
  descriptors, source bindings, and adapter-normalization fixtures.
- `tools/hand_animation_matter_bridge.py`: hand-animation bridge validation
  over Matter artifact references.
- `tools/check_device_readiness.py`: host-profile readiness validation for
  desktop, mobile, and headset package use.
- `packages/*/crates/*-core`: deterministic package-local processor cores for
  golden checks. These cores stay package-scoped and must not become platform
  runtimes.
- `packages/polar-h10/crates/polar-h10-core/src/lib.rs`: Polar H10 core
  facade, graph/runtime computation, stream shaping, and public processor API.
- `packages/polar-h10/crates/polar-h10-core/src/goldens.rs`: Polar H10
  processor golden fixture validation plus golden comparison helpers.
- `packages/projected-motion-breath/crates/projected-motion-breath-core/src/lib.rs`:
  PMB core facade, public reexports, tracker flow, controller preflight, and
  shared adapter/profile helpers.
- `packages/projected-motion-breath/crates/projected-motion-breath-core/src/live_route.rs`:
  live-route report models, route execution, transport-event conversion,
  incremental transport processor state, and live estimator state.
- `packages/projected-motion-breath/crates/projected-motion-breath-core/src/documents.rs`:
  private serde document models and fixture readers for PMB profiles, commands,
  source bindings, adapter-normalization cases, controller preflight, live-route
  fixtures, and processor goldens.
- `packages/projected-motion-breath/crates/projected-motion-breath-core/src/math.rs`:
  private scalar, vector, projection-axis, quantile, quaternion, and deadband
  helpers shared by PMB tracker, live-route, adapter-normalization, and
  validation code.
- `packages/projected-motion-breath/crates/projected-motion-breath-core/src/validation.rs`:
  PMB profile, command, source-binding, adapter-normalization, controller
  preflight, live-route, and golden-fixture validators plus the
  `validate_package_goldens` report boundary.

## Boundaries

Keep package files behavior-oriented and generic. Device-specific runtime
details, private planning paths, product names, captured data, and local
operator evidence belong outside this public package repo until a provenance
and publication pass has sanitized them.

Do not add dynamic loading, sockets, platform SDKs, renderer toolkit
dependencies, or application-shell behavior here. Add a focused host, adapter,
or package helper only after the package contract and validation gate are clear.
