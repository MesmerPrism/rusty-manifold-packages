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
pub const PACKAGE_PROJECTED_MOTION_BREATH: &str = "package.projected_motion_breath";
pub const GOLDEN_PROJECTED_MOTION: &str =
    "golden.projected_motion_breath.pose_and_vector_projection";
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

enum NormalizedAdapterSample {
    Rigid(RigidMotionSample),
    Vector(VectorMotionSample),
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
    if !stream_supported
        || adapter.source_kind != binding.selected_source_kind
        || adapter.input_kind != binding.selected_input_kind
        || adapter.output_stream_id != binding.selected_output_stream_id
        || binding.source_stream_id != adapter.output_stream_id
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
        assert_eq!(report.checked_source_bindings, 4);
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
