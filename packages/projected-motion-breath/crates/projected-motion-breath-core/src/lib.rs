use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const MODULE_PROJECTED_MOTION_BREATH: &str = "module.breath.projected_motion";
pub const STREAM_OBJECT_POSE: &str = "stream.motion.object_pose";
pub const STREAM_VECTOR3: &str = "stream.motion.vector3";
pub const STREAM_BREATH_VOLUME: &str = "stream.breath.volume";
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
const EPSILON: f64 = 0.000_000_000_001;

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
pub struct CoreValidationReport {
    pub schema: &'static str,
    pub package_root: String,
    pub status: String,
    pub checked_profiles: usize,
    pub checked_command_payloads: usize,
    pub checked_damaged_command_payloads: usize,
    pub checked_source_bindings: usize,
    pub checked_damaged_source_bindings: usize,
    pub checked_adapter_normalization_cases: usize,
    pub checked_damaged_adapter_normalization_cases: usize,
    pub checked_cases: usize,
    pub checked_damaged_cases: usize,
    pub issues: Vec<String>,
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

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    golden_id: String,
    package_id: String,
    module_id: String,
    input_stream_ids: Vec<String>,
    output_stream_id: String,
    settings: GoldenSettings,
    cases: Vec<GoldenCase>,
    damaged_cases: Vec<DamagedGoldenCase>,
}

