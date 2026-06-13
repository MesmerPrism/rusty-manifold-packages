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
- `tools/package_testkit.py`: shared validation helpers for package loading,
  public-boundary scans, exports, graph/deployment/runtime links, scorecards,
  handoffs, and package orchestration.
- `tools/hand_animation_matter_bridge.py`: hand-animation bridge validation
  over Matter artifact references.
- `tools/check_device_readiness.py`: host-profile readiness validation for
  desktop, mobile, and headset package use.
- `packages/*/crates/*-core`: deterministic package-local processor cores for
  golden checks. These cores stay package-scoped and must not become platform
  runtimes.

## Boundaries

Keep package files behavior-oriented and generic. Device-specific runtime
details, private planning paths, product names, captured data, and local
operator evidence belong outside this public package repo until a provenance
and publication pass has sanitized them.

Do not add dynamic loading, sockets, platform SDKs, renderer toolkit
dependencies, or application-shell behavior here. Add a focused host, adapter,
or package helper only after the package contract and validation gate are clear.
