//! Private scalar, vector, and projection math helpers for PMB processing.

pub(super) const EPSILON: f64 = 0.000_000_000_001;

#[derive(Clone, Copy, Debug)]
pub(super) struct AccXzModel {
    pub(super) axis: [f64; 2],
    pub(super) bound_min: f64,
    pub(super) bound_max: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DeadbandVec3 {
    last_observed: Option<[f64; 3]>,
    has_accepted: bool,
    accumulated_distance: f64,
}

impl DeadbandVec3 {
    pub(super) const fn new() -> Self {
        Self {
            last_observed: None,
            has_accepted: false,
            accumulated_distance: 0.0,
        }
    }

    pub(super) fn should_accept(&mut self, value: [f64; 3], min_delta: f64) -> bool {
        let min_delta = min_delta.max(0.0);
        if let Some(last) = self.last_observed {
            self.accumulated_distance += length3(sub3(value, last)).max(0.0);
        }
        self.last_observed = Some(value);

        if !self.has_accepted {
            self.has_accepted = true;
            self.accumulated_distance = 0.0;
            return true;
        }
        if self.accumulated_distance + EPSILON < min_delta {
            return false;
        }
        self.accumulated_distance = 0.0;
        true
    }
}

pub(super) fn mean3(samples: &[[f64; 3]]) -> [f64; 3] {
    if samples.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let mut sum = [0.0, 0.0, 0.0];
    for sample in samples {
        sum[0] += sample[0];
        sum[1] += sample[1];
        sum[2] += sample[2];
    }
    let inv = 1.0 / samples.len() as f64;
    [sum[0] * inv, sum[1] * inv, sum[2] * inv]
}

pub(super) fn sub3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

pub(super) fn add3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

pub(super) fn scale3(value: [f64; 3], scalar: f64) -> [f64; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

pub(super) fn lerp3(left: [f64; 3], right: [f64; 3], t: f64) -> [f64; 3] {
    add3(left, scale3(sub3(right, left), t.clamp(0.0, 1.0)))
}

pub(super) fn length3(value: [f64; 3]) -> f64 {
    dot3(value, value).sqrt()
}

pub(super) fn normalize3(value: [f64; 3]) -> Option<[f64; 3]> {
    if !finite_array3(value) {
        return None;
    }
    let length = length3(value);
    if length <= EPSILON {
        return None;
    }
    Some(scale3(value, 1.0 / length))
}

pub(super) fn normalize3_or(value: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    normalize3(value)
        .or_else(|| normalize3(fallback))
        .unwrap_or([0.0, 1.0, 0.0])
}

pub(super) fn principal_axis3(
    samples: &[[f64; 3]],
    center: [f64; 3],
    fallback: [f64; 3],
) -> Option<[f64; 3]> {
    if samples.len() < 3 {
        return None;
    }
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c02 = 0.0;
    let mut c11 = 0.0;
    let mut c12 = 0.0;
    let mut c22 = 0.0;
    for sample in samples {
        let d = sub3(*sample, center);
        c00 += d[0] * d[0];
        c01 += d[0] * d[1];
        c02 += d[0] * d[2];
        c11 += d[1] * d[1];
        c12 += d[1] * d[2];
        c22 += d[2] * d[2];
    }
    let inv = 1.0 / samples.len() as f64;
    c00 *= inv;
    c01 *= inv;
    c02 *= inv;
    c11 *= inv;
    c12 *= inv;
    c22 *= inv;

    let mut axis = if c00 >= c11 && c00 >= c22 {
        [1.0, 0.0, 0.0]
    } else if c11 >= c00 && c11 >= c22 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let fallback = normalize3_or(fallback, axis);
    if dot3(axis, fallback) < 0.0 {
        axis = scale3(axis, -1.0);
    }
    for _ in 0..8 {
        let next = [
            c00 * axis[0] + c01 * axis[1] + c02 * axis[2],
            c01 * axis[0] + c11 * axis[1] + c12 * axis[2],
            c02 * axis[0] + c12 * axis[1] + c22 * axis[2],
        ];
        axis = normalize3(next)?;
    }
    Some(axis)
}

pub(super) fn principal_axis_xz(samples: &[[f64; 3]], center: [f64; 3]) -> Option<[f64; 2]> {
    if samples.len() < 3 {
        return None;
    }
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c11 = 0.0;
    for sample in samples {
        let d = sub3(*sample, center);
        c00 += d[0] * d[0];
        c01 += d[0] * d[2];
        c11 += d[2] * d[2];
    }
    let inv = 1.0 / samples.len() as f64;
    c00 *= inv;
    c01 *= inv;
    c11 *= inv;

    let mut axis = if c00 >= c11 { [1.0, 0.0] } else { [0.0, 1.0] };
    for _ in 0..8 {
        let next = [c00 * axis[0] + c01 * axis[1], c01 * axis[0] + c11 * axis[1]];
        let length = (next[0] * next[0] + next[1] * next[1]).sqrt();
        if length <= EPSILON {
            return None;
        }
        axis = [next[0] / length, next[1] / length];
    }
    Some(axis)
}

pub(super) fn quantile_bounds_linear(
    values: &mut [f64],
    lower_q: f64,
    upper_q: f64,
) -> Option<(f64, f64)> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let lower = quantile_sorted_linear(values, lower_q);
    let upper = quantile_sorted_linear(values, upper_q);
    (upper > lower).then_some((lower, upper))
}

pub(super) fn quantile_sorted_linear(values: &[f64], quantile: f64) -> f64 {
    let max_index = values.len().saturating_sub(1);
    if max_index == 0 {
        return values[0];
    }
    let position = max_index as f64 * quantile.clamp(0.0, 1.0);
    let lo = position.floor() as usize;
    let hi = position.ceil() as usize;
    if lo == hi {
        values[lo]
    } else {
        lerp_f64(values[lo], values[hi], position - lo as f64)
    }
}

pub(super) fn apply_edge_ease_f64(min: &mut f64, max: &mut f64, edge_ease01: f64) {
    let span = (*max - *min).max(0.0);
    if span <= EPSILON {
        return;
    }
    let shrink = (span * edge_ease01.clamp(0.0, 1.0)).clamp(0.0, span * 0.49);
    *min += shrink;
    *max -= shrink;
}

pub(super) fn enforce_span_bounds_f64(min: &mut f64, max: &mut f64, min_span: f64, max_span: f64) {
    let min_span = min_span.max(EPSILON);
    let max_span = if max_span.is_finite() {
        max_span.max(min_span)
    } else {
        f64::INFINITY
    };
    let center = (*min + *max) * 0.5;
    let mut span = (*max - *min).max(min_span);
    if max_span.is_finite() {
        span = span.min(max_span);
    }
    let half = span * 0.5;
    *min = center - half;
    *max = center + half;
}

pub(super) fn inverse_lerp_f64(min: f64, max: f64, value: f64) -> f64 {
    if (max - min).abs() <= EPSILON {
        0.5
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    }
}

pub(super) fn lerp_f64(left: f64, right: f64, t: f64) -> f64 {
    left + (right - left) * t.clamp(0.0, 1.0)
}

pub(super) fn smooth_scalar_f64(has_previous: bool, previous: f64, next: f64, alpha: f64) -> f64 {
    if has_previous {
        lerp_f64(previous, next, alpha.clamp(0.01, 1.0))
    } else {
        next
    }
}

pub(super) fn push_window(values: &mut Vec<f64>, value: f64, cap: usize) {
    values.push(value);
    while values.len() > cap.max(1) {
        values.remove(0);
    }
}

pub(super) fn odd_window(value: usize) -> usize {
    (value.max(1) | 1).max(1)
}

pub(super) fn median_f64(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() & 1 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) * 0.5
    }
}

