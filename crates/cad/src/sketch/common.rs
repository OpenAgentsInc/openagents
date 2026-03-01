use crate::{CadError, CadResult};

pub(super) fn arc_point(center: [f64; 2], radius_mm: f64, angle_deg: f64) -> CadResult<[f64; 2]> {
    if !angle_deg.is_finite() {
        return Err(CadError::ParseFailed {
            reason: "arc angle must be finite".to_string(),
        });
    }
    let radians = angle_deg.to_radians();
    Ok([
        center[0] + radius_mm * radians.cos(),
        center[1] + radius_mm * radians.sin(),
    ])
}

pub(super) fn tangent_residual_mm(
    line_start: [f64; 2],
    line_end: [f64; 2],
    arc_center: [f64; 2],
    arc_radius: f64,
    tolerance_mm: f64,
) -> CadResult<f64> {
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];
    let denom = (dx * dx + dy * dy).sqrt();
    if denom <= tolerance_mm {
        return Err(CadError::ParseFailed {
            reason: "tangent constraint line is degenerate".to_string(),
        });
    }
    let numerator = (dy * arc_center[0] - dx * arc_center[1] + line_end[0] * line_start[1]
        - line_end[1] * line_start[0])
        .abs();
    let distance_to_line = numerator / denom;
    Ok((distance_to_line - arc_radius).abs())
}

pub(super) fn distance_mm(a: [f64; 2], b: [f64; 2]) -> f64 {
    vector_length_mm([a[0] - b[0], a[1] - b[1]])
}

pub(super) fn vector_length_mm(vector: [f64; 2]) -> f64 {
    (vector[0] * vector[0] + vector[1] * vector[1]).sqrt()
}

pub(super) fn validate_tolerance_opt(value: Option<f64>, label: &str) -> CadResult<()> {
    if let Some(value) = value {
        if !value.is_finite() || value <= 0.0 {
            return Err(CadError::ParseFailed {
                reason: format!("{label} must be finite and > 0"),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_stable_id(value: &str, label: &str) -> CadResult<()> {
    if value.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: format!("{label} must not be empty"),
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(CadError::ParseFailed {
            reason: format!(
                "{label} contains invalid characters: {value} (allowed: [A-Za-z0-9._-])"
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_vec2_finite(value: [f64; 2], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}

pub(super) fn validate_vec3_finite(value: [f64; 3], label: &str) -> CadResult<()> {
    if value.iter().all(|component| component.is_finite()) {
        return Ok(());
    }
    Err(CadError::ParseFailed {
        reason: format!("{label} must contain finite values"),
    })
}
