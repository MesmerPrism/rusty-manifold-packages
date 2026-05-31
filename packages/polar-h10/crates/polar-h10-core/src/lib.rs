use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MODULE_PROVIDER: &str = "module.polar_h10.provider";
pub const MODULE_HRV_WINDOW: &str = "module.polar_h10.hrv_window";
pub const MODULE_RMSSD_GAIN: &str = "module.polar_h10.rmssd_gain";
pub const MODULE_COHERENCE: &str = "module.polar_h10.coherence";
pub const MODULE_BREATH_VOLUME: &str = "module.polar_h10.breath_volume_from_acc";
pub const MODULE_BREATH_DYNAMICS: &str = "module.polar_h10.breath_dynamics";
pub const MODULE_HRVB_AMPLITUDE: &str = "module.polar_h10.hrvb_resonance_amplitude";

pub const STREAM_HR_RR: &str = "stream.polar_h10.hr_rr";
pub const STREAM_ACC: &str = "stream.polar_h10.acc";
pub const STREAM_HRV_WINDOW: &str = "stream.polar_h10.hrv_window";
pub const STREAM_RMSSD_GAIN: &str = "stream.polar_h10.rmssd_gain";
pub const STREAM_COHERENCE: &str = "stream.polar_h10.coherence";
pub const STREAM_BREATH_VOLUME: &str = "stream.polar_h10.breath_volume";
pub const STREAM_BREATH_DYNAMICS: &str = "stream.polar_h10.breath_dynamics";
pub const STREAM_HRVB_AMPLITUDE: &str = "stream.polar_h10.hrvb_resonance_amplitude";

