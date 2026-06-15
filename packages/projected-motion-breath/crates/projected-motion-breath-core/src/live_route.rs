//! Live route execution, transport-event conversion, and incremental processor state.

use crate::documents::{
    read_live_route_fixture, read_profile, read_source_binding, AdapterNormalizationInput,
    LiveRouteExecutionMode, LiveRouteFixture, LiveRouteSourceFixture, NormalizedAdapterSample,
    ProfileControllerStateClassifier, ProfileDocument, SourceBinding,
};
use crate::math::*;
use crate::state_value::{
    breath_state01, normalize_breath_state, BreathStateValueConfig, BreathStateValueProcessor,
};
use crate::validation::{
    dedup_issue_codes, source_payload_kind_matches, validate_live_route_expected,
    validate_live_route_fixture, validate_live_transport_route_observed, validate_profile_document,
    validate_source_binding,
};
use crate::{
    classify_phase, motion_profile_from_document, normalize_adapter_sample, InputKind,
    ProjectedMotionBreathTracker, RigidMotionSample, ValidationError, VectorMotionSample,
    EXTERNAL_STREAM_POLAR_ACC, LIVE_ROUTE_REPORT_SCHEMA, STREAM_BREATH_FEEDBACK_STATE,
    STREAM_BREATH_STATE, STREAM_BREATH_STATE_VALUE, STREAM_BREATH_VOLUME, STREAM_OBJECT_POSE,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct LiveRouteReport {
    pub schema: &'static str,
    pub package_root: String,
    pub status: String,
    pub route_id: String,
    pub input_stream_ids: Vec<String>,
    pub normalized_stream_ids: Vec<String>,
    pub output_stream_ids: Vec<String>,
    pub processor_core_executed: bool,
    pub runtime_execution_performed: bool,
    pub external_transport_used: bool,
    pub live_sensor_used: bool,
    pub headset_execution_performed: bool,
    pub plan_only: bool,
    pub source_routes: Vec<LiveSourceRouteReport>,
    pub breath_samples: Vec<LiveBreathSample>,
    pub state_samples: Vec<LiveBreathStateSample>,
    pub state_value_samples: Vec<LiveBreathStateValueSample>,
    pub feedback_samples: Vec<LiveFeedbackSample>,
    pub receiver_subscription: ReceiverBreathSubscriptionPlan,
    pub receiver_receipts: Vec<ReceiverBreathReceiptPlan>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveSourceRouteReport {
    pub source_id: String,
    pub source_stream_id: String,
    pub normalized_stream_id: String,
    pub binding_id: String,
    pub selected_adapter_id: String,
    pub selected_source_kind: String,
    pub source_payload_kind: String,
    pub sample_count: usize,
    pub normalized_sample_count: usize,
    pub estimate_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveBreathSample {
    pub sequence_id: u64,
    pub source_id: String,
    pub input_stream_id: String,
    pub normalized_stream_id: String,
    pub output_stream_id: String,
    pub sample_index: usize,
    pub sample_time_s: f64,
    pub host_time_s: f64,
    pub projection: f64,
    pub volume01: f64,
    pub phase: String,
    pub tracking01: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveFeedbackSample {
    pub sequence_id: u64,
    pub stream_id: String,
    pub source_breath_sequence_id: u64,
    pub source_id: String,
    pub sample_time_unix_ns: i64,
    pub volume01: f64,
    pub phase: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveBreathStateSample {
    pub sequence_id: u64,
    pub stream_id: String,
    pub source_breath_sequence_id: u64,
    pub source_id: String,
    pub sample_time_unix_ns: i64,
    pub state: String,
    pub state01: f64,
    pub phase: String,
    pub tracking01: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveBreathStateValueSample {
    pub sequence_id: u64,
    pub stream_id: String,
    pub source_breath_sequence_id: u64,
    pub source_state_sequence_id: u64,
    pub source_id: String,
    pub sample_time_unix_ns: i64,
    pub state: String,
    pub state01: f64,
    pub target01: f64,
    pub value01: f64,
    pub delta_seconds: f64,
    pub stale_gap: bool,
    pub tracking01: f64,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiverBreathSubscriptionPlan {
    pub command: String,
    pub stream: String,
    pub receiver_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReceiverBreathReceiptPlan {
    pub command: String,
    pub schema: String,
    pub received_stream: String,
    pub received_sequence_id: u64,
    pub receiver_id: String,
    pub acknowledged: bool,
}

pub fn run_live_route_self_test(
    package_root: impl AsRef<Path>,
) -> Result<LiveRouteReport, ValidationError> {
    let package_root = package_root.as_ref();
    let fixture_path = package_root
        .join("fixtures")
        .join("valid")
        .join("live-route-self-test.json");
    let fixture = read_live_route_fixture(&fixture_path)?;
    let source_samples = fixture
        .sources
        .iter()
        .map(|source| (source.source_stream_id.clone(), source.samples.clone()))
        .collect();
    run_live_route_with_source_samples(
        package_root,
        fixture,
        source_samples,
        Vec::new(),
        LiveRouteExecutionMode {
            runtime_execution_performed: true,
            external_transport_used: false,
            live_sensor_used: false,
            headset_execution_performed: false,
            plan_only: true,
            validate_fixture_expected: true,
        },
    )
}

pub fn run_live_route_from_transport_events(
    package_root: impl AsRef<Path>,
    events_jsonl: impl AsRef<Path>,
) -> Result<LiveRouteReport, ValidationError> {
    let package_root = package_root.as_ref();
    let fixture_path = package_root
        .join("fixtures")
        .join("valid")
        .join("live-route-self-test.json");
    let fixture = read_live_route_fixture(&fixture_path)?;
    let (source_samples, event_issues) =
        read_live_transport_event_samples(events_jsonl.as_ref(), &fixture)?;
    run_live_route_with_source_samples(
        package_root,
        fixture,
        source_samples,
        event_issues,
        LiveRouteExecutionMode {
            runtime_execution_performed: true,
            external_transport_used: true,
            live_sensor_used: true,
            headset_execution_performed: true,
            plan_only: false,
            validate_fixture_expected: false,
        },
    )
}

pub struct LiveTransportProcessor {
    package_root: PathBuf,
    route_id: String,
    input_stream_ids: Vec<String>,
    normalized_stream_ids: Vec<String>,
    output_stream_ids: Vec<String>,
    sources: BTreeMap<String, LiveTransportSourceProcessor>,
    next_sequence_id: u64,
}

#[derive(Debug, Serialize)]
pub struct LiveTransportProcessorUpdate {
    pub schema: &'static str,
    pub status: String,
    pub route_id: String,
    pub package_root: String,
    pub input_stream_ids: Vec<String>,
    pub normalized_stream_ids: Vec<String>,
    pub output_stream_ids: Vec<String>,
    pub input_stream_id: String,
    pub selected_source_preference: String,
    pub selected_source_effective: String,
    pub event_sample_count: usize,
    pub normalized_sample_count: usize,
    pub output_sample_count: usize,
    pub source_updates: Vec<LiveTransportSourceUpdate>,
    pub breath_samples: Vec<LiveBreathSample>,
    pub state_samples: Vec<LiveBreathStateSample>,
    pub state_value_samples: Vec<LiveBreathStateValueSample>,
    pub feedback_samples: Vec<LiveFeedbackSample>,
    pub issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LiveTransportSourceUpdate {
    pub source_id: String,
    pub source_stream_id: String,
    pub source_kind: String,
    pub calibration_status: String,
    pub calibration_sample_count: usize,
    pub calibration_sample_target: usize,
    pub estimate_count: usize,
    pub output_sample_count: usize,
}

impl LiveTransportProcessor {
    pub fn open(package_root: impl AsRef<Path>) -> Result<Self, String> {
        let package_root = package_root.as_ref().to_path_buf();
        let fixture_path = package_root
            .join("fixtures")
            .join("valid")
            .join("live-route-self-test.json");
        let fixture = read_live_route_fixture(&fixture_path).map_err(|error| error.to_string())?;
        let issues = validate_live_route_fixture(&fixture);
        if !issues.is_empty() {
            return Err(issues.join(","));
        }

        let mut sources = BTreeMap::new();
        for source_fixture in fixture.sources {
            let binding_path = package_root.join(&source_fixture.binding_path);
            let binding = read_source_binding(&binding_path).map_err(|error| error.to_string())?;
            if let Some(issue) = validate_source_binding(&package_root, &binding) {
                return Err(format!("{}:{issue}", binding.binding_id));
            }
            if !source_payload_kind_matches(
                &binding.selected_source_kind,
                &source_fixture.source_payload_kind,
            ) {
                return Err(format!(
                    "{}:issue.adapter_payload_kind_mismatch",
                    source_fixture.source_id
                ));
            }
            let profile = read_profile(&package_root.join(&binding.profile_path))
                .map_err(|error| error.to_string())?;
            let profile_issues = validate_profile_document(&profile);
            if !profile_issues.is_empty() {
                return Err(format!(
                    "{}:{}",
                    source_fixture.source_id,
                    profile_issues.join(",")
                ));
            }
            let source = LiveTransportSourceProcessor::new(source_fixture, binding, profile)?;
            sources.insert(source.source_stream_id.clone(), source);
        }

        Ok(Self {
            package_root,
            route_id: fixture.route_id,
            input_stream_ids: fixture.input_stream_ids,
            normalized_stream_ids: fixture.normalized_stream_ids,
            output_stream_ids: fixture.output_stream_ids,
            sources,
            next_sequence_id: 1,
        })
    }

    pub fn push_transport_event_json(
        &mut self,
        event_json: &str,
        selected_source_preference: &str,
    ) -> LiveTransportProcessorUpdate {
        let selected_source_preference =
            normalize_live_selected_source_preference(selected_source_preference);
        let mut issues = Vec::new();
        let mut input_stream_id = String::new();
        let mut event_sample_count = 0;
        let mut normalized_sample_count = 0;
        let mut breath_samples = Vec::new();
        let mut state_samples = Vec::new();
        let mut state_value_samples = Vec::new();
        let mut source_updates = Vec::new();

        let event: serde_json::Value = match serde_json::from_str(event_json) {
            Ok(event) => event,
            Err(error) => {
                return self.update_report(
                    "fail",
                    input_stream_id,
                    selected_source_preference,
                    "unknown".to_string(),
                    event_sample_count,
                    normalized_sample_count,
                    source_updates,
                    breath_samples,
                    state_samples,
                    state_value_samples,
                    vec![format!("issue.transport_event_json_invalid:{error}")],
                );
            }
        };
        let payload = event.get("payload").unwrap_or(&serde_json::Value::Null);
        input_stream_id = event
            .get("stream")
            .and_then(serde_json::Value::as_str)
            .or_else(|| event.get("stream_id").and_then(serde_json::Value::as_str))
            .or_else(|| payload.get("stream_id").and_then(serde_json::Value::as_str))
            .unwrap_or("")
            .to_string();

        if let Some(source) = self.sources.get_mut(&input_stream_id) {
            match source.transport_inputs(&event, payload) {
                Ok(inputs) => {
                    event_sample_count = inputs.len();
                    for input in inputs {
                        match normalize_adapter_sample(
                            &source.binding,
                            &source.source_payload_kind,
                            &input,
                        ) {
                            Ok(normalized) => {
                                normalized_sample_count += 1;
                                let (mut produced, mut source_issues) = source
                                    .push_normalized_sample(normalized, &mut self.next_sequence_id);
                                let (mut produced_state_samples, mut produced_state_value_samples) =
                                    source.derive_state_outputs(&produced);
                                breath_samples.append(&mut produced);
                                state_samples.append(&mut produced_state_samples);
                                state_value_samples.append(&mut produced_state_value_samples);
                                issues.append(&mut source_issues);
                            }
                            Err(issue) => issues.push(format!(
                                "{}:sample.{}:{issue}",
                                source.source_id, source.next_sample_index
                            )),
                        }
                    }
                    source_updates.push(source.update_summary());
                }
                Err(issue) => issues.push(format!("{}:{issue}", source.source_id)),
            }
        }

        let selected_source_effective =
            self.effective_selected_source(&selected_source_preference, &breath_samples);
        self.update_report(
            "pass",
            input_stream_id,
            selected_source_preference,
            selected_source_effective,
            event_sample_count,
            normalized_sample_count,
            source_updates,
            breath_samples,
            state_samples,
            state_value_samples,
            issues,
        )
    }

    pub fn close_report(&self) -> LiveTransportProcessorUpdate {
        let source_updates = self
            .sources
            .values()
            .map(LiveTransportSourceProcessor::update_summary)
            .collect();
        self.update_report(
            "pass",
            String::new(),
            "auto".to_string(),
            self.effective_selected_source("auto", &[]),
            0,
            0,
            source_updates,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn effective_selected_source(
        &self,
        preference: &str,
        current_samples: &[LiveBreathSample],
    ) -> String {
        if preference == "polar" || preference == "controller" {
            return preference.to_string();
        }
        if self.source_kind_has_output("polar") || current_samples.iter().any(is_polar_sample) {
            return "polar".to_string();
        }
        if self.source_kind_has_output("controller")
            || current_samples.iter().any(is_controller_sample)
        {
            return "controller".to_string();
        }
        "unknown".to_string()
    }

    fn source_kind_has_output(&self, kind: &str) -> bool {
        self.sources
            .values()
            .any(|source| source.source_kind == kind && source.estimate_count > 0)
    }

    fn update_report(
        &self,
        status: &str,
        input_stream_id: String,
        selected_source_preference: String,
        selected_source_effective: String,
        event_sample_count: usize,
        normalized_sample_count: usize,
        source_updates: Vec<LiveTransportSourceUpdate>,
        breath_samples: Vec<LiveBreathSample>,
        state_samples: Vec<LiveBreathStateSample>,
        state_value_samples: Vec<LiveBreathStateValueSample>,
        issues: Vec<String>,
    ) -> LiveTransportProcessorUpdate {
        let feedback_samples = breath_samples
            .iter()
            .map(|sample| LiveFeedbackSample {
                sequence_id: sample.sequence_id,
                stream_id: STREAM_BREATH_FEEDBACK_STATE.to_string(),
                source_breath_sequence_id: sample.sequence_id,
                source_id: sample.source_id.clone(),
                sample_time_unix_ns: seconds_to_unix_ns(sample.sample_time_s),
                volume01: sample.volume01,
                phase: sample.phase.clone(),
                quality: sample.quality.clone(),
            })
            .collect();
        LiveTransportProcessorUpdate {
            schema: "rusty.manifold.projected_motion_breath.live_transport_update.v1",
            status: if issues.iter().any(|issue| issue.contains("json_invalid")) {
                "fail".to_string()
            } else {
                status.to_string()
            },
            route_id: self.route_id.clone(),
            package_root: self.package_root.display().to_string(),
            input_stream_ids: self.input_stream_ids.clone(),
            normalized_stream_ids: self.normalized_stream_ids.clone(),
            output_stream_ids: self.output_stream_ids.clone(),
            input_stream_id,
            selected_source_preference,
            selected_source_effective,
            event_sample_count,
            normalized_sample_count,
            output_sample_count: breath_samples.len(),
            source_updates,
            breath_samples,
            state_samples,
            state_value_samples,
            feedback_samples,
            issues,
        }
    }
}

struct LiveTransportSourceProcessor {
    source_id: String,
    source_stream_id: String,
    source_payload_kind: String,
    source_kind: String,
    binding: SourceBinding,
    profile: ProfileDocument,
    state: LiveTransportSourceState,
    state_value_processor: BreathStateValueProcessor,
    next_sample_index: usize,
    estimate_count: usize,
}

impl LiveTransportSourceProcessor {
    fn new(
        source_fixture: LiveRouteSourceFixture,
        binding: SourceBinding,
        profile: ProfileDocument,
    ) -> Result<Self, String> {
        let source_kind = live_source_kind(
            &binding.selected_source_kind,
            &source_fixture.source_stream_id,
        );
        let state = match source_kind.as_str() {
            "polar" => LiveTransportSourceState::Acc(AccLiveEstimatorState::new(&profile)),
            "controller" => {
                LiveTransportSourceState::Controller(ControllerLiveEstimatorState::new(&profile))
            }
            _ => {
                return Err(format!(
                    "{}:issue.live_transport_source_kind_unsupported",
                    source_fixture.source_id
                ))
            }
        };
        let state_value_processor = BreathStateValueProcessor::new(state_value_config(&profile));
        Ok(Self {
            source_id: source_fixture.source_id,
            source_stream_id: source_fixture.source_stream_id,
            source_payload_kind: source_fixture.source_payload_kind,
            source_kind,
            binding,
            profile,
            state,
            state_value_processor,
            next_sample_index: 0,
            estimate_count: 0,
        })
    }

    fn transport_inputs(
        &self,
        event: &serde_json::Value,
        payload: &serde_json::Value,
    ) -> Result<Vec<AdapterNormalizationInput>, &'static str> {
        match self.source_stream_id.as_str() {
            EXTERNAL_STREAM_POLAR_ACC => transport_polar_acc_samples(event, payload),
            STREAM_OBJECT_POSE => {
                transport_object_pose_sample(event, payload).map(|sample| vec![sample])
            }
            _ => Err("issue.transport_event_stream_unsupported"),
        }
    }

    fn push_normalized_sample(
        &mut self,
        normalized: NormalizedAdapterSample,
        next_sequence_id: &mut u64,
    ) -> (Vec<LiveBreathSample>, Vec<String>) {
        let sample_index = self.next_sample_index;
        self.next_sample_index = self.next_sample_index.saturating_add(1);
        let result = match (&mut self.state, normalized) {
            (LiveTransportSourceState::Acc(state), NormalizedAdapterSample::Vector(sample)) => {
                state.push_sample(
                    &self.source_id,
                    &self.source_stream_id,
                    &self.binding.selected_output_stream_id,
                    &self.profile,
                    sample_index,
                    &sample,
                    next_sequence_id,
                )
            }
            (
                LiveTransportSourceState::Controller(state),
                NormalizedAdapterSample::Rigid(sample),
            ) => state.push_sample(
                &self.source_id,
                &self.source_stream_id,
                &self.binding.selected_output_stream_id,
                &self.profile,
                sample_index,
                &sample,
                next_sequence_id,
            ),
            _ => (
                Vec::new(),
                vec![format!(
                    "{}:sample.{sample_index}:issue.live_transport_normalized_kind_mismatch",
                    self.source_id
                )],
            ),
        };
        self.estimate_count = self.estimate_count.saturating_add(result.0.len());
        result
    }

    fn derive_state_outputs(
        &mut self,
        breath_samples: &[LiveBreathSample],
    ) -> (Vec<LiveBreathStateSample>, Vec<LiveBreathStateValueSample>) {
        derive_breath_state_outputs(
            &self.profile,
            &mut self.state_value_processor,
            breath_samples,
        )
    }

    fn update_summary(&self) -> LiveTransportSourceUpdate {
        let (status, calibration_sample_count, calibration_sample_target) = match &self.state {
            LiveTransportSourceState::Acc(state) => state.calibration_summary(),
            LiveTransportSourceState::Controller(state) => state.calibration_summary(),
        };
        LiveTransportSourceUpdate {
            source_id: self.source_id.clone(),
            source_stream_id: self.source_stream_id.clone(),
            source_kind: self.source_kind.clone(),
            calibration_status: status,
            calibration_sample_count,
            calibration_sample_target,
            estimate_count: self.estimate_count,
            output_sample_count: self.estimate_count,
        }
    }
}

enum LiveTransportSourceState {
    Acc(AccLiveEstimatorState),
    Controller(ControllerLiveEstimatorState),
}

#[derive(Clone, Copy, Debug)]
struct AccCalibrationModel {
    center: [f64; 3],
    axis: [f64; 3],
    bound_min: f64,
    bound_max: f64,
    xz_model: Option<AccXzModel>,
}

#[derive(Debug)]
struct AccLiveEstimatorState {
    accepted_target: usize,
    has_filtered: bool,
    filtered: [f64; 3],
    calibration_gate: DeadbandVec3,
    calibration_last_time: Option<f64>,
    output_last_time: Option<f64>,
    accepted_filtered: Vec<[f64; 3]>,
    model: Option<AccCalibrationModel>,
    has_projection_ema: bool,
    projection_ema: f64,
    has_xz_projection_ema: bool,
    xz_projection_ema: f64,
    previous_volume: Option<f64>,
}

impl AccLiveEstimatorState {
    fn new(profile: &ProfileDocument) -> Self {
        Self {
            accepted_target: profile.calibration.accepted_sample_count.max(16) as usize,
            has_filtered: false,
            filtered: [0.0, 0.0, 0.0],
            calibration_gate: DeadbandVec3::new(),
            calibration_last_time: None,
            output_last_time: None,
            accepted_filtered: Vec::new(),
            model: None,
            has_projection_ema: false,
            projection_ema: 0.0,
            has_xz_projection_ema: false,
            xz_projection_ema: 0.0,
            previous_volume: None,
        }
    }

    fn calibration_summary(&self) -> (String, usize, usize) {
        (
            if self.model.is_some() {
                "calibrated"
            } else {
                "calibrating"
            }
            .to_string(),
            self.accepted_filtered.len(),
            self.accepted_target,
        )
    }

    fn push_sample(
        &mut self,
        source_id: &str,
        source_stream_id: &str,
        normalized_stream_id: &str,
        profile: &ProfileDocument,
        sample_index: usize,
        sample: &VectorMotionSample,
        next_sequence_id: &mut u64,
    ) -> (Vec<LiveBreathSample>, Vec<String>) {
        if !vector_sample_ready(profile, sample) {
            return (Vec::new(), Vec::new());
        }
        self.filtered = if self.has_filtered {
            lerp3(
                self.filtered,
                sample.vector3,
                profile.smoothing.ema_alpha.clamp(0.01, 1.0),
            )
        } else {
            self.has_filtered = true;
            sample.vector3
        };

        if self.model.is_none() {
            if should_emit_analysis_time(
                &mut self.calibration_last_time,
                sample.sample_time_s,
                profile.smoothing.analysis_rate_hz,
            ) && self
                .calibration_gate
                .should_accept(self.filtered, profile.calibration.min_accepted_delta)
            {
                self.accepted_filtered.push(self.filtered);
                if self.accepted_filtered.len() >= self.accepted_target {
                    if let Some(model) = self.build_model(source_id, profile) {
                        self.model = Some(model);
                    } else {
                        return (
                            Vec::new(),
                            vec![format!(
                                "{source_id}:issue.live_transport_acc_calibration_invalid"
                            )],
                        );
                    }
                }
            }
            return (Vec::new(), Vec::new());
        }

        let Some(model) = self.model else {
            return (Vec::new(), Vec::new());
        };
        let centered = sub3(self.filtered, model.center);
        let projection = dot3(centered, model.axis);
        self.projection_ema = smooth_scalar_f64(
            self.has_projection_ema,
            self.projection_ema,
            projection,
            profile.smoothing.ema_alpha,
        );
        self.has_projection_ema = true;
        let volume3d = inverse_lerp_f64(model.bound_min, model.bound_max, self.projection_ema);
        let mut volume01 = volume3d;
        if let Some(xz_model) = model.xz_model {
            let xz_projection = centered[0] * xz_model.axis[0] + centered[2] * xz_model.axis[1];
            self.xz_projection_ema = smooth_scalar_f64(
                self.has_xz_projection_ema,
                self.xz_projection_ema,
                xz_projection,
                profile.smoothing.ema_alpha,
            );
            self.has_xz_projection_ema = true;
            volume01 = inverse_lerp_f64(
                xz_model.bound_min,
                xz_model.bound_max,
                self.xz_projection_ema,
            );
        }
        volume01 = volume01.clamp(0.0, 1.0);
        if !should_emit_analysis_time(
            &mut self.output_last_time,
            sample.sample_time_s,
            profile.smoothing.analysis_rate_hz,
        ) {
            return (Vec::new(), Vec::new());
        }
        let phase = classify_phase(
            volume01,
            self.previous_volume,
            profile.classifier.delta_threshold,
        );
        self.previous_volume = Some(volume01);
        let output = live_breath_sample(
            next_sequence_id,
            sample.source_id.clone(),
            source_stream_id,
            normalized_stream_id,
            sample_index,
            sample.sample_time_s,
            sample.host_time_s,
            projection,
            volume01,
            phase.as_str(),
            sample.quality01,
        );
        (vec![output], Vec::new())
    }

    fn build_model(
        &self,
        source_id: &str,
        profile: &ProfileDocument,
    ) -> Option<AccCalibrationModel> {
        if self.accepted_filtered.len() < self.accepted_target {
            return None;
        }
        let center = mean3(&self.accepted_filtered);
        let fallback_axis =
            normalized_axis(profile.projection.fixed_axis.unwrap_or([0.0, 1.0, 0.0]))
                .unwrap_or([0.0, 1.0, 0.0]);
        let axis = normalize3_or(
            principal_axis3(&self.accepted_filtered, center, fallback_axis)
                .unwrap_or(fallback_axis),
            fallback_axis,
        );
        let mut projection_scratch: Vec<f64> = self
            .accepted_filtered
            .iter()
            .map(|sample| dot3(sub3(*sample, center), axis))
            .collect();
        let (mut bound_min, mut bound_max) = quantile_bounds_linear(
            &mut projection_scratch,
            profile.calibration.lower_quantile,
            profile.calibration.upper_quantile,
        )?;
        let raw_travel = (bound_max - bound_min).max(0.0);
        if raw_travel < profile.calibration.min_span {
            return None;
        }
        apply_edge_ease_f64(
            &mut bound_min,
            &mut bound_max,
            profile.normalization.edge_ease,
        );
        enforce_span_bounds_f64(
            &mut bound_min,
            &mut bound_max,
            profile.calibration.min_span * 0.25,
            f64::INFINITY,
        );
        let _ = source_id;
        Some(AccCalibrationModel {
            center,
            axis,
            bound_min,
            bound_max,
            xz_model: build_xz_acc_model(&self.accepted_filtered, center, profile),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct ControllerCalibrationModel {
    origin: [f64; 3],
    center: [f64; 3],
    axis: [f64; 3],
    bound_min: f64,
    bound_max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControllerStateClassifierMode {
    ProjectedVolumeDelta,
    FixedControllerOrientation,
}

#[derive(Clone, Copy, Debug)]
struct FixedControllerStateEstimate {
    projection: f64,
    phase: &'static str,
    quality: &'static str,
    tracking01: f64,
}

#[derive(Debug)]
struct FixedControllerStateClassifier {
    orientation_axis: [f64; 3],
    last_position: Option<[f64; 3]>,
    last_orientation: [f64; 4],
    delta_accumulator: f64,
    delta_history: Vec<f64>,
}

impl FixedControllerStateClassifier {
    fn new(settings: &ProfileControllerStateClassifier) -> Self {
        Self {
            orientation_axis: normalized_axis(settings.orientation_axis)
                .unwrap_or([0.0, 0.0, -1.0]),
            last_position: None,
            last_orientation: [0.0, 0.0, 0.0, 1.0],
            delta_accumulator: 0.0,
            delta_history: Vec::new(),
        }
    }

    fn push_sample(
        &mut self,
        settings: &ProfileControllerStateClassifier,
        sample: &RigidMotionSample,
        orientation: [f64; 4],
    ) -> FixedControllerStateEstimate {
        let Some(last_position) = self.last_position else {
            self.last_position = Some(sample.position_m);
            self.last_orientation = orientation;
            return FixedControllerStateEstimate {
                projection: 0.0,
                phase: "pause",
                quality: "state_fixed_orientation",
                tracking01: sample.quality01,
            };
        };

        let rotation_delta = quat_angle_degrees(self.last_orientation, orientation);
        let axis_world = rotate_vec3_by_quat(
            self.orientation_axis,
            normalize_quat_or_identity(orientation),
        );
        let delta = dot3(sub3(sample.position_m, last_position), axis_world);
        self.delta_accumulator += delta;
        push_window(
            &mut self.delta_history,
            self.delta_accumulator,
            settings.long_window as usize,
        );
        let short_mean = mean_trailing_f64(&self.delta_history, settings.short_window as usize);
        let long_mean = mean_trailing_f64(&self.delta_history, self.delta_history.len());
        let ma_diff = short_mean - long_mean;
        self.last_position = Some(sample.position_m);
        self.last_orientation = orientation;

        if rotation_delta > settings.rotation_guard_degrees
            || ma_diff.abs() > settings.moving_average_guard
        {
            return FixedControllerStateEstimate {
                projection: ma_diff,
                phase: "bad_tracking",
                quality: "bad_tracking",
                tracking01: 0.0,
            };
        }

        let mut phase = if ma_diff > settings.inhale_threshold {
            "inhale"
        } else if ma_diff < settings.exhale_threshold {
            "exhale"
        } else {
            "pause"
        };
        if settings.invert_left_hand && controller_sample_is_left_hand(sample) {
            phase = match phase {
                "inhale" => "exhale",
                "exhale" => "inhale",
                other => other,
            };
        }

        FixedControllerStateEstimate {
            projection: ma_diff,
            phase,
            quality: "state_fixed_orientation",
            tracking01: sample.quality01,
        }
    }
}

#[derive(Debug)]
struct ControllerLiveEstimatorState {
    state_mode: ControllerStateClassifierMode,
    accepted_target: usize,
    origin: Option<[f64; 3]>,
    calibration_gate: DeadbandVec3,
    calibration_last_time: Option<f64>,
    output_last_time: Option<f64>,
    accepted_relative: Vec<[f64; 3]>,
    accepted_last_orientation: [f64; 4],
    model: Option<ControllerCalibrationModel>,
    last_position: Option<[f64; 3]>,
    last_orientation: [f64; 4],
    median_buffer: Vec<f64>,
    median_scratch: Vec<f64>,
    has_projection_ema: bool,
    projection_ema: f64,
    previous_volume: Option<f64>,
    fixed_state_classifier: FixedControllerStateClassifier,
}

impl ControllerLiveEstimatorState {
    fn new(profile: &ProfileDocument) -> Self {
        Self {
            state_mode: controller_state_classifier_mode(&profile.controller_state),
            accepted_target: profile.calibration.accepted_sample_count.max(8) as usize,
            origin: None,
            calibration_gate: DeadbandVec3::new(),
            calibration_last_time: None,
            output_last_time: None,
            accepted_relative: Vec::new(),
            accepted_last_orientation: [0.0, 0.0, 0.0, 1.0],
            model: None,
            last_position: None,
            last_orientation: [0.0, 0.0, 0.0, 1.0],
            median_buffer: Vec::new(),
            median_scratch: Vec::new(),
            has_projection_ema: false,
            projection_ema: 0.0,
            previous_volume: None,
            fixed_state_classifier: FixedControllerStateClassifier::new(&profile.controller_state),
        }
    }

    fn calibration_summary(&self) -> (String, usize, usize) {
        if self.state_mode == ControllerStateClassifierMode::FixedControllerOrientation {
            return ("state_fixed_orientation".to_string(), 0, 0);
        }
        (
            if self.model.is_some() {
                "calibrated"
            } else {
                "calibrating"
            }
            .to_string(),
            self.accepted_relative.len(),
            self.accepted_target,
        )
    }

    fn push_sample(
        &mut self,
        source_id: &str,
        source_stream_id: &str,
        normalized_stream_id: &str,
        profile: &ProfileDocument,
        sample_index: usize,
        sample: &RigidMotionSample,
        next_sequence_id: &mut u64,
    ) -> (Vec<LiveBreathSample>, Vec<String>) {
        const MAX_DEGREES_PER_SAMPLE: f64 = 2.0;
        const MAX_POSITION_JUMP_M: f64 = 0.008;

        if !rigid_sample_ready(profile, sample) {
            return (Vec::new(), Vec::new());
        }
        let origin = *self.origin.get_or_insert(sample.position_m);
        let orientation = sample.orientation_xyzw.unwrap_or(self.last_orientation);
        if self.state_mode == ControllerStateClassifierMode::FixedControllerOrientation {
            if !should_emit_analysis_time(
                &mut self.output_last_time,
                sample.sample_time_s,
                profile.smoothing.analysis_rate_hz,
            ) {
                return (Vec::new(), Vec::new());
            }
            let estimate = self.fixed_state_classifier.push_sample(
                &profile.controller_state,
                sample,
                orientation,
            );
            let output = live_breath_sample_with_quality(
                next_sequence_id,
                sample.source_id.clone(),
                source_stream_id,
                normalized_stream_id,
                sample_index,
                sample.sample_time_s,
                sample.host_time_s,
                estimate.projection,
                profile.controller_state.neutral_volume01,
                estimate.phase,
                estimate.tracking01,
                estimate.quality,
            );
            return (vec![output], Vec::new());
        }
        if self.model.is_none() {
            if should_emit_analysis_time(
                &mut self.calibration_last_time,
                sample.sample_time_s,
                profile.smoothing.analysis_rate_hz,
            ) {
                let relative = sub3(sample.position_m, origin);
                if self
                    .calibration_gate
                    .should_accept(relative, profile.calibration.min_accepted_delta)
                {
                    self.accepted_relative.push(relative);
                    self.accepted_last_orientation = orientation;
                    if self.accepted_relative.len() >= self.accepted_target {
                        if let Some(model) = self.build_model(origin, source_id, profile) {
                            self.model = Some(model);
                            self.last_position = Some(sample.position_m);
                            self.last_orientation = orientation;
                        } else {
                            return (
                                Vec::new(),
                                vec![format!(
                                    "{source_id}:issue.live_transport_controller_calibration_invalid"
                                )],
                            );
                        }
                    }
                }
            }
            return (Vec::new(), Vec::new());
        }

        let Some(model) = self.model else {
            return (Vec::new(), Vec::new());
        };
        if let Some(last_position) = self.last_position {
            let position_delta = length3(sub3(sample.position_m, last_position));
            let rotation_delta = quat_angle_degrees(self.last_orientation, orientation);
            self.last_position = Some(sample.position_m);
            self.last_orientation = orientation;
            if position_delta > MAX_POSITION_JUMP_M || rotation_delta > MAX_DEGREES_PER_SAMPLE {
                return (Vec::new(), Vec::new());
            }
        } else {
            self.last_position = Some(sample.position_m);
            self.last_orientation = orientation;
        }

        let relative = sub3(sample.position_m, model.origin);
        let projection = dot3(sub3(relative, model.center), model.axis);
        push_window(
            &mut self.median_buffer,
            projection,
            odd_window(profile.smoothing.median_window as usize).max(3),
        );
        self.median_scratch.clear();
        self.median_scratch.extend_from_slice(&self.median_buffer);
        let smoothed_projection = median_f64(&mut self.median_scratch);
        self.projection_ema = smooth_scalar_f64(
            self.has_projection_ema,
            self.projection_ema,
            smoothed_projection,
            profile.smoothing.ema_alpha.clamp(0.02, 1.0),
        );
        self.has_projection_ema = true;
        let mut volume01 = inverse_lerp_f64(model.bound_min, model.bound_max, self.projection_ema);
        if (profile.normalization.progress_gamma - 1.0).abs() > EPSILON {
            volume01 = volume01.powf(profile.normalization.progress_gamma.max(EPSILON));
        }
        volume01 = volume01.clamp(0.0, 1.0);
        if !should_emit_analysis_time(
            &mut self.output_last_time,
            sample.sample_time_s,
            profile.smoothing.analysis_rate_hz,
        ) {
            return (Vec::new(), Vec::new());
        }
        let phase = classify_phase(
            volume01,
            self.previous_volume,
            profile.classifier.delta_threshold,
        );
        self.previous_volume = Some(volume01);
        let output = live_breath_sample(
            next_sequence_id,
            sample.source_id.clone(),
            source_stream_id,
            normalized_stream_id,
            sample_index,
            sample.sample_time_s,
            sample.host_time_s,
            projection,
            volume01,
            phase.as_str(),
            sample.quality01,
        );
        (vec![output], Vec::new())
    }

    fn build_model(
        &self,
        origin: [f64; 3],
        source_id: &str,
        profile: &ProfileDocument,
    ) -> Option<ControllerCalibrationModel> {
        if self.accepted_relative.len() < self.accepted_target {
            return None;
        }
        let center = mean3(&self.accepted_relative);
        let reference_axis = quat_forward_neg_z(self.accepted_last_orientation);
        let axis = normalize3_or(
            principal_axis3(&self.accepted_relative, center, reference_axis)
                .unwrap_or(reference_axis),
            reference_axis,
        );
        let mut projection_scratch: Vec<f64> = self
            .accepted_relative
            .iter()
            .map(|sample| dot3(sub3(*sample, center), axis))
            .collect();
        let (mut bound_min, mut bound_max) = quantile_bounds_linear(
            &mut projection_scratch,
            profile.calibration.lower_quantile,
            profile.calibration.upper_quantile,
        )?;
        let raw_travel = (bound_max - bound_min).max(0.0);
        if raw_travel < profile.calibration.min_span {
            return None;
        }
        let soft = (raw_travel * profile.normalization.soft_margin.clamp(0.0, 1.0))
            .clamp(0.0, raw_travel * 0.49);
        bound_min += soft;
        bound_max -= soft;
        apply_edge_ease_f64(
            &mut bound_min,
            &mut bound_max,
            profile.normalization.edge_ease,
        );
        enforce_span_bounds_f64(
            &mut bound_min,
            &mut bound_max,
            profile.calibration.min_span * 0.25,
            f64::INFINITY,
        );
        let _ = source_id;
        Some(ControllerCalibrationModel {
            origin,
            center,
            axis,
            bound_min,
            bound_max,
        })
    }
}

fn live_breath_sample(
    next_sequence_id: &mut u64,
    source_id: String,
    source_stream_id: &str,
    normalized_stream_id: &str,
    sample_index: usize,
    sample_time_s: f64,
    host_time_s: f64,
    projection: f64,
    volume01: f64,
    phase: &str,
    tracking01: f64,
) -> LiveBreathSample {
    live_breath_sample_with_quality(
        next_sequence_id,
        source_id,
        source_stream_id,
        normalized_stream_id,
        sample_index,
        sample_time_s,
        host_time_s,
        projection,
        volume01,
        phase,
        tracking01,
        "stable",
    )
}

fn live_breath_sample_with_quality(
    next_sequence_id: &mut u64,
    source_id: String,
    source_stream_id: &str,
    normalized_stream_id: &str,
    sample_index: usize,
    sample_time_s: f64,
    host_time_s: f64,
    projection: f64,
    volume01: f64,
    phase: &str,
    tracking01: f64,
    quality: &str,
) -> LiveBreathSample {
    let sequence_id = *next_sequence_id;
    *next_sequence_id = (*next_sequence_id).saturating_add(1);
    LiveBreathSample {
        sequence_id,
        source_id,
        input_stream_id: source_stream_id.to_string(),
        normalized_stream_id: normalized_stream_id.to_string(),
        output_stream_id: STREAM_BREATH_VOLUME.to_string(),
        sample_index,
        sample_time_s,
        host_time_s,
        projection,
        volume01,
        phase: phase.to_string(),
        tracking01: tracking01.clamp(0.0, 1.0),
        quality: quality.to_string(),
    }
}

fn derive_breath_state_outputs(
    profile: &ProfileDocument,
    processor: &mut BreathStateValueProcessor,
    breath_samples: &[LiveBreathSample],
) -> (Vec<LiveBreathStateSample>, Vec<LiveBreathStateValueSample>) {
    let mut state_samples = Vec::new();
    let mut state_value_samples = Vec::new();
    for sample in breath_samples {
        let state = normalize_breath_state(&sample.phase).to_string();
        let state_sample = LiveBreathStateSample {
            sequence_id: sample.sequence_id,
            stream_id: STREAM_BREATH_STATE.to_string(),
            source_breath_sequence_id: sample.sequence_id,
            source_id: sample.source_id.clone(),
            sample_time_unix_ns: seconds_to_unix_ns(sample.sample_time_s),
            state: state.clone(),
            state01: breath_state01(&state),
            phase: state.clone(),
            tracking01: sample.tracking01,
            quality: sample.quality.clone(),
        };
        if profile.state_value.enabled {
            let step = processor.push(&state, sample.sample_time_s);
            state_value_samples.push(LiveBreathStateValueSample {
                sequence_id: sample.sequence_id,
                stream_id: STREAM_BREATH_STATE_VALUE.to_string(),
                source_breath_sequence_id: sample.sequence_id,
                source_state_sequence_id: state_sample.sequence_id,
                source_id: sample.source_id.clone(),
                sample_time_unix_ns: state_sample.sample_time_unix_ns,
                state: state.clone(),
                state01: step.state01,
                target01: step.target01,
                value01: step.value01,
                delta_seconds: step.delta_seconds,
                stale_gap: step.stale_gap,
                tracking01: sample.tracking01,
                quality: sample.quality.clone(),
            });
        }
        state_samples.push(state_sample);
    }
    (state_samples, state_value_samples)
}

fn state_value_config(profile: &ProfileDocument) -> BreathStateValueConfig {
    BreathStateValueConfig {
        min_value01: profile.state_value.min_value01,
        max_value01: profile.state_value.max_value01,
        initial_value01: profile.state_value.initial_value01,
        fallback_value01: profile.state_value.fallback_value01,
        inhale_seconds_min_to_max: profile.state_value.inhale_seconds_min_to_max,
        exhale_seconds_max_to_min: profile.state_value.exhale_seconds_max_to_min,
        smoothing_s: profile.state_value.smoothing_s,
        stale_timeout_s: profile.state_value.stale_timeout_s,
        hold_bad_tracking: profile.state_value.hold_bad_tracking,
    }
}

fn controller_state_classifier_mode(
    settings: &ProfileControllerStateClassifier,
) -> ControllerStateClassifierMode {
    match settings.mode.as_str() {
        "fixed_controller_orientation" => ControllerStateClassifierMode::FixedControllerOrientation,
        _ => ControllerStateClassifierMode::ProjectedVolumeDelta,
    }
}

fn controller_sample_is_left_hand(sample: &RigidMotionSample) -> bool {
    let source_id = sample.source_id.to_ascii_lowercase();
    let frame_id = sample.frame_id.to_ascii_lowercase();
    source_id.contains("left") || frame_id.contains("left")
}

fn mean_trailing_f64(values: &[f64], window: usize) -> f64 {
    let len = values.len();
    if len == 0 {
        return 0.0;
    }
    let start = len.saturating_sub(window.max(1));
    let count = len - start;
    values[start..].iter().sum::<f64>() / count as f64
}

fn normalize_live_selected_source_preference(value: &str) -> String {
    match value {
        "polar" | "controller" => value.to_string(),
        _ => "auto".to_string(),
    }
}

fn live_source_kind(selected_source_kind: &str, source_stream_id: &str) -> String {
    let text = format!("{selected_source_kind} {source_stream_id}").to_lowercase();
    if text.contains("polar") || text.contains("wearable") || text.contains("bio:polar") {
        "polar".to_string()
    } else if text.contains("controller") || text.contains("object_pose") {
        "controller".to_string()
    } else {
        "unknown".to_string()
    }
}

fn is_polar_sample(sample: &LiveBreathSample) -> bool {
    live_source_kind(
        "",
        &format!("{} {}", sample.source_id, sample.input_stream_id),
    ) == "polar"
}

fn is_controller_sample(sample: &LiveBreathSample) -> bool {
    live_source_kind(
        "",
        &format!("{} {}", sample.source_id, sample.input_stream_id),
    ) == "controller"
}

fn run_live_route_with_source_samples(
    package_root: &Path,
    fixture: LiveRouteFixture,
    source_samples: BTreeMap<String, Vec<AdapterNormalizationInput>>,
    event_issues: Vec<String>,
    mode: LiveRouteExecutionMode,
) -> Result<LiveRouteReport, ValidationError> {
    let mut issues = validate_live_route_fixture(&fixture);
    issues.extend(event_issues);
    let mut source_routes = Vec::new();
    let mut breath_samples = Vec::new();
    let mut state_samples = Vec::new();
    let mut state_value_samples = Vec::new();
    let mut next_sequence_id = 1_u64;

    for source_fixture in &fixture.sources {
        let route_start_len = breath_samples.len();
        let samples = source_samples
            .get(&source_fixture.source_stream_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if samples.is_empty() {
            issues.push(format!(
                "{}:issue.source_samples_missing",
                source_fixture.source_id
            ));
        }
        let binding_path = package_root.join(&source_fixture.binding_path);
        let binding = match read_source_binding(&binding_path) {
            Ok(binding) => binding,
            Err(error) => {
                issues.push(format!(
                    "{}:issue.source_binding_read_failed:{error}",
                    source_fixture.source_id
                ));
                continue;
            }
        };
        if let Some(issue) = validate_source_binding(package_root, &binding) {
            issues.push(format!("{}:{issue}", binding.binding_id));
        }
        if !source_payload_kind_matches(
            &binding.selected_source_kind,
            &source_fixture.source_payload_kind,
        ) {
            issues.push(format!(
                "{}:issue.adapter_payload_kind_mismatch",
                source_fixture.source_id
            ));
        }
        if binding.source_stream_id != source_fixture.source_stream_id {
            issues.push(format!(
                "{}:issue.source_stream_binding_mismatch",
                source_fixture.source_id
            ));
        }
        if binding.selected_output_stream_id != source_fixture.expected_normalized_stream_id {
            issues.push(format!(
                "{}:issue.normalized_stream_binding_mismatch",
                source_fixture.source_id
            ));
        }

        if let Err(issue) = InputKind::parse(&binding.selected_input_kind) {
            issues.push(format!("{}:{}", source_fixture.source_id, issue.code()));
            continue;
        }
        let profile = match read_profile(&package_root.join(&binding.profile_path)) {
            Ok(profile) => profile,
            Err(error) => {
                issues.push(format!(
                    "{}:issue.profile_read_failed:{error}",
                    source_fixture.source_id
                ));
                continue;
            }
        };
        if !validate_profile_document(&profile).is_empty() {
            issues.push(format!(
                "{}:issue.profile_invalid",
                source_fixture.source_id
            ));
            continue;
        }
        let mut normalized_samples = Vec::new();
        for (sample_index, input) in samples.iter().enumerate() {
            let normalized = match normalize_adapter_sample(
                &binding,
                &source_fixture.source_payload_kind,
                input,
            ) {
                Ok(sample) => sample,
                Err(issue) => {
                    issues.push(format!(
                        "{}:sample.{sample_index}:{issue}",
                        source_fixture.source_id
                    ));
                    continue;
                }
            };
            normalized_samples.push(normalized);
        }
        let normalized_sample_count = normalized_samples.len();
        let (mut source_breath_samples, source_issues) = estimate_live_route_source_breath_samples(
            source_fixture,
            &binding,
            &profile,
            &normalized_samples,
            mode,
            &mut next_sequence_id,
        );
        let mut state_value_processor =
            BreathStateValueProcessor::new(state_value_config(&profile));
        let (mut source_state_samples, mut source_state_value_samples) =
            derive_breath_state_outputs(
                &profile,
                &mut state_value_processor,
                &source_breath_samples,
            );
        breath_samples.append(&mut source_breath_samples);
        state_samples.append(&mut source_state_samples);
        state_value_samples.append(&mut source_state_value_samples);
        issues.extend(source_issues);
        let estimate_count = breath_samples.len().saturating_sub(route_start_len);
        if mode.validate_fixture_expected
            && estimate_count < source_fixture.expected_min_estimate_count
        {
            issues.push(format!(
                "{}:issue.expected_estimate_count",
                source_fixture.source_id
            ));
        }
        source_routes.push(LiveSourceRouteReport {
            source_id: source_fixture.source_id.clone(),
            source_stream_id: source_fixture.source_stream_id.clone(),
            normalized_stream_id: binding.selected_output_stream_id,
            binding_id: binding.binding_id,
            selected_adapter_id: binding.selected_adapter_id,
            selected_source_kind: binding.selected_source_kind,
            source_payload_kind: source_fixture.source_payload_kind.clone(),
            sample_count: samples.len(),
            normalized_sample_count,
            estimate_count,
        });
    }

    let feedback_samples: Vec<LiveFeedbackSample> = breath_samples
        .iter()
        .map(|sample| LiveFeedbackSample {
            sequence_id: sample.sequence_id,
            stream_id: STREAM_BREATH_FEEDBACK_STATE.to_string(),
            source_breath_sequence_id: sample.sequence_id,
            source_id: sample.source_id.clone(),
            sample_time_unix_ns: seconds_to_unix_ns(sample.sample_time_s),
            volume01: sample.volume01,
            phase: sample.phase.clone(),
            quality: sample.quality.clone(),
        })
        .collect();
    let receiver_receipts: Vec<ReceiverBreathReceiptPlan> = breath_samples
        .iter()
        .map(|sample| ReceiverBreathReceiptPlan {
            command: fixture.receiver.receipt_command.clone(),
            schema: fixture.receiver.receipt_schema.clone(),
            received_stream: fixture.receiver.subscription_stream_id.clone(),
            received_sequence_id: sample.sequence_id,
            receiver_id: fixture.receiver.receiver_id.clone(),
            acknowledged: true,
        })
        .collect();

    if mode.validate_fixture_expected {
        issues.extend(validate_live_route_expected(
            &fixture,
            &source_routes,
            &breath_samples,
            &state_samples,
            &state_value_samples,
            &feedback_samples,
            &receiver_receipts,
        ));
    } else {
        issues.extend(validate_live_transport_route_observed(
            &source_routes,
            &breath_samples,
            &state_samples,
            &state_value_samples,
            &feedback_samples,
        ));
    }
    issues = dedup_issue_codes(issues);
    let status = if issues.is_empty() { "pass" } else { "fail" }.to_string();

    Ok(LiveRouteReport {
        schema: LIVE_ROUTE_REPORT_SCHEMA,
        package_root: package_root.display().to_string(),
        status,
        route_id: fixture.route_id,
        input_stream_ids: fixture.input_stream_ids,
        normalized_stream_ids: fixture.normalized_stream_ids,
        output_stream_ids: fixture.output_stream_ids,
        processor_core_executed: true,
        runtime_execution_performed: mode.runtime_execution_performed,
        external_transport_used: mode.external_transport_used,
        live_sensor_used: mode.live_sensor_used,
        headset_execution_performed: mode.headset_execution_performed,
        plan_only: mode.plan_only,
        source_routes,
        breath_samples,
        state_samples,
        state_value_samples,
        feedback_samples,
        receiver_subscription: ReceiverBreathSubscriptionPlan {
            command: fixture.receiver.subscription_command,
            stream: fixture.receiver.subscription_stream_id,
            receiver_id: fixture.receiver.receiver_id,
        },
        receiver_receipts,
        issues,
    })
}

fn estimate_live_route_source_breath_samples(
    source_fixture: &LiveRouteSourceFixture,
    binding: &SourceBinding,
    profile: &ProfileDocument,
    normalized_samples: &[NormalizedAdapterSample],
    mode: LiveRouteExecutionMode,
    next_sequence_id: &mut u64,
) -> (Vec<LiveBreathSample>, Vec<String>) {
    if mode.external_transport_used && !mode.plan_only {
        match binding.selected_source_kind.as_str() {
            "xr_controller_pose" | "object_pose" => {
                return estimate_controller_transport_breath_samples(
                    source_fixture,
                    binding,
                    profile,
                    normalized_samples,
                    next_sequence_id,
                );
            }
            "wearable_acceleration" | "vector_motion" => {
                return estimate_acc_transport_breath_samples(
                    source_fixture,
                    binding,
                    profile,
                    normalized_samples,
                    next_sequence_id,
                );
            }
            _ => {}
        }
    }
    estimate_fixture_projection_breath_samples(
        source_fixture,
        binding,
        profile,
        normalized_samples,
        next_sequence_id,
    )
}

fn estimate_fixture_projection_breath_samples(
    source_fixture: &LiveRouteSourceFixture,
    binding: &SourceBinding,
    profile: &ProfileDocument,
    normalized_samples: &[NormalizedAdapterSample],
    next_sequence_id: &mut u64,
) -> (Vec<LiveBreathSample>, Vec<String>) {
    let mut samples = Vec::new();
    let mut issues = Vec::new();
    let input_kind = match InputKind::parse(&binding.selected_input_kind) {
        Ok(input_kind) => input_kind,
        Err(issue) => {
            issues.push(format!("{}:{}", source_fixture.source_id, issue.code()));
            return (samples, issues);
        }
    };
    let motion_profile = motion_profile_from_document(input_kind, profile);
    let tracker = match ProjectedMotionBreathTracker::calibrated(
        motion_profile,
        &source_fixture.calibration.projection_values,
    ) {
        Ok(tracker) => tracker,
        Err(issue) => {
            issues.push(format!("{}:{}", source_fixture.source_id, issue.code()));
            return (samples, issues);
        }
    };
    let Some(axis) = normalized_axis(source_fixture.projection.axis) else {
        issues.push(format!(
            "{}:issue.projection_axis_invalid",
            source_fixture.source_id
        ));
        return (samples, issues);
    };

    let mut previous_projection = None;
    for (sample_index, normalized) in normalized_samples.iter().enumerate() {
        let (source_id, sample_time_s, host_time_s, projection, quality01) = match normalized {
            NormalizedAdapterSample::Rigid(sample) => {
                let quality01 =
                    if profile.quality.require_tracked && (!sample.connected || !sample.tracked) {
                        0.0
                    } else {
                        sample.quality01
                    };
                (
                    sample.source_id.clone(),
                    sample.sample_time_s,
                    sample.host_time_s,
                    dot3(sample.position_m, axis),
                    quality01,
                )
            }
            NormalizedAdapterSample::Vector(sample) => (
                sample.source_id.clone(),
                sample.sample_time_s,
                sample.host_time_s,
                dot3(sample.vector3, axis),
                sample.quality01,
            ),
        };
        let sample_age_s = (host_time_s - sample_time_s).max(0.0);
        match tracker.estimate_from_projection(
            projection,
            previous_projection,
            quality01,
            sample_age_s,
        ) {
            Ok(estimate) => {
                previous_projection = Some(projection);
                samples.push(LiveBreathSample {
                    sequence_id: *next_sequence_id,
                    source_id,
                    input_stream_id: source_fixture.source_stream_id.clone(),
                    normalized_stream_id: binding.selected_output_stream_id.clone(),
                    output_stream_id: STREAM_BREATH_VOLUME.to_string(),
                    sample_index,
                    sample_time_s,
                    host_time_s,
                    projection,
                    volume01: estimate.volume01,
                    phase: estimate.phase.as_str().to_string(),
                    tracking01: estimate.tracking01,
                    quality: estimate.quality,
                });
                *next_sequence_id = next_sequence_id.saturating_add(1);
            }
            Err(issue) => issues.push(format!(
                "{}:sample.{sample_index}:{}",
                source_fixture.source_id,
                issue.code()
            )),
        }
    }
    (samples, issues)
}

fn estimate_controller_transport_breath_samples(
    source_fixture: &LiveRouteSourceFixture,
    binding: &SourceBinding,
    profile: &ProfileDocument,
    normalized_samples: &[NormalizedAdapterSample],
    next_sequence_id: &mut u64,
) -> (Vec<LiveBreathSample>, Vec<String>) {
    if controller_state_classifier_mode(&profile.controller_state)
        == ControllerStateClassifierMode::FixedControllerOrientation
    {
        return estimate_controller_fixed_state_breath_samples(
            source_fixture,
            binding,
            profile,
            normalized_samples,
            next_sequence_id,
        );
    }

    const MAX_DEGREES_PER_SAMPLE: f64 = 2.0;
    const MAX_POSITION_JUMP_M: f64 = 0.008;

    let rigid_samples: Vec<(usize, &RigidMotionSample)> = normalized_samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| match sample {
            NormalizedAdapterSample::Rigid(sample) => Some((index, sample)),
            NormalizedAdapterSample::Vector(_) => None,
        })
        .collect();
    let mut output = Vec::new();
    let mut issues = Vec::new();
    let Some((_, first_sample)) = rigid_samples.first().copied() else {
        issues.push(format!(
            "{}:issue.live_transport_controller_samples_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    };

    let origin = first_sample.position_m;
    let mut analysis_last_time = None;
    let mut accepted_gate = DeadbandVec3::new();
    let accepted_target = profile.calibration.accepted_sample_count.max(8) as usize;
    let mut accepted_relative = Vec::with_capacity(accepted_target);
    let mut accepted_last_sample_index = None;
    let mut accepted_last_orientation = first_sample
        .orientation_xyzw
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    for (sample_index, sample) in &rigid_samples {
        if !rigid_sample_ready(profile, sample) {
            continue;
        }
        if !should_emit_analysis_time(
            &mut analysis_last_time,
            sample.sample_time_s,
            profile.smoothing.analysis_rate_hz,
        ) {
            continue;
        }
        let relative = sub3(sample.position_m, origin);
        if accepted_gate.should_accept(relative, profile.calibration.min_accepted_delta) {
            accepted_relative.push(relative);
            accepted_last_sample_index = Some(*sample_index);
            accepted_last_orientation =
                sample.orientation_xyzw.unwrap_or(accepted_last_orientation);
            if accepted_relative.len() >= accepted_target {
                break;
            }
        }
    }

    let Some(calibration_last_sample_index) = accepted_last_sample_index else {
        issues.push(format!(
            "{}:issue.live_transport_controller_calibration_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    };
    if accepted_relative.len() < accepted_target {
        issues.push(format!(
            "{}:issue.live_transport_controller_calibration_samples_low",
            source_fixture.source_id
        ));
        return (output, issues);
    }

    let center = mean3(&accepted_relative);
    let reference_axis = quat_forward_neg_z(accepted_last_orientation);
    let axis = normalize3_or(
        principal_axis3(&accepted_relative, center, reference_axis).unwrap_or(reference_axis),
        reference_axis,
    );
    let mut projection_scratch: Vec<f64> = accepted_relative
        .iter()
        .map(|sample| dot3(sub3(*sample, center), axis))
        .collect();
    let Some((mut bound_min, mut bound_max)) = quantile_bounds_linear(
        &mut projection_scratch,
        profile.calibration.lower_quantile,
        profile.calibration.upper_quantile,
    ) else {
        issues.push(format!(
            "{}:issue.live_transport_controller_bounds_invalid",
            source_fixture.source_id
        ));
        return (output, issues);
    };
    let raw_travel = (bound_max - bound_min).max(0.0);
    if raw_travel < profile.calibration.min_span {
        issues.push(format!(
            "{}:issue.live_transport_controller_travel_low",
            source_fixture.source_id
        ));
        return (output, issues);
    }
    let soft = (raw_travel * profile.normalization.soft_margin.clamp(0.0, 1.0))
        .clamp(0.0, raw_travel * 0.49);
    bound_min += soft;
    bound_max -= soft;
    apply_edge_ease_f64(
        &mut bound_min,
        &mut bound_max,
        profile.normalization.edge_ease,
    );
    enforce_span_bounds_f64(
        &mut bound_min,
        &mut bound_max,
        profile.calibration.min_span * 0.25,
        f64::INFINITY,
    );

    let mut last_position = first_sample.position_m;
    let mut last_orientation = first_sample
        .orientation_xyzw
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    for (_, sample) in rigid_samples
        .iter()
        .filter(|(sample_index, _)| *sample_index <= calibration_last_sample_index)
    {
        last_position = sample.position_m;
        last_orientation = sample.orientation_xyzw.unwrap_or(last_orientation);
    }

    let mut median_buffer = Vec::new();
    let mut median_scratch = Vec::new();
    let mut has_projection_ema = false;
    let mut projection_ema = 0.0;
    let mut previous_volume = None;
    for (sample_index, sample) in rigid_samples
        .iter()
        .filter(|(sample_index, _)| *sample_index > calibration_last_sample_index)
    {
        if !rigid_sample_ready(profile, sample) {
            continue;
        }
        let orientation = sample.orientation_xyzw.unwrap_or(last_orientation);
        let position_delta = length3(sub3(sample.position_m, last_position));
        let rotation_delta = quat_angle_degrees(last_orientation, orientation);
        last_position = sample.position_m;
        last_orientation = orientation;
        if position_delta > MAX_POSITION_JUMP_M || rotation_delta > MAX_DEGREES_PER_SAMPLE {
            continue;
        }

        let relative = sub3(sample.position_m, origin);
        let projection = dot3(sub3(relative, center), axis);
        push_window(
            &mut median_buffer,
            projection,
            odd_window(profile.smoothing.median_window as usize).max(3),
        );
        median_scratch.clear();
        median_scratch.extend_from_slice(&median_buffer);
        let smoothed_projection = median_f64(&mut median_scratch);
        if has_projection_ema {
            projection_ema = lerp_f64(
                projection_ema,
                smoothed_projection,
                profile.smoothing.ema_alpha.clamp(0.02, 1.0),
            );
        } else {
            projection_ema = smoothed_projection;
            has_projection_ema = true;
        }
        let mut volume01 = inverse_lerp_f64(bound_min, bound_max, projection_ema);
        if (profile.normalization.progress_gamma - 1.0).abs() > EPSILON {
            volume01 = volume01.powf(profile.normalization.progress_gamma.max(EPSILON));
        }
        volume01 = volume01.clamp(0.0, 1.0);
        let phase = classify_phase(
            volume01,
            previous_volume,
            profile.classifier.delta_threshold,
        );
        previous_volume = Some(volume01);
        output.push(LiveBreathSample {
            sequence_id: *next_sequence_id,
            source_id: sample.source_id.clone(),
            input_stream_id: source_fixture.source_stream_id.clone(),
            normalized_stream_id: binding.selected_output_stream_id.clone(),
            output_stream_id: STREAM_BREATH_VOLUME.to_string(),
            sample_index: *sample_index,
            sample_time_s: sample.sample_time_s,
            host_time_s: sample.host_time_s,
            projection,
            volume01,
            phase: phase.as_str().to_string(),
            tracking01: sample.quality01.clamp(0.0, 1.0),
            quality: "stable".to_string(),
        });
        *next_sequence_id = (*next_sequence_id).saturating_add(1);
    }

    if output.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_controller_estimates_missing_after_calibration",
            source_fixture.source_id
        ));
    }
    (output, issues)
}

fn estimate_controller_fixed_state_breath_samples(
    source_fixture: &LiveRouteSourceFixture,
    binding: &SourceBinding,
    profile: &ProfileDocument,
    normalized_samples: &[NormalizedAdapterSample],
    next_sequence_id: &mut u64,
) -> (Vec<LiveBreathSample>, Vec<String>) {
    let rigid_samples: Vec<(usize, &RigidMotionSample)> = normalized_samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| match sample {
            NormalizedAdapterSample::Rigid(sample) => Some((index, sample)),
            NormalizedAdapterSample::Vector(_) => None,
        })
        .collect();
    let mut output = Vec::new();
    let mut issues = Vec::new();
    if rigid_samples.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_controller_samples_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    }

    let mut analysis_last_time = None;
    let mut classifier = FixedControllerStateClassifier::new(&profile.controller_state);
    for (sample_index, sample) in rigid_samples {
        if !rigid_sample_ready(profile, sample) {
            continue;
        }
        if !should_emit_analysis_time(
            &mut analysis_last_time,
            sample.sample_time_s,
            profile.smoothing.analysis_rate_hz,
        ) {
            continue;
        }
        let orientation = sample.orientation_xyzw.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let estimate = classifier.push_sample(&profile.controller_state, sample, orientation);
        output.push(LiveBreathSample {
            sequence_id: *next_sequence_id,
            source_id: sample.source_id.clone(),
            input_stream_id: source_fixture.source_stream_id.clone(),
            normalized_stream_id: binding.selected_output_stream_id.clone(),
            output_stream_id: STREAM_BREATH_VOLUME.to_string(),
            sample_index,
            sample_time_s: sample.sample_time_s,
            host_time_s: sample.host_time_s,
            projection: estimate.projection,
            volume01: profile.controller_state.neutral_volume01,
            phase: estimate.phase.to_string(),
            tracking01: estimate.tracking01.clamp(0.0, 1.0),
            quality: estimate.quality.to_string(),
        });
        *next_sequence_id = (*next_sequence_id).saturating_add(1);
    }

    if output.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_controller_fixed_state_estimates_missing",
            source_fixture.source_id
        ));
    }
    (output, issues)
}

fn estimate_acc_transport_breath_samples(
    source_fixture: &LiveRouteSourceFixture,
    binding: &SourceBinding,
    profile: &ProfileDocument,
    normalized_samples: &[NormalizedAdapterSample],
    next_sequence_id: &mut u64,
) -> (Vec<LiveBreathSample>, Vec<String>) {
    let vector_samples: Vec<(usize, &VectorMotionSample)> = normalized_samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| match sample {
            NormalizedAdapterSample::Vector(sample) => Some((index, sample)),
            NormalizedAdapterSample::Rigid(_) => None,
        })
        .collect();
    let mut output = Vec::new();
    let mut issues = Vec::new();
    if vector_samples.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_acc_samples_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    }

    let mut filtered_samples = Vec::with_capacity(vector_samples.len());
    let mut has_filtered = false;
    let mut filtered = [0.0, 0.0, 0.0];
    for (sample_index, sample) in vector_samples {
        if !vector_sample_ready(profile, sample) {
            continue;
        }
        if has_filtered {
            filtered = lerp3(
                filtered,
                sample.vector3,
                profile.smoothing.ema_alpha.clamp(0.01, 1.0),
            );
        } else {
            filtered = sample.vector3;
            has_filtered = true;
        }
        filtered_samples.push((sample_index, sample, filtered));
    }

    if filtered_samples.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_acc_usable_samples_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    }

    let mut analysis_last_time = None;
    let mut calibration_gate = DeadbandVec3::new();
    let accepted_target = profile.calibration.accepted_sample_count.max(16) as usize;
    let mut accepted_filtered = Vec::with_capacity(accepted_target);
    let mut accepted_last_sample_index = None;
    for (sample_index, sample, filtered) in &filtered_samples {
        if !should_emit_analysis_time(
            &mut analysis_last_time,
            sample.sample_time_s,
            profile.smoothing.analysis_rate_hz,
        ) {
            continue;
        }
        if calibration_gate.should_accept(*filtered, profile.calibration.min_accepted_delta) {
            accepted_filtered.push(*filtered);
            accepted_last_sample_index = Some(*sample_index);
            if accepted_filtered.len() >= accepted_target {
                break;
            }
        }
    }

    let Some(calibration_last_sample_index) = accepted_last_sample_index else {
        issues.push(format!(
            "{}:issue.live_transport_acc_calibration_missing",
            source_fixture.source_id
        ));
        return (output, issues);
    };
    if accepted_filtered.len() < accepted_target {
        issues.push(format!(
            "{}:issue.live_transport_acc_calibration_samples_low",
            source_fixture.source_id
        ));
        return (output, issues);
    }

    let center = mean3(&accepted_filtered);
    let fallback_axis = normalized_axis(profile.projection.fixed_axis.unwrap_or([0.0, 1.0, 0.0]))
        .unwrap_or([0.0, 1.0, 0.0]);
    let axis = normalize3_or(
        principal_axis3(&accepted_filtered, center, fallback_axis).unwrap_or(fallback_axis),
        fallback_axis,
    );
    let mut projection_scratch: Vec<f64> = accepted_filtered
        .iter()
        .map(|sample| dot3(sub3(*sample, center), axis))
        .collect();
    let Some((mut bound_min, mut bound_max)) = quantile_bounds_linear(
        &mut projection_scratch,
        profile.calibration.lower_quantile,
        profile.calibration.upper_quantile,
    ) else {
        issues.push(format!(
            "{}:issue.live_transport_acc_bounds_invalid",
            source_fixture.source_id
        ));
        return (output, issues);
    };
    let raw_travel = (bound_max - bound_min).max(0.0);
    if raw_travel < profile.calibration.min_span {
        issues.push(format!(
            "{}:issue.live_transport_acc_travel_low",
            source_fixture.source_id
        ));
        return (output, issues);
    }
    apply_edge_ease_f64(
        &mut bound_min,
        &mut bound_max,
        profile.normalization.edge_ease,
    );
    enforce_span_bounds_f64(
        &mut bound_min,
        &mut bound_max,
        profile.calibration.min_span * 0.25,
        f64::INFINITY,
    );

    let xz_model = build_xz_acc_model(&accepted_filtered, center, profile);

    let mut has_projection_ema = false;
    let mut projection_ema = 0.0;
    let mut has_xz_projection_ema = false;
    let mut xz_projection_ema = 0.0;
    let mut previous_volume = None;
    for (sample_index, sample, filtered) in filtered_samples
        .iter()
        .filter(|(sample_index, _, _)| *sample_index > calibration_last_sample_index)
    {
        let centered = sub3(*filtered, center);
        let projection = dot3(centered, axis);
        projection_ema = smooth_scalar_f64(
            has_projection_ema,
            projection_ema,
            projection,
            profile.smoothing.ema_alpha,
        );
        has_projection_ema = true;
        let volume3d = inverse_lerp_f64(bound_min, bound_max, projection_ema);

        let mut volume01 = volume3d;
        if let Some(model) = xz_model {
            let xz_projection = centered[0] * model.axis[0] + centered[2] * model.axis[1];
            xz_projection_ema = smooth_scalar_f64(
                has_xz_projection_ema,
                xz_projection_ema,
                xz_projection,
                profile.smoothing.ema_alpha,
            );
            has_xz_projection_ema = true;
            volume01 = inverse_lerp_f64(model.bound_min, model.bound_max, xz_projection_ema);
        }
        volume01 = volume01.clamp(0.0, 1.0);
        let phase = classify_phase(
            volume01,
            previous_volume,
            profile.classifier.delta_threshold,
        );
        previous_volume = Some(volume01);
        output.push(LiveBreathSample {
            sequence_id: *next_sequence_id,
            source_id: sample.source_id.clone(),
            input_stream_id: source_fixture.source_stream_id.clone(),
            normalized_stream_id: binding.selected_output_stream_id.clone(),
            output_stream_id: STREAM_BREATH_VOLUME.to_string(),
            sample_index: *sample_index,
            sample_time_s: sample.sample_time_s,
            host_time_s: sample.host_time_s,
            projection,
            volume01,
            phase: phase.as_str().to_string(),
            tracking01: sample.quality01.clamp(0.0, 1.0),
            quality: "stable".to_string(),
        });
        *next_sequence_id = (*next_sequence_id).saturating_add(1);
    }

    if output.is_empty() {
        issues.push(format!(
            "{}:issue.live_transport_acc_estimates_missing_after_calibration",
            source_fixture.source_id
        ));
    }
    (output, issues)
}

fn build_xz_acc_model(
    calibration_samples: &[[f64; 3]],
    center: [f64; 3],
    profile: &ProfileDocument,
) -> Option<AccXzModel> {
    if calibration_samples.len() < 3 {
        return None;
    }
    let axis = principal_axis_xz(calibration_samples, center)?;
    let mut scratch: Vec<f64> = calibration_samples
        .iter()
        .map(|sample| {
            let d = sub3(*sample, center);
            d[0] * axis[0] + d[2] * axis[1]
        })
        .collect();
    let (mut bound_min, mut bound_max) = quantile_bounds_linear(
        &mut scratch,
        profile.calibration.lower_quantile,
        profile.calibration.upper_quantile,
    )?;
    apply_edge_ease_f64(
        &mut bound_min,
        &mut bound_max,
        profile.normalization.edge_ease,
    );
    enforce_span_bounds_f64(
        &mut bound_min,
        &mut bound_max,
        profile.calibration.min_span * 0.5,
        f64::INFINITY,
    );
    Some(AccXzModel {
        axis,
        bound_min,
        bound_max,
    })
}

fn rigid_sample_ready(profile: &ProfileDocument, sample: &RigidMotionSample) -> bool {
    if profile.quality.require_tracked && (!sample.connected || !sample.tracked) {
        return false;
    }
    sample.quality01 >= profile.quality.min_quality01
        && finite_array3(sample.position_m)
        && sample.orientation_xyzw.is_some_and(finite_array4)
        && sample.sample_time_s.is_finite()
        && sample.host_time_s.is_finite()
}

fn vector_sample_ready(profile: &ProfileDocument, sample: &VectorMotionSample) -> bool {
    sample.quality01 >= profile.quality.min_quality01
        && finite_array3(sample.vector3)
        && sample.sample_time_s.is_finite()
        && sample.host_time_s.is_finite()
}

fn should_emit_analysis_time(
    last_time: &mut Option<f64>,
    time_s: f64,
    analysis_rate_hz: f64,
) -> bool {
    if !time_s.is_finite() {
        return false;
    }
    let interval_s = 1.0 / analysis_rate_hz.max(0.1);
    match *last_time {
        None => {
            *last_time = Some(time_s);
            true
        }
        Some(last) if time_s - last + EPSILON >= interval_s => {
            *last_time = Some(time_s);
            true
        }
        Some(_) => false,
    }
}

fn read_live_transport_event_samples(
    path: &Path,
    fixture: &LiveRouteFixture,
) -> Result<
    (
        BTreeMap<String, Vec<AdapterNormalizationInput>>,
        Vec<String>,
    ),
    ValidationError,
> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut samples_by_stream: BTreeMap<String, Vec<AdapterNormalizationInput>> = BTreeMap::new();
    let mut issues = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let event: serde_json::Value =
            serde_json::from_str(line).map_err(|source| ValidationError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        let payload = event.get("payload").unwrap_or(&serde_json::Value::Null);
        let Some(stream) = event
            .get("stream")
            .and_then(serde_json::Value::as_str)
            .or_else(|| payload.get("stream_id").and_then(serde_json::Value::as_str))
        else {
            continue;
        };
        if !fixture
            .sources
            .iter()
            .any(|source| source.source_stream_id == stream)
        {
            continue;
        }
        let converted = match stream {
            EXTERNAL_STREAM_POLAR_ACC => transport_polar_acc_samples(&event, payload),
            STREAM_OBJECT_POSE => {
                transport_object_pose_sample(&event, payload).map(|sample| vec![sample])
            }
            _ => Err("issue.transport_event_stream_unsupported"),
        };
        match converted {
            Ok(converted) => samples_by_stream
                .entry(stream.to_string())
                .or_default()
                .extend(converted),
            Err(issue) => issues.push(format!(
                "transport_event.line_{}:{stream}:{issue}",
                line_index + 1
            )),
        }
    }
    Ok((samples_by_stream, issues))
}

fn transport_polar_acc_samples(
    event: &serde_json::Value,
    payload: &serde_json::Value,
) -> Result<Vec<AdapterNormalizationInput>, &'static str> {
    let Some(samples_mg) = payload
        .get("samples_mg")
        .and_then(serde_json::Value::as_array)
    else {
        return Err("issue.transport_event_source_samples_missing");
    };
    let base_sample_time_s = ns_to_seconds(
        first_i64(payload, &["sample_time_unix_ns", "source_time_unix_ns"])
            .or_else(|| first_i64(event, &["transport_time_unix_ns"]))
            .unwrap_or(0),
    );
    let host_time_s = ns_to_seconds(
        first_i64(
            payload,
            &["transport_receive_time_unix_ns", "client_send_time_unix_ns"],
        )
        .or_else(|| first_i64(event, &["transport_time_unix_ns"]))
        .unwrap_or(0),
    );
    let mut converted = Vec::new();
    for (index, sample) in samples_mg.iter().enumerate() {
        let Some(vector_mg) = json_array3(sample) else {
            return Err("issue.transport_event_source_sample_invalid");
        };
        converted.push(AdapterNormalizationInput {
            source_id: "source.polar_h10.acc.live_transport".to_string(),
            sample_time_s: base_sample_time_s + (index as f64 * 0.005),
            host_time_s,
            frame_id: "frame.polar_h10.body".to_string(),
            position_m: None,
            orientation_xyzw: None,
            connected: None,
            tracked: None,
            tracking01: None,
            vector3: Some([
                vector_mg[0] * 0.001,
                vector_mg[1] * 0.001,
                vector_mg[2] * 0.001,
            ]),
            units: Some("g".to_string()),
            quality01: Some(unit_interval_or(
                payload,
                &["quality01", "tracking01"],
                0.96,
            )),
            channel_values: BTreeMap::new(),
            channel_map: None,
        });
    }
    Ok(converted)
}

fn transport_object_pose_sample(
    event: &serde_json::Value,
    payload: &serde_json::Value,
) -> Result<AdapterNormalizationInput, &'static str> {
    let Some(position_m) = object_pose_position(payload) else {
        return Err("issue.transport_event_pose_position_missing");
    };
    let Some(orientation_xyzw) = object_pose_orientation(payload) else {
        return Err("issue.transport_event_pose_orientation_missing");
    };
    let sample_time_s = ns_to_seconds(
        first_i64(payload, &["sample_time_unix_ns", "source_time_unix_ns"])
            .or_else(|| first_i64(event, &["transport_time_unix_ns"]))
            .unwrap_or(0),
    );
    let host_time_s = ns_to_seconds(
        first_i64(
            payload,
            &["transport_receive_time_unix_ns", "client_send_time_unix_ns"],
        )
        .or_else(|| first_i64(event, &["transport_time_unix_ns"]))
        .unwrap_or(0),
    );
    Ok(AdapterNormalizationInput {
        source_id: "source.downstream.controller_pose.live_transport".to_string(),
        sample_time_s,
        host_time_s,
        frame_id: payload
            .get("reference_space")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("frame.headset.stage")
            .to_string(),
        position_m: Some(position_m),
        orientation_xyzw: Some(orientation_xyzw),
        connected: Some(
            payload
                .get("connected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        ),
        tracked: Some(
            payload
                .get("tracked")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        ),
        tracking01: Some(unit_interval_or(payload, &["tracking01", "quality01"], 1.0)),
        vector3: None,
        units: None,
        quality01: None,
        channel_values: BTreeMap::new(),
        channel_map: None,
    })
}

fn object_pose_position(payload: &serde_json::Value) -> Option<[f64; 3]> {
    payload
        .get("position_m")
        .and_then(json_vec3)
        .or_else(|| payload.get("position").and_then(json_vec3))
        .or_else(|| {
            payload
                .get("pose")
                .and_then(|pose| pose.get("position_m"))
                .and_then(json_vec3)
        })
}

fn object_pose_orientation(payload: &serde_json::Value) -> Option<[f64; 4]> {
    payload
        .get("orientation_xyzw")
        .and_then(json_array4)
        .or_else(|| {
            payload
                .get("pose")
                .and_then(|pose| pose.get("orientation_xyzw"))
                .and_then(json_array4)
        })
}

fn json_vec3(value: &serde_json::Value) -> Option<[f64; 3]> {
    json_array3(value).or_else(|| json_object3(value))
}

fn json_array3(value: &serde_json::Value) -> Option<[f64; 3]> {
    let array = value.as_array()?;
    if array.len() != 3 {
        return None;
    }
    Some([array[0].as_f64()?, array[1].as_f64()?, array[2].as_f64()?])
}

fn json_object3(value: &serde_json::Value) -> Option<[f64; 3]> {
    let object = value.as_object()?;
    let x = object
        .get("x")
        .or_else(|| object.get("x_m"))
        .and_then(serde_json::Value::as_f64)?;
    let y = object
        .get("y")
        .or_else(|| object.get("y_m"))
        .and_then(serde_json::Value::as_f64)?;
    let z = object
        .get("z")
        .or_else(|| object.get("z_m"))
        .and_then(serde_json::Value::as_f64)?;
    Some([x, y, z])
}

fn json_array4(value: &serde_json::Value) -> Option<[f64; 4]> {
    let array = value.as_array()?;
    if array.len() != 4 {
        return None;
    }
    Some([
        array[0].as_f64()?,
        array[1].as_f64()?,
        array[2].as_f64()?,
        array[3].as_f64()?,
    ])
}

fn first_i64(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(json_i64))
}

fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn ns_to_seconds(ns: i64) -> f64 {
    ns as f64 / 1_000_000_000.0
}

fn unit_interval_or(payload: &serde_json::Value, keys: &[&str], default: f64) -> f64 {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(serde_json::Value::as_f64))
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(default)
}

fn seconds_to_unix_ns(seconds: f64) -> i64 {
    if !seconds.is_finite() {
        return 0;
    }
    (seconds.max(0.0) * 1_000_000_000.0)
        .round()
        .clamp(0.0, i64::MAX as f64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_state_settings() -> ProfileControllerStateClassifier {
        ProfileControllerStateClassifier {
            mode: "fixed_controller_orientation".to_string(),
            orientation_axis: [0.0, 1.0, 0.0],
            inhale_threshold: 0.0002,
            exhale_threshold: -0.0002,
            rotation_guard_degrees: 30.0,
            moving_average_guard: 0.25,
            short_window: 2,
            long_window: 4,
            invert_left_hand: false,
            neutral_volume01: 0.5,
        }
    }

    fn rigid_sample(y: f64, orientation_xyzw: [f64; 4]) -> RigidMotionSample {
        RigidMotionSample {
            source_id: "source.downstream.controller_pose.right".to_string(),
            sample_time_s: 1.0,
            host_time_s: 1.01,
            frame_id: "frame.headset.stage.right".to_string(),
            position_m: [0.0, y, 0.0],
            orientation_xyzw: Some(orientation_xyzw),
            connected: true,
            tracked: true,
            quality01: 0.98,
        }
    }

    #[test]
    fn fixed_controller_state_classifier_detects_inhale_and_exhale() {
        let settings = fixed_state_settings();
        let mut classifier = FixedControllerStateClassifier::new(&settings);
        let identity = [0.0, 0.0, 0.0, 1.0];

        let phases: Vec<&str> = [0.0, 0.002, 0.004, 0.006, 0.004, 0.002]
            .into_iter()
            .map(|y| {
                classifier
                    .push_sample(&settings, &rigid_sample(y, identity), identity)
                    .phase
            })
            .collect();

        assert!(phases.contains(&"inhale"));
        assert_eq!(phases.last(), Some(&"exhale"));
    }

    #[test]
    fn fixed_controller_state_classifier_marks_rotation_guard_as_bad_tracking() {
        let mut settings = fixed_state_settings();
        settings.rotation_guard_degrees = 0.5;
        let mut classifier = FixedControllerStateClassifier::new(&settings);
        let identity = [0.0, 0.0, 0.0, 1.0];
        let flipped = [0.0, 1.0, 0.0, 0.0];

        assert_eq!(
            classifier
                .push_sample(&settings, &rigid_sample(0.0, identity), identity)
                .phase,
            "pause"
        );
        let estimate = classifier.push_sample(&settings, &rigid_sample(0.002, flipped), flipped);
        assert_eq!(estimate.phase, "bad_tracking");
        assert_eq!(estimate.quality, "bad_tracking");
    }
}
