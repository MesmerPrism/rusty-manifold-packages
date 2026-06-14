use crate::{
    compute_breath_dynamics, compute_breath_volume, compute_coherence, compute_hrv_window,
    compute_hrvb_resonance_amplitude, compute_rmssd_gain, BreathDynamicsFixtureInput,
    BreathVolumeFixtureInput, CoherenceFixtureInput, HrvWindow, HrvbFixtureInput, RmssdBaseline,
};
use serde_json::Value;
use std::path::Path;

pub fn validate_goldens(package_root: &Path) -> Result<(), Vec<String>> {
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

fn check_hrv_golden(path: &Path, errors: &mut Vec<String>) {
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

fn check_rmssd_gain_golden(path: &Path, errors: &mut Vec<String>) {
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

fn check_coherence_golden(path: &Path, errors: &mut Vec<String>) {
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

fn check_breath_volume_golden(path: &Path, errors: &mut Vec<String>) {
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

fn check_breath_dynamics_golden(path: &Path, errors: &mut Vec<String>) {
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

fn check_hrvb_golden(path: &Path, errors: &mut Vec<String>) {
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

fn read_json(path: &Path, errors: &mut Vec<String>) -> Result<Value, ()> {
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