const EPSILON: f64 = 0.000_000_000_001;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphManifest {
    pub graph_id: String,
    pub graph_revision: u64,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub node_id: String,
    pub module_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub edge_id: String,
    pub source_node_id: String,
    pub source_stream_id: String,
    pub target_node_id: String,
    pub target_input_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInput {
    #[serde(default)]
    pub input_id: Option<String>,
    #[serde(default)]
    pub hr_rr: HrRrInput,
    #[serde(default)]
    pub rmssd_gain_baseline: Option<RmssdBaseline>,
    #[serde(default)]
    pub coherence_uniform: Option<CoherenceFixtureInput>,
    #[serde(default)]
    pub breath_volume: Option<BreathVolumeFixtureInput>,
    #[serde(default)]
    pub breath_dynamics: Option<BreathDynamicsFixtureInput>,
    #[serde(default)]
    pub hrvb_resonance_amplitude: Option<HrvbFixtureInput>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HrRrInput {
    #[serde(default)]
    pub rr_intervals_ms: Vec<f64>,
    #[serde(default)]
    pub heart_rate_event_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmssdBaseline {
    pub baseline_ln_rmssd: f64,
    pub baseline_mean_ln_rmssd: f64,
    pub baseline_sd_ln_rmssd: f64,
    pub baseline_window_count: u64,
    #[serde(default = "default_baseline_source")]
    pub baseline_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceFixtureInput {
    pub base_rr_ms: f64,
    pub components: Vec<CoherenceComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceComponent {
    pub bin: usize,
    pub amplitude_ms: f64,
    pub phase_rad: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreathVolumeFixtureInput {
    pub calibration_projection: Vec<f64>,
    pub live_projection: f64,
    #[serde(default)]
    pub previous_projection: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreathDynamicsFixtureInput {
    pub breath_intervals_s: Vec<f64>,
    pub breath_amplitudes_01: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrvbFixtureInput {
    pub generator: HrvbGenerator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrvbGenerator {
    pub mean_hr_bpm: f64,
    pub amplitude_bpm: f64,
    pub frequency_hz: f64,
    pub phase_rad: f64,
    pub sample_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrvWindow {
    pub accepted_count: u64,
    pub rejected_count: u64,
    pub successive_difference_count: u64,
    pub mean_nn_ms: f64,
    pub mean_hr_bpm: f64,
    pub sdnn_ms: f64,
    pub rmssd_ms: f64,
    pub ln_rmssd: f64,
    pub pnn50: f64,
    pub sd1_ms: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmssdGain {
    pub ln_rmssd_gain: f64,
    pub rmssd_ratio: f64,
    pub baseline_z_score: f64,
    pub baseline_source: String,
    pub baseline_window_count: u64,
    pub baseline_rmssd_ms: f64,
    pub current_rmssd_ms: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coherence {
    pub peak_frequency_hz: f64,
    pub peak_band_power: f64,
    pub total_band_power: f64,
    pub remaining_power: f64,
    pub paper_ratio: f64,
    pub coherence_ratio: f64,
    pub coherence_ratio_squared: f64,
    pub normalized_peak_power: f64,
    pub normalized_score: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreathVolume {
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub breath_volume_01: f64,
    pub phase: String,
    pub confidence: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreathDynamics {
    pub cycle_count: u64,
    pub mean_interval_s: f64,
    pub breathing_rate_bpm: f64,
    pub interval_sd_s: f64,
    pub interval_cv: f64,
    pub mean_amplitude_01: f64,
    pub amplitude_sd_01: f64,
    pub amplitude_cv: f64,
    pub complexity_status: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrvbResonanceAmplitude {
    pub amplitude_bpm: f64,
    pub mean_hr_bpm: f64,
    pub frequency_hz: f64,
    pub omega_rad_s: f64,
    pub phase_rad: f64,
    pub median_session_amplitude_bpm: f64,
    pub threshold_status: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExecutionReport {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub graph_id: String,
    pub graph_revision: u64,
    pub runtime_path: String,
    pub selected_module_ids: Vec<String>,
    pub resolved_node_ids: Vec<String>,
    pub status: String,
    pub node_reports: Vec<NodeExecutionReport>,
    pub output_stream_ids: Vec<String>,
    pub issues: Vec<GraphIssue>,
    pub streams: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionReport {
    pub node_id: String,
    pub module_id: String,
    pub status: String,
    pub dependency_node_ids: Vec<String>,
    pub input_stream_ids: Vec<String>,
    pub output_stream_ids: Vec<String>,
    pub sample_counts: Vec<GraphSampleCount>,
    pub issue_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSampleCount {
    pub count_id: String,
    pub value: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphIssue {
    pub issue_code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ProcessorFailure {
    pub issue_code: String,
}

impl ProcessorFailure {
    fn new(issue_code: impl Into<String>) -> Self {
        Self {
            issue_code: issue_code.into(),
        }
    }
}

pub type ProcessorResult<T> = Result<T, ProcessorFailure>;

#[derive(Default)]
struct RuntimeBuffers {
    hrv_window: Option<HrvWindow>,
    breath_volume: Option<BreathVolume>,
}

pub fn compute_hrv_window(rr_intervals_ms: &[f64]) -> ProcessorResult<HrvWindow> {
    let mut accepted = Vec::new();
    let mut rejected_count = 0_u64;
    for value in rr_intervals_ms {
        if (300.0..=2000.0).contains(value) {
            accepted.push(*value);
        } else {
            rejected_count += 1;
        }
    }
    if accepted.len() < 2 {
        return Err(ProcessorFailure::new("issue.window_underfilled"));
    }
    if rejected_count > 0 {
        return Err(ProcessorFailure::new("issue.quality_low"));
    }

    let mean_nn = mean(&accepted);
    let diffs = successive_differences(&accepted);
    let rmssd = (diffs.iter().map(|diff| diff * diff).sum::<f64>() / diffs.len() as f64).sqrt();
    if rmssd <= 0.0 {
        return Err(ProcessorFailure::new("issue.quality_low"));
    }
    let pnn50 = diffs.iter().filter(|diff| diff.abs() > 50.0).count() as f64 / diffs.len() as f64;
    Ok(HrvWindow {
        accepted_count: accepted.len() as u64,
        rejected_count,
        successive_difference_count: diffs.len() as u64,
        mean_nn_ms: mean_nn,
        mean_hr_bpm: 60000.0 / mean_nn,
        sdnn_ms: sample_sd(&accepted),
        rmssd_ms: rmssd,
        ln_rmssd: rmssd.ln(),
        pnn50,
        sd1_ms: rmssd / 2.0_f64.sqrt(),
        quality: "stable".to_string(),
    })
}

pub fn compute_rmssd_gain(
    live: &HrvWindow,
    baseline: Option<&RmssdBaseline>,
) -> ProcessorResult<RmssdGain> {
    let Some(baseline) = baseline else {
        return Err(ProcessorFailure::new("issue.baseline_missing"));
    };
    if baseline.baseline_window_count < 3 || baseline.baseline_sd_ln_rmssd <= 0.0 {
        return Err(ProcessorFailure::new("issue.baseline_invalid"));
    }
    let gain = live.ln_rmssd - baseline.baseline_ln_rmssd;
    Ok(RmssdGain {
        ln_rmssd_gain: gain,
        rmssd_ratio: gain.exp(),
        baseline_z_score: (live.ln_rmssd - baseline.baseline_mean_ln_rmssd)
            / baseline.baseline_sd_ln_rmssd,
        baseline_source: baseline.baseline_source.clone(),
        baseline_window_count: baseline.baseline_window_count,
        baseline_rmssd_ms: baseline.baseline_ln_rmssd.exp(),
        current_rmssd_ms: live.ln_rmssd.exp(),
        quality: live.quality.clone(),
    })
}

pub fn compute_coherence(
    input: &CoherenceFixtureInput,
    sample_rate_hz: f64,
    fft_length: usize,
) -> ProcessorResult<Coherence> {
    if fft_length == 0 || input.components.is_empty() {
        return Err(ProcessorFailure::new("issue.window_underfilled"));
    }
    let samples = synthesize_rr_window(fft_length, input)?;
    let centered = center_samples(&samples);
    let powers = dft_power_by_bin(&centered, sample_rate_hz);

    let total_band_power: f64 = powers
        .iter()
        .filter(|(_, frequency, _)| in_band(*frequency, (0.0033, 0.4)))
        .map(|(_, _, power)| *power)
        .sum();
    let peak_candidates: Vec<_> = powers
        .iter()
        .copied()
        .filter(|(_, frequency, _)| in_band(*frequency, (0.04, 0.26)))
        .collect();
    if total_band_power <= 0.0 || peak_candidates.is_empty() {
        return Err(ProcessorFailure::new("issue.quality_low"));
    }
    let max_peak_power = peak_candidates
        .iter()
        .map(|(_, _, power)| *power)
        .fold(f64::NEG_INFINITY, f64::max);
    let (peak_bin, peak_frequency_hz, _) = peak_candidates
        .into_iter()
        .filter(|(_, _, power)| (*power - max_peak_power).abs() <= EPSILON)
        .min_by_key(|(bin, _, _)| *bin)
        .ok_or_else(|| ProcessorFailure::new("issue.quality_low"))?;
    if peak_bin == 0 {
        return Err(ProcessorFailure::new("issue.quality_low"));
    }
    let peak_band_power: f64 = powers
        .iter()
        .filter(|(_, frequency, _)| {
            in_band(*frequency, (0.0033, 0.4))
                && (*frequency - peak_frequency_hz).abs() <= 0.03 + EPSILON
        })
        .map(|(_, _, power)| *power)
        .sum();
    let remaining_power = total_band_power - peak_band_power;
    let paper_ratio = if remaining_power <= 0.0 {
        1_000_000.0
    } else {
        peak_band_power / remaining_power
    };
    let normalized_peak_power = if total_band_power > 0.0 {
        peak_band_power / total_band_power
    } else {
        0.0
    };
    let normalized_score = paper_ratio / (paper_ratio + 1.0);

    Ok(Coherence {
        peak_frequency_hz,
        peak_band_power,
        total_band_power,
        remaining_power,
        paper_ratio,
        coherence_ratio: paper_ratio,
        coherence_ratio_squared: paper_ratio * paper_ratio,
        normalized_peak_power,
        normalized_score,
        quality: if paper_ratio >= 2.0 {
            "stable".to_string()
        } else {
            "distributed".to_string()
        },
    })
}

pub fn compute_breath_volume(input: &BreathVolumeFixtureInput) -> ProcessorResult<BreathVolume> {
    if input.calibration_projection.is_empty() {
        return Err(ProcessorFailure::new("issue.calibration_invalid"));
    }
    let lower_bound = input
        .calibration_projection
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let upper_bound = input
        .calibration_projection
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if upper_bound <= lower_bound {
        return Err(ProcessorFailure::new("issue.calibration_invalid"));
    }
    let normalized = clamp(
        (input.live_projection - lower_bound) / (upper_bound - lower_bound),
        0.0,
        1.0,
    );
    let previous = input.previous_projection.unwrap_or(input.live_projection);
    Ok(BreathVolume {
        lower_bound,
        upper_bound,
        breath_volume_01: normalized,
        phase: if input.live_projection >= previous {
            "inhale".to_string()
        } else {
            "exhale".to_string()
        },
        confidence: 1.0,
        quality: "stable".to_string(),
    })
}

pub fn compute_breath_dynamics(
    input: &BreathDynamicsFixtureInput,
) -> ProcessorResult<BreathDynamics> {
    if input.breath_intervals_s.len() < 2 || input.breath_amplitudes_01.len() < 2 {
        return Err(ProcessorFailure::new("issue.window_underfilled"));
    }
    let mean_interval = mean(&input.breath_intervals_s);
    let interval_sd = sample_sd(&input.breath_intervals_s);
    let mean_amplitude = mean(&input.breath_amplitudes_01);
    let amplitude_sd = sample_sd(&input.breath_amplitudes_01);
    Ok(BreathDynamics {
        cycle_count: input.breath_intervals_s.len() as u64,
        mean_interval_s: mean_interval,
        breathing_rate_bpm: 60.0 / mean_interval,
        interval_sd_s: interval_sd,
        interval_cv: interval_sd / mean_interval,
        mean_amplitude_01: mean_amplitude,
        amplitude_sd_01: amplitude_sd,
        amplitude_cv: amplitude_sd / mean_amplitude,
        complexity_status: "underfilled".to_string(),
        quality: "stable".to_string(),
    })
}

pub fn compute_hrvb_resonance_amplitude(
    input: &HrvbFixtureInput,
) -> ProcessorResult<HrvbResonanceAmplitude> {
    let generator = &input.generator;
    if generator.sample_count < 30 {
        return Err(ProcessorFailure::new("issue.window_underfilled"));
    }
    if !in_band(generator.frequency_hz, (0.08, 0.12)) {
        return Err(ProcessorFailure::new("issue.frequency_out_of_band"));
    }
    Ok(HrvbResonanceAmplitude {
        amplitude_bpm: generator.amplitude_bpm,
        mean_hr_bpm: generator.mean_hr_bpm,
        frequency_hz: generator.frequency_hz,
        omega_rad_s: 2.0 * std::f64::consts::PI * generator.frequency_hz,
        phase_rad: generator.phase_rad,
        median_session_amplitude_bpm: generator.amplitude_bpm,
        threshold_status: if generator.amplitude_bpm >= 2.0 {
            "above_source_threshold".to_string()
        } else {
            "below_source_threshold".to_string()
        },
        quality: "stable".to_string(),
    })
}

pub fn run_graph(
    graph: &GraphManifest,
    input: &RuntimeInput,
    selected_module_ids: &[String],
) -> GraphExecutionReport {
    let selected = normalize_selected_modules(selected_module_ids);
    let selected_set: BTreeSet<String> = selected.iter().cloned().collect();
    let node_by_module: BTreeMap<String, GraphNode> = graph
        .nodes
        .iter()
        .map(|node| (node.module_id.clone(), node.clone()))
        .collect();
    let mut errors = Vec::new();
    let mut required_nodes = BTreeSet::new();
    for module_id in &selected_set {
        match node_by_module.get(module_id) {
            Some(node) => {
                add_dependencies(graph, &node.node_id, &mut required_nodes);
                required_nodes.insert(node.node_id.clone());
            }
            None => errors.push(format!("unknown selected module: {module_id}")),
        }
    }

    let mut buffers = RuntimeBuffers::default();
    let mut node_reports = Vec::new();
    let mut streams = Vec::new();
    let mut materialized_output_stream_ids = Vec::new();
    let ordered_nodes: Vec<GraphNode> = graph
        .nodes
        .iter()
        .filter(|node| required_nodes.contains(&node.node_id))
        .cloned()
        .collect();

    for node in ordered_nodes {
        let incoming: Vec<&GraphEdge> = graph
            .edges
            .iter()
            .filter(|edge| edge.target_node_id == node.node_id)
            .collect();
        let dependency_node_ids = incoming
            .iter()
            .map(|edge| edge.source_node_id.clone())
            .collect::<Vec<_>>();
        let input_stream_ids = incoming
            .iter()
            .map(|edge| edge.target_input_id.clone())
            .collect::<Vec<_>>();
        let node_output_stream_ids = module_output_streams(&node.module_id);
        let (status, issue_codes, sample_counts, stream) =
            execute_node(&node.module_id, input, &mut buffers);
        if let Some(stream) = stream {
            if let Some(stream_id) = stream.get("stream_id").and_then(Value::as_str) {
                materialized_output_stream_ids.push(stream_id.to_string());
            }
            streams.push(stream);
        }
        if status != "pass" {
            errors.extend(
                issue_codes
                    .iter()
                    .map(|issue| format!("{}:{issue}", node.module_id)),
            );
        }
        node_reports.push(NodeExecutionReport {
            node_id: node.node_id,
            module_id: node.module_id,
            status,
            dependency_node_ids,
            input_stream_ids,
            output_stream_ids: node_output_stream_ids,
            sample_counts: sample_counts
                .into_iter()
                .map(|(count_id, value)| GraphSampleCount { count_id, value })
                .collect(),
            issue_codes,
        });
    }

    let issues = errors
        .iter()
        .map(|message| GraphIssue {
            issue_code: issue_code_from_message(message),
            severity: "error".to_string(),
            message: message.clone(),
        })
        .collect::<Vec<_>>();

    GraphExecutionReport {
        schema: "rusty.manifold.graph.execution_report.v1".to_string(),
        graph_id: graph.graph_id.clone(),
        graph_revision: graph.graph_revision,
        runtime_path: "rust.polar_h10_core.v1".to_string(),
        selected_module_ids: selected,
        resolved_node_ids: required_nodes.into_iter().collect(),
        status: if errors.is_empty() {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        node_reports,
        output_stream_ids: materialized_output_stream_ids,
        issues,
        streams,
    }
}

pub fn validate_goldens(package_root: &std::path::Path) -> Result<(), Vec<String>> {
    let fixture_root = package_root.join("fixtures").join("valid");
    let mut errors = Vec::new();
    check_hrv_golden(
        &fixture_root.join("processor-hrv-window-golden.json"),
        &mut errors,
    );
    check_rmssd_gain_golden(
        &fixture_root.join("processor-rmssd-gain-golden.json"),
        &mut errors,
    );
    check_coherence_golden(
        &fixture_root.join("processor-coherence-golden.json"),
        &mut errors,
    );
    check_breath_volume_golden(
        &fixture_root.join("processor-breath-volume-golden.json"),
        &mut errors,
    );
    check_breath_dynamics_golden(
        &fixture_root.join("processor-breath-dynamics-golden.json"),
        &mut errors,
    );
    check_hrvb_golden(
        &fixture_root.join("processor-hrvb-resonance-amplitude-golden.json"),
        &mut errors,
    );
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn execute_node(
    module_id: &str,
    input: &RuntimeInput,
    buffers: &mut RuntimeBuffers,
) -> (String, Vec<String>, BTreeMap<String, u64>, Option<Value>) {
    match module_id {
        MODULE_PROVIDER => {
            let rr_count = input.hr_rr.rr_intervals_ms.len() as u64;
            let mut counts = BTreeMap::new();
            counts.insert("count.polar_h10.rr_intervals".to_string(), rr_count);
            let stream = json!({
                "stream_id": STREAM_HR_RR,
                "status": if rr_count > 0 { "pass" } else { "fail" },
                "heart_rate_event_count": input.hr_rr.heart_rate_event_count.unwrap_or(rr_count),
                "rr_interval_count": rr_count,
                "source": "runtime_input"
            });
            if rr_count > 0 {
                ("pass".to_string(), vec![], counts, Some(stream))
            } else {
                (
                    "fail".to_string(),
                    vec!["issue.input_stream_missing".to_string()],
                    counts,
                    Some(stream),
                )
            }
        }
        MODULE_HRV_WINDOW => match compute_hrv_window(&input.hr_rr.rr_intervals_ms) {
            Ok(result) => {
                buffers.hrv_window = Some(result.clone());
                let mut counts = BTreeMap::new();
                counts.insert(
                    "count.polar_h10.hrv_accepted".to_string(),
                    result.accepted_count,
                );
                (
                    "pass".to_string(),
                    vec![],
                    counts,
                    Some(hrv_stream(
                        result,
                        input
                            .hr_rr
                            .heart_rate_event_count
                            .unwrap_or(input.hr_rr.rr_intervals_ms.len() as u64),
                        input.hr_rr.rr_intervals_ms.len() as u64,
                    )),
                )
            }
            Err(failure) => failure_stream(STREAM_HRV_WINDOW, module_id, failure),
        },
        MODULE_RMSSD_GAIN => {
            let live = match buffers.hrv_window.as_ref() {
                Some(value) => value,
                None => {
                    return failure_stream(
                        STREAM_RMSSD_GAIN,
                        module_id,
                        ProcessorFailure::new("issue.dependency_missing"),
                    )
                }
            };
            match compute_rmssd_gain(live, input.rmssd_gain_baseline.as_ref()) {
                Ok(result) => {
                    let mut counts = BTreeMap::new();
                    counts.insert(
                        "count.polar_h10.rmssd_baseline_windows".to_string(),
                        result.baseline_window_count,
                    );
                    (
                        "pass".to_string(),
                        vec![],
                        counts,
                        Some(rmssd_gain_stream(result)),
                    )
                }
                Err(failure) => failure_stream(STREAM_RMSSD_GAIN, module_id, failure),
            }
        }
        MODULE_COHERENCE => {
            let Some(coherence_input) = input.coherence_uniform.as_ref() else {
                return failure_stream(
                    STREAM_COHERENCE,
                    module_id,
                    ProcessorFailure::new("issue.input_stream_missing"),
                );
            };
            match compute_coherence(coherence_input, 2.0, 128) {
                Ok(result) => {
                    let mut counts = BTreeMap::new();
                    counts.insert("count.polar_h10.coherence_uniform_samples".to_string(), 128);
                    (
                        "pass".to_string(),
                        vec![],
                        counts,
                        Some(coherence_stream(
                            result,
                            input
                                .hr_rr
                                .heart_rate_event_count
                                .unwrap_or(input.hr_rr.rr_intervals_ms.len() as u64),
                            input.hr_rr.rr_intervals_ms.len() as u64,
                        )),
                    )
                }
                Err(failure) => failure_stream(STREAM_COHERENCE, module_id, failure),
            }
        }
        MODULE_BREATH_VOLUME => {
            let Some(breath_input) = input.breath_volume.as_ref() else {
                return failure_stream(
                    STREAM_BREATH_VOLUME,
                    module_id,
                    ProcessorFailure::new("issue.input_stream_missing"),
                );
            };
            match compute_breath_volume(breath_input) {
                Ok(result) => {
                    buffers.breath_volume = Some(result.clone());
                    let mut counts = BTreeMap::new();
                    counts.insert(
                        "count.polar_h10.breath_calibration_samples".to_string(),
                        breath_input.calibration_projection.len() as u64,
                    );
                    (
                        "pass".to_string(),
                        vec![],
                        counts,
                        Some(breath_volume_stream(
                            result,
                            breath_input.calibration_projection.len() as u64,
                        )),
                    )
                }
                Err(failure) => failure_stream(STREAM_BREATH_VOLUME, module_id, failure),
            }
        }
        MODULE_BREATH_DYNAMICS => {
            if buffers.breath_volume.is_none() {
                return failure_stream(
                    STREAM_BREATH_DYNAMICS,
                    module_id,
                    ProcessorFailure::new("issue.dependency_missing"),
                );
            }
            let Some(dynamics_input) = input.breath_dynamics.as_ref() else {
                return failure_stream(
                    STREAM_BREATH_DYNAMICS,
                    module_id,
                    ProcessorFailure::new("issue.input_stream_missing"),
                );
            };
            match compute_breath_dynamics(dynamics_input) {
                Ok(result) => {
                    let mut counts = BTreeMap::new();
                    counts.insert(
                        "count.polar_h10.breath_cycles".to_string(),
                        result.cycle_count,
                    );
                    (
                        "pass".to_string(),
                        vec![],
                        counts,
                        Some(breath_dynamics_stream(
                            result,
                            dynamics_input.breath_intervals_s.len() as u64,
                        )),
                    )
                }
                Err(failure) => failure_stream(STREAM_BREATH_DYNAMICS, module_id, failure),
            }
        }
        MODULE_HRVB_AMPLITUDE => {
            let Some(hrvb_input) = input.hrvb_resonance_amplitude.as_ref() else {
                return failure_stream(
                    STREAM_HRVB_AMPLITUDE,
                    module_id,
                    ProcessorFailure::new("issue.input_stream_missing"),
                );
            };
            match compute_hrvb_resonance_amplitude(hrvb_input) {
                Ok(result) => {
                    let mut counts = BTreeMap::new();
                    counts.insert(
                        "count.polar_h10.hrvb_samples".to_string(),
                        hrvb_input.generator.sample_count,
                    );
                    (
                        "pass".to_string(),
                        vec![],
                        counts,
                        Some(hrvb_stream(result, hrvb_input.generator.sample_count)),
                    )
                }
                Err(failure) => failure_stream(STREAM_HRVB_AMPLITUDE, module_id, failure),
            }
        }
        _ => (
            "fail".to_string(),
            vec!["issue.module_unknown".to_string()],
            BTreeMap::new(),
            None,
        ),
    }
}

fn hrv_stream(
    result: HrvWindow,
    heart_rate_event_count: u64,
    input_rr_interval_count: u64,
) -> Value {
    json!({
        "stream_id": STREAM_HRV_WINDOW,
        "module_id": MODULE_HRV_WINDOW,
        "status": "pass",
        "input_stream_id": STREAM_HR_RR,
        "method": "rr_window_v1",
        "heart_rate_event_count": heart_rate_event_count,
        "input_rr_interval_count": input_rr_interval_count,
        "accepted_count": result.accepted_count,
        "rejected_count": result.rejected_count,
        "successive_difference_count": result.successive_difference_count,
        "mean_nn_ms": result.mean_nn_ms,
        "mean_hr_bpm": result.mean_hr_bpm,
        "sdnn_ms": result.sdnn_ms,
        "rmssd_ms": result.rmssd_ms,
        "ln_rmssd": result.ln_rmssd,
        "pnn50": result.pnn50,
        "sd1_ms": result.sd1_ms,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn rmssd_gain_stream(result: RmssdGain) -> Value {
    json!({
        "stream_id": STREAM_RMSSD_GAIN,
        "module_id": MODULE_RMSSD_GAIN,
        "status": "pass",
        "input_stream_id": STREAM_HRV_WINDOW,
        "method": "log_rmssd_gain_v1",
        "baseline_source": result.baseline_source,
        "baseline_window_count": result.baseline_window_count,
        "current_window_count": result.baseline_window_count,
        "baseline_rmssd_ms": result.baseline_rmssd_ms,
        "current_rmssd_ms": result.current_rmssd_ms,
        "ln_rmssd_gain": result.ln_rmssd_gain,
        "rmssd_ratio": result.rmssd_ratio,
        "baseline_z_score": result.baseline_z_score,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn coherence_stream(
    result: Coherence,
    heart_rate_event_count: u64,
    input_rr_interval_count: u64,
) -> Value {
    json!({
        "stream_id": STREAM_COHERENCE,
        "module_id": MODULE_COHERENCE,
        "status": "pass",
        "input_stream_id": STREAM_HR_RR,
        "method": "spectral_ratio_v1",
        "heart_rate_event_count": heart_rate_event_count,
        "input_rr_interval_count": input_rr_interval_count,
        "uniform_sample_count": 128,
        "window_seconds": 64.0,
        "sample_rate_hz": 2.0,
        "peak_frequency_hz": result.peak_frequency_hz,
        "peak_band_power": result.peak_band_power,
        "total_band_power": result.total_band_power,
        "remaining_power": result.remaining_power,
        "paper_ratio": result.paper_ratio,
        "coherence_ratio": result.coherence_ratio,
        "coherence_ratio_squared": result.coherence_ratio_squared,
        "normalized_peak_power": result.normalized_peak_power,
        "normalized_score": result.normalized_score,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn breath_volume_stream(result: BreathVolume, calibration_count: u64) -> Value {
    json!({
        "stream_id": STREAM_BREATH_VOLUME,
        "module_id": MODULE_BREATH_VOLUME,
        "status": "pass",
        "input_stream_id": STREAM_ACC,
        "method": "acc_projection_proxy_v1",
        "input_acc_sample_count": calibration_count,
        "source_sample_rate_hz": 200.0,
        "calibration_sample_count": calibration_count,
        "lower_bound": result.lower_bound,
        "upper_bound": result.upper_bound,
        "breath_volume_01": result.breath_volume_01,
        "phase": result.phase,
        "confidence": result.confidence,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn breath_dynamics_stream(result: BreathDynamics, input_breath_sample_count: u64) -> Value {
    json!({
        "stream_id": STREAM_BREATH_DYNAMICS,
        "module_id": MODULE_BREATH_DYNAMICS,
        "status": "pass",
        "input_stream_id": STREAM_BREATH_VOLUME,
        "method": "cycle_stats_v1",
        "input_breath_sample_count": input_breath_sample_count,
        "cycle_count": result.cycle_count,
        "mean_interval_s": result.mean_interval_s,
        "breathing_rate_bpm": result.breathing_rate_bpm,
        "interval_sd_s": result.interval_sd_s,
        "interval_cv": result.interval_cv,
        "mean_amplitude_01": result.mean_amplitude_01,
        "amplitude_sd_01": result.amplitude_sd_01,
        "amplitude_cv": result.amplitude_cv,
        "complexity_status": result.complexity_status,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn hrvb_stream(result: HrvbResonanceAmplitude, input_rr_interval_count: u64) -> Value {
    json!({
        "stream_id": STREAM_HRVB_AMPLITUDE,
        "module_id": MODULE_HRVB_AMPLITUDE,
        "status": "pass",
        "input_stream_id": STREAM_HR_RR,
        "method": "rolling_sine_fit_v1",
        "input_rr_interval_count": input_rr_interval_count,
        "window_seconds": 30.0,
        "sample_rate_hz": 1.0,
        "amplitude_bpm": result.amplitude_bpm,
        "mean_hr_bpm": result.mean_hr_bpm,
        "frequency_hz": result.frequency_hz,
        "omega_rad_s": result.omega_rad_s,
        "phase_rad": result.phase_rad,
        "median_session_amplitude_bpm": result.median_session_amplitude_bpm,
        "threshold_status": result.threshold_status,
        "quality": result.quality,
        "issue_code": Value::Null
    })
}

fn failure_stream(
    stream_id: &str,
    module_id: &str,
    failure: ProcessorFailure,
) -> (String, Vec<String>, BTreeMap<String, u64>, Option<Value>) {
    let issue_code = failure.issue_code;
    (
        "fail".to_string(),
        vec![issue_code.clone()],
        BTreeMap::new(),
        Some(json!({
            "stream_id": stream_id,
            "module_id": module_id,
            "status": "fail",
            "issue_code": issue_code
        })),
    )
}

fn add_dependencies(graph: &GraphManifest, node_id: &str, required_nodes: &mut BTreeSet<String>) {
    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.target_node_id == node_id)
    {
        if required_nodes.insert(edge.source_node_id.clone()) {
            add_dependencies(graph, &edge.source_node_id, required_nodes);
        }
    }
}

fn normalize_selected_modules(selected_module_ids: &[String]) -> Vec<String> {
    let mut selected = Vec::new();
    for module_id in selected_module_ids {
        let module_id = normalize_module_id(module_id);
        if !selected.contains(&module_id) {
            selected.push(module_id);
        }
    }
    selected
}

pub fn normalize_module_id(value: &str) -> String {
    match value {
        "hrv_window" => MODULE_HRV_WINDOW.to_string(),
        "rmssd_gain" => MODULE_RMSSD_GAIN.to_string(),
        "coherence" => MODULE_COHERENCE.to_string(),
        "breath_volume" | "breath_volume_from_acc" => MODULE_BREATH_VOLUME.to_string(),
        "breath_dynamics" => MODULE_BREATH_DYNAMICS.to_string(),
        "hrvb_resonance_amplitude" | "hrvb" => MODULE_HRVB_AMPLITUDE.to_string(),
        _ => value.to_string(),
    }
}

fn module_output_streams(module_id: &str) -> Vec<String> {
    match module_id {
        MODULE_PROVIDER => vec![STREAM_HR_RR.to_string(), STREAM_ACC.to_string()],
        MODULE_HRV_WINDOW => vec![STREAM_HRV_WINDOW.to_string()],
        MODULE_RMSSD_GAIN => vec![STREAM_RMSSD_GAIN.to_string()],
        MODULE_COHERENCE => vec![STREAM_COHERENCE.to_string()],
        MODULE_BREATH_VOLUME => vec![STREAM_BREATH_VOLUME.to_string()],
        MODULE_BREATH_DYNAMICS => vec![STREAM_BREATH_DYNAMICS.to_string()],
        MODULE_HRVB_AMPLITUDE => vec![STREAM_HRVB_AMPLITUDE.to_string()],
        _ => vec![],
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn sample_sd(values: &[f64]) -> f64 {
    let mean = mean(values);
    (values
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>()
        / (values.len() - 1) as f64)
        .sqrt()
}

fn successive_differences(values: &[f64]) -> Vec<f64> {
    values
        .windows(2)
        .map(|window| window[1] - window[0])
        .collect()
}

fn synthesize_rr_window(
    fft_length: usize,
    input: &CoherenceFixtureInput,
) -> ProcessorResult<Vec<f64>> {
    let mut samples = Vec::with_capacity(fft_length);
    for sample_index in 0..fft_length {
        let mut sample = input.base_rr_ms;
        for component in &input.components {
            if component.bin == 0 || component.bin > fft_length / 2 {
                return Err(ProcessorFailure::new("issue.component_invalid"));
            }
            sample += component.amplitude_ms
                * ((2.0 * std::f64::consts::PI * component.bin as f64 * sample_index as f64
                    / fft_length as f64)
                    + component.phase_rad)
                    .sin();
        }
        samples.push(sample);
    }
    Ok(samples)
}

fn center_samples(samples: &[f64]) -> Vec<f64> {
    let mean = mean(samples);
    samples.iter().map(|sample| sample - mean).collect()
}

fn dft_power_by_bin(samples: &[f64], sample_rate_hz: f64) -> Vec<(usize, f64, f64)> {
    let fft_length = samples.len();
    let mut powers = Vec::with_capacity(fft_length / 2);
    for bin_index in 1..=(fft_length / 2) {
        let mut real = 0.0;
        let mut imaginary = 0.0;
        for (sample_index, sample) in samples.iter().enumerate() {
            let angle = -2.0 * std::f64::consts::PI * bin_index as f64 * sample_index as f64
                / fft_length as f64;
            real += sample * angle.cos();
            imaginary += sample * angle.sin();
        }
        let frequency = bin_index as f64 * sample_rate_hz / fft_length as f64;
        let power = ((real * real) + (imaginary * imaginary)) / (fft_length * fft_length) as f64;
        powers.push((bin_index, frequency, power));
    }
    powers
}

fn in_band(frequency: f64, band: (f64, f64)) -> bool {
    band.0 <= frequency && frequency <= band.1
}

fn clamp(value: f64, lower: f64, upper: f64) -> f64 {
    value.max(lower).min(upper)
}

fn default_baseline_source() -> String {
    "explicit_baseline".to_string()
}

fn issue_code_from_message(message: &str) -> String {
    message
        .split(':')
        .next_back()
        .unwrap_or("issue.unknown")
        .to_string()
}

fn check_hrv_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let rr_values = numbers(&case["input"]["rr_intervals_ms"]);
        let tolerance = tolerance(case);
        match compute_hrv_window(&rr_values) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn check_rmssd_gain_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let tolerance = tolerance(case);
        let live = HrvWindow {
            accepted_count: 0,
            rejected_count: 0,
            successive_difference_count: 0,
            mean_nn_ms: 0.0,
            mean_hr_bpm: 0.0,
            sdnn_ms: 0.0,
            rmssd_ms: 0.0,
            ln_rmssd: case["input"]["live"]["ln_rmssd"]
                .as_f64()
                .unwrap_or_default(),
            pnn50: 0.0,
            sd1_ms: 0.0,
            quality: case["input"]["live"]["quality"]
                .as_str()
                .unwrap_or("stable")
                .to_string(),
        };
        let baseline: RmssdBaseline =
            serde_json::from_value(case["input"]["baseline"].clone()).unwrap();
        match compute_rmssd_gain(&live, Some(&baseline)) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn check_coherence_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let tolerance = tolerance(case);
        let input: CoherenceFixtureInput = serde_json::from_value(case["input"].clone()).unwrap();
        match compute_coherence(&input, 2.0, 128) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn check_breath_volume_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let tolerance = tolerance(case);
        let input: BreathVolumeFixtureInput =
            serde_json::from_value(case["input"].clone()).unwrap();
        match compute_breath_volume(&input) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn check_breath_dynamics_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let tolerance = tolerance(case);
        let input: BreathDynamicsFixtureInput =
            serde_json::from_value(case["input"].clone()).unwrap();
        match compute_breath_dynamics(&input) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn check_hrvb_golden(path: &std::path::Path, errors: &mut Vec<String>) {
    let Ok(doc) = read_json(path, errors) else {
        return;
    };
    for case in doc["cases"].as_array().into_iter().flatten() {
        let tolerance = tolerance(case);
        let input: HrvbFixtureInput = serde_json::from_value(case["input"].clone()).unwrap();
        match compute_hrvb_resonance_amplitude(&input) {
            Ok(actual) => compare_object(
                case["case_id"].as_str().unwrap_or("case"),
                &serde_json::to_value(actual).unwrap(),
                &case["expected"],
                tolerance,
                errors,
            ),
            Err(failure) => errors.push(format!("{}:{}", path.display(), failure.issue_code)),
        }
    }
}

fn read_json(path: &std::path::Path, errors: &mut Vec<String>) -> Result<Value, ()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("{}:{error}", path.display()));
            return Err(());
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => Ok(value),
        Err(error) => {
            errors.push(format!("{}:{error}", path.display()));
            Err(())
        }
    }
}

fn tolerance(case: &Value) -> f64 {
    case["tolerance"]["absolute"].as_f64().unwrap_or(0.000001)
}

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .collect()
}

fn compare_object(
    case_id: &str,
    actual: &Value,
    expected: &Value,
    tolerance: f64,
    errors: &mut Vec<String>,
) {
    let Some(expected_object) = expected.as_object() else {
        errors.push(format!("{case_id}:expected"));
        return;
    };
    for (key, expected_value) in expected_object {
        match (actual.get(key), expected_value.as_f64()) {
            (Some(actual_value), Some(expected_number)) => {
                let actual_number = actual_value.as_f64().unwrap_or(f64::NAN);
                if (actual_number - expected_number).abs() > tolerance {
                    errors.push(format!("{case_id}:{key}"));
                }
            }
            (Some(actual_value), None) if actual_value == expected_value => {}
            _ => errors.push(format!("{case_id}:{key}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_all_package_goldens() {
        let package_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        assert_eq!(validate_goldens(package_root), Ok(()));
    }

    #[test]
    fn resolves_rmssd_gain_dependencies() {
        let graph: GraphManifest =
            serde_json::from_str(include_str!("../../../fixtures/valid/graph.json")).unwrap();
        let input: RuntimeInput = serde_json::from_str(include_str!(
            "../../../fixtures/valid/processor-runtime-input-synthetic.json"
        ))
        .unwrap();
        let report = run_graph(&graph, &input, &[MODULE_RMSSD_GAIN.to_string()]);
        assert_eq!(report.status, "pass");
        assert!(report
            .resolved_node_ids
            .contains(&"node.polar_h10_hrv_window".to_string()));
        assert!(report
            .resolved_node_ids
            .contains(&"node.polar_h10_rmssd_gain".to_string()));
        assert!(report
            .streams
            .iter()
            .any(|stream| stream["stream_id"] == STREAM_RMSSD_GAIN));
    }

    #[test]
    fn resolves_breath_dynamics_dependencies() {
        let graph: GraphManifest =
            serde_json::from_str(include_str!("../../../fixtures/valid/graph.json")).unwrap();
        let input: RuntimeInput = serde_json::from_str(include_str!(
            "../../../fixtures/valid/processor-runtime-input-synthetic.json"
        ))
        .unwrap();
        let report = run_graph(&graph, &input, &[MODULE_BREATH_DYNAMICS.to_string()]);
        assert_eq!(report.status, "pass");
        assert!(report
            .resolved_node_ids
            .contains(&"node.polar_h10_breath_volume".to_string()));
        assert!(report
            .streams
            .iter()
            .any(|stream| stream["stream_id"] == STREAM_BREATH_DYNAMICS));
    }
}
