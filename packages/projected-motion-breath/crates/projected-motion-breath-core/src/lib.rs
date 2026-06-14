use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

mod documents;
mod math;
mod validation;

use documents::*;
use math::*;
use validation::{
    dedup_issue_codes, source_payload_kind_matches, validate_controller_preflight_expected,
    validate_controller_preflight_fixture, validate_live_route_expected,
    validate_live_route_fixture, validate_live_transport_route_observed, validate_profile_document,
    validate_source_binding,
};
pub use validation::{validate_package_goldens, CoreValidationReport};

pub const MODULE_PROJECTED_MOTION_BREATH: &str = "module.breath.projected_motion";
pub const STREAM_OBJECT_POSE: &str = "stream.motion.object_pose";
pub const STREAM_VECTOR3: &str = "stream.motion.vector3";
pub const STREAM_BREATH_VOLUME: &str = "stream.breath.volume";
pub const STREAM_BREATH_VOLUME_SELECTED: &str = "stream.breath.volume.selected";
pub const STREAM_BREATH_VOLUME_POLAR: &str = "stream.breath.volume.polar";
pub const STREAM_BREATH_VOLUME_CONTROLLER: &str = "stream.breath.volume.controller";
pub const STREAM_BREATH_SELECTION_STATE: &str = "stream.breath.selection_state";
pub const STREAM_BREATH_FEEDBACK_STATE: &str = "stream.breath.feedback_state";
pub const EXTERNAL_STREAM_POLAR_ACC: &str = "bio:polar_acc";
pub const PACKAGE_PROJECTED_MOTION_BREATH: &str = "package.projected_motion_breath";
pub const GOLDEN_PROJECTED_MOTION: &str =
    "golden.projected_motion_breath.pose_and_vector_projection";
pub const RECEIVER_COMMAND_SUBSCRIBE: &str = "subscribe";
pub const RECEIVER_COMMAND_BREATH_FEEDBACK_RECEIVED: &str = "breath_feedback.received";
pub const BREATH_FEEDBACK_RECEIPT_SCHEMA: &str = "rusty.manifold.breath.feedback_receipt.v1";
pub const COMMAND_BREATH_CONFIGURE: &str = "command.breath.configure";
pub const COMMAND_BREATH_SET_PROFILE: &str = "command.breath.set_profile";
pub const COMMAND_BREATH_BEGIN_CALIBRATION: &str = "command.breath.begin_calibration";
pub const COMMAND_BREATH_RESET_CALIBRATION: &str = "command.breath.reset_calibration";
pub const COMMAND_BREATH_STATUS: &str = "command.breath.status";
pub const SOURCE_BINDING_SCHEMA: &str = "rusty.manifold.projected_motion_breath.source_binding.v1";
pub const SOURCE_ADAPTER_DESCRIPTOR_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.source_adapter_descriptors.v1";
pub const ADAPTER_NORMALIZATION_CASE_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.adapter_normalization_case.v1";
pub const CONTROLLER_PREFLIGHT_FIXTURE_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.controller_preflight_fixture.v1";
pub const CONTROLLER_PREFLIGHT_REPORT_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.controller_preflight_report.v1";
pub const LIVE_ROUTE_FIXTURE_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.live_route_fixture.v1";
pub const LIVE_ROUTE_REPORT_SCHEMA: &str =
    "rusty.manifold.projected_motion_breath.live_route_report.v1";

const DEFAULT_STALE_TIMEOUT_S: f64 = 0.5;
const DEFAULT_DELTA_THRESHOLD: f64 = 0.000_001;
const DEFAULT_MIN_QUALITY01: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Pose,
    Vector3,
}

