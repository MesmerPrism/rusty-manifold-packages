//! PMB validation, fixture expectation, and golden-check helpers.

use super::*;
use serde::Serialize;
use std::path::Path;

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

pub(super) fn validate_live_transport_route_observed(
    source_routes: &[LiveSourceRouteReport],
    breath_samples: &[LiveBreathSample],
    state_samples: &[LiveBreathStateSample],
    state_value_samples: &[LiveBreathStateValueSample],
    feedback_samples: &[LiveFeedbackSample],
    selected_source_preference: &str,
) -> Vec<String> {
    let mut issues = Vec::new();
    let observed_routes: Vec<&LiveSourceRouteReport> = source_routes
        .iter()
        .filter(|route| live_route_source_observed_or_required(route, selected_source_preference))
        .collect();
    if observed_routes.is_empty() {
        issues.push("issue.live_transport_route_source_route_count".to_string());
    }
    for route in observed_routes {
        if route.sample_count == 0 {
            issues.push(format!(
                "{}:issue.live_transport_samples_missing",
                route.source_id
            ));
        } else if route.estimate_count == 0 {
            issues.push(format!(
                "{}:issue.live_transport_estimates_missing",
                route.source_id
            ));
        }
    }
    if breath_samples.is_empty() {
        issues.push("issue.live_transport_breath_samples_missing".to_string());
    }
    if state_samples.is_empty() {
        issues.push("issue.live_transport_state_samples_missing".to_string());
    }
    if state_value_samples.is_empty() {
        issues.push("issue.live_transport_state_value_samples_missing".to_string());
    }
    if feedback_samples.is_empty() {
        issues.push("issue.live_transport_feedback_samples_missing".to_string());
    }
    issues
}

fn live_route_source_observed_or_required(
    route: &LiveSourceRouteReport,
    selected_source_preference: &str,
) -> bool {
    let source_kind = live_route_source_kind(route);
    match selected_source_preference {
        "polar" | "controller" => source_kind == selected_source_preference,
        _ => route.sample_count > 0 || route.estimate_count > 0,
    }
}

