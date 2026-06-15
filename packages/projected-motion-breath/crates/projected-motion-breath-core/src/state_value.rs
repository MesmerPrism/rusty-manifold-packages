//! Raw breath-state normalization and optional state-value processing.

#[derive(Clone, Copy, Debug)]
pub(super) struct BreathStateValueConfig {
    pub(super) min_value01: f64,
    pub(super) max_value01: f64,
    pub(super) initial_value01: f64,
    pub(super) fallback_value01: f64,
    pub(super) inhale_seconds_min_to_max: f64,
    pub(super) exhale_seconds_max_to_min: f64,
    pub(super) smoothing_s: f64,
    pub(super) stale_timeout_s: f64,
    pub(super) hold_bad_tracking: bool,
}

impl Default for BreathStateValueConfig {
    fn default() -> Self {
        Self {
            min_value01: 0.0,
            max_value01: 1.0,
            initial_value01: 0.5,
            fallback_value01: 0.5,
            inhale_seconds_min_to_max: 4.0,
            exhale_seconds_max_to_min: 4.0,
            smoothing_s: 0.03,
            stale_timeout_s: 1.0,
            hold_bad_tracking: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BreathStateValueStep {
    pub(super) state01: f64,
    pub(super) target01: f64,
    pub(super) value01: f64,
    pub(super) delta_seconds: f64,
    pub(super) stale_gap: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BreathStateValueProcessor {
    config: BreathStateValueConfig,
    target01: f64,
    value01: f64,
    last_sample_time_s: Option<f64>,
}

impl BreathStateValueProcessor {
    pub(super) fn new(config: BreathStateValueConfig) -> Self {
        let initial = clamp_to_config(config.initial_value01, &config);
        Self {
            config,
            target01: initial,
            value01: initial,
            last_sample_time_s: None,
        }
    }

    pub(super) fn push(&mut self, state: &str, sample_time_s: f64) -> BreathStateValueStep {
        let state = normalize_breath_state(state);
        let mut delta_seconds = self
            .last_sample_time_s
            .map(|last| {
                if sample_time_s.is_finite() && last.is_finite() {
                    (sample_time_s - last).max(0.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let stale_gap = self.config.stale_timeout_s.is_finite()
            && self.config.stale_timeout_s > 0.0
            && delta_seconds > self.config.stale_timeout_s;
        if stale_gap {
            let fallback = clamp_to_config(self.config.fallback_value01, &self.config);
            self.target01 = fallback;
            self.value01 = fallback;
            delta_seconds = 0.0;
        }

        match state {
            "inhale" => {
                self.target01 += ramp_delta(delta_seconds, self.config.inhale_seconds_min_to_max);
            }
            "exhale" => {
                self.target01 -= ramp_delta(delta_seconds, self.config.exhale_seconds_max_to_min);
            }
            "bad_tracking" if !self.config.hold_bad_tracking => {
                self.target01 = clamp_to_config(self.config.fallback_value01, &self.config);
            }
            _ => {}
        }
        self.target01 = clamp_to_config(self.target01, &self.config);

        if self.config.smoothing_s.is_finite() && self.config.smoothing_s > 0.0 {
            let alpha = if delta_seconds <= 0.0 {
                0.0
            } else {
                1.0 - (-delta_seconds / self.config.smoothing_s).exp()
            };
            self.value01 += (self.target01 - self.value01) * alpha.clamp(0.0, 1.0);
        } else {
            self.value01 = self.target01;
        }
        self.value01 = clamp_to_config(self.value01, &self.config);
        self.last_sample_time_s = Some(sample_time_s);

        BreathStateValueStep {
            state01: breath_state01(state),
            target01: self.target01,
            value01: self.value01,
            delta_seconds,
            stale_gap,
        }
    }
}

pub(super) fn normalize_breath_state(state: &str) -> &'static str {
    match state {
        "inhale" | "inhaling" => "inhale",
        "exhale" | "exhaling" => "exhale",
        "pause" | "pausing" | "retention" | "hold" => "pause",
        "bad_tracking" | "bad-tracking" | "tracking_lost" | "lost_tracking" => "bad_tracking",
        _ => "pause",
    }
}

pub(super) fn breath_state01(state: &str) -> f64 {
    match normalize_breath_state(state) {
        "inhale" => 1.0,
        "exhale" => 0.0,
        _ => 0.5,
    }
}

fn ramp_delta(delta_seconds: f64, seconds_full_range: f64) -> f64 {
    if !seconds_full_range.is_finite() || seconds_full_range <= 0.0 {
        1.0
    } else {
        (delta_seconds / seconds_full_range).max(0.0)
    }
}

fn clamp_to_config(value: f64, config: &BreathStateValueConfig) -> f64 {
    let min_value01 = config.min_value01.clamp(0.0, 1.0);
    let max_value01 = config.max_value01.clamp(min_value01, 1.0);
    value.clamp(min_value01, max_value01)
}
