"""Projected-motion-breath package fixture validation."""

from __future__ import annotations

import math
from typing import Any

from package_testkit_common import (
    Check,
    ID_RE,
    PackageBundle,
    append_check,
    find_one,
    finite_list,
    finite_number,
    float_close,
    list_close,
    numeric,
    prefix_errors,
    read_json,
    read_json_dir,
    unit_interval,
    within_tolerance,
)


def validate_projected_motion_breath(
    prefix: str, package: PackageBundle, checks: list[Check]
) -> None:
    if package.manifest.get("package_id") != "package.projected_motion_breath":
        return

    module_ids = {module["module_id"] for module in package.modules}
    stream_ids = {stream["stream_id"] for stream in package.streams}
    command_ids = {command["command_id"] for command in package.commands}

    required_modules = {
        "module.motion.object_pose_provider",
        "module.motion.vector_provider",
        "module.breath.projected_motion",
        "module.breath.dynamics",
        "module.breath.feedback_sink",
    }
    required_streams = {
        "stream.motion.object_pose",
        "stream.motion.vector3",
        "stream.breath.volume",
        "stream.breath.volume.selected",
        "stream.breath.volume.polar",
        "stream.breath.volume.controller",
        "stream.breath.selection_state",
        "stream.breath.dynamics",
        "stream.breath.feedback_state",
    }
    required_commands = {
        "command.breath.configure",
        "command.breath.set_profile",
        "command.breath.begin_calibration",
        "command.breath.reset_calibration",
        "command.breath.status",
    }

    missing_contract = sorted(
        (required_modules - module_ids)
        | (required_streams - stream_ids)
        | (required_commands - command_ids)
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_contract",
        not missing_contract,
        "projected-motion modules, streams, and commands are exported",
        f"missing projected-motion ids: {missing_contract}",
    )

    profile_errors = validate_projected_motion_profile_fixture(package)
    profile_errors += validate_projected_motion_command_fixtures(package, command_ids)
    append_check(
        checks,
        f"{prefix}.projected_motion_profile_commands",
        not profile_errors,
        "projected-motion profile and command payload fixtures validate",
        f"profile or command issues: {profile_errors}",
    )

    golden_errors = validate_projected_motion_golden_fixture(package)
    append_check(
        checks,
        f"{prefix}.projected_motion_goldens",
        not golden_errors,
        "projected-motion processor golden fixture recomputes expected outputs",
        f"projected-motion golden issues: {golden_errors}",
    )

    source_adapter_errors = validate_projected_motion_source_adapters(
        package,
        module_ids,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_source_adapters",
        not source_adapter_errors,
        "projected-motion source adapter descriptors map source shapes to pose/vector streams",
        f"projected-motion source adapter issues: {source_adapter_errors}",
    )

    source_binding_errors = validate_projected_motion_source_bindings(
        package,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_source_bindings",
        not source_binding_errors,
        "projected-motion source binding fixtures map profile intent to selected source streams",
        f"projected-motion source binding issues: {source_binding_errors}",
    )

    adapter_normalization_errors = validate_projected_motion_adapter_normalization(
        package,
        stream_ids,
    )
    append_check(
        checks,
        f"{prefix}.projected_motion_adapter_normalization",
        not adapter_normalization_errors,
        "projected-motion adapter normalization fixtures produce processor input samples",
        f"projected-motion adapter normalization issues: {adapter_normalization_errors}",
    )


