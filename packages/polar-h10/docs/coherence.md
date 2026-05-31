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

- `paper_ratio`: peak-band power divided by the remaining total-band power.
- `normalized_score`: package display score using
  `paper_ratio / (paper_ratio + 1)`.
- `peak_frequency_hz`: frequency of the strongest bin inside the peak-search
  band.
- `peak_band_power` and `total_band_power`: positive-frequency band powers.
- `quality`: `ok`, `underfilled`, or `low_signal`.

The package keeps `paper_ratio` separate from `normalized_score` so downstream
apps can choose their own display mapping without rewriting the source method.

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
- both `paper_ratio` and `normalized_score`;
- an empty issue code on pass.

## Claim Boundary

The coherence stream is validation and operator-feedback telemetry. It is not
medical advice, diagnosis, treatment, or a clinical score. The source binding is
recorded in `provenance.polar_h10.source_manifest` as
`source.method.coherence_ratio`.
