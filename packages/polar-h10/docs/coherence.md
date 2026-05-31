# Polar H10 Coherence Processor

The coherence processor consumes HR/RR-derived beat-interval spans and emits
`stream.polar_h10.coherence`. It is a package processor, not provider logic:
the provider owns acquisition, ownership, and direct sensor streams; this
processor owns a bounded spectral summary over an already collected beat
window.

## Contract

Input:

- Source stream: `stream.polar_h10.hr_rr`.
- Source module: `module.polar_h10.provider`.
- Window input for the spectral fixture: a uniformly sampled RR-interval
  series in milliseconds.

Output:

- `peak_band_power`: power inside the peak-centered band.
- `total_band_power`: power in the declared total band.
- `remaining_power = total_band_power - peak_band_power`.
- `paper_ratio`: peak-band power divided by remaining power.
- `coherence_ratio`: same unsquared ratio, kept as an explicit contract field.
- `coherence_ratio_squared = coherence_ratio * coherence_ratio`.
- `normalized_peak_power = peak_band_power / total_band_power`.
- `normalized_score`: package display score using the same primitive powers as
  `normalized_peak_power`.
- `peak_frequency_hz`: frequency of the strongest bin inside the peak-search
  band.
- `peak_band_power` and `total_band_power`: positive-frequency band powers.
- `quality`: `ok`, `underfilled`, or `low_signal`.

The package keeps primitive powers, the unsquared ratio, the squared ratio,
and the normalized score separate so downstream apps can choose a display
mapping without rewriting the source method.

## Fixture Method

The current golden fixture is intentionally narrow and deterministic:

- sample rate: 2 Hz;
- FFT length: 128 samples;
- window length: 64 seconds;
- detrending: subtract the window mean;
- analysis window: rectangular;
- total band: 0.0033 Hz through 0.4 Hz;
- peak-search band: 0.04 Hz through 0.26 Hz;
- peak-band half-width: 0.03 Hz around the strongest peak-search bin.

This fixture is not a claim that every runtime implementation must use a
rectangular analysis window. It is the first stable checksum for the package
contract: source binding, units, band definitions, ratio formula, score
separation, and damaged-input handling.

The runtime processor may start from irregular beat intervals. Before applying
the spectral ratio it resamples the latest 64-second beat-interval window to a
2 Hz uniform RR-interval series, subtracts the mean, and computes the same
positive-frequency band powers used by the golden fixture.

## Live Output Gate

A live coherence stream passes only when the evidence includes:

- at least 128 uniform RR samples;
- the 64-second window length and 2 Hz sample rate;
- the peak frequency inside the configured peak-search band;
- positive peak-band and total-band powers;
- `paper_ratio`, `coherence_ratio`, `coherence_ratio_squared`,
  `normalized_peak_power`, and `normalized_score`;
- an empty issue code on pass.

## Claim Boundary

The coherence stream is validation and operator-feedback telemetry. It is not
medical advice, diagnosis, treatment, or a clinical score. The source binding is
recorded in `provenance.polar_h10.source_manifest` as
`source.method.coherence_ratio`.