#[derive(Debug, Deserialize)]
struct GoldenSettings {
    calibration_quantiles: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct GoldenCase {
    case_id: String,
    input: GoldenCaseInput,
    expected: GoldenExpected,
    tolerance: Option<GoldenTolerance>,
}

#[derive(Debug, Deserialize)]
struct GoldenCaseInput {
    input_kind: String,
    calibration_projection: Vec<f64>,
    live_projection: f64,
    previous_projection: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    lower_bound: f64,
    upper_bound: f64,
    volume01: f64,
    phase: String,
    tracking01: f64,
    quality: String,
}

#[derive(Debug, Deserialize)]
struct GoldenTolerance {
    absolute: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DamagedGoldenCase {
    case_id: String,
    input: DamagedGoldenInput,
    expected_issue_code: String,
}

#[derive(Debug, Deserialize)]
struct DamagedGoldenInput {
    #[serde(default)]
    calibration_projection: Vec<f64>,
    #[serde(default)]
    live_projection: Option<f64>,
    #[serde(default)]
    sample_age_s: Option<f64>,
    #[serde(default)]
    stale_timeout_s: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProfileDocument {
    #[serde(rename = "$schema")]
    schema: String,
    profile_id: String,
    target_module_id: String,
    input_kinds: Vec<String>,
    projection: ProfileProjection,
    calibration: ProfileCalibration,
    normalization: ProfileNormalization,
    smoothing: ProfileSmoothing,
    classifier: ProfileClassifier,
    quality: ProfileQuality,
}

#[derive(Debug, Deserialize)]
struct ProfileProjection {
    mode: String,
    #[serde(default)]
    fallback_mode: Option<String>,
    #[serde(default)]
    fixed_axis: Option<[f64; 3]>,
}

#[derive(Debug, Deserialize)]
struct ProfileCalibration {
    accepted_sample_count: u64,
    min_accepted_delta: f64,
    min_span: f64,
    lower_quantile: f64,
    upper_quantile: f64,
}

#[derive(Debug, Deserialize)]
struct ProfileNormalization {
    soft_margin: f64,
    edge_ease: f64,
    progress_gamma: f64,
}

#[derive(Debug, Deserialize)]
struct ProfileSmoothing {
    analysis_rate_hz: f64,
    median_window: u64,
    ema_alpha: f64,
}

#[derive(Debug, Deserialize)]
struct ProfileClassifier {
    delta_threshold: f64,
    stale_timeout_s: f64,
}

#[derive(Debug, Deserialize)]
struct ProfileQuality {
    require_tracked: bool,
    min_quality01: f64,
}

#[derive(Debug, Default, Deserialize)]
struct ProfilePatch {
    #[serde(default)]
    projection: Option<ProjectionPatch>,
    #[serde(default)]
    calibration: Option<CalibrationPatch>,
    #[serde(default)]
    classifier: Option<ClassifierPatch>,
    #[serde(default)]
    quality: Option<QualityPatch>,
}

#[derive(Debug, Default, Deserialize)]
struct ProjectionPatch {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    fixed_axis: Option<[f64; 3]>,
}

#[derive(Debug, Default, Deserialize)]
struct CalibrationPatch {
    #[serde(default)]
    min_span: Option<f64>,
    #[serde(default)]
    lower_quantile: Option<f64>,
    #[serde(default)]
    upper_quantile: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct ClassifierPatch {
    #[serde(default)]
    delta_threshold: Option<f64>,
    #[serde(default)]
    stale_timeout_s: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct QualityPatch {
    #[serde(default)]
    min_quality01: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CommandPayload {
    #[serde(rename = "$schema")]
    schema: String,
    request_id: String,
    command_id: String,
    target_module_id: String,
    #[serde(default)]
    profile_path: Option<String>,
    #[serde(default)]
    profile_patch: Option<ProfilePatch>,
    #[serde(default)]
    source_stream_ids: Vec<String>,
    #[serde(default)]
    calibration_projection: Vec<f64>,
    #[serde(default)]
    source_status: Option<SourceStatus>,
    #[serde(default)]
    expected_issue_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourceStatus {
    sample_age_s: f64,
    stale_timeout_s: f64,
    quality01: f64,
    min_quality01: f64,
}

#[derive(Debug, Deserialize)]
struct SourceBinding {
    #[serde(rename = "$schema")]
    schema: String,
    binding_id: String,
    package_id: String,
    target_module_id: String,
    profile_id: String,
    profile_path: String,
    descriptor_set_path: String,
    selected_adapter_id: String,
    selected_source_kind: String,
    selected_input_kind: String,
    selected_output_stream_id: String,
    source_stream_id: String,
    binding_policy: String,
    execution_policy: String,
    runtime_execution_performed: bool,
    platform_execution_performed: bool,
    device_required: bool,
    #[serde(default)]
    expected_issue_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SourceAdapterDescriptorSet {
    #[serde(rename = "$schema")]
    schema: String,
    package_id: String,
    target_module_id: String,
    source_adapters: Vec<SourceAdapterDescriptor>,
}

#[derive(Debug, Deserialize)]
struct SourceAdapterDescriptor {
    adapter_id: String,
    source_kind: String,
    input_kind: String,
    output_stream_id: String,
}

#[derive(Debug, Deserialize)]
struct AdapterNormalizationCase {
    #[serde(rename = "$schema")]
    schema: String,
    case_id: String,
    package_id: String,
    binding_path: String,
    source_payload_kind: String,
    input: AdapterNormalizationInput,
    expected_sample_kind: String,
    expected: AdapterNormalizationExpected,
    execution_policy: String,
    runtime_execution_performed: bool,
    platform_execution_performed: bool,
    device_required: bool,
    #[serde(default)]
    expected_issue_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdapterNormalizationInput {
    source_id: String,
    sample_time_s: f64,
    host_time_s: f64,
    frame_id: String,
    #[serde(default)]
    position_m: Option<[f64; 3]>,
    #[serde(default)]
    orientation_xyzw: Option<[f64; 4]>,
    #[serde(default)]
    connected: Option<bool>,
    #[serde(default)]
    tracked: Option<bool>,
    #[serde(default)]
    tracking01: Option<f64>,
    #[serde(default)]
    vector3: Option<[f64; 3]>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    quality01: Option<f64>,
    #[serde(default)]
    channel_values: BTreeMap<String, f64>,
    #[serde(default)]
    channel_map: Option<AdapterChannelMap>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdapterChannelMap {
    x: String,
    y: String,
    z: String,
}

#[derive(Debug, Deserialize)]
struct AdapterNormalizationExpected {
    source_id: String,
    sample_time_s: f64,
    host_time_s: f64,
    frame_id: String,
    #[serde(default)]
    position_m: Option<[f64; 3]>,
    #[serde(default)]
    orientation_xyzw: Option<[f64; 4]>,
    #[serde(default)]
    connected: Option<bool>,
    #[serde(default)]
    tracked: Option<bool>,
    #[serde(default)]
    vector3: Option<[f64; 3]>,
    #[serde(default)]
    units: Option<String>,
    quality01: f64,
}

#[derive(Debug, Deserialize)]
struct ControllerPreflightFixture {
    #[serde(rename = "$schema")]
    schema: String,
    preflight_id: String,
    package_id: String,
    target_module_id: String,
    binding_path: String,
    source_payload_kind: String,
    provider: ControllerPreflightProvider,
    projection: ControllerPreflightProjection,
    calibration: ControllerPreflightCalibration,
    samples: Vec<AdapterNormalizationInput>,
    expected: ControllerPreflightExpected,
}

#[derive(Debug, Deserialize)]
struct ControllerPreflightProvider {
    provider_id: String,
    provider_kind: String,
    output_stream_id: String,
    physical_controller_input_used: bool,
    manual_controller_trial_required: bool,
}

#[derive(Debug, Deserialize)]
struct ControllerPreflightProjection {
    axis: [f64; 3],
}

#[derive(Debug, Deserialize)]
struct ControllerPreflightCalibration {
    projection_values: Vec<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct ControllerPreflightExpected {
    #[serde(default)]
    output_stream_id: String,
    #[serde(default)]
    min_sample_count: usize,
    #[serde(default)]
    min_estimate_count: usize,
    #[serde(default)]
    phases: Vec<String>,
    #[serde(default)]
    physical_controller_input_used: bool,
    #[serde(default)]
    manual_controller_trial_required: bool,
}

#[derive(Debug, Deserialize)]
struct LiveRouteFixture {
    #[serde(rename = "$schema")]
    schema: String,
    route_id: String,
    package_id: String,
    target_module_id: String,
    execution_policy: String,
    input_stream_ids: Vec<String>,
    normalized_stream_ids: Vec<String>,
    output_stream_ids: Vec<String>,
    external_transport_used: bool,
    live_sensor_used: bool,
    headset_execution_performed: bool,
    receiver: LiveRouteReceiverPlanFixture,
    sources: Vec<LiveRouteSourceFixture>,
    expected: LiveRouteExpected,
}

#[derive(Debug, Deserialize)]
struct LiveRouteReceiverPlanFixture {
    receiver_id: String,
    subscription_command: String,
    subscription_stream_id: String,
    receipt_command: String,
    receipt_schema: String,
}

#[derive(Debug, Deserialize)]
struct LiveRouteSourceFixture {
    source_id: String,
    source_stream_id: String,
    binding_path: String,
    source_payload_kind: String,
    projection: ControllerPreflightProjection,
    calibration: ControllerPreflightCalibration,
    samples: Vec<AdapterNormalizationInput>,
    expected_normalized_stream_id: String,
    expected_min_estimate_count: usize,
}

#[derive(Debug, Default, Deserialize)]
struct LiveRouteExpected {
    #[serde(default)]
    min_source_route_count: usize,
    #[serde(default)]
    min_breath_sample_count: usize,
    #[serde(default)]
    min_feedback_sample_count: usize,
    #[serde(default)]
    min_receipt_count: usize,
    #[serde(default)]
    required_phases: Vec<String>,
}

enum NormalizedAdapterSample {
    Rigid(RigidMotionSample),
    Vector(VectorMotionSample),
}

#[derive(Debug, Clone, Copy)]
struct LiveRouteExecutionMode {
    runtime_execution_performed: bool,
    external_transport_used: bool,
    live_sensor_used: bool,
    headset_execution_performed: bool,
    plan_only: bool,
    validate_fixture_expected: bool,
}

pub fn validate_package_goldens(
    package_root: impl AsRef<Path>,
) -> Result<CoreValidationReport, ValidationError> {
    let package_root = package_root.as_ref();
    let golden_path = package_root
        .join("fixtures")
        .join("valid")
        .join("processor-projected-motion-golden.json");
    let golden = read_golden(&golden_path)?;
    let mut issues = validate_golden_fixture(&golden);
    let profile_path = package_root
        .join("fixtures")
        .join("valid")
        .join("profile-synthetic.json");
    let profile = read_profile(&profile_path)?;
    issues.extend(prefix_issues(
        "profile.synthetic",
        validate_profile_document(&profile),
    ));
    let valid_command_payloads =
        read_command_payloads(&package_root.join("fixtures").join("valid"))?;
    let damaged_command_payloads =
        read_command_payloads(&package_root.join("fixtures").join("damaged"))?;
    let valid_source_bindings = read_source_bindings(&package_root.join("fixtures").join("valid"))?;
    let damaged_source_bindings =
        read_source_bindings(&package_root.join("fixtures").join("damaged"))?;
    let valid_adapter_normalization_cases =
        read_adapter_normalization_cases(&package_root.join("fixtures").join("valid"))?;
    let damaged_adapter_normalization_cases =
        read_adapter_normalization_cases(&package_root.join("fixtures").join("damaged"))?;
    for payload in &valid_command_payloads {
        if let Err(issue) = validate_command_payload(package_root, payload) {
            issues.push(format!("{}:{}", payload.request_id, issue.code()));
        }
    }
    for payload in &damaged_command_payloads {
        issues.extend(validate_damaged_command_payload(package_root, payload));
    }
    for binding in &valid_source_bindings {
        if let Some(issue) = validate_source_binding(package_root, binding) {
            issues.push(format!("{}:{issue}", binding.binding_id));
        }
    }
    for binding in &damaged_source_bindings {
        issues.extend(validate_damaged_source_binding(package_root, binding));
    }
    for normalization_case in &valid_adapter_normalization_cases {
        if let Some(issue) = validate_adapter_normalization_case(package_root, normalization_case) {
            issues.push(format!("{}:{issue}", normalization_case.case_id));
        }
    }
    for normalization_case in &damaged_adapter_normalization_cases {
        issues.extend(validate_damaged_adapter_normalization_case(
            package_root,
            normalization_case,
        ));
    }

    Ok(CoreValidationReport {
        schema: "rusty.manifold.projected_motion_breath.core_validation_report.v1",
        package_root: package_root.display().to_string(),
        status: if issues.is_empty() { "pass" } else { "fail" }.to_string(),
        checked_profiles: 1,
        checked_command_payloads: valid_command_payloads.len(),
        checked_damaged_command_payloads: damaged_command_payloads.len(),
        checked_source_bindings: valid_source_bindings.len(),
        checked_damaged_source_bindings: damaged_source_bindings.len(),
        checked_adapter_normalization_cases: valid_adapter_normalization_cases.len(),
        checked_damaged_adapter_normalization_cases: damaged_adapter_normalization_cases.len(),
        checked_cases: golden.cases.len(),
        checked_damaged_cases: golden.damaged_cases.len(),
        issues: std::mem::take(&mut issues),
    })
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

pub fn run_live_route_from_broker_events(
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
        read_live_broker_event_samples(events_jsonl.as_ref(), &fixture)?;
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

        let input_kind = match InputKind::parse(&binding.selected_input_kind) {
            Ok(input_kind) => input_kind,
            Err(issue) => {
                issues.push(format!("{}:{}", source_fixture.source_id, issue.code()));
                continue;
            }
        };
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
        let motion_profile = motion_profile_from_document(input_kind, &profile);
        let tracker = match ProjectedMotionBreathTracker::calibrated(
            motion_profile,
            &source_fixture.calibration.projection_values,
        ) {
            Ok(tracker) => tracker,
            Err(issue) => {
                issues.push(format!("{}:{}", source_fixture.source_id, issue.code()));
                continue;
            }
        };
        let Some(axis) = normalized_axis(source_fixture.projection.axis) else {
            issues.push(format!(
                "{}:issue.projection_axis_invalid",
                source_fixture.source_id
            ));
            continue;
        };

        let mut normalized_sample_count = 0_usize;
        let mut previous_projection = None;
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
            normalized_sample_count += 1;
            let (source_id, sample_time_s, host_time_s, projection, quality01) = match normalized {
                NormalizedAdapterSample::Rigid(sample) => {
                    let quality01 = if profile.quality.require_tracked
                        && (!sample.connected || !sample.tracked)
                    {
                        0.0
                    } else {
                        sample.quality01
                    };
                    (
                        sample.source_id,
                        sample.sample_time_s,
                        sample.host_time_s,
                        dot3(sample.position_m, axis),
                        quality01,
                    )
                }
                NormalizedAdapterSample::Vector(sample) => (
                    sample.source_id,
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
                    breath_samples.push(LiveBreathSample {
                        sequence_id: next_sequence_id,
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
                    next_sequence_id = next_sequence_id.saturating_add(1);
                }
                Err(issue) => issues.push(format!(
                    "{}:sample.{sample_index}:{}",
                    source_fixture.source_id,
                    issue.code()
                )),
            }
        }
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
    let receiver_receipts: Vec<ReceiverBreathReceiptPlan> = feedback_samples
        .iter()
        .map(|sample| ReceiverBreathReceiptPlan {
            command: fixture.receiver.receipt_command.clone(),
            schema: fixture.receiver.receipt_schema.clone(),
            received_stream: sample.stream_id.clone(),
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
        issues.extend(validate_live_broker_route_observed(
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

fn read_live_broker_event_samples(
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
            EXTERNAL_STREAM_POLAR_ACC => broker_polar_acc_samples(&event, payload),
            STREAM_OBJECT_POSE => {
                broker_object_pose_sample(&event, payload).map(|sample| vec![sample])
            }
            _ => Err("issue.broker_event_stream_unsupported"),
        };
        match converted {
            Ok(converted) => samples_by_stream
                .entry(stream.to_string())
                .or_default()
                .extend(converted),
            Err(issue) => issues.push(format!(
                "broker_event.line_{}:{stream}:{issue}",
                line_index + 1
            )),
        }
    }
    Ok((samples_by_stream, issues))
}

fn broker_polar_acc_samples(
    event: &serde_json::Value,
    payload: &serde_json::Value,
) -> Result<Vec<AdapterNormalizationInput>, &'static str> {
    let Some(samples_mg) = payload
        .get("samples_mg")
        .and_then(serde_json::Value::as_array)
    else {
        return Err("issue.broker_event_polar_samples_missing");
    };
    let base_sample_time_s = ns_to_seconds(
        first_i64(payload, &["sample_time_unix_ns", "source_time_unix_ns"])
            .or_else(|| first_i64(event, &["broker_time_unix_ns"]))
            .unwrap_or(0),
    );
    let host_time_s = ns_to_seconds(
        first_i64(
            payload,
            &["broker_receive_time_unix_ns", "client_send_time_unix_ns"],
        )
        .or_else(|| first_i64(event, &["broker_time_unix_ns"]))
        .unwrap_or(0),
    );
    let mut converted = Vec::new();
    for (index, sample) in samples_mg.iter().enumerate() {
        let Some(vector_mg) = json_array3(sample) else {
            return Err("issue.broker_event_polar_sample_invalid");
        };
        converted.push(AdapterNormalizationInput {
            source_id: "source.polar_h10.acc.live_broker".to_string(),
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

fn broker_object_pose_sample(
    event: &serde_json::Value,
    payload: &serde_json::Value,
) -> Result<AdapterNormalizationInput, &'static str> {
    let Some(position_m) = object_pose_position(payload) else {
        return Err("issue.broker_event_pose_position_missing");
    };
    let Some(orientation_xyzw) = object_pose_orientation(payload) else {
        return Err("issue.broker_event_pose_orientation_missing");
    };
    let sample_time_s = ns_to_seconds(
        first_i64(payload, &["sample_time_unix_ns", "source_time_unix_ns"])
            .or_else(|| first_i64(event, &["broker_time_unix_ns"]))
            .unwrap_or(0),
    );
    let host_time_s = ns_to_seconds(
        first_i64(
            payload,
            &["broker_receive_time_unix_ns", "client_send_time_unix_ns"],
        )
        .or_else(|| first_i64(event, &["broker_time_unix_ns"]))
        .unwrap_or(0),
    );
    Ok(AdapterNormalizationInput {
        source_id: "source.downstream.controller_pose.live_broker".to_string(),
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

fn validate_live_broker_route_observed(
    source_routes: &[LiveSourceRouteReport],
    breath_samples: &[LiveBreathSample],
    feedback_samples: &[LiveFeedbackSample],
) -> Vec<String> {
    let mut issues = Vec::new();
    if source_routes.len() < 2 {
        issues.push("issue.live_broker_route_source_route_count".to_string());
    }
    for route in source_routes {
        if route.sample_count == 0 {
            issues.push(format!(
                "{}:issue.live_broker_samples_missing",
                route.source_id
            ));
        } else if route.estimate_count == 0 {
            issues.push(format!(
                "{}:issue.live_broker_estimates_missing",
                route.source_id
            ));
        }
    }
    if breath_samples.is_empty() {
        issues.push("issue.live_broker_breath_samples_missing".to_string());
    }
    if feedback_samples.is_empty() {
        issues.push("issue.live_broker_feedback_samples_missing".to_string());
    }
    issues
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

fn read_golden(path: &Path) -> Result<GoldenFixture, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_profile(path: &Path) -> Result<ProfileDocument, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_command_payload(path: &Path) -> Result<CommandPayload, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_source_binding(path: &Path) -> Result<SourceBinding, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_source_adapter_descriptor_set(
    path: &Path,
) -> Result<SourceAdapterDescriptorSet, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_adapter_normalization_case(
    path: &Path,
) -> Result<AdapterNormalizationCase, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_controller_preflight_fixture(
    path: &Path,
) -> Result<ControllerPreflightFixture, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_live_route_fixture(path: &Path) -> Result<LiveRouteFixture, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_command_payloads(directory: &Path) -> Result<Vec<CommandPayload>, ValidationError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| ValidationError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ValidationError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with("command-")
            && path.extension().and_then(|ext| ext.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| read_command_payload(path))
        .collect()
}

fn read_source_bindings(directory: &Path) -> Result<Vec<SourceBinding>, ValidationError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| ValidationError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ValidationError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with("source-binding-")
            && path.extension().and_then(|ext| ext.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths.iter().map(|path| read_source_binding(path)).collect()
}

fn read_adapter_normalization_cases(
    directory: &Path,
) -> Result<Vec<AdapterNormalizationCase>, ValidationError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|source| ValidationError::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ValidationError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with("adapter-normalization-")
            && path.extension().and_then(|ext| ext.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| read_adapter_normalization_case(path))
        .collect()
}

fn prefix_issues(prefix: &str, issues: Vec<String>) -> Vec<String> {
    issues
        .into_iter()
        .map(|issue| format!("{prefix}:{issue}"))
        .collect()
}

fn validate_golden_fixture(golden: &GoldenFixture) -> Vec<String> {
    let mut issues = Vec::new();
    if golden.golden_id != GOLDEN_PROJECTED_MOTION {
        issues.push(format!("golden_id:{}", golden.golden_id));
    }
    if golden.package_id != PACKAGE_PROJECTED_MOTION_BREATH {
        issues.push(format!("package_id:{}", golden.package_id));
    }
    if golden.module_id != MODULE_PROJECTED_MOTION_BREATH {
        issues.push(format!("module_id:{}", golden.module_id));
    }
    if golden.output_stream_id != STREAM_BREATH_VOLUME {
        issues.push(format!("output_stream_id:{}", golden.output_stream_id));
    }
    for stream_id in [STREAM_OBJECT_POSE, STREAM_VECTOR3] {
        if !golden.input_stream_ids.iter().any(|item| item == stream_id) {
            issues.push(format!("input_stream_id:{stream_id}"));
        }
    }

    for case in &golden.cases {
        issues.extend(validate_case(case, &golden.settings));
    }
    for damaged_case in &golden.damaged_cases {
        issues.extend(validate_damaged_case(damaged_case, &golden.settings));
    }
    issues
}

fn validate_profile_document(profile: &ProfileDocument) -> Vec<String> {
    let mut issues = Vec::new();
    if profile.schema != "rusty.motion_breath_profile.v1" {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    if profile.profile_id.is_empty() || profile.target_module_id != MODULE_PROJECTED_MOTION_BREATH {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    if profile.input_kinds.is_empty()
        || profile
            .input_kinds
            .iter()
            .any(|input_kind| InputKind::parse(input_kind).is_err())
    {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    issues.extend(validate_projection(
        &profile.projection.mode,
        profile.projection.fallback_mode.as_deref(),
        profile.projection.fixed_axis,
    ));
    issues.extend(validate_calibration(
        profile.calibration.accepted_sample_count,
        profile.calibration.min_accepted_delta,
        profile.calibration.min_span,
        profile.calibration.lower_quantile,
        profile.calibration.upper_quantile,
    ));
    issues.extend(validate_normalization(&profile.normalization));
    issues.extend(validate_smoothing(&profile.smoothing));
    issues.extend(validate_classifier(
        profile.classifier.delta_threshold,
        profile.classifier.stale_timeout_s,
    ));
    issues.extend(validate_quality(
        profile.quality.require_tracked,
        profile.quality.min_quality01,
    ));
    dedup_issue_codes(issues)
}

fn validate_projection(
    mode: &str,
    fallback_mode: Option<&str>,
    fixed_axis: Option<[f64; 3]>,
) -> Vec<String> {
    let mut issues = Vec::new();
    let allowed_modes = [
        "principal_motion_axis",
        "fixed_axis",
        "orientation_axis",
        "vector_component",
        "gravity_relative_vector",
    ];
    if !allowed_modes.contains(&mode) {
        issues.push(BreathIssue::ProjectionUnsupported.code().to_string());
    }
    if let Some(fallback_mode) = fallback_mode {
        if !allowed_modes.contains(&fallback_mode) {
            issues.push(BreathIssue::ProjectionUnsupported.code().to_string());
        }
    }
    if mode == "fixed_axis" || fallback_mode == Some("fixed_axis") {
        match fixed_axis {
            Some(axis) if finite_nonzero_axis(axis) => {}
            _ => issues.push(BreathIssue::ProfileInvalid.code().to_string()),
        }
    }
    issues
}

fn finite_nonzero_axis(axis: [f64; 3]) -> bool {
    axis.iter().all(|value| value.is_finite())
        && axis.iter().map(|value| value * value).sum::<f64>() > EPSILON
}

fn validate_calibration(
    accepted_sample_count: u64,
    min_accepted_delta: f64,
    min_span: f64,
    lower_quantile: f64,
    upper_quantile: f64,
) -> Vec<String> {
    let invalid = accepted_sample_count == 0
        || !min_accepted_delta.is_finite()
        || min_accepted_delta < 0.0
        || !min_span.is_finite()
        || min_span <= EPSILON
        || !valid_quantile_pair(lower_quantile, upper_quantile);
    if invalid {
        vec![BreathIssue::ProfileInvalid.code().to_string()]
    } else {
        Vec::new()
    }
}

fn validate_normalization(normalization: &ProfileNormalization) -> Vec<String> {
    let invalid = !normalization.soft_margin.is_finite()
        || normalization.soft_margin < 0.0
        || !normalization.edge_ease.is_finite()
        || normalization.edge_ease < 0.0
        || !normalization.progress_gamma.is_finite()
        || normalization.progress_gamma <= 0.0;
    if invalid {
        vec![BreathIssue::ProfileInvalid.code().to_string()]
    } else {
        Vec::new()
    }
}

fn validate_smoothing(smoothing: &ProfileSmoothing) -> Vec<String> {
    let invalid = !smoothing.analysis_rate_hz.is_finite()
        || smoothing.analysis_rate_hz <= 0.0
        || smoothing.median_window == 0
        || !smoothing.ema_alpha.is_finite()
        || smoothing.ema_alpha <= 0.0
        || smoothing.ema_alpha > 1.0;
    if invalid {
        vec![BreathIssue::ProfileInvalid.code().to_string()]
    } else {
        Vec::new()
    }
}

fn validate_classifier(delta_threshold: f64, stale_timeout_s: f64) -> Vec<String> {
    let invalid = !delta_threshold.is_finite()
        || delta_threshold < 0.0
        || !stale_timeout_s.is_finite()
        || stale_timeout_s <= 0.0;
    if invalid {
        vec![BreathIssue::ProfileInvalid.code().to_string()]
    } else {
        Vec::new()
    }
}

fn validate_quality(_require_tracked: bool, min_quality01: f64) -> Vec<String> {
    if min_quality01.is_finite() && (0.0..=1.0).contains(&min_quality01) {
        Vec::new()
    } else {
        vec![BreathIssue::ProfileInvalid.code().to_string()]
    }
}

fn validate_profile_patch(patch: &ProfilePatch) -> Vec<String> {
    let mut issues = Vec::new();
    if let Some(projection) = &patch.projection {
        if let Some(mode) = projection.mode.as_deref() {
            issues.extend(validate_projection(mode, None, projection.fixed_axis));
        }
    }
    if let Some(calibration) = &patch.calibration {
        let lower_quantile = calibration.lower_quantile.unwrap_or(0.05);
        let upper_quantile = calibration.upper_quantile.unwrap_or(0.95);
        let invalid = calibration
            .min_span
            .is_some_and(|value| !value.is_finite() || value <= EPSILON)
            || !valid_quantile_pair(lower_quantile, upper_quantile);
        if invalid {
            issues.push(BreathIssue::ProfileInvalid.code().to_string());
        }
    }
    if let Some(classifier) = &patch.classifier {
        issues.extend(validate_classifier(
            classifier.delta_threshold.unwrap_or(0.0),
            classifier
                .stale_timeout_s
                .unwrap_or(DEFAULT_STALE_TIMEOUT_S),
        ));
    }
    if let Some(quality) = &patch.quality {
        if let Some(min_quality01) = quality.min_quality01 {
            issues.extend(validate_quality(true, min_quality01));
        }
    }
    dedup_issue_codes(issues)
}

fn valid_quantile_pair(lower_quantile: f64, upper_quantile: f64) -> bool {
    lower_quantile.is_finite()
        && upper_quantile.is_finite()
        && (0.0..=1.0).contains(&lower_quantile)
        && (0.0..=1.0).contains(&upper_quantile)
        && lower_quantile < upper_quantile
}

fn dedup_issue_codes(issues: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for issue in issues {
        if !deduped.contains(&issue) {
            deduped.push(issue);
        }
    }
    deduped
}

fn validate_command_payload(
    package_root: &Path,
    payload: &CommandPayload,
) -> Result<(), BreathIssue> {
    if payload.schema.is_empty()
        || payload.request_id.is_empty()
        || payload.target_module_id != MODULE_PROJECTED_MOTION_BREATH
    {
        return Err(BreathIssue::ProfileInvalid);
    }

    match payload.command_id.as_str() {
        COMMAND_BREATH_SET_PROFILE => validate_set_profile_payload(package_root, payload),
        COMMAND_BREATH_CONFIGURE => validate_configure_payload(payload),
        COMMAND_BREATH_BEGIN_CALIBRATION => validate_begin_calibration_payload(payload),
        COMMAND_BREATH_RESET_CALIBRATION | COMMAND_BREATH_STATUS => Ok(()),
        _ => Err(BreathIssue::ProfileInvalid),
    }
}

fn validate_set_profile_payload(
    package_root: &Path,
    payload: &CommandPayload,
) -> Result<(), BreathIssue> {
    let Some(profile_path) = payload.profile_path.as_deref() else {
        return Err(BreathIssue::ProfileInvalid);
    };
    let profile_path = package_root.join(profile_path);
    let profile = read_profile(&profile_path).map_err(|_| BreathIssue::ProfileInvalid)?;
    first_issue(validate_profile_document(&profile))
}

fn validate_configure_payload(payload: &CommandPayload) -> Result<(), BreathIssue> {
    let Some(patch) = &payload.profile_patch else {
        return Err(BreathIssue::ProfileInvalid);
    };
    first_issue(validate_profile_patch(patch))
}

fn validate_begin_calibration_payload(payload: &CommandPayload) -> Result<(), BreathIssue> {
    if !payload
        .source_stream_ids
        .iter()
        .any(|stream| stream == STREAM_OBJECT_POSE || stream == STREAM_VECTOR3)
    {
        return Err(BreathIssue::ProfileInvalid);
    }
    if !payload.calibration_projection.is_empty() {
        let profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        let tracker =
            ProjectedMotionBreathTracker::calibrated(profile, &payload.calibration_projection)?;
        if let Some(status) = &payload.source_status {
            let mut profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
            profile.stale_timeout_s = status.stale_timeout_s;
            profile.min_quality01 = status.min_quality01;
            let tracker =
                ProjectedMotionBreathTracker::calibrated(profile, &payload.calibration_projection)?;
            tracker.estimate_from_projection(0.5, None, status.quality01, status.sample_age_s)?;
        } else {
            tracker.estimate_from_projection(0.5, None, 1.0, 0.0)?;
        }
    }
    Ok(())
}

fn first_issue(issues: Vec<String>) -> Result<(), BreathIssue> {
    match issues.first().map(String::as_str) {
        None => Ok(()),
        Some("issue.calibration_invalid") => Err(BreathIssue::CalibrationInvalid),
        Some("issue.motion_quality_low") => Err(BreathIssue::MotionQualityLow),
        Some("issue.projection_unsupported") => Err(BreathIssue::ProjectionUnsupported),
        Some("issue.source_stale") => Err(BreathIssue::SourceStale),
        _ => Err(BreathIssue::ProfileInvalid),
    }
}

fn validate_damaged_command_payload(package_root: &Path, payload: &CommandPayload) -> Vec<String> {
    let Some(expected_issue_code) = payload.expected_issue_code.as_deref() else {
        return vec![format!("{}:expected_issue_code", payload.request_id)];
    };
    let actual = validate_command_payload(package_root, payload)
        .map(|_| "ok".to_string())
        .unwrap_or_else(|issue| issue.code().to_string());
    if actual == expected_issue_code {
        Vec::new()
    } else {
        vec![format!(
            "{}:expected:{}:actual:{}",
            payload.request_id, expected_issue_code, actual
        )]
    }
}

fn validate_source_binding(package_root: &Path, binding: &SourceBinding) -> Option<&'static str> {
    if binding.schema != SOURCE_BINDING_SCHEMA
        || binding.binding_id.is_empty()
        || binding.package_id != PACKAGE_PROJECTED_MOTION_BREATH
        || binding.target_module_id != MODULE_PROJECTED_MOTION_BREATH
        || binding.binding_policy != "descriptor_only.owner_review_required"
        || binding.execution_policy != "not_executed.schema_binding_only"
        || binding.runtime_execution_performed
        || binding.platform_execution_performed
        || binding.device_required
    {
        return Some("issue.source_binding_invalid");
    }

    let profile_path = package_root.join(&binding.profile_path);
    let profile = match read_profile(&profile_path) {
        Ok(profile) => profile,
        Err(_) => return Some("issue.source_binding_invalid"),
    };
    if profile.profile_id != binding.profile_id {
        return Some("issue.source_binding_invalid");
    }
    if !validate_profile_document(&profile).is_empty() {
        return Some("issue.profile_invalid");
    }

    let descriptor_set_path = package_root.join(&binding.descriptor_set_path);
    let descriptor_set = match read_source_adapter_descriptor_set(&descriptor_set_path) {
        Ok(descriptor_set) => descriptor_set,
        Err(_) => return Some("issue.source_binding_invalid"),
    };
    if descriptor_set.schema != SOURCE_ADAPTER_DESCRIPTOR_SCHEMA
        || descriptor_set.package_id != PACKAGE_PROJECTED_MOTION_BREATH
        || descriptor_set.target_module_id != MODULE_PROJECTED_MOTION_BREATH
    {
        return Some("issue.source_binding_invalid");
    }

    let Some(adapter) = descriptor_set
        .source_adapters
        .iter()
        .find(|adapter| adapter.adapter_id == binding.selected_adapter_id)
    else {
        return Some("issue.source_adapter_missing");
    };

    let stream_supported = binding.selected_output_stream_id == STREAM_OBJECT_POSE
        || binding.selected_output_stream_id == STREAM_VECTOR3;
    let source_stream_supported = binding.source_stream_id == adapter.output_stream_id
        || (binding.selected_source_kind == "wearable_acceleration"
            && binding.source_stream_id == EXTERNAL_STREAM_POLAR_ACC);
    if !stream_supported
        || adapter.source_kind != binding.selected_source_kind
        || adapter.input_kind != binding.selected_input_kind
        || adapter.output_stream_id != binding.selected_output_stream_id
        || !source_stream_supported
        || !profile
            .input_kinds
            .iter()
            .any(|input_kind| input_kind == &binding.selected_input_kind)
        || InputKind::parse(&binding.selected_input_kind).is_err()
    {
        return Some("issue.source_binding_stream_mismatch");
    }

    None
}

fn validate_damaged_source_binding(package_root: &Path, binding: &SourceBinding) -> Vec<String> {
    let Some(expected_issue_code) = binding.expected_issue_code.as_deref() else {
        return vec![format!("{}:expected_issue_code", binding.binding_id)];
    };
    let actual = validate_source_binding(package_root, binding).unwrap_or("ok");
    if actual == expected_issue_code {
        Vec::new()
    } else {
        vec![format!(
            "{}:expected:{}:actual:{}",
            binding.binding_id, expected_issue_code, actual
        )]
    }
}

fn validate_adapter_normalization_case(
    package_root: &Path,
    normalization_case: &AdapterNormalizationCase,
) -> Option<&'static str> {
    if normalization_case.schema != ADAPTER_NORMALIZATION_CASE_SCHEMA
        || normalization_case.case_id.is_empty()
        || normalization_case.package_id != PACKAGE_PROJECTED_MOTION_BREATH
        || normalization_case.execution_policy != "not_executed.fixture_normalization_only"
        || normalization_case.runtime_execution_performed
        || normalization_case.platform_execution_performed
        || normalization_case.device_required
    {
        return Some("issue.adapter_normalization_invalid");
    }
    let binding_path = package_root.join(&normalization_case.binding_path);
    let binding = match read_source_binding(&binding_path) {
        Ok(binding) => binding,
        Err(_) => return Some("issue.source_binding_invalid"),
    };
    if let Some(issue) = validate_source_binding(package_root, &binding) {
        return Some(issue);
    }
    if !source_payload_kind_matches(
        &binding.selected_source_kind,
        &normalization_case.source_payload_kind,
    ) {
        return Some("issue.adapter_payload_kind_mismatch");
    }
    let sample = match normalize_adapter_sample(
        &binding,
        &normalization_case.source_payload_kind,
        &normalization_case.input,
    ) {
        Ok(sample) => sample,
        Err(issue) => return Some(issue),
    };
    compare_normalized_sample(
        &sample,
        &normalization_case.expected_sample_kind,
        &normalization_case.expected,
    )
}

fn validate_damaged_adapter_normalization_case(
    package_root: &Path,
    normalization_case: &AdapterNormalizationCase,
) -> Vec<String> {
    let Some(expected_issue_code) = normalization_case.expected_issue_code.as_deref() else {
        return vec![format!(
            "{}:expected_issue_code",
            normalization_case.case_id
        )];
    };
    let actual =
        validate_adapter_normalization_case(package_root, normalization_case).unwrap_or("ok");
    if actual == expected_issue_code {
        Vec::new()
    } else {
        vec![format!(
            "{}:expected:{}:actual:{}",
            normalization_case.case_id, expected_issue_code, actual
        )]
    }
}

fn source_payload_kind_matches(selected_source_kind: &str, source_payload_kind: &str) -> bool {
    matches!(
        (selected_source_kind, source_payload_kind),
        ("object_pose", "object_pose")
            | ("xr_controller_pose", "object_pose")
            | ("vector_motion", "vector_motion")
            | ("wearable_acceleration", "vector_motion")
            | ("external_patch_stream_bridge", "external_patch_channels")
    )
}

fn validate_controller_preflight_fixture(fixture: &ControllerPreflightFixture) -> Vec<String> {
    let mut issues = Vec::new();
    if fixture.schema != CONTROLLER_PREFLIGHT_FIXTURE_SCHEMA {
        issues.push("issue.controller_preflight_schema_invalid".to_string());
    }
    if fixture.preflight_id.is_empty()
        || fixture.package_id != PACKAGE_PROJECTED_MOTION_BREATH
        || fixture.target_module_id != MODULE_PROJECTED_MOTION_BREATH
    {
        issues.push("issue.controller_preflight_identity_invalid".to_string());
    }
    if fixture.provider.provider_id.is_empty()
        || fixture.provider.provider_kind != "headset_controller_pose"
        || fixture.provider.output_stream_id != STREAM_OBJECT_POSE
    {
        issues.push("issue.controller_provider_invalid".to_string());
    }
    if fixture.provider.physical_controller_input_used
        || !fixture.provider.manual_controller_trial_required
    {
        issues.push("issue.controller_preflight_manual_gate_invalid".to_string());
    }
    if fixture.source_payload_kind != "object_pose" {
        issues.push("issue.controller_source_payload_kind_mismatch".to_string());
    }
    if normalized_axis(fixture.projection.axis).is_none() {
        issues.push("issue.controller_projection_axis_invalid".to_string());
    }
    if fixture.calibration.projection_values.len() < 2 {
        issues.push("issue.calibration_invalid".to_string());
    }
    if fixture.samples.is_empty() {
        issues.push("issue.controller_preflight_samples_missing".to_string());
    }
    issues
}

fn validate_controller_preflight_expected(
    fixture: &ControllerPreflightFixture,
    normalized_sample_count: usize,
    estimates: &[ControllerPreflightEstimate],
) -> Vec<String> {
    let mut issues = Vec::new();
    if !fixture.expected.output_stream_id.is_empty()
        && fixture.expected.output_stream_id != STREAM_OBJECT_POSE
    {
        issues.push("issue.controller_expected_stream_mismatch".to_string());
    }
    if fixture.expected.min_sample_count > 0
        && normalized_sample_count < fixture.expected.min_sample_count
    {
        issues.push("issue.controller_sample_count_low".to_string());
    }
    if fixture.expected.min_estimate_count > 0
        && estimates.len() < fixture.expected.min_estimate_count
    {
        issues.push("issue.controller_estimate_count_low".to_string());
    }
    if !fixture.expected.phases.is_empty() {
        let actual: Vec<&str> = estimates
            .iter()
            .map(|estimate| estimate.phase.as_str())
            .collect();
        let expected: Vec<&str> = fixture.expected.phases.iter().map(String::as_str).collect();
        if actual != expected {
            issues.push(format!(
                "issue.controller_phase_sequence_mismatch:actual:{}:expected:{}",
                actual.join(","),
                expected.join(",")
            ));
        }
    }
    if fixture.expected.physical_controller_input_used
        || !fixture.expected.manual_controller_trial_required
    {
        issues.push("issue.controller_preflight_expected_manual_gate_invalid".to_string());
    }
    issues
}

fn validate_live_route_fixture(fixture: &LiveRouteFixture) -> Vec<String> {
    let mut issues = Vec::new();
    if fixture.schema != LIVE_ROUTE_FIXTURE_SCHEMA
        || fixture.route_id.is_empty()
        || fixture.package_id != PACKAGE_PROJECTED_MOTION_BREATH
        || fixture.target_module_id != MODULE_PROJECTED_MOTION_BREATH
        || fixture.execution_policy != "plan_only.synthetic_stream_events_no_transport"
    {
        issues.push("issue.live_route_identity_invalid".to_string());
    }
    for stream_id in [EXTERNAL_STREAM_POLAR_ACC, STREAM_OBJECT_POSE] {
        if !fixture
            .input_stream_ids
            .iter()
            .any(|item| item == stream_id)
        {
            issues.push(format!("issue.live_route_input_missing:{stream_id}"));
        }
    }
    for stream_id in [STREAM_VECTOR3, STREAM_OBJECT_POSE] {
        if !fixture
            .normalized_stream_ids
            .iter()
            .any(|item| item == stream_id)
        {
            issues.push(format!(
                "issue.live_route_normalized_stream_missing:{stream_id}"
            ));
        }
    }
    for stream_id in [STREAM_BREATH_VOLUME, STREAM_BREATH_FEEDBACK_STATE] {
        if !fixture
            .output_stream_ids
            .iter()
            .any(|item| item == stream_id)
        {
            issues.push(format!("issue.live_route_output_missing:{stream_id}"));
        }
    }
    if fixture.external_transport_used
        || fixture.live_sensor_used
        || fixture.headset_execution_performed
        || fixture.sources.len() < 2
    {
        issues.push("issue.live_route_non_live_gate_invalid".to_string());
    }
    if fixture.receiver.subscription_command != RECEIVER_COMMAND_SUBSCRIBE
        || fixture.receiver.subscription_stream_id != STREAM_BREATH_FEEDBACK_STATE
        || fixture.receiver.receipt_command != RECEIVER_COMMAND_BREATH_FEEDBACK_RECEIVED
        || fixture.receiver.receipt_schema != BREATH_FEEDBACK_RECEIPT_SCHEMA
        || fixture.receiver.receiver_id.is_empty()
    {
        issues.push("issue.live_route_receiver_plan_invalid".to_string());
    }
    for source in &fixture.sources {
        if source.source_id.is_empty()
            || source.source_stream_id.is_empty()
            || source.binding_path.is_empty()
            || source.samples.is_empty()
            || source.calibration.projection_values.len() < 2
            || normalized_axis(source.projection.axis).is_none()
        {
            issues.push(format!("{}:issue.live_source_invalid", source.source_id));
        }
    }
    issues
}

fn validate_live_route_expected(
    fixture: &LiveRouteFixture,
    source_routes: &[LiveSourceRouteReport],
    breath_samples: &[LiveBreathSample],
    feedback_samples: &[LiveFeedbackSample],
    receipts: &[ReceiverBreathReceiptPlan],
) -> Vec<String> {
    let mut issues = Vec::new();
    if source_routes.len() < fixture.expected.min_source_route_count {
        issues.push("issue.live_route_source_count".to_string());
    }
    if breath_samples.len() < fixture.expected.min_breath_sample_count {
        issues.push("issue.live_route_breath_sample_count".to_string());
    }
    if feedback_samples.len() < fixture.expected.min_feedback_sample_count {
        issues.push("issue.live_route_feedback_sample_count".to_string());
    }
    if receipts.len() < fixture.expected.min_receipt_count {
        issues.push("issue.live_route_receipt_count".to_string());
    }
    for phase in &fixture.expected.required_phases {
        if !breath_samples
            .iter()
            .any(|sample| sample.phase.as_str() == phase.as_str())
        {
            issues.push(format!("issue.live_route_phase_missing:{phase}"));
        }
    }
    if !source_routes
        .iter()
        .any(|route| route.source_stream_id == EXTERNAL_STREAM_POLAR_ACC)
    {
        issues.push("issue.live_route_polar_acc_missing".to_string());
    }
    if !source_routes
        .iter()
        .any(|route| route.source_stream_id == STREAM_OBJECT_POSE)
    {
        issues.push("issue.live_route_object_pose_missing".to_string());
    }
    if receipts.iter().any(|receipt| {
        receipt.command != RECEIVER_COMMAND_BREATH_FEEDBACK_RECEIVED
            || receipt.schema != BREATH_FEEDBACK_RECEIPT_SCHEMA
            || receipt.received_stream != STREAM_BREATH_FEEDBACK_STATE
            || !receipt.acknowledged
    }) {
        issues.push("issue.live_route_receipt_invalid".to_string());
    }
    issues
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

fn normalized_axis(axis: [f64; 3]) -> Option<[f64; 3]> {
    if !finite_nonzero_axis(axis) {
        return None;
    }
    let length = axis.iter().map(|value| value * value).sum::<f64>().sqrt();
    Some([axis[0] / length, axis[1] / length, axis[2] / length])
}

fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
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

fn compare_normalized_sample(
    sample: &NormalizedAdapterSample,
    expected_sample_kind: &str,
    expected: &AdapterNormalizationExpected,
) -> Option<&'static str> {
    match sample {
        NormalizedAdapterSample::Rigid(sample) => {
            if expected_sample_kind != "rigid_motion"
                || sample.source_id != expected.source_id
                || !close(sample.sample_time_s, expected.sample_time_s)
                || !close(sample.host_time_s, expected.host_time_s)
                || sample.frame_id != expected.frame_id
                || expected
                    .position_m
                    .is_none_or(|position| !array3_close(sample.position_m, position))
                || expected
                    .orientation_xyzw
                    .is_none_or(|orientation| sample.orientation_xyzw != Some(orientation))
                || expected.connected != Some(sample.connected)
                || expected.tracked != Some(sample.tracked)
                || !close(sample.quality01, expected.quality01)
            {
                Some("issue.adapter_normalization_expected_mismatch")
            } else {
                None
            }
        }
        NormalizedAdapterSample::Vector(sample) => {
            if expected_sample_kind != "vector_motion"
                || sample.source_id != expected.source_id
                || !close(sample.sample_time_s, expected.sample_time_s)
                || !close(sample.host_time_s, expected.host_time_s)
                || sample.frame_id != expected.frame_id
                || expected
                    .vector3
                    .is_none_or(|vector| !array3_close(sample.vector3, vector))
                || expected.units.as_deref() != Some(sample.units.as_str())
                || !close(sample.quality01, expected.quality01)
            {
                Some("issue.adapter_normalization_expected_mismatch")
            } else {
                None
            }
        }
    }
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn finite_array3(values: [f64; 3]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn finite_array4(values: [f64; 4]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn array3_close(left: [f64; 3], right: [f64; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| close(*left, *right))
}

fn close(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= 0.000_000_001
}

fn validate_case(case: &GoldenCase, settings: &GoldenSettings) -> Vec<String> {
    let mut issues = Vec::new();
    let tolerance = case
        .tolerance
        .as_ref()
        .and_then(|tolerance| tolerance.absolute)
        .unwrap_or(0.000_001);
    let input_kind = match InputKind::parse(&case.input.input_kind) {
        Ok(input_kind) => input_kind,
        Err(issue) => {
            issues.push(format!("{}:{}", case.case_id, issue.code()));
            return issues;
        }
    };
    let mut profile = MotionBreathProfile::fixture_default(input_kind);
    profile.lower_quantile = settings.calibration_quantiles[0];
    profile.upper_quantile = settings.calibration_quantiles[1];
    let tracker =
        match ProjectedMotionBreathTracker::calibrated(profile, &case.input.calibration_projection)
        {
            Ok(tracker) => tracker,
            Err(issue) => {
                issues.push(format!("{}:{}", case.case_id, issue.code()));
                return issues;
            }
        };
    let estimate = match tracker.estimate_from_projection(
        case.input.live_projection,
        case.input.previous_projection,
        1.0,
        0.0,
    ) {
        Ok(estimate) => estimate,
        Err(issue) => {
            issues.push(format!("{}:{}", case.case_id, issue.code()));
            return issues;
        }
    };

    compare_float(
        &mut issues,
        &case.case_id,
        "lower_bound",
        estimate.lower_bound,
        case.expected.lower_bound,
        tolerance,
    );
    compare_float(
        &mut issues,
        &case.case_id,
        "upper_bound",
        estimate.upper_bound,
        case.expected.upper_bound,
        tolerance,
    );
    compare_float(
        &mut issues,
        &case.case_id,
        "volume01",
        estimate.volume01,
        case.expected.volume01,
        tolerance,
    );
    compare_float(
        &mut issues,
        &case.case_id,
        "tracking01",
        estimate.tracking01,
        case.expected.tracking01,
        tolerance,
    );
    if estimate.phase.as_str() != case.expected.phase {
        issues.push(format!("{}:phase", case.case_id));
    }
    if estimate.quality != case.expected.quality {
        issues.push(format!("{}:quality", case.case_id));
    }
    issues
}

fn validate_damaged_case(case: &DamagedGoldenCase, settings: &GoldenSettings) -> Vec<String> {
    let actual = if !case.input.calibration_projection.is_empty() {
        let mut profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        profile.lower_quantile = settings.calibration_quantiles[0];
        profile.upper_quantile = settings.calibration_quantiles[1];
        match ProjectedMotionBreathTracker::calibrated(profile, &case.input.calibration_projection)
        {
            Ok(tracker) => tracker
                .estimate_from_projection(case.input.live_projection.unwrap_or(0.0), None, 1.0, 0.0)
                .map(|_| "ok".to_string())
                .unwrap_or_else(|issue| issue.code().to_string()),
            Err(issue) => issue.code().to_string(),
        }
    } else if let Some(sample_age_s) = case.input.sample_age_s {
        let mut profile = MotionBreathProfile::fixture_default(InputKind::Vector3);
        profile.stale_timeout_s = case
            .input
            .stale_timeout_s
            .unwrap_or(DEFAULT_STALE_TIMEOUT_S);
        match ProjectedMotionBreathTracker::calibrated(profile, &[0.0, 1.0]) {
            Ok(tracker) => tracker
                .estimate_from_projection(0.5, None, 1.0, sample_age_s)
                .map(|_| "ok".to_string())
                .unwrap_or_else(|issue| issue.code().to_string()),
            Err(issue) => issue.code().to_string(),
        }
    } else {
        "ok".to_string()
    };

    if actual == case.expected_issue_code {
        Vec::new()
    } else {
        vec![format!(
            "{}:expected:{}:actual:{}",
            case.case_id, case.expected_issue_code, actual
        )]
    }
}

fn compare_float(
    issues: &mut Vec<String>,
    case_id: &str,
    field: &str,
    actual: f64,
    expected: f64,
    tolerance: f64,
) {
    if (actual - expected).abs() > tolerance {
        issues.push(format!(
            "{case_id}:{field}:actual:{actual}:expected:{expected}"
        ));
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
            .contains(&STREAM_BREATH_FEEDBACK_STATE.to_string()));
        assert_eq!(
            report.receiver_subscription.command,
            RECEIVER_COMMAND_SUBSCRIBE
        );
        assert_eq!(
            report.receiver_subscription.stream,
            STREAM_BREATH_FEEDBACK_STATE
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
    fn runs_live_route_from_broker_event_jsonl() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let events_path = std::env::temp_dir().join(format!(
            "projected-motion-breath-live-route-events-{}.jsonl",
            std::process::id()
        ));
        let mut events = Vec::new();
        events.push(
            serde_json::json!({
                "type": "stream_event",
                "stream": EXTERNAL_STREAM_POLAR_ACC,
                "broker_time_unix_ns": 1_010_000_000_i64,
                "payload": {
                    "stream_id": EXTERNAL_STREAM_POLAR_ACC,
                    "sample_time_unix_ns": 1_000_000_000_i64,
                    "broker_receive_time_unix_ns": 1_010_000_000_i64,
                    "samples_mg": [[20, 200, -10], [20, 280, -10], [20, 180, -10]],
                    "quality01": 0.96
                }
            })
            .to_string(),
        );
        for (index, y) in [1.10, 1.16, 1.08].iter().enumerate() {
            events.push(
                serde_json::json!({
                    "type": "stream_event",
                    "stream": STREAM_OBJECT_POSE,
                    "broker_time_unix_ns": 1_010_000_000_i64 + (index as i64 * 50_000_000),
                    "payload": {
                        "stream": STREAM_OBJECT_POSE,
                        "sample_time_unix_ns": 1_000_000_000_i64 + (index as i64 * 50_000_000),
                        "broker_receive_time_unix_ns": 1_010_000_000_i64 + (index as i64 * 50_000_000),
                        "reference_space": "frame.headset.stage",
                        "position_m": [0.15, y, -0.20],
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
        let report = run_live_route_from_broker_events(&package_root, &events_path)
            .expect("broker events load");
        let _ = fs::remove_file(&events_path);
        assert_eq!(report.status, "pass");
        assert!(report.processor_core_executed);
        assert!(report.runtime_execution_performed);
        assert!(!report.plan_only);
        assert!(report.external_transport_used);
        assert!(report.live_sensor_used);
        assert!(report.headset_execution_performed);
        assert_eq!(report.breath_samples.len(), 6);
        assert_eq!(report.feedback_samples.len(), 6);
        assert_eq!(report.receiver_receipts.len(), 6);
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
