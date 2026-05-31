# Polar H10 HRVB Resonance Amplitude Processor

The HRVB resonance-amplitude processor consumes `stream.polar_h10.hr_rr` and
emits `stream.polar_h10.hrvb_resonance_amplitude`. It models respiration-linked
heart-rate oscillation during paced breathing by fitting a sine function to a
short instant-heart-rate analysis window.

## Contract

Input:

- Source stream: `stream.polar_h10.hr_rr`.
- Accepted RR intervals are converted to instant heart rate:
  `hr_bpm = 60000 / rr_ms`.

Fit model:

- Analysis span: 30 seconds.
- Target frequency band: `0.08..0.12 Hz`.
- Model:
  `hr_bpm(t) = mean_hr_bpm + amplitude_bpm *
  sin(omega_rad_s * t + phase_rad)`.
- Derived frequency: `frequency_hz = omega_rad_s / (2 * pi)`.
- Amplitude definition: half peak-to-trough distance.

Output:

- `amplitude_bpm`
- `mean_hr_bpm`
- `frequency_hz`
- `phase_rad`
- `fit_residual`
- `fit_converged`
- `coverage`
- `threshold_status` when compared with the source-method threshold of `2.0`
- `median_session_amplitude_bpm` for session summary output

## Fixture Scope

The first package slice is a deterministic synthetic fixture. It proves field
names, units, source bindings, and damaged-input behavior. A live runtime pass
must later prove the same formula version, input coverage, fit convergence,
and quality fields.

## Claim Boundary

This stream is operator-feedback telemetry for resonance-style HRV
biofeedback. It is separate from RMSSD gain and spectral coherence. It is not a
diagnostic measure, treatment recommendation, or clinical score.