fn live_route_source_kind(route: &LiveSourceRouteReport) -> String {
    let text = format!("{} {}", route.selected_source_kind, route.source_stream_id).to_lowercase();
    if text.contains("polar") || text.contains("wearable") || text.contains("bio:polar") {
        "polar".to_string()
    } else if text.contains("controller") || text.contains("object_pose") {
        "controller".to_string()
    } else {
        "unknown".to_string()
    }
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

pub(super) fn validate_profile_document(profile: &ProfileDocument) -> Vec<String> {
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
    issues.extend(validate_controller_state_classifier(
        &profile.controller_state,
    ));
    issues.extend(validate_state_value_processor(&profile.state_value));
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

fn validate_controller_state_classifier(
    classifier: &ProfileControllerStateClassifier,
) -> Vec<String> {
    let mut issues = Vec::new();
    let allowed_modes = [
        "projected_volume_delta",
        "volume_delta",
        "fixed_controller_orientation",
    ];
    if !allowed_modes.contains(&classifier.mode.as_str()) {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    if !finite_nonzero_axis(classifier.orientation_axis)
        || !classifier.inhale_threshold.is_finite()
        || !classifier.exhale_threshold.is_finite()
        || classifier.exhale_threshold >= classifier.inhale_threshold
        || !classifier.rotation_guard_degrees.is_finite()
        || classifier.rotation_guard_degrees <= 0.0
        || !classifier.moving_average_guard.is_finite()
        || classifier.moving_average_guard <= 0.0
        || classifier.short_window == 0
        || classifier.long_window < classifier.short_window
        || !classifier.short_window_s.is_finite()
        || classifier.short_window_s <= 0.0
        || !classifier.long_window_s.is_finite()
        || classifier.long_window_s < classifier.short_window_s
        || !unit_interval(classifier.neutral_volume01)
    {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    issues
}

fn validate_state_value_processor(processor: &ProfileStateValueProcessor) -> Vec<String> {
    let mut issues = Vec::new();
    let valid_limits = processor.min_value01.is_finite()
        && processor.max_value01.is_finite()
        && (0.0..=1.0).contains(&processor.min_value01)
        && (0.0..=1.0).contains(&processor.max_value01)
        && processor.min_value01 < processor.max_value01;
    let values_valid = processor.initial_value01.is_finite()
        && processor.fallback_value01.is_finite()
        && processor.initial_value01 >= processor.min_value01
        && processor.initial_value01 <= processor.max_value01
        && processor.fallback_value01 >= processor.min_value01
        && processor.fallback_value01 <= processor.max_value01;
    let timing_valid = processor.inhale_seconds_min_to_max.is_finite()
        && processor.inhale_seconds_min_to_max > 0.0
        && processor.exhale_seconds_max_to_min.is_finite()
        && processor.exhale_seconds_max_to_min > 0.0
        && processor.smoothing_s.is_finite()
        && processor.smoothing_s >= 0.0
        && processor.stale_timeout_s.is_finite()
        && processor.stale_timeout_s > 0.0;
    if !valid_limits || !values_valid || !timing_valid {
        issues.push(BreathIssue::ProfileInvalid.code().to_string());
    }
    issues
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
    if let Some(controller_state) = &patch.controller_state {
        let default = ProfileControllerStateClassifier::default();
        let classifier = ProfileControllerStateClassifier {
            mode: controller_state
                .mode
                .clone()
                .unwrap_or_else(|| default.mode.clone()),
            orientation_axis: controller_state
                .orientation_axis
                .unwrap_or(default.orientation_axis),
            inhale_threshold: controller_state
                .inhale_threshold
                .unwrap_or(default.inhale_threshold),
            exhale_threshold: controller_state
                .exhale_threshold
                .unwrap_or(default.exhale_threshold),
            rotation_guard_degrees: controller_state
                .rotation_guard_degrees
                .unwrap_or(default.rotation_guard_degrees),
            moving_average_guard: controller_state
                .moving_average_guard
                .unwrap_or(default.moving_average_guard),
            short_window: controller_state
                .short_window
                .unwrap_or(default.short_window),
            long_window: controller_state.long_window.unwrap_or(default.long_window),
            short_window_s: controller_state
                .short_window_s
                .unwrap_or(default.short_window_s),
            long_window_s: controller_state
                .long_window_s
                .unwrap_or(default.long_window_s),
            invert_left_hand: default.invert_left_hand,
            neutral_volume01: controller_state
                .neutral_volume01
                .unwrap_or(default.neutral_volume01),
        };
        issues.extend(validate_controller_state_classifier(&classifier));
    }
    if let Some(state_value) = &patch.state_value {
        let default = ProfileStateValueProcessor::default();
        let processor = ProfileStateValueProcessor {
            enabled: state_value.enabled.unwrap_or(default.enabled),
            min_value01: state_value.min_value01.unwrap_or(default.min_value01),
            max_value01: state_value.max_value01.unwrap_or(default.max_value01),
            initial_value01: state_value
                .initial_value01
                .unwrap_or(default.initial_value01),
            fallback_value01: state_value
                .fallback_value01
                .unwrap_or(default.fallback_value01),
            inhale_seconds_min_to_max: state_value
                .inhale_seconds_min_to_max
                .unwrap_or(default.inhale_seconds_min_to_max),
            exhale_seconds_max_to_min: state_value
                .exhale_seconds_max_to_min
                .unwrap_or(default.exhale_seconds_max_to_min),
            smoothing_s: state_value.smoothing_s.unwrap_or(default.smoothing_s),
            stale_timeout_s: state_value
                .stale_timeout_s
                .unwrap_or(default.stale_timeout_s),
            hold_bad_tracking: state_value
                .hold_bad_tracking
                .unwrap_or(default.hold_bad_tracking),
        };
        issues.extend(validate_state_value_processor(&processor));
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

pub(super) fn dedup_issue_codes(issues: Vec<String>) -> Vec<String> {
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

pub(super) fn validate_source_binding(
    package_root: &Path,
    binding: &SourceBinding,
) -> Option<&'static str> {
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

pub(super) fn source_payload_kind_matches(
    selected_source_kind: &str,
    source_payload_kind: &str,
) -> bool {
    matches!(
        (selected_source_kind, source_payload_kind),
        ("object_pose", "object_pose")
            | ("xr_controller_pose", "object_pose")
            | ("vector_motion", "vector_motion")
            | ("wearable_acceleration", "vector_motion")
            | ("external_patch_stream_bridge", "external_patch_channels")
    )
}

pub(super) fn validate_controller_preflight_fixture(
    fixture: &ControllerPreflightFixture,
) -> Vec<String> {
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

pub(super) fn validate_controller_preflight_expected(
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

pub(super) fn validate_live_route_fixture(fixture: &LiveRouteFixture) -> Vec<String> {
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
    for stream_id in [
        STREAM_BREATH_VOLUME,
        STREAM_BREATH_VOLUME_SELECTED,
        STREAM_BREATH_VOLUME_POLAR,
        STREAM_BREATH_VOLUME_CONTROLLER,
        STREAM_BREATH_SELECTION_STATE,
        STREAM_BREATH_STATE,
        STREAM_BREATH_STATE_VALUE,
        STREAM_BREATH_FEEDBACK_STATE,
    ] {
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
        || fixture.receiver.subscription_stream_id != STREAM_BREATH_VOLUME_SELECTED
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

pub(super) fn validate_live_route_expected(
    fixture: &LiveRouteFixture,
    source_routes: &[LiveSourceRouteReport],
    breath_samples: &[LiveBreathSample],
    state_samples: &[LiveBreathStateSample],
    state_value_samples: &[LiveBreathStateValueSample],
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
    if state_samples.len() < fixture.expected.min_state_sample_count {
        issues.push("issue.live_route_state_sample_count".to_string());
    }
    if state_value_samples.len() < fixture.expected.min_state_value_sample_count {
        issues.push("issue.live_route_state_value_sample_count".to_string());
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
            || receipt.received_stream != STREAM_BREATH_VOLUME_SELECTED
            || !receipt.acknowledged
    }) {
        issues.push("issue.live_route_receipt_invalid".to_string());
    }
    issues
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
