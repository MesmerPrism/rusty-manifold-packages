# Polar H10 Breath Volume From ACC Processor

The breath-volume processor consumes `stream.polar_h10.acc` and emits
`stream.polar_h10.breath_volume`. The output is a package-defined breathing
proxy derived from accelerometer movement. It is not externally validated
respiratory volume.

## Contract

Input:

- Source stream: `stream.polar_h10.acc`.
- Required fields: timestamped acceleration samples and host timestamp.

Calibration:

- Smooth each acceleration axis with an exponential moving average.
- Build a calibration span after signal readiness checks pass.
- Select or solve a projection axis from the calibration span.
- Project centered acceleration onto the selected axis.
- Derive low and high bounds from declared quantiles.

Output:

- `projection = dot(smoothed_acc - center, axis)`
- `breath_volume_01 = clamp((projection - lower_bound) /
  (upper_bound - lower_bound), 0, 1)`
- `phase` derived from slope and state thresholds
- `confidence` derived from calibration validity, range, freshness, and sample
  coverage

## Damaged Input

The processor must report damaged state for missing calibration, underfilled
calibration, flat projection, invalid bounds, stale acceleration input, and
low movement range.

## Claim Boundary

This stream is a movement-derived breathing proxy for package validation and
operator feedback. Do not describe it as measured respiratory volume.
