# Polar H10 RMSSD Gain Processor

The RMSSD gain processor consumes `stream.polar_h10.hrv_window` and emits
`stream.polar_h10.rmssd_gain`. It compares live lnRMSSD against a declared
baseline and keeps the sourced gain separate from ratio, z-score, and any
display mapping.

## Contract

Input:

- Source stream: `stream.polar_h10.hrv_window`.
- Required live field: `ln_rmssd`.
- Required baseline field: `baseline_ln_rmssd`.
- Optional baseline fields: `baseline_mean_ln_rmssd`,
  `baseline_sd_ln_rmssd`, baseline sample count, and baseline timestamp span.

Output:

- `ln_rmssd_gain = ln_rmssd_live - baseline_ln_rmssd`
- `rmssd_ratio = exp(ln_rmssd_gain)`
- `baseline_z_score = (ln_rmssd_live - baseline_mean_ln_rmssd) /
  baseline_sd_ln_rmssd` when baseline SD is present and positive

`ln_rmssd_gain` is the sourced metric. `rmssd_ratio` and `baseline_z_score`
are explicit derived fields. Any clipped or smoothed operator score must be a
separate field with its own formula version.

## Damaged Input

The processor must reject or degrade output for missing baseline, underfilled
baseline, nonpositive baseline SD when z-score is requested, low-quality HRV
input, stale baseline, NaN, or infinite values.

## Claim Boundary

This stream is baseline-relative HRV telemetry for validation and
operator-feedback workflows. It is not a diagnostic measure and is not the
same as spectral coherence or resonance-amplitude feedback.
