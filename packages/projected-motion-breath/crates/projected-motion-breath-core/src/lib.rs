use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

mod documents;
mod live_route;
mod math;
mod state_value;
mod validation;

use documents::*;
pub use live_route::{
    run_live_route_from_transport_events,
    run_live_route_from_transport_events_with_source_preference, run_live_route_self_test,
    LiveBreathSample, LiveBreathStateSample, LiveBreathStateValueSample, LiveFeedbackSample,
    LiveRouteReport, LiveSourceRouteReport, LiveTransportProcessor, LiveTransportProcessorUpdate,
    LiveTransportSourceUpdate, ReceiverBreathReceiptPlan, ReceiverBreathSubscriptionPlan,
};
use math::*;
use validation::{
    dedup_issue_codes, source_payload_kind_matches, validate_controller_preflight_expected,
    validate_controller_preflight_fixture, validate_profile_document, validate_source_binding,
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
pub const STREAM_BREATH_STATE: &str = "stream.breath.state";
pub const STREAM_BREATH_STATE_VALUE: &str = "stream.breath.state.value";
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
    use std::fs;

    #[test]
    fn validates_fixture_goldens() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = validate_package_goldens(package_root).expect("goldens load");
        assert_eq!(report.status, "pass");
        assert_eq!(report.checked_profiles, 1);
        assert_eq!(report.checked_command_payloads, 6);
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
            .contains(&STREAM_BREATH_STATE.to_string()));
        assert!(report
            .output_stream_ids
            .contains(&STREAM_BREATH_STATE_VALUE.to_string()));
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
        assert_eq!(report.state_samples.len(), 6);
        assert_eq!(report.state_value_samples.len(), 6);
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
        assert_eq!(report.state_samples.len(), report.breath_samples.len());
        assert_eq!(
            report.state_value_samples.len(),
            report.breath_samples.len()
        );
        assert_eq!(report.feedback_samples.len(), report.breath_samples.len());
        assert_eq!(report.receiver_receipts.len(), report.breath_samples.len());
    }

    #[test]
    fn controller_selected_transport_route_does_not_require_polar_samples() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let events_path = std::env::temp_dir().join(format!(
            "projected-motion-breath-controller-only-live-route-events-{}.jsonl",
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
        let report = run_live_route_from_transport_events_with_source_preference(
            &package_root,
            &events_path,
            "controller",
        )
        .expect("controller-only transport events load");
        let _ = fs::remove_file(&events_path);
        assert_eq!(report.status, "pass");
        assert!(report.breath_samples.len() > 10);
        assert!(report
            .issues
            .iter()
            .all(|issue| !issue.contains("polar") && !issue.contains("bio:polar")));
    }

    #[test]
    fn live_transport_processor_emits_during_event_push_loop() {
        let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut processor =
            LiveTransportProcessor::open(&package_root).expect("live transport processor opens");
        let mut first_output_event_index = None;
        let mut selected_source_effective = String::new();
        let mut output_count = 0_usize;
        let mut state_count = 0_usize;
        let mut state_value_count = 0_usize;
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
                state_count += update.state_samples.len();
                state_value_count += update.state_value_samples.len();
                selected_source_effective = update.selected_source_effective;
            }
        }

        let first_output_event_index =
            first_output_event_index.expect("processor emits before the stream ends");
        assert!(first_output_event_index < events.len() - 10);
        assert!(output_count > 10);
        assert_eq!(state_count, output_count);
        assert_eq!(state_value_count, output_count);
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