def validate_projected_motion_source_adapters(
    package: PackageBundle,
    module_ids: set[str],
    stream_ids: set[str],
) -> list[str]:
    descriptor_set = find_one(
        package.source_adapter_descriptors,
        "descriptor_set_id",
        "descriptor_set.projected_motion_breath.source_adapters.synthetic",
    )
    if descriptor_set is None:
        return ["descriptor_set.projected_motion_breath.source_adapters.synthetic"]

    errors: list[str] = []
    if (
        descriptor_set.get("$schema")
        != "rusty.manifold.projected_motion_breath.source_adapter_descriptors.v1"
    ):
        errors.append("source_adapter_descriptors:schema")
    if descriptor_set.get("package_id") != "package.projected_motion_breath":
        errors.append("source_adapter_descriptors:package_id")
    if descriptor_set.get("target_module_id") != "module.breath.projected_motion":
        errors.append("source_adapter_descriptors:target_module_id")
    if descriptor_set.get("execution_policy") != "not_executed.schema_descriptors_only":
        errors.append("source_adapter_descriptors:execution_policy")
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if descriptor_set.get(flag) is not False:
            errors.append(f"source_adapter_descriptors:{flag}")

    adapters = descriptor_set.get("source_adapters", [])
    if not isinstance(adapters, list):
        return errors + ["source_adapter_descriptors:source_adapters"]

    required = {
        "adapter.projected_motion_breath.object_pose_generic": {
            "source_kind": "object_pose",
            "input_kind": "pose",
            "module_id": "module.motion.object_pose_provider",
            "output_stream_id": "stream.motion.object_pose",
        },
        "adapter.projected_motion_breath.vector_motion_generic": {
            "source_kind": "vector_motion",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
        "adapter.projected_motion_breath.xr_controller_pose_shape": {
            "source_kind": "xr_controller_pose",
            "input_kind": "pose",
            "module_id": "module.motion.object_pose_provider",
            "output_stream_id": "stream.motion.object_pose",
        },
        "adapter.projected_motion_breath.wearable_acceleration_shape": {
            "source_kind": "wearable_acceleration",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
        "adapter.projected_motion_breath.external_patch_stream_bridge_shape": {
            "source_kind": "external_patch_stream_bridge",
            "input_kind": "vector3",
            "module_id": "module.motion.vector_provider",
            "output_stream_id": "stream.motion.vector3",
        },
    }
    by_id = {
        adapter.get("adapter_id"): adapter
        for adapter in adapters
        if isinstance(adapter, dict)
    }
    errors += [
        f"source_adapter:{adapter_id}:missing"
        for adapter_id in sorted(set(required) - set(by_id))
    ]
    for adapter_id, adapter in by_id.items():
        if not isinstance(adapter_id, str) or not ID_RE.match(adapter_id):
            errors.append(f"source_adapter:{adapter_id}:adapter_id")
            continue
        expected = required.get(adapter_id)
        if expected is None:
            errors.append(f"source_adapter:{adapter_id}:unexpected")
            continue
        for key, expected_value in expected.items():
            if adapter.get(key) != expected_value:
                errors.append(f"source_adapter:{adapter_id}:{key}")
        if adapter.get("module_id") not in module_ids:
            errors.append(f"source_adapter:{adapter_id}:module_link")
        if adapter.get("output_stream_id") not in stream_ids:
            errors.append(f"source_adapter:{adapter_id}:stream_link")
        if adapter.get("transport_kind") != "descriptor_only":
            errors.append(f"source_adapter:{adapter_id}:transport_kind")
        for flag in (
            "requires_platform_sdk",
            "requires_device_api",
            "runtime_adapter_included",
        ):
            if adapter.get(flag) is not False:
                errors.append(f"source_adapter:{adapter_id}:{flag}")
        sample_shape = adapter.get("sample_value_shape")
        if not isinstance(sample_shape, dict) or not sample_shape:
            errors.append(f"source_adapter:{adapter_id}:sample_value_shape")
        quality_fields = adapter.get("quality_fields", [])
        if not isinstance(quality_fields, list) or "sample_age_s" not in quality_fields:
            errors.append(f"source_adapter:{adapter_id}:quality_fields")
        for field in (
            "source_shape",
            "projection_role",
            "coordinate_frame_policy",
            "timestamp_policy",
        ):
            if not isinstance(adapter.get(field), str) or not adapter.get(field):
                errors.append(f"source_adapter:{adapter_id}:{field}")
    return errors


def validate_projected_motion_source_bindings(
    package: PackageBundle,
    stream_ids: set[str],
) -> list[str]:
    valid_bindings = read_json_dir(
        package.root / "fixtures/valid",
        glob_pattern="source-binding-*.json",
    )
    damaged_bindings = read_json_dir(
        package.root / "fixtures/damaged",
        glob_pattern="source-binding-*.json",
    )
    errors: list[str] = []
    expected_valid = {
        "binding.projected_motion_breath.synthetic.object_pose",
        "binding.projected_motion_breath.synthetic.vector_motion",
        "binding.projected_motion_breath.synthetic.external_patch_stream",
    }
    present_valid = {binding.get("binding_id") for binding in valid_bindings}
    errors += [
        f"valid_source_binding:{binding_id}"
        for binding_id in sorted(expected_valid - present_valid)
    ]
    for binding in valid_bindings:
        issue = projected_motion_source_binding_issue(package, stream_ids, binding)
        if issue is not None:
            errors.append(f"{binding.get('binding_id')}:{issue}")

    required_damaged = {
        "issue.source_adapter_missing",
        "issue.source_binding_stream_mismatch",
    }
    present_damaged = {
        str(binding.get("expected_issue_code", "")) for binding in damaged_bindings
    }
    errors += [
        f"damaged_source_binding:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for binding in damaged_bindings:
        expected = str(binding.get("expected_issue_code", ""))
        actual = projected_motion_source_binding_issue(package, stream_ids, binding) or "ok"
        if expected != actual:
            errors.append(f"{binding.get('binding_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_source_binding_issue(
    package: PackageBundle,
    stream_ids: set[str],
    binding: dict[str, Any],
) -> str | None:
    if binding.get("$schema") != "rusty.manifold.projected_motion_breath.source_binding.v1":
        return "issue.source_binding_invalid"
    if not ID_RE.match(str(binding.get("binding_id", ""))):
        return "issue.source_binding_invalid"
    if binding.get("package_id") != "package.projected_motion_breath":
        return "issue.source_binding_invalid"
    if binding.get("target_module_id") != "module.breath.projected_motion":
        return "issue.source_binding_invalid"
    if binding.get("binding_policy") != "descriptor_only.owner_review_required":
        return "issue.source_binding_invalid"
    if binding.get("execution_policy") != "not_executed.schema_binding_only":
        return "issue.source_binding_invalid"
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if binding.get(flag) is not False:
            return "issue.source_binding_invalid"

    profile_path = binding.get("profile_path")
    descriptor_set_path = binding.get("descriptor_set_path")
    if not isinstance(profile_path, str) or not isinstance(descriptor_set_path, str):
        return "issue.source_binding_invalid"
    try:
        profile = read_json(package.root / profile_path)
        descriptor_set = read_json(package.root / descriptor_set_path)
    except ValueError:
        return "issue.source_binding_invalid"

    if profile.get("profile_id") != binding.get("profile_id"):
        return "issue.source_binding_invalid"
    if validate_projected_motion_profile(profile):
        return "issue.profile_invalid"

    adapters = descriptor_set.get("source_adapters", [])
    if not isinstance(adapters, list):
        return "issue.source_binding_invalid"
    adapter = find_one(adapters, "adapter_id", str(binding.get("selected_adapter_id", "")))
    if adapter is None:
        return "issue.source_adapter_missing"
    if adapter.get("source_kind") != binding.get("selected_source_kind"):
        return "issue.source_binding_stream_mismatch"
    if adapter.get("input_kind") != binding.get("selected_input_kind"):
        return "issue.source_binding_stream_mismatch"
    if adapter.get("output_stream_id") != binding.get("selected_output_stream_id"):
        return "issue.source_binding_stream_mismatch"
    source_stream_supported = binding.get("source_stream_id") == adapter.get(
        "output_stream_id"
    ) or (
        binding.get("selected_source_kind") == "wearable_acceleration"
        and binding.get("source_stream_id") == "bio:polar_acc"
    )
    if not source_stream_supported:
        return "issue.source_binding_stream_mismatch"
    if (
        binding.get("source_stream_id") not in stream_ids
        and binding.get("source_stream_id") != "bio:polar_acc"
    ):
        return "issue.source_binding_stream_mismatch"
    profile_input_kinds = profile.get("input_kinds", [])
    if (
        not isinstance(profile_input_kinds, list)
        or binding.get("selected_input_kind") not in profile_input_kinds
    ):
        return "issue.source_binding_stream_mismatch"
    return None


def validate_projected_motion_adapter_normalization(
    package: PackageBundle,
    stream_ids: set[str],
) -> list[str]:
    valid_cases = read_json_dir(
        package.root / "fixtures/valid",
        glob_pattern="adapter-normalization-*.json",
    )
    damaged_cases = read_json_dir(
        package.root / "fixtures/damaged",
        glob_pattern="adapter-normalization-*.json",
    )
    errors: list[str] = []
    expected_valid = {
        "case.projected_motion_breath.normalize.object_pose_generic",
        "case.projected_motion_breath.normalize.vector_motion",
        "case.projected_motion_breath.normalize.external_patch_vector",
    }
    present_valid = {case.get("case_id") for case in valid_cases}
    errors += [
        f"valid_adapter_normalization:{case_id}"
        for case_id in sorted(expected_valid - present_valid)
    ]
    for case in valid_cases:
        issue = projected_motion_adapter_normalization_issue(package, stream_ids, case)
        if issue is not None:
            errors.append(f"{case.get('case_id')}:{issue}")

    required_damaged = {
        "issue.adapter_payload_invalid",
        "issue.adapter_payload_kind_mismatch",
    }
    present_damaged = {str(case.get("expected_issue_code", "")) for case in damaged_cases}
    errors += [
        f"damaged_adapter_normalization:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for case in damaged_cases:
        expected = str(case.get("expected_issue_code", ""))
        actual = projected_motion_adapter_normalization_issue(package, stream_ids, case) or "ok"
        if expected != actual:
            errors.append(f"{case.get('case_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_adapter_normalization_issue(
    package: PackageBundle,
    stream_ids: set[str],
    case: dict[str, Any],
) -> str | None:
    if (
        case.get("$schema")
        != "rusty.manifold.projected_motion_breath.adapter_normalization_case.v1"
    ):
        return "issue.adapter_normalization_invalid"
    if not ID_RE.match(str(case.get("case_id", ""))):
        return "issue.adapter_normalization_invalid"
    if case.get("package_id") != "package.projected_motion_breath":
        return "issue.adapter_normalization_invalid"
    if case.get("execution_policy") != "not_executed.fixture_normalization_only":
        return "issue.adapter_normalization_invalid"
    for flag in (
        "runtime_execution_performed",
        "platform_execution_performed",
        "device_required",
    ):
        if case.get(flag) is not False:
            return "issue.adapter_normalization_invalid"

    binding_path = case.get("binding_path")
    if not isinstance(binding_path, str):
        return "issue.source_binding_invalid"
    try:
        binding = read_json(package.root / binding_path)
    except ValueError:
        return "issue.source_binding_invalid"
    binding_issue = projected_motion_source_binding_issue(package, stream_ids, binding)
    if binding_issue is not None:
        return binding_issue
    source_payload_kind = str(case.get("source_payload_kind", ""))
    if not source_payload_kind_matches(
        str(binding.get("selected_source_kind", "")),
        source_payload_kind,
    ):
        return "issue.adapter_payload_kind_mismatch"
    normalized = normalize_adapter_payload(binding, source_payload_kind, case.get("input"))
    if isinstance(normalized, str):
        return normalized
    sample_kind, sample = normalized
    if not adapter_expected_matches(
        sample_kind,
        sample,
        str(case.get("expected_sample_kind", "")),
        case.get("expected"),
    ):
        return "issue.adapter_normalization_expected_mismatch"
    return None


def source_payload_kind_matches(selected_source_kind: str, source_payload_kind: str) -> bool:
    return (selected_source_kind, source_payload_kind) in {
        ("object_pose", "object_pose"),
        ("vector_motion", "vector_motion"),
        ("wearable_acceleration", "vector_motion"),
        ("external_patch_stream_bridge", "external_patch_channels"),
    }


def normalize_adapter_payload(
    binding: dict[str, Any],
    source_payload_kind: str,
    payload: Any,
) -> tuple[str, dict[str, Any]] | str:
    if not isinstance(payload, dict):
        return "issue.adapter_payload_invalid"
    base = normalize_adapter_base(payload)
    if isinstance(base, str):
        return base
    selected_input_kind = binding.get("selected_input_kind")
    if selected_input_kind == "pose" and source_payload_kind == "object_pose":
        return normalize_object_pose_payload(payload, base)
    if selected_input_kind == "vector3" and source_payload_kind == "vector_motion":
        vector = payload.get("vector3")
        if not finite_list(vector, 3):
            return "issue.adapter_payload_invalid"
        return normalize_vector_payload(payload, base, [float(value) for value in vector])
    if selected_input_kind == "vector3" and source_payload_kind == "external_patch_channels":
        channel_values = payload.get("channel_values")
        channel_map = payload.get("channel_map")
        if not isinstance(channel_values, dict) or not isinstance(channel_map, dict):
            return "issue.adapter_payload_invalid"
        vector: list[float] = []
        for axis in ("x", "y", "z"):
            channel_id = channel_map.get(axis)
            if not isinstance(channel_id, str):
                return "issue.adapter_payload_invalid"
            value = channel_values.get(channel_id)
            if not finite_number(value):
                return "issue.adapter_payload_invalid"
            vector.append(float(value))
        return normalize_vector_payload(payload, base, vector)
    return "issue.adapter_payload_kind_mismatch"


def normalize_adapter_base(payload: dict[str, Any]) -> dict[str, Any] | str:
    source_id = payload.get("source_id")
    frame_id = payload.get("frame_id")
    sample_time_s = payload.get("sample_time_s")
    host_time_s = payload.get("host_time_s")
    if (
        not isinstance(source_id, str)
        or not source_id
        or not isinstance(frame_id, str)
        or not frame_id
        or not finite_number(sample_time_s)
        or not finite_number(host_time_s)
    ):
        return "issue.adapter_payload_invalid"
    return {
        "source_id": source_id,
        "sample_time_s": float(sample_time_s),
        "host_time_s": float(host_time_s),
        "frame_id": frame_id,
    }


def normalize_object_pose_payload(
    payload: dict[str, Any],
    base: dict[str, Any],
) -> tuple[str, dict[str, Any]] | str:
    position = payload.get("position_m")
    orientation = payload.get("orientation_xyzw")
    tracking01 = payload.get("tracking01")
    connected = payload.get("connected")
    tracked = payload.get("tracked")
    if (
        not finite_list(position, 3)
        or not finite_list(orientation, 4)
        or not unit_interval(tracking01)
        or not isinstance(connected, bool)
        or not isinstance(tracked, bool)
    ):
        return "issue.adapter_payload_invalid"
    sample = dict(base)
    sample.update(
        {
            "position_m": [float(value) for value in position],
            "orientation_xyzw": [float(value) for value in orientation],
            "connected": connected,
            "tracked": tracked,
            "quality01": float(tracking01),
        }
    )
    return "rigid_motion", sample


def normalize_vector_payload(
    payload: dict[str, Any],
    base: dict[str, Any],
    vector: list[float],
) -> tuple[str, dict[str, Any]] | str:
    units = payload.get("units")
    quality01 = payload.get("quality01")
    if not isinstance(units, str) or not units or not unit_interval(quality01):
        return "issue.adapter_payload_invalid"
    sample = dict(base)
    sample.update(
        {
            "vector3": vector,
            "units": units,
            "quality01": float(quality01),
        }
    )
    return "vector_motion", sample


def adapter_expected_matches(
    sample_kind: str,
    sample: dict[str, Any],
    expected_sample_kind: str,
    expected: Any,
) -> bool:
    if sample_kind != expected_sample_kind or not isinstance(expected, dict):
        return False
    for field in ("source_id", "frame_id"):
        if sample.get(field) != expected.get(field):
            return False
    for field in ("sample_time_s", "host_time_s", "quality01"):
        if not float_close(sample.get(field), expected.get(field)):
            return False
    if sample_kind == "rigid_motion":
        return (
            list_close(sample.get("position_m"), expected.get("position_m"))
            and list_close(sample.get("orientation_xyzw"), expected.get("orientation_xyzw"))
            and sample.get("connected") == expected.get("connected")
            and sample.get("tracked") == expected.get("tracked")
        )
    if sample_kind == "vector_motion":
        return (
            list_close(sample.get("vector3"), expected.get("vector3"))
            and sample.get("units") == expected.get("units")
        )
    return False


def validate_projected_motion_profile_fixture(package: PackageBundle) -> list[str]:
    profile_path = package.root / "fixtures/valid/profile-synthetic.json"
    profile = read_json(profile_path)
    return prefix_errors("profile.synthetic", validate_projected_motion_profile(profile))


def validate_projected_motion_profile(profile: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if profile.get("$schema") != "rusty.motion_breath_profile.v1":
        errors.append("issue.profile_invalid:schema")
    if profile.get("target_module_id") != "module.breath.projected_motion":
        errors.append("issue.profile_invalid:target_module_id")
    input_kinds = profile.get("input_kinds", [])
    if not isinstance(input_kinds, list) or {"pose", "vector3"} - set(input_kinds):
        errors.append("issue.profile_invalid:input_kinds")

    projection = profile.get("projection", {})
    if not isinstance(projection, dict):
        errors.append("issue.profile_invalid:projection")
    else:
        errors += validate_projected_motion_projection(projection)

    calibration = profile.get("calibration", {})
    if not isinstance(calibration, dict):
        errors.append("issue.profile_invalid:calibration")
    else:
        errors += validate_projected_motion_calibration(calibration)

    normalization = profile.get("normalization", {})
    if not isinstance(normalization, dict):
        errors.append("issue.profile_invalid:normalization")
    else:
        if numeric(normalization.get("soft_margin")) < 0.0:
            errors.append("issue.profile_invalid:soft_margin")
        if numeric(normalization.get("edge_ease")) < 0.0:
            errors.append("issue.profile_invalid:edge_ease")
        if numeric(normalization.get("progress_gamma")) <= 0.0:
            errors.append("issue.profile_invalid:progress_gamma")

    smoothing = profile.get("smoothing", {})
    if not isinstance(smoothing, dict):
        errors.append("issue.profile_invalid:smoothing")
    else:
        if numeric(smoothing.get("analysis_rate_hz")) <= 0.0:
            errors.append("issue.profile_invalid:analysis_rate_hz")
        if not isinstance(smoothing.get("median_window"), int) or smoothing.get("median_window") <= 0:
            errors.append("issue.profile_invalid:median_window")
        ema_alpha = numeric(smoothing.get("ema_alpha"))
        if ema_alpha <= 0.0 or ema_alpha > 1.0:
            errors.append("issue.profile_invalid:ema_alpha")

    classifier = profile.get("classifier", {})
    if not isinstance(classifier, dict):
        errors.append("issue.profile_invalid:classifier")
    else:
        errors += validate_projected_motion_classifier(classifier)

    quality = profile.get("quality", {})
    if not isinstance(quality, dict):
        errors.append("issue.profile_invalid:quality")
    else:
        min_quality = numeric(quality.get("min_quality01"))
        if min_quality < 0.0 or min_quality > 1.0:
            errors.append("issue.profile_invalid:min_quality01")
    return errors


def validate_projected_motion_projection(projection: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    allowed_modes = {
        "principal_motion_axis",
        "fixed_axis",
        "orientation_axis",
        "vector_component",
        "gravity_relative_vector",
    }
    mode = projection.get("mode")
    fallback = projection.get("fallback_mode")
    if mode not in allowed_modes:
        errors.append("issue.projection_unsupported:mode")
    if fallback is not None and fallback not in allowed_modes:
        errors.append("issue.projection_unsupported:fallback_mode")
    if mode == "fixed_axis" or fallback == "fixed_axis":
        axis = projection.get("fixed_axis")
        if not finite_nonzero_axis(axis):
            errors.append("issue.profile_invalid:fixed_axis")
    return errors


def validate_projected_motion_calibration(calibration: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if not isinstance(calibration.get("accepted_sample_count"), int) or calibration.get(
        "accepted_sample_count"
    ) <= 0:
        errors.append("issue.profile_invalid:accepted_sample_count")
    if numeric(calibration.get("min_accepted_delta")) < 0.0:
        errors.append("issue.profile_invalid:min_accepted_delta")
    if numeric(calibration.get("min_span")) <= 0.0:
        errors.append("issue.profile_invalid:min_span")
    if not valid_quantile_pair(
        numeric(calibration.get("lower_quantile")),
        numeric(calibration.get("upper_quantile")),
    ):
        errors.append("issue.profile_invalid:quantiles")
    return errors


def validate_projected_motion_classifier(classifier: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if numeric(classifier.get("delta_threshold")) < 0.0:
        errors.append("issue.profile_invalid:delta_threshold")
    if numeric(classifier.get("stale_timeout_s")) <= 0.0:
        errors.append("issue.profile_invalid:stale_timeout_s")
    return errors


def validate_projected_motion_command_fixtures(
    package: PackageBundle, command_ids: set[str]
) -> list[str]:
    errors: list[str] = []
    valid_payloads = read_json_dir(package.root / "fixtures/valid", glob_pattern="command-*.json")
    damaged_payloads = read_json_dir(
        package.root / "fixtures/damaged", glob_pattern="command-*.json"
    )
    expected_valid = {
        "command.breath.configure",
        "command.breath.set_profile",
        "command.breath.begin_calibration",
        "command.breath.reset_calibration",
        "command.breath.status",
    }
    present_valid = {payload.get("command_id") for payload in valid_payloads}
    errors += [f"valid_command:{command_id}" for command_id in sorted(expected_valid - present_valid)]

    for payload in valid_payloads:
        command_id = str(payload.get("command_id", ""))
        if command_id not in command_ids:
            errors.append(f"{payload.get('request_id')}:command_id")
            continue
        issue = projected_motion_command_issue(package, payload)
        if issue is not None:
            errors.append(f"{payload.get('request_id')}:{issue}")

    required_damaged = {
        "issue.profile_invalid",
        "issue.projection_unsupported",
        "issue.calibration_invalid",
        "issue.source_stale",
        "issue.motion_quality_low",
    }
    present_damaged = {
        str(payload.get("expected_issue_code", "")) for payload in damaged_payloads
    }
    errors += [
        f"damaged_command:{issue_code}"
        for issue_code in sorted(required_damaged - present_damaged)
    ]
    for payload in damaged_payloads:
        expected = str(payload.get("expected_issue_code", ""))
        actual = projected_motion_command_issue(package, payload) or "ok"
        if expected != actual:
            errors.append(f"{payload.get('request_id')}:expected:{expected}:actual:{actual}")
    return errors


def projected_motion_command_issue(package: PackageBundle, payload: dict[str, Any]) -> str | None:
    if not ID_RE.match(str(payload.get("request_id", ""))):
        return "issue.profile_invalid"
    if payload.get("target_module_id") != "module.breath.projected_motion":
        return "issue.profile_invalid"
    command_id = payload.get("command_id")
    if command_id == "command.breath.set_profile":
        profile_path = payload.get("profile_path")
        if not isinstance(profile_path, str):
            return "issue.profile_invalid"
        profile_errors = validate_projected_motion_profile(read_json(package.root / profile_path))
        return first_projected_motion_issue(profile_errors)
    if command_id == "command.breath.configure":
        patch = payload.get("profile_patch")
        if not isinstance(patch, dict):
            return "issue.profile_invalid"
        return first_projected_motion_issue(validate_projected_motion_profile_patch(patch))
    if command_id == "command.breath.begin_calibration":
        streams = payload.get("source_stream_ids", [])
        if not isinstance(streams, list) or not set(streams).intersection(
            {"stream.motion.object_pose", "stream.motion.vector3"}
        ):
            return "issue.profile_invalid"
        projection = payload.get("calibration_projection", [])
        if not isinstance(projection, list) or len(set(projection)) <= 1:
            return "issue.calibration_invalid"
        source_status = payload.get("source_status")
        if isinstance(source_status, dict):
            if numeric(source_status.get("sample_age_s")) > numeric(source_status.get("stale_timeout_s")):
                return "issue.source_stale"
            if numeric(source_status.get("quality01")) < numeric(source_status.get("min_quality01")):
                return "issue.motion_quality_low"
        return None
    if command_id in {"command.breath.reset_calibration", "command.breath.status"}:
        return None
    return "issue.profile_invalid"


def validate_projected_motion_profile_patch(patch: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    projection = patch.get("projection")
    if isinstance(projection, dict):
        errors += validate_projected_motion_projection(projection)
    calibration = patch.get("calibration")
    if isinstance(calibration, dict):
        lower = numeric(calibration.get("lower_quantile", 0.05))
        upper = numeric(calibration.get("upper_quantile", 0.95))
        if not valid_quantile_pair(lower, upper):
            errors.append("issue.profile_invalid:quantiles")
        if "min_span" in calibration and numeric(calibration.get("min_span")) <= 0.0:
            errors.append("issue.profile_invalid:min_span")
    classifier = patch.get("classifier")
    if isinstance(classifier, dict):
        errors += validate_projected_motion_classifier(classifier)
    quality = patch.get("quality")
    if isinstance(quality, dict) and "min_quality01" in quality:
        min_quality = numeric(quality.get("min_quality01"))
        if min_quality < 0.0 or min_quality > 1.0:
            errors.append("issue.profile_invalid:min_quality01")
    return errors


def validate_projected_motion_golden_fixture(package: PackageBundle) -> list[str]:
    golden = find_one(
        package.processing_goldens,
        "golden_id",
        "golden.projected_motion_breath.pose_and_vector_projection",
    )
    if golden is None:
        return ["golden.projected_motion_breath.pose_and_vector_projection"]

    errors: list[str] = []
    if golden.get("package_id") != "package.projected_motion_breath":
        errors.append(f"package_id:{golden.get('package_id')}")
    if golden.get("module_id") != "module.breath.projected_motion":
        errors.append(f"module_id:{golden.get('module_id')}")
    if golden.get("output_stream_id") != "stream.breath.volume":
        errors.append(f"output_stream_id:{golden.get('output_stream_id')}")
    input_stream_ids = set(golden.get("input_stream_ids", []))
    for stream_id in {"stream.motion.object_pose", "stream.motion.vector3"}:
        if stream_id not in input_stream_ids:
            errors.append(f"input_stream_id:{stream_id}")

    settings = golden.get("settings", {})
    quantiles = settings.get("calibration_quantiles", []) if isinstance(settings, dict) else []
    if not isinstance(quantiles, list) or len(quantiles) != 2 or not valid_quantile_pair(
        numeric(quantiles[0] if len(quantiles) > 0 else None),
        numeric(quantiles[1] if len(quantiles) > 1 else None),
    ):
        errors.append("settings.calibration_quantiles")
        quantiles = [0.0, 1.0]

    cases = golden.get("cases", [])
    if not isinstance(cases, list) or len(cases) < 2:
        errors.append("cases")
    else:
        for case in cases:
            if isinstance(case, dict):
                errors += validate_projected_motion_case(case, quantiles)
            else:
                errors.append("case")

    damaged_cases = golden.get("damaged_cases", [])
    if not isinstance(damaged_cases, list) or not damaged_cases:
        errors.append("damaged_cases")
    else:
        present = {
            str(case.get("expected_issue_code", "")) for case in damaged_cases if isinstance(case, dict)
        }
        for issue_code in {"issue.calibration_invalid", "issue.source_stale"} - present:
            errors.append(f"damaged_issue:{issue_code}")
        for damaged_case in damaged_cases:
            if isinstance(damaged_case, dict):
                errors += validate_projected_motion_damaged_case(damaged_case)
            else:
                errors.append("damaged_case")
    return errors


def validate_projected_motion_case(case: dict[str, Any], quantiles: list[Any]) -> list[str]:
    errors: list[str] = []
    case_id = str(case.get("case_id", ""))
    case_input = case.get("input", {})
    expected = case.get("expected", {})
    if not isinstance(case_input, dict) or not isinstance(expected, dict):
        return [f"{case_id}:shape"]
    calibration = case_input.get("calibration_projection", [])
    if not isinstance(calibration, list) or not calibration:
        return [f"{case_id}:calibration_projection"]
    values = [numeric(item) for item in calibration]
    lower_bound = nearest_quantile_value(values, numeric(quantiles[0]))
    upper_bound = nearest_quantile_value(values, numeric(quantiles[1]))
    if upper_bound <= lower_bound:
        return [f"{case_id}:bounds"]
    live_projection = numeric(case_input.get("live_projection"))
    previous_projection = numeric(case_input.get("previous_projection"))
    volume = max(0.0, min(1.0, (live_projection - lower_bound) / (upper_bound - lower_bound)))
    phase = "inhale" if live_projection > previous_projection else "exhale"
    tolerance = numeric(case.get("tolerance", {}).get("absolute")) or 0.000001
    actual = {
        "lower_bound": lower_bound,
        "upper_bound": upper_bound,
        "volume01": volume,
        "tracking01": 1.0,
    }
    for key, actual_value in actual.items():
        if key not in expected:
            errors.append(f"{case_id}:{key}:missing")
        elif not within_tolerance(actual_value, numeric(expected.get(key)), tolerance):
            errors.append(f"{case_id}:{key}")
    if expected.get("phase") != phase:
        errors.append(f"{case_id}:phase")
    if expected.get("quality") != "stable":
        errors.append(f"{case_id}:quality")
    return errors


def validate_projected_motion_damaged_case(case: dict[str, Any]) -> list[str]:
    case_id = str(case.get("case_id", ""))
    expected = str(case.get("expected_issue_code", ""))
    case_input = case.get("input", {})
    actual = "ok"
    if isinstance(case_input, dict):
        calibration = case_input.get("calibration_projection", [])
        if isinstance(calibration, list) and calibration and len(set(calibration)) <= 1:
            actual = "issue.calibration_invalid"
        elif numeric(case_input.get("sample_age_s")) > numeric(case_input.get("stale_timeout_s")):
            actual = "issue.source_stale"
    if actual != expected:
        return [f"{case_id}:expected:{expected}:actual:{actual}"]
    return []


def finite_nonzero_axis(value: Any) -> bool:
    if not isinstance(value, list) or len(value) != 3:
        return False
    values = [numeric(item) for item in value]
    return all(math.isfinite(item) for item in values) and sum(item * item for item in values) > 0.0


def valid_quantile_pair(lower: float, upper: float) -> bool:
    return (
        math.isfinite(lower)
        and math.isfinite(upper)
        and 0.0 <= lower <= 1.0
        and 0.0 <= upper <= 1.0
        and lower < upper
    )


def nearest_quantile_value(values: list[float], quantile: float) -> float:
    sorted_values = sorted(values)
    index = int(math.floor((max(0.0, min(1.0, quantile)) * (len(sorted_values) - 1)) + 0.5))
    return sorted_values[index]


def first_projected_motion_issue(errors: list[str]) -> str | None:
    if not errors:
        return None
    issue = errors[0].split(":", 1)[0]
    if issue in {
        "issue.calibration_invalid",
        "issue.motion_quality_low",
        "issue.profile_invalid",
        "issue.projection_unsupported",
        "issue.source_stale",
    }:
        return issue
    return "issue.profile_invalid"
