# Polar H10 Breathing Dynamics Processor

The breathing-dynamics processor consumes `stream.polar_h10.breath_volume` and
emits `stream.polar_h10.breath_dynamics`. The feature family is source-backed,
but the input is the package accelerometer breathing proxy, not the original
respiration sensor setup used by cited breathing-dynamics research.

## Contract

Input:

- Source stream: `stream.polar_h10.breath_volume`.
- Required fields: timestamped normalized breathing proxy and quality.

Cycle features:

- Detect alternating extrema using declared amplitude and timing thresholds.
- Compute interval series from consecutive cycle markers.
- Compute amplitude series from peak-to-trough excursion.

Window features:

- `mean`, sample `sd`, and `cv = sd / mean` for intervals and amplitudes.
- `breathing_rate_bpm = 60 / mean_interval_s` when interval mean is positive.
- `acw50`: first lag where normalized autocorrelation falls to or below 0.5.
- `psd_slope`: slope of log power versus log frequency over the declared
  range.
- `lzc`: normalized Lempel-Ziv complexity after declared binarization.
- `sampen`: `-ln(A / B)` with declared `m`, `r`, and delay.
- `mse`: sample entropy over coarse-grained scales, with undefined reasons
  when the series is too short.

## Damaged Input

The processor must mark undefined fields rather than publishing NaN. Damaged
cases include underfilled cycle series, flat waveform, invalid timestamps,
low-amplitude input, and insufficient entropy sample count.

## Claim Boundary

The output is a source-bound dynamics summary over the package breathing proxy.
It is validation and operator-feedback telemetry, not a clinical score.
