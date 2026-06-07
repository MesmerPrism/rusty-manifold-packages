# Hand Animation Package

This package contains public-safe Manifold manifests and fixtures for recording
hand rig data, joint motion, sparse validation meshes, coordinate maps, and
animated mesh export jobs. It also describes the contract bridge from those
same Matter mesh payloads into SDF grids, dynamic mesh collider updates, and
particle simulation artifacts.

It is contract-only. Device runtime adapters, app shells, GUI code, private
preview behavior, and exporter implementations live outside this package.

Matter schema ids describe geometry and animation payloads. Manifold package
manifests describe commands, streams, modules, deployment selections, and
evidence boundaries. The bridge fixtures use artifact references rather than
embedding particle arrays, SDF grids, collider shells, or simulation math in the
package workspace.
