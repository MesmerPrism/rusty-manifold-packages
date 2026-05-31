# Polar H10 HRV Window Processor

The HRV window processor consumes `stream.polar_h10.hr_rr` and emits
`stream.polar_h10.hrv_window`. It derives short-horizon beat-interval metrics
from accepted RR intervals and exposes quality fields so downstream processors
can reject weak input instead of silently publishing unstable values.

## Contract

Input:

- Source stream: `stream.polar_h10.hr_rr`.
- Interval unit: milliseconds.
- Validity gate: finite RR or NN interval inside the package bounds.

Output:

- `mean_nn_ms = sum(NN_i) / n`
- `mean_hr_bpm = 60000 / mean_nn_ms`
- `sdnn_ms = sqrt(sum((NN_i - mean_nn_ms)^2) / (n - 1))`
- `rmssd_ms = sqrt(sum((NN_{i+1} - NN_i)^2) / (n - 1))`
- `ln_rmssd = ln(rmssd_ms)`
- `pnn50 = count(abs(NN_{i+1} - NN_i) > 50 ms) / (n - 1)`
- `sd1_ms = rmssd_ms / sqrt(2)`

The stream also carries accepted count, rejected count, coverage, quality, and
issue code. `rmssd_ms` and `ln_rmssd` are stable fields because
`module.polar_h10.rmssd_gain` depends on them.

## Claim Boundary

This is validation and operator-feedback telemetry. It is not a diagnostic
measure. The source binding is recorded in
`provenance.polar_h10.source_manifest` as `source.method.hrv_metrics` and
`source.method.hrv_transform_context`.