impl InputKind {
    fn parse(value: &str) -> Result<Self, BreathIssue> {
        match value {
            "pose" => Ok(Self::Pose),
            "vector3" => Ok(Self::Vector3),
            _ => Err(BreathIssue::ProjectionUnsupported),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RigidMotionSample {
    pub source_id: String,
    pub sample_time_s: f64,
    pub host_time_s: f64,
    pub frame_id: String,
    pub position_m: [f64; 3],
    pub orientation_xyzw: Option<[f64; 4]>,
    pub connected: bool,
    pub tracked: bool,
    pub quality01: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMotionSample {
    pub source_id: String,
    pub sample_time_s: f64,
    pub host_time_s: f64,
    pub frame_id: String,
    pub vector3: [f64; 3],
    pub units: String,
    pub quality01: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionBreathProfile {
    pub input_kind: InputKind,
    pub lower_quantile: f64,
    pub upper_quantile: f64,
    pub stale_timeout_s: f64,
    pub delta_threshold: f64,
    pub min_quality01: f64,
}

impl MotionBreathProfile {
    pub fn fixture_default(input_kind: InputKind) -> Self {
        Self {
            input_kind,
            lower_quantile: 0.05,
            upper_quantile: 0.95,
            stale_timeout_s: DEFAULT_STALE_TIMEOUT_S,
            delta_threshold: DEFAULT_DELTA_THRESHOLD,
            min_quality01: DEFAULT_MIN_QUALITY01,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreathPhase {
    Inhale,
    Exhale,
    Pause,
}

impl BreathPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inhale => "inhale",
            Self::Exhale => "exhale",
            Self::Pause => "pause",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreathIssue {
    CalibrationInvalid,
    MotionQualityLow,
    ProfileInvalid,
    ProjectionUnsupported,
    SourceStale,
}

impl BreathIssue {
    pub fn code(self) -> &'static str {
        match self {
            Self::CalibrationInvalid => "issue.calibration_invalid",
            Self::MotionQualityLow => "issue.motion_quality_low",
            Self::ProfileInvalid => "issue.profile_invalid",
            Self::ProjectionUnsupported => "issue.projection_unsupported",
            Self::SourceStale => "issue.source_stale",
        }
    }
}

impl fmt::Display for BreathIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for BreathIssue {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreathEstimate {
    pub input_kind: InputKind,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub volume01: f64,
    pub phase: BreathPhase,
    pub tracking01: f64,
    pub quality: String,
}

#[derive(Debug, Clone)]
pub struct ProjectedMotionBreathTracker {
    profile: MotionBreathProfile,
    lower_bound: f64,
    upper_bound: f64,
}

impl ProjectedMotionBreathTracker {
    pub fn calibrated(
        profile: MotionBreathProfile,
        calibration_projection: &[f64],
    ) -> Result<Self, BreathIssue> {
        let lower_bound = nearest_quantile(calibration_projection, profile.lower_quantile)?;
        let upper_bound = nearest_quantile(calibration_projection, profile.upper_quantile)?;
        if upper_bound - lower_bound <= EPSILON {
            return Err(BreathIssue::CalibrationInvalid);
        }
        Ok(Self {
            profile,
            lower_bound,
            upper_bound,
        })
    }

    pub fn estimate_from_projection(
        &self,
        live_projection: f64,
        previous_projection: Option<f64>,
        quality01: f64,
        sample_age_s: f64,
    ) -> Result<BreathEstimate, BreathIssue> {
        if sample_age_s > self.profile.stale_timeout_s {
            return Err(BreathIssue::SourceStale);
        }
        if quality01 < self.profile.min_quality01 {
            return Err(BreathIssue::MotionQualityLow);
        }

        let span = self.upper_bound - self.lower_bound;
        if span <= EPSILON {
            return Err(BreathIssue::CalibrationInvalid);
        }
        let volume01 = ((live_projection - self.lower_bound) / span).clamp(0.0, 1.0);
        let phase = classify_phase(
            live_projection,
            previous_projection,
            self.profile.delta_threshold,
        );

        Ok(BreathEstimate {
            input_kind: self.profile.input_kind,
            lower_bound: self.lower_bound,
            upper_bound: self.upper_bound,
            volume01,
            phase,
            tracking01: quality01.clamp(0.0, 1.0),
            quality: "stable".to_string(),
        })
    }
}

fn classify_phase(
    live_projection: f64,
    previous_projection: Option<f64>,
    delta_threshold: f64,
) -> BreathPhase {
    let Some(previous_projection) = previous_projection else {
        return BreathPhase::Pause;
    };
    let delta = live_projection - previous_projection;
    if delta > delta_threshold {
        BreathPhase::Inhale
    } else if delta < -delta_threshold {
        BreathPhase::Exhale
    } else {
        BreathPhase::Pause
    }
}

fn nearest_quantile(values: &[f64], quantile: f64) -> Result<f64, BreathIssue> {
    if values.is_empty() || !quantile.is_finite() {
        return Err(BreathIssue::CalibrationInvalid);
    }
    let mut sorted = values.to_vec();
    if sorted.iter().any(|value| !value.is_finite()) {
        return Err(BreathIssue::CalibrationInvalid);
    }
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let clamped = quantile.clamp(0.0, 1.0);
    let index = (clamped * ((sorted.len() - 1) as f64)).round() as usize;
    Ok(sorted[index])
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerPreflightReport {
    pub schema: &'static str,
    pub package_root: String,
    pub status: String,
    pub preflight_id: String,
    pub provider_id: String,
    pub provider_kind: String,
    pub binding_id: String,
    pub selected_adapter_id: String,
    pub selected_source_kind: String,
    pub source_payload_kind: String,
    pub input_stream_id: String,
    pub output_stream_id: String,
    pub source_id: String,
    pub frame_id: String,
    pub sample_count: usize,
    pub normalized_sample_count: usize,
    pub estimate_count: usize,
    pub processor_core_executed: bool,
    pub runtime_execution_performed: bool,
    pub provider_boundary_exercised: bool,
    pub controller_provider_route_ready: bool,
    pub headset_controller_shape_used: bool,
    pub physical_controller_input_used: bool,
    pub controller_input_used: bool,
    pub manual_controller_trial_required: bool,
    pub estimates: Vec<ControllerPreflightEstimate>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerPreflightEstimate {
    pub sample_index: usize,
    pub source_id: String,
    pub sample_time_s: f64,
    pub host_time_s: f64,
    pub frame_id: String,
    pub projection: f64,
    pub volume01: f64,
    pub phase: String,
    pub tracking01: f64,
    pub quality: String,
}

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

#[derive(Debug)]
pub enum ValidationError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
        }
    }
}

impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

pub fn run_controller_preflight(
    package_root: impl AsRef<Path>,
) -> Result<ControllerPreflightReport, ValidationError> {
    let package_root = package_root.as_ref();
    let fixture_path = package_root
        .join("fixtures")
        .join("valid")
        .join("controller-preflight-headset-controller.json");
    let fixture = read_controller_preflight_fixture(&fixture_path)?;
    let mut issues = validate_controller_preflight_fixture(&fixture);
    let binding_path = package_root.join(&fixture.binding_path);
    let binding = match read_source_binding(&binding_path) {
        Ok(binding) => Some(binding),
        Err(error) => {
            issues.push(format!("issue.controller_binding_read_failed:{error}"));
            None
        }
    };

    let mut estimates = Vec::new();
    let mut normalized_sample_count = 0usize;
    let mut binding_id = String::new();
    let mut selected_adapter_id = String::new();
    let mut selected_source_kind = String::new();
    let mut input_stream_id = String::new();
    let mut source_id = String::new();
    let mut frame_id = String::new();

    if let Some(binding) = binding.as_ref() {
        binding_id = binding.binding_id.clone();
        selected_adapter_id = binding.selected_adapter_id.clone();
        selected_source_kind = binding.selected_source_kind.clone();
        input_stream_id = binding.source_stream_id.clone();
        if let Some(issue) = validate_source_binding(package_root, binding) {
            issues.push(format!("{}:{issue}", binding.binding_id));
        }
        if !source_payload_kind_matches(&binding.selected_source_kind, &fixture.source_payload_kind)
        {
            issues.push("issue.controller_source_payload_kind_mismatch".to_string());
        }
        let profile = read_profile(&package_root.join(&binding.profile_path));
        match profile {
            Ok(profile) => {
                if !validate_profile_document(&profile).is_empty() {
                    issues.push("issue.controller_profile_invalid".to_string());
                } else {
                    let motion_profile = motion_profile_from_document(InputKind::Pose, &profile);
                    match ProjectedMotionBreathTracker::calibrated(
                        motion_profile,
                        &fixture.calibration.projection_values,
                    ) {
                        Ok(tracker) => {
                            let axis = normalized_axis(fixture.projection.axis);
                            if let Some(axis) = axis {
                                let mut previous_projection = None;
                                for (index, sample) in fixture.samples.iter().enumerate() {
                                    let normalized = normalize_adapter_sample(
                                        binding,
                                        &fixture.source_payload_kind,
                                        sample,
                                    );
                                    let sample = match normalized {
                                        Ok(NormalizedAdapterSample::Rigid(sample)) => sample,
                                        Ok(NormalizedAdapterSample::Vector(_)) => {
                                            issues.push(format!(
                                                "sample.{index}:issue.controller_expected_rigid_motion"
                                            ));
                                            continue;
                                        }
                                        Err(issue) => {
                                            issues.push(format!("sample.{index}:{issue}"));
                                            continue;
                                        }
                                    };
                                    normalized_sample_count += 1;
                                    source_id = sample.source_id.clone();
                                    frame_id = sample.frame_id.clone();
                                    let projection = dot3(sample.position_m, axis);
                                    let sample_age_s =
                                        (sample.host_time_s - sample.sample_time_s).max(0.0);
                                    let quality01 = if profile.quality.require_tracked
                                        && (!sample.connected || !sample.tracked)
                                    {
                                        0.0
                                    } else {
                                        sample.quality01
                                    };
                                    match tracker.estimate_from_projection(
                                        projection,
                                        previous_projection,
                                        quality01,
                                        sample_age_s,
                                    ) {
                                        Ok(estimate) => {
                                            previous_projection = Some(projection);
                                            estimates.push(ControllerPreflightEstimate {
                                                sample_index: index,
                                                source_id: sample.source_id,
                                                sample_time_s: sample.sample_time_s,
                                                host_time_s: sample.host_time_s,
                                                frame_id: sample.frame_id,
                                                projection,
                                                volume01: estimate.volume01,
                                                phase: estimate.phase.as_str().to_string(),
                                                tracking01: estimate.tracking01,
                                                quality: estimate.quality,
                                            });
                                        }
                                        Err(issue) => {
                                            issues.push(format!("sample.{index}:{}", issue.code()))
                                        }
                                    }
                                }
                            } else {
                                issues.push("issue.controller_projection_axis_invalid".to_string());
                            }
                        }
                        Err(issue) => issues.push(format!("calibration:{}", issue.code())),
                    }
                }
            }
            Err(error) => issues.push(format!("issue.controller_profile_read_failed:{error}")),
        }
    }

    issues.extend(validate_controller_preflight_expected(
        &fixture,
        normalized_sample_count,
        &estimates,
    ));
    issues = dedup_issue_codes(issues);
    let status = if issues.is_empty() { "pass" } else { "fail" }.to_string();
    let route_ready = status == "pass"
        && normalized_sample_count >= fixture.expected.min_sample_count
        && estimates.len() >= fixture.expected.min_estimate_count;

    Ok(ControllerPreflightReport {
        schema: CONTROLLER_PREFLIGHT_REPORT_SCHEMA,
        package_root: package_root.display().to_string(),
        status,
        preflight_id: fixture.preflight_id,
        provider_id: fixture.provider.provider_id,
        provider_kind: fixture.provider.provider_kind.clone(),
        binding_id,
        selected_adapter_id,
        selected_source_kind,
        source_payload_kind: fixture.source_payload_kind,
        input_stream_id,
        output_stream_id: fixture.provider.output_stream_id,
        source_id,
        frame_id,
        sample_count: fixture.samples.len(),
        normalized_sample_count,
        estimate_count: estimates.len(),
        processor_core_executed: true,
        runtime_execution_performed: true,
        provider_boundary_exercised: normalized_sample_count > 0,
        controller_provider_route_ready: route_ready,
        headset_controller_shape_used: fixture.provider.provider_kind == "headset_controller_pose",
        physical_controller_input_used: fixture.provider.physical_controller_input_used,
        controller_input_used: fixture.provider.physical_controller_input_used,
        manual_controller_trial_required: fixture.provider.manual_controller_trial_required,
        estimates,
        issues,
    })
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
                                breath_samples.append(&mut produced);
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
        Ok(Self {
            source_id: source_fixture.source_id,
            source_stream_id: source_fixture.source_stream_id,
            source_payload_kind: source_fixture.source_payload_kind,
            source_kind,
            binding,
            profile,
            state,
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

#[derive(Debug)]
struct ControllerLiveEstimatorState {
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
}

impl ControllerLiveEstimatorState {
    fn new(profile: &ProfileDocument) -> Self {
        Self {
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
        quality: "stable".to_string(),
    }
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
        breath_samples.append(&mut source_breath_samples);
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
            &feedback_samples,
            &receiver_receipts,
        ));
    } else {
        issues.extend(validate_live_transport_route_observed(
            &source_routes,
            &breath_samples,
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

fn motion_profile_from_document(
    input_kind: InputKind,
    profile: &ProfileDocument,
) -> MotionBreathProfile {
    let mut motion_profile = MotionBreathProfile::fixture_default(input_kind);
    motion_profile.lower_quantile = profile.calibration.lower_quantile;
    motion_profile.upper_quantile = profile.calibration.upper_quantile;
    motion_profile.stale_timeout_s = profile.classifier.stale_timeout_s;
    motion_profile.delta_threshold = profile.classifier.delta_threshold;
    motion_profile.min_quality01 = profile.quality.min_quality01;
    motion_profile
}

fn normalize_adapter_sample(
    binding: &SourceBinding,
    source_payload_kind: &str,
    input: &AdapterNormalizationInput,
) -> Result<NormalizedAdapterSample, &'static str> {
    if input.source_id.is_empty()
        || input.frame_id.is_empty()
        || !input.sample_time_s.is_finite()
        || !input.host_time_s.is_finite()
    {
        return Err("issue.adapter_payload_invalid");
    }
    match (binding.selected_input_kind.as_str(), source_payload_kind) {
        ("pose", "object_pose") => normalize_object_pose_sample(input),
        ("vector3", "vector_motion") => normalize_vector_motion_sample(input),
        ("vector3", "external_patch_channels") => normalize_external_patch_sample(input),
        _ => Err("issue.adapter_payload_kind_mismatch"),
    }
}

fn normalize_object_pose_sample(
    input: &AdapterNormalizationInput,
) -> Result<NormalizedAdapterSample, &'static str> {
    let Some(position_m) = input.position_m else {
        return Err("issue.adapter_payload_invalid");
    };
    if !finite_array3(position_m) {
        return Err("issue.adapter_payload_invalid");
    }
    let Some(orientation_xyzw) = input.orientation_xyzw else {
        return Err("issue.adapter_payload_invalid");
    };
    if !finite_array4(orientation_xyzw) {
        return Err("issue.adapter_payload_invalid");
    }
    let Some(connected) = input.connected else {
        return Err("issue.adapter_payload_invalid");
    };
    let Some(tracked) = input.tracked else {
        return Err("issue.adapter_payload_invalid");
    };
    let Some(tracking01) = input.tracking01 else {
        return Err("issue.adapter_payload_invalid");
    };
    if !unit_interval(tracking01) {
        return Err("issue.adapter_payload_invalid");
    }
    Ok(NormalizedAdapterSample::Rigid(RigidMotionSample {
        source_id: input.source_id.clone(),
        sample_time_s: input.sample_time_s,
        host_time_s: input.host_time_s,
        frame_id: input.frame_id.clone(),
        position_m,
        orientation_xyzw: Some(orientation_xyzw),
        connected,
        tracked,
        quality01: tracking01,
    }))
}

fn normalize_vector_motion_sample(
    input: &AdapterNormalizationInput,
) -> Result<NormalizedAdapterSample, &'static str> {
    let Some(vector3) = input.vector3 else {
        return Err("issue.adapter_payload_invalid");
    };
    normalize_vector_sample(input, vector3)
}

fn normalize_external_patch_sample(
    input: &AdapterNormalizationInput,
) -> Result<NormalizedAdapterSample, &'static str> {
    let Some(channel_map) = input.channel_map.as_ref() else {
        return Err("issue.adapter_payload_invalid");
    };
    let vector3 = [
        channel_value(&input.channel_values, &channel_map.x)?,
        channel_value(&input.channel_values, &channel_map.y)?,
        channel_value(&input.channel_values, &channel_map.z)?,
    ];
    normalize_vector_sample(input, vector3)
}

fn normalize_vector_sample(
    input: &AdapterNormalizationInput,
    vector3: [f64; 3],
) -> Result<NormalizedAdapterSample, &'static str> {
    if !finite_array3(vector3) {
        return Err("issue.adapter_payload_invalid");
    }
    let Some(units) = input.units.as_ref() else {
        return Err("issue.adapter_payload_invalid");
    };
    if units.is_empty() {
        return Err("issue.adapter_payload_invalid");
    }
    let Some(quality01) = input.quality01 else {
        return Err("issue.adapter_payload_invalid");
    };
    if !unit_interval(quality01) {
        return Err("issue.adapter_payload_invalid");
    }
    Ok(NormalizedAdapterSample::Vector(VectorMotionSample {
        source_id: input.source_id.clone(),
        sample_time_s: input.sample_time_s,
        host_time_s: input.host_time_s,
        frame_id: input.frame_id.clone(),
        vector3,
        units: units.clone(),
        quality01,
    }))
}

fn channel_value(
    channel_values: &BTreeMap<String, f64>,
    channel_id: &str,
) -> Result<f64, &'static str> {
    let Some(value) = channel_values.get(channel_id) else {
        return Err("issue.adapter_payload_invalid");
    };
    if value.is_finite() {
        Ok(*value)
    } else {
        Err("issue.adapter_payload_invalid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_fixture_goldens() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = validate_package_goldens(package_root).expect("goldens load");
        assert_eq!(report.status, "pass");
        assert_eq!(report.checked_profiles, 1);
        assert_eq!(report.checked_command_payloads, 5);
        assert_eq!(report.checked_damaged_command_payloads, 6);
        assert_eq!(report.checked_source_bindings, 5);
        assert_eq!(report.checked_damaged_source_bindings, 2);
        assert_eq!(report.checked_adapter_normalization_cases, 3);
        assert_eq!(report.checked_damaged_adapter_normalization_cases, 2);
        assert_eq!(report.checked_cases, 2);
        assert_eq!(report.checked_damaged_cases, 2);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn runs_controller_provider_preflight() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = run_controller_preflight(package_root).expect("preflight loads");
        assert_eq!(report.status, "pass");
        assert!(report.processor_core_executed);
        assert!(report.provider_boundary_exercised);
        assert!(report.controller_provider_route_ready);
        assert_eq!(report.provider_kind, "headset_controller_pose");
        assert!(report.headset_controller_shape_used);
        assert_eq!(report.output_stream_id, STREAM_OBJECT_POSE);
        assert!(!report.physical_controller_input_used);
        assert!(!report.controller_input_used);
        assert!(report.manual_controller_trial_required);
        assert_eq!(report.estimate_count, 3);
        let phases: Vec<&str> = report
            .estimates
            .iter()
            .map(|estimate| estimate.phase.as_str())
            .collect();
        assert_eq!(phases, vec!["pause", "inhale", "exhale"]);
    }

    #[test]
    fn runs_live_route_self_test_without_live_transport() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = run_live_route_self_test(package_root).expect("route self-test loads");
        assert_eq!(report.status, "pass");
        assert!(report.processor_core_executed);
        assert!(report.runtime_execution_performed);
        assert!(report.plan_only);
        assert!(!report.external_transport_used);
        assert!(!report.live_sensor_used);
        assert!(!report.headset_execution_performed);
        assert!(report
            .input_stream_ids
            .contains(&EXTERNAL_STREAM_POLAR_ACC.to_string()));
        assert!(report
            .input_stream_ids
            .contains(&STREAM_OBJECT_POSE.to_string()));
        assert!(report
            .output_stream_ids
            .contains(&STREAM_BREATH_VOLUME.to_string()));
        assert!(report
            .output_stream_ids
            .contains(&STREAM_BREATH_VOLUME_SELECTED.to_string()));
        assert!(report
            .output_stream_ids
            .contains(&STREAM_BREATH_FEEDBACK_STATE.to_string()));
        assert_eq!(
            report.receiver_subscription.command,
            RECEIVER_COMMAND_SUBSCRIBE
        );
        assert_eq!(
            report.receiver_subscription.stream,
            STREAM_BREATH_VOLUME_SELECTED
        );
        assert_eq!(report.breath_samples.len(), 6);
        assert_eq!(report.feedback_samples.len(), 6);
        assert_eq!(report.receiver_receipts.len(), 6);
        assert!(report
            .receiver_receipts
            .iter()
            .all(|receipt| receipt.command == RECEIVER_COMMAND_BREATH_FEEDBACK_RECEIVED));
    }

    #[test]
    fn runs_live_route_from_transport_event_jsonl() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let events_path = std::env::temp_dir().join(format!(
            "projected-motion-breath-live-route-events-{}.jsonl",
            std::process::id()
        ));
        let mut events = Vec::new();
        for index in 0..260 {
            let sample_time_ns = 1_000_000_000_i64 + (index as i64 * 100_000_000);
            let transport_time_ns = sample_time_ns + 10_000_000;
            let phase = (index as f64) * std::f64::consts::TAU * 0.015;
            let wave = phase.sin();
            events.push(
                serde_json::json!({
                    "type": "stream_event",
                    "stream": EXTERNAL_STREAM_POLAR_ACC,
                    "transport_time_unix_ns": transport_time_ns,
                    "payload": {
                        "stream_id": EXTERNAL_STREAM_POLAR_ACC,
                        "sample_time_unix_ns": sample_time_ns,
                        "transport_receive_time_unix_ns": transport_time_ns,
                        "samples_mg": [[20.0 * wave, 1000.0 + (5.0 * wave), 10.0 * wave]],
                        "quality01": 0.96
                    }
                })
                .to_string(),
            );
            events.push(
                serde_json::json!({
                    "type": "stream_event",
                    "stream": STREAM_OBJECT_POSE,
                    "transport_time_unix_ns": transport_time_ns,
                    "payload": {
                        "stream": STREAM_OBJECT_POSE,
                        "sample_time_unix_ns": sample_time_ns,
                        "transport_receive_time_unix_ns": transport_time_ns,
                        "reference_space": "frame.headset.stage",
                        "position_m": [0.15, 1.10 + (0.04 * wave), -0.20],
                        "orientation_xyzw": [0.0, 0.0, 0.0, 1.0],
                        "connected": true,
                        "tracked": true,
                        "quality01": 0.98
                    }
                })
                .to_string(),
            );
        }
        fs::write(&events_path, events.join("\n")).expect("events jsonl writes");
        let report = run_live_route_from_transport_events(&package_root, &events_path)
            .expect("transport events load");
        let _ = fs::remove_file(&events_path);
        assert_eq!(report.status, "pass");
        assert!(report.processor_core_executed);
        assert!(report.runtime_execution_performed);
        assert!(!report.plan_only);
        assert!(report.external_transport_used);
        assert!(report.live_sensor_used);
        assert!(report.headset_execution_performed);
        assert!(report.breath_samples.len() >= 100);
        assert_eq!(report.feedback_samples.len(), report.breath_samples.len());
        assert_eq!(report.receiver_receipts.len(), report.breath_samples.len());
    }

    #[test]
    fn live_transport_processor_emits_during_event_push_loop() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut processor =
            LiveTransportProcessor::open(&package_root).expect("live transport processor opens");
        let mut first_output_event_index = None;
        let mut selected_source_effective = String::new();
        let mut output_count = 0_usize;
        let mut events = Vec::new();
        for index in 0..260 {
            let sample_time_ns = 1_000_000_000_i64 + (index as i64 * 100_000_000);
            let transport_time_ns = sample_time_ns + 10_000_000;
            let phase = (index as f64) * std::f64::consts::TAU * 0.015;
            let wave = phase.sin();
            events.push(
                serde_json::json!({
                    "type": "stream_event",
                    "stream": EXTERNAL_STREAM_POLAR_ACC,
                    "transport_time_unix_ns": transport_time_ns,
                    "payload": {
                        "stream_id": EXTERNAL_STREAM_POLAR_ACC,
                        "sample_time_unix_ns": sample_time_ns,
                        "transport_receive_time_unix_ns": transport_time_ns,
                        "samples_mg": [[20.0 * wave, 1000.0 + (5.0 * wave), 10.0 * wave]],
                        "quality01": 0.96
                    }
                })
                .to_string(),
            );
            events.push(
                serde_json::json!({
                    "type": "stream_event",
                    "stream": STREAM_OBJECT_POSE,
                    "transport_time_unix_ns": transport_time_ns,
                    "payload": {
                        "stream": STREAM_OBJECT_POSE,
                        "sample_time_unix_ns": sample_time_ns,
                        "transport_receive_time_unix_ns": transport_time_ns,
                        "reference_space": "frame.headset.stage",
                        "position_m": [0.15, 1.10 + (0.04 * wave), -0.20],
                        "orientation_xyzw": [0.0, 0.0, 0.0, 1.0],
                        "connected": true,
                        "tracked": true,
                        "quality01": 0.98
                    }
                })
                .to_string(),
            );
        }
        for (event_index, event) in events.iter().enumerate() {
            let update = processor.push_transport_event_json(event, "auto");
            if update.output_sample_count > 0 {
                first_output_event_index.get_or_insert(event_index);
                output_count += update.output_sample_count;
                selected_source_effective = update.selected_source_effective;
            }
        }

        let first_output_event_index =
            first_output_event_index.expect("processor emits before the stream ends");
        assert!(first_output_event_index < events.len() - 10);
        assert!(output_count > 10);
        assert_eq!(selected_source_effective, "polar");
    }

    #[test]
    fn estimates_pose_projection_volume() {
        let profile = MotionBreathProfile::fixture_default(InputKind::Pose);
        let tracker =
            ProjectedMotionBreathTracker::calibrated(profile, &[-0.02, -0.01, 0.0, 0.01, 0.02])
                .expect("calibrates");
        let estimate = tracker
            .estimate_from_projection(0.01, Some(0.0), 1.0, 0.0)
            .expect("estimates");
        assert_eq!(estimate.phase, BreathPhase::Inhale);
        assert!((estimate.volume01 - 0.75).abs() < 0.000_001);
    }

    #[test]
    fn estimates_vector_projection_volume() {
        let profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        let tracker =
            ProjectedMotionBreathTracker::calibrated(profile, &[0.0, 0.25, 0.5, 0.75, 1.0])
                .expect("calibrates");
        let estimate = tracker
            .estimate_from_projection(0.25, Some(0.5), 1.0, 0.0)
            .expect("estimates");
        assert_eq!(estimate.phase, BreathPhase::Exhale);
        assert!((estimate.volume01 - 0.25).abs() < 0.000_001);
    }

    #[test]
    fn rejects_flat_calibration() {
        let profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        let issue = ProjectedMotionBreathTracker::calibrated(profile, &[0.5, 0.5, 0.5])
            .expect_err("flat calibration is invalid");
        assert_eq!(issue.code(), "issue.calibration_invalid");
    }

    #[test]
    fn rejects_stale_source() {
        let profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        let tracker =
            ProjectedMotionBreathTracker::calibrated(profile, &[0.0, 1.0]).expect("calibrates");
        let issue = tracker
            .estimate_from_projection(0.5, None, 1.0, 1.0)
            .expect_err("stale source is invalid");
        assert_eq!(issue.code(), "issue.source_stale");
    }
}
