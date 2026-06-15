//! Serde document models and fixture readers for the PMB core validation surface.

use super::{RigidMotionSample, ValidationError, VectorMotionSample};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub(super) struct GoldenFixture {
    pub(super) golden_id: String,
    pub(super) package_id: String,
    pub(super) module_id: String,
    pub(super) input_stream_ids: Vec<String>,
    pub(super) output_stream_id: String,
    pub(super) settings: GoldenSettings,
    pub(super) cases: Vec<GoldenCase>,
    pub(super) damaged_cases: Vec<DamagedGoldenCase>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenSettings {
    pub(super) calibration_quantiles: [f64; 2],
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenCase {
    pub(super) case_id: String,
    pub(super) input: GoldenCaseInput,
    pub(super) expected: GoldenExpected,
    pub(super) tolerance: Option<GoldenTolerance>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenCaseInput {
    pub(super) input_kind: String,
    pub(super) calibration_projection: Vec<f64>,
    pub(super) live_projection: f64,
    pub(super) previous_projection: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenExpected {
    pub(super) lower_bound: f64,
    pub(super) upper_bound: f64,
    pub(super) volume01: f64,
    pub(super) phase: String,
    pub(super) tracking01: f64,
    pub(super) quality: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoldenTolerance {
    pub(super) absolute: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DamagedGoldenCase {
    pub(super) case_id: String,
    pub(super) input: DamagedGoldenInput,
    pub(super) expected_issue_code: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct DamagedGoldenInput {
    #[serde(default)]
    pub(super) calibration_projection: Vec<f64>,
    #[serde(default)]
    pub(super) live_projection: Option<f64>,
    #[serde(default)]
    pub(super) sample_age_s: Option<f64>,
    #[serde(default)]
    pub(super) stale_timeout_s: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileDocument {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) profile_id: String,
    pub(super) target_module_id: String,
    pub(super) input_kinds: Vec<String>,
    pub(super) projection: ProfileProjection,
    pub(super) calibration: ProfileCalibration,
    pub(super) normalization: ProfileNormalization,
    pub(super) smoothing: ProfileSmoothing,
    pub(super) classifier: ProfileClassifier,
    #[serde(default)]
    pub(super) controller_state: ProfileControllerStateClassifier,
    #[serde(default)]
    pub(super) state_value: ProfileStateValueProcessor,
    pub(super) quality: ProfileQuality,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileProjection {
    pub(super) mode: String,
    #[serde(default)]
    pub(super) fallback_mode: Option<String>,
    #[serde(default)]
    pub(super) fixed_axis: Option<[f64; 3]>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileCalibration {
    pub(super) accepted_sample_count: u64,
    pub(super) min_accepted_delta: f64,
    pub(super) min_span: f64,
    pub(super) lower_quantile: f64,
    pub(super) upper_quantile: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileNormalization {
    pub(super) soft_margin: f64,
    pub(super) edge_ease: f64,
    pub(super) progress_gamma: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileSmoothing {
    pub(super) analysis_rate_hz: f64,
    pub(super) median_window: u64,
    pub(super) ema_alpha: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileClassifier {
    pub(super) delta_threshold: f64,
    pub(super) stale_timeout_s: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileControllerStateClassifier {
    #[serde(default = "default_controller_state_mode")]
    pub(super) mode: String,
    #[serde(default = "default_controller_state_orientation_axis")]
    pub(super) orientation_axis: [f64; 3],
    #[serde(default = "default_controller_state_inhale_threshold")]
    pub(super) inhale_threshold: f64,
    #[serde(default = "default_controller_state_exhale_threshold")]
    pub(super) exhale_threshold: f64,
    #[serde(default = "default_controller_state_rotation_guard_degrees")]
    pub(super) rotation_guard_degrees: f64,
    #[serde(default = "default_controller_state_moving_average_guard")]
    pub(super) moving_average_guard: f64,
    #[serde(default = "default_controller_state_short_window")]
    pub(super) short_window: u64,
    #[serde(default = "default_controller_state_long_window")]
    pub(super) long_window: u64,
    #[serde(default)]
    pub(super) invert_left_hand: bool,
    #[serde(default = "default_controller_state_neutral_volume01")]
    pub(super) neutral_volume01: f64,
}

impl Default for ProfileControllerStateClassifier {
    fn default() -> Self {
        Self {
            mode: default_controller_state_mode(),
            orientation_axis: default_controller_state_orientation_axis(),
            inhale_threshold: default_controller_state_inhale_threshold(),
            exhale_threshold: default_controller_state_exhale_threshold(),
            rotation_guard_degrees: default_controller_state_rotation_guard_degrees(),
            moving_average_guard: default_controller_state_moving_average_guard(),
            short_window: default_controller_state_short_window(),
            long_window: default_controller_state_long_window(),
            invert_left_hand: false,
            neutral_volume01: default_controller_state_neutral_volume01(),
        }
    }
}

fn default_controller_state_mode() -> String {
    "projected_volume_delta".to_string()
}

fn default_controller_state_orientation_axis() -> [f64; 3] {
    [0.0, 0.0, -1.0]
}

fn default_controller_state_inhale_threshold() -> f64 {
    0.001
}

fn default_controller_state_exhale_threshold() -> f64 {
    -0.00057
}

fn default_controller_state_rotation_guard_degrees() -> f64 {
    0.5
}

fn default_controller_state_moving_average_guard() -> f64 {
    0.025
}

fn default_controller_state_short_window() -> u64 {
    24
}

fn default_controller_state_long_window() -> u64 {
    180
}

fn default_controller_state_neutral_volume01() -> f64 {
    0.5
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileStateValueProcessor {
    #[serde(default = "default_state_value_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_state_value_min_value01")]
    pub(super) min_value01: f64,
    #[serde(default = "default_state_value_max_value01")]
    pub(super) max_value01: f64,
    #[serde(default = "default_state_value_initial_value01")]
    pub(super) initial_value01: f64,
    #[serde(default = "default_state_value_fallback_value01")]
    pub(super) fallback_value01: f64,
    #[serde(default = "default_state_value_inhale_seconds_min_to_max")]
    pub(super) inhale_seconds_min_to_max: f64,
    #[serde(default = "default_state_value_exhale_seconds_max_to_min")]
    pub(super) exhale_seconds_max_to_min: f64,
    #[serde(default = "default_state_value_smoothing_s")]
    pub(super) smoothing_s: f64,
    #[serde(default = "default_state_value_stale_timeout_s")]
    pub(super) stale_timeout_s: f64,
    #[serde(default = "default_state_value_hold_bad_tracking")]
    pub(super) hold_bad_tracking: bool,
}

impl Default for ProfileStateValueProcessor {
    fn default() -> Self {
        Self {
            enabled: default_state_value_enabled(),
            min_value01: default_state_value_min_value01(),
            max_value01: default_state_value_max_value01(),
            initial_value01: default_state_value_initial_value01(),
            fallback_value01: default_state_value_fallback_value01(),
            inhale_seconds_min_to_max: default_state_value_inhale_seconds_min_to_max(),
            exhale_seconds_max_to_min: default_state_value_exhale_seconds_max_to_min(),
            smoothing_s: default_state_value_smoothing_s(),
            stale_timeout_s: default_state_value_stale_timeout_s(),
            hold_bad_tracking: default_state_value_hold_bad_tracking(),
        }
    }
}

fn default_state_value_enabled() -> bool {
    true
}

fn default_state_value_min_value01() -> f64 {
    0.0
}

fn default_state_value_max_value01() -> f64 {
    1.0
}

fn default_state_value_initial_value01() -> f64 {
    0.5
}

fn default_state_value_fallback_value01() -> f64 {
    0.5
}

fn default_state_value_inhale_seconds_min_to_max() -> f64 {
    4.0
}

fn default_state_value_exhale_seconds_max_to_min() -> f64 {
    4.0
}

fn default_state_value_smoothing_s() -> f64 {
    0.03
}

fn default_state_value_stale_timeout_s() -> f64 {
    1.0
}

fn default_state_value_hold_bad_tracking() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileQuality {
    pub(super) require_tracked: bool,
    pub(super) min_quality01: f64,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ProfilePatch {
    #[serde(default)]
    pub(super) projection: Option<ProjectionPatch>,
    #[serde(default)]
    pub(super) calibration: Option<CalibrationPatch>,
    #[serde(default)]
    pub(super) classifier: Option<ClassifierPatch>,
    #[serde(default)]
    pub(super) controller_state: Option<ControllerStatePatch>,
    #[serde(default)]
    pub(super) state_value: Option<StateValuePatch>,
    #[serde(default)]
    pub(super) quality: Option<QualityPatch>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ProjectionPatch {
    #[serde(default)]
    pub(super) mode: Option<String>,
    #[serde(default)]
    pub(super) fixed_axis: Option<[f64; 3]>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct CalibrationPatch {
    #[serde(default)]
    pub(super) min_span: Option<f64>,
    #[serde(default)]
    pub(super) lower_quantile: Option<f64>,
    #[serde(default)]
    pub(super) upper_quantile: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ClassifierPatch {
    #[serde(default)]
    pub(super) delta_threshold: Option<f64>,
    #[serde(default)]
    pub(super) stale_timeout_s: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ControllerStatePatch {
    #[serde(default)]
    pub(super) mode: Option<String>,
    #[serde(default)]
    pub(super) orientation_axis: Option<[f64; 3]>,
    #[serde(default)]
    pub(super) inhale_threshold: Option<f64>,
    #[serde(default)]
    pub(super) exhale_threshold: Option<f64>,
    #[serde(default)]
    pub(super) rotation_guard_degrees: Option<f64>,
    #[serde(default)]
    pub(super) moving_average_guard: Option<f64>,
    #[serde(default)]
    pub(super) short_window: Option<u64>,
    #[serde(default)]
    pub(super) long_window: Option<u64>,
    #[serde(default)]
    pub(super) neutral_volume01: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct StateValuePatch {
    #[serde(default)]
    pub(super) enabled: Option<bool>,
    #[serde(default)]
    pub(super) min_value01: Option<f64>,
    #[serde(default)]
    pub(super) max_value01: Option<f64>,
    #[serde(default)]
    pub(super) initial_value01: Option<f64>,
    #[serde(default)]
    pub(super) fallback_value01: Option<f64>,
    #[serde(default)]
    pub(super) inhale_seconds_min_to_max: Option<f64>,
    #[serde(default)]
    pub(super) exhale_seconds_max_to_min: Option<f64>,
    #[serde(default)]
    pub(super) smoothing_s: Option<f64>,
    #[serde(default)]
    pub(super) stale_timeout_s: Option<f64>,
    #[serde(default)]
    pub(super) hold_bad_tracking: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct QualityPatch {
    #[serde(default)]
    pub(super) min_quality01: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CommandPayload {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) request_id: String,
    pub(super) command_id: String,
    pub(super) target_module_id: String,
    #[serde(default)]
    pub(super) profile_path: Option<String>,
    #[serde(default)]
    pub(super) profile_patch: Option<ProfilePatch>,
    #[serde(default)]
    pub(super) source_stream_ids: Vec<String>,
    #[serde(default)]
    pub(super) calibration_projection: Vec<f64>,
    #[serde(default)]
    pub(super) source_status: Option<SourceStatus>,
    #[serde(default)]
    pub(super) expected_issue_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceStatus {
    pub(super) sample_age_s: f64,
    pub(super) stale_timeout_s: f64,
    pub(super) quality01: f64,
    pub(super) min_quality01: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceBinding {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) binding_id: String,
    pub(super) package_id: String,
    pub(super) target_module_id: String,
    pub(super) profile_id: String,
    pub(super) profile_path: String,
    pub(super) descriptor_set_path: String,
    pub(super) selected_adapter_id: String,
    pub(super) selected_source_kind: String,
    pub(super) selected_input_kind: String,
    pub(super) selected_output_stream_id: String,
    pub(super) source_stream_id: String,
    pub(super) binding_policy: String,
    pub(super) execution_policy: String,
    pub(super) runtime_execution_performed: bool,
    pub(super) platform_execution_performed: bool,
    pub(super) device_required: bool,
    #[serde(default)]
    pub(super) expected_issue_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceAdapterDescriptorSet {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) package_id: String,
    pub(super) target_module_id: String,
    pub(super) source_adapters: Vec<SourceAdapterDescriptor>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceAdapterDescriptor {
    pub(super) adapter_id: String,
    pub(super) source_kind: String,
    pub(super) input_kind: String,
    pub(super) output_stream_id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdapterNormalizationCase {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) case_id: String,
    pub(super) package_id: String,
    pub(super) binding_path: String,
    pub(super) source_payload_kind: String,
    pub(super) input: AdapterNormalizationInput,
    pub(super) expected_sample_kind: String,
    pub(super) expected: AdapterNormalizationExpected,
    pub(super) execution_policy: String,
    pub(super) runtime_execution_performed: bool,
    pub(super) platform_execution_performed: bool,
    pub(super) device_required: bool,
    #[serde(default)]
    pub(super) expected_issue_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AdapterNormalizationInput {
    pub(super) source_id: String,
    pub(super) sample_time_s: f64,
    pub(super) host_time_s: f64,
    pub(super) frame_id: String,
    #[serde(default)]
    pub(super) position_m: Option<[f64; 3]>,
    #[serde(default)]
    pub(super) orientation_xyzw: Option<[f64; 4]>,
    #[serde(default)]
    pub(super) connected: Option<bool>,
    #[serde(default)]
    pub(super) tracked: Option<bool>,
    #[serde(default)]
    pub(super) tracking01: Option<f64>,
    #[serde(default)]
    pub(super) vector3: Option<[f64; 3]>,
    #[serde(default)]
    pub(super) units: Option<String>,
    #[serde(default)]
    pub(super) quality01: Option<f64>,
    #[serde(default)]
    pub(super) channel_values: BTreeMap<String, f64>,
    #[serde(default)]
    pub(super) channel_map: Option<AdapterChannelMap>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AdapterChannelMap {
    pub(super) x: String,
    pub(super) y: String,
    pub(super) z: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdapterNormalizationExpected {
    pub(super) source_id: String,
    pub(super) sample_time_s: f64,
    pub(super) host_time_s: f64,
    pub(super) frame_id: String,
    #[serde(default)]
    pub(super) position_m: Option<[f64; 3]>,
    #[serde(default)]
    pub(super) orientation_xyzw: Option<[f64; 4]>,
    #[serde(default)]
    pub(super) connected: Option<bool>,
    #[serde(default)]
    pub(super) tracked: Option<bool>,
    #[serde(default)]
    pub(super) vector3: Option<[f64; 3]>,
    #[serde(default)]
    pub(super) units: Option<String>,
    pub(super) quality01: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ControllerPreflightFixture {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) preflight_id: String,
    pub(super) package_id: String,
    pub(super) target_module_id: String,
    pub(super) binding_path: String,
    pub(super) source_payload_kind: String,
    pub(super) provider: ControllerPreflightProvider,
    pub(super) projection: ControllerPreflightProjection,
    pub(super) calibration: ControllerPreflightCalibration,
    pub(super) samples: Vec<AdapterNormalizationInput>,
    pub(super) expected: ControllerPreflightExpected,
}

#[derive(Debug, Deserialize)]
pub(super) struct ControllerPreflightProvider {
    pub(super) provider_id: String,
    pub(super) provider_kind: String,
    pub(super) output_stream_id: String,
    pub(super) physical_controller_input_used: bool,
    pub(super) manual_controller_trial_required: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ControllerPreflightProjection {
    pub(super) axis: [f64; 3],
}

#[derive(Debug, Deserialize)]
pub(super) struct ControllerPreflightCalibration {
    pub(super) projection_values: Vec<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ControllerPreflightExpected {
    #[serde(default)]
    pub(super) output_stream_id: String,
    #[serde(default)]
    pub(super) min_sample_count: usize,
    #[serde(default)]
    pub(super) min_estimate_count: usize,
    #[serde(default)]
    pub(super) phases: Vec<String>,
    #[serde(default)]
    pub(super) physical_controller_input_used: bool,
    #[serde(default)]
    pub(super) manual_controller_trial_required: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveRouteFixture {
    #[serde(rename = "$schema")]
    pub(super) schema: String,
    pub(super) route_id: String,
    pub(super) package_id: String,
    pub(super) target_module_id: String,
    pub(super) execution_policy: String,
    pub(super) input_stream_ids: Vec<String>,
    pub(super) normalized_stream_ids: Vec<String>,
    pub(super) output_stream_ids: Vec<String>,
    pub(super) external_transport_used: bool,
    pub(super) live_sensor_used: bool,
    pub(super) headset_execution_performed: bool,
    pub(super) receiver: LiveRouteReceiverPlanFixture,
    pub(super) sources: Vec<LiveRouteSourceFixture>,
    pub(super) expected: LiveRouteExpected,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveRouteReceiverPlanFixture {
    pub(super) receiver_id: String,
    pub(super) subscription_command: String,
    pub(super) subscription_stream_id: String,
    pub(super) receipt_command: String,
    pub(super) receipt_schema: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LiveRouteSourceFixture {
    pub(super) source_id: String,
    pub(super) source_stream_id: String,
    pub(super) binding_path: String,
    pub(super) source_payload_kind: String,
    pub(super) projection: ControllerPreflightProjection,
    pub(super) calibration: ControllerPreflightCalibration,
    pub(super) samples: Vec<AdapterNormalizationInput>,
    pub(super) expected_normalized_stream_id: String,
    pub(super) expected_min_estimate_count: usize,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct LiveRouteExpected {
    #[serde(default)]
    pub(super) min_source_route_count: usize,
    #[serde(default)]
    pub(super) min_breath_sample_count: usize,
    #[serde(default)]
    pub(super) min_state_sample_count: usize,
    #[serde(default)]
    pub(super) min_state_value_sample_count: usize,
    #[serde(default)]
    pub(super) min_feedback_sample_count: usize,
    #[serde(default)]
    pub(super) min_receipt_count: usize,
    #[serde(default)]
    pub(super) required_phases: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) enum NormalizedAdapterSample {
    Rigid(RigidMotionSample),
    Vector(VectorMotionSample),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LiveRouteExecutionMode {
    pub(super) runtime_execution_performed: bool,
    pub(super) external_transport_used: bool,
    pub(super) live_sensor_used: bool,
    pub(super) headset_execution_performed: bool,
    pub(super) plan_only: bool,
    pub(super) validate_fixture_expected: bool,
}

pub(super) fn read_golden(path: &Path) -> Result<GoldenFixture, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_profile(path: &Path) -> Result<ProfileDocument, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_command_payload(path: &Path) -> Result<CommandPayload, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_source_binding(path: &Path) -> Result<SourceBinding, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_source_adapter_descriptor_set(
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

pub(super) fn read_adapter_normalization_case(
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

pub(super) fn read_controller_preflight_fixture(
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

pub(super) fn read_live_route_fixture(path: &Path) -> Result<LiveRouteFixture, ValidationError> {
    let text = fs::read_to_string(path).map_err(|source| ValidationError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ValidationError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn read_command_payloads(
    directory: &Path,
) -> Result<Vec<CommandPayload>, ValidationError> {
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

pub(super) fn read_source_bindings(
    directory: &Path,
) -> Result<Vec<SourceBinding>, ValidationError> {
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

pub(super) fn read_adapter_normalization_cases(
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