pub(super) fn quat_forward_neg_z(orientation_xyzw: [f64; 4]) -> [f64; 3] {
    rotate_vec3_by_quat(
        [0.0, 0.0, -1.0],
        normalize_quat_or_identity(orientation_xyzw),
    )
}

pub(super) fn quat_angle_degrees(left: [f64; 4], right: [f64; 4]) -> f64 {
    let left = normalize_quat_or_identity(left);
    let right = normalize_quat_or_identity(right);
    let dot = (left[0] * right[0] + left[1] * right[1] + left[2] * right[2] + left[3] * right[3])
        .abs()
        .clamp(-1.0, 1.0);
    (2.0 * dot.acos()).to_degrees()
}

pub(super) fn normalize_quat_or_identity(value: [f64; 4]) -> [f64; 4] {
    if !finite_array4(value) {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let length =
        (value[0] * value[0] + value[1] * value[1] + value[2] * value[2] + value[3] * value[3])
            .sqrt();
    if length <= EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [
            value[0] / length,
            value[1] / length,
            value[2] / length,
            value[3] / length,
        ]
    }
}

pub(super) fn rotate_vec3_by_quat(value: [f64; 3], quat_xyzw: [f64; 4]) -> [f64; 3] {
    let q = quat_xyzw;
    let u = [q[0], q[1], q[2]];
    let s = q[3];
    let uv = cross3(u, value);
    let uuv = cross3(u, uv);
    add3(value, add3(scale3(uv, 2.0 * s), scale3(uuv, 2.0)))
}

pub(super) fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

pub(super) fn finite_nonzero_axis(axis: [f64; 3]) -> bool {
    axis.iter().all(|value| value.is_finite())
        && axis.iter().map(|value| value * value).sum::<f64>() > EPSILON
}

pub(super) fn normalized_axis(axis: [f64; 3]) -> Option<[f64; 3]> {
    if !finite_nonzero_axis(axis) {
        return None;
    }
    let length = axis.iter().map(|value| value * value).sum::<f64>().sqrt();
    Some([axis[0] / length, axis[1] / length, axis[2] / length])
}

pub(super) fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

pub(super) fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub(super) fn finite_array3(values: [f64; 3]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub(super) fn finite_array4(values: [f64; 4]) -> bool {
    values.iter().all(|value| value.is_finite())
}

pub(super) fn array3_close(left: [f64; 3], right: [f64; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| close(*left, *right))
}

pub(super) fn close(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= 0.000_000_001
}
