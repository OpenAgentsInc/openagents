use crate::hash::stable_hex_digest;
use crate::params::{ParameterStore, ScalarUnit};
use crate::{CadError, CadResult};

const DEFAULT_PATH_SEGMENTS: u32 = 32;
const MIN_PATH_LENGTH_MM: f64 = 1e-12;

/// Feature op: sweep an existing profile/feature output along a 3D path with twist + scale.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub path_points_mm: Vec<[f64; 3]>,
    pub twist_angle_param: String,
    pub scale_start_param: String,
    pub scale_end_param: String,
    /// Number of path segments; `0` means auto/default (vcad parity behavior).
    pub path_segments: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SweepFeatureStation {
    pub station_index: u32,
    pub t: f64,
    pub center_mm: [f64; 3],
    pub scale: f64,
    pub twist_angle_rad: f64,
    pub geometry_hash: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SweepFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub geometry_hash: String,
    pub segment_count: u32,
    pub path_length_mm: f64,
    pub stations: Vec<SweepFeatureStation>,
}

impl SweepFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep feature ids must not be empty".to_string(),
            });
        }
        if self.twist_angle_param.trim().is_empty()
            || self.scale_start_param.trim().is_empty()
            || self.scale_end_param.trim().is_empty()
        {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep parameter bindings must not be empty".to_string(),
            });
        }
        if self.path_points_mm.len() < 2 {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep path must include at least 2 points".to_string(),
            });
        }
        for point in &self.path_points_mm {
            if point.iter().any(|value| !value.is_finite()) {
                return Err(CadError::InvalidPrimitive {
                    reason: "sweep path points must use finite values".to_string(),
                });
            }
        }
        if self.path_segments == 1 {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep path segments must be 0 (auto) or >= 2".to_string(),
            });
        }
        let path_length_mm = polyline_length_mm(&self.path_points_mm);
        if path_length_mm <= MIN_PATH_LENGTH_MM {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep path has zero length".to_string(),
            });
        }
        Ok(())
    }

    fn resolve_controls(&self, params: &ParameterStore) -> CadResult<ResolvedSweepControls> {
        self.validate()?;

        let twist_angle_rad =
            params.get_required_with_unit(&self.twist_angle_param, ScalarUnit::Unitless)?;
        if !twist_angle_rad.is_finite() {
            return Err(CadError::InvalidParameter {
                name: self.twist_angle_param.clone(),
                reason: "sweep twist angle must be finite".to_string(),
            });
        }

        let scale_start =
            params.get_required_with_unit(&self.scale_start_param, ScalarUnit::Unitless)?;
        if !scale_start.is_finite() || scale_start <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: self.scale_start_param.clone(),
                reason: "sweep start scale must be finite and > 0".to_string(),
            });
        }

        let scale_end =
            params.get_required_with_unit(&self.scale_end_param, ScalarUnit::Unitless)?;
        if !scale_end.is_finite() || scale_end <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: self.scale_end_param.clone(),
                reason: "sweep end scale must be finite and > 0".to_string(),
            });
        }

        let segment_count = if self.path_segments == 0 {
            DEFAULT_PATH_SEGMENTS
        } else {
            self.path_segments
        };
        if segment_count < 2 {
            return Err(CadError::InvalidPrimitive {
                reason: "sweep path segments must be at least 2".to_string(),
            });
        }

        Ok(ResolvedSweepControls {
            twist_angle_rad,
            scale_start,
            scale_end,
            segment_count,
            path_length_mm: polyline_length_mm(&self.path_points_mm),
        })
    }

    fn station_hash(
        &self,
        source_geometry_hash: &str,
        station_index: u32,
        t: f64,
        center_mm: [f64; 3],
        scale: f64,
        twist_angle_rad: f64,
    ) -> String {
        let payload = format!(
            "sweep_station|feature={}|source={}|src_hash={}|index={}|t={:.6}|cx={:.6}|cy={:.6}|cz={:.6}|scale={:.6}|twist={:.6}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            station_index,
            t,
            center_mm[0],
            center_mm[1],
            center_mm[2],
            scale,
            twist_angle_rad
        );
        stable_hex_digest(payload.as_bytes())
    }

    fn geometry_hash(
        &self,
        source_geometry_hash: &str,
        controls: &ResolvedSweepControls,
        stations: &[SweepFeatureStation],
    ) -> String {
        let station_payload = stations
            .iter()
            .map(|station| format!("{}:{}", station.station_index, station.geometry_hash))
            .collect::<Vec<_>>()
            .join(",");
        let path_payload = self
            .path_points_mm
            .iter()
            .map(|point| format!("{:.6},{:.6},{:.6}", point[0], point[1], point[2]))
            .collect::<Vec<_>>()
            .join(";");
        let payload = format!(
            "sweep|feature={}|source={}|src_hash={}|path={}|segments={}|path_len={:.6}|twist={:.6}|scale_start={:.6}|scale_end={:.6}|stations={}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            path_payload,
            controls.segment_count,
            controls.path_length_mm,
            controls.twist_angle_rad,
            controls.scale_start,
            controls.scale_end,
            station_payload
        );
        stable_hex_digest(payload.as_bytes())
    }
}

pub fn evaluate_sweep_feature(
    op: &SweepFeatureOp,
    params: &ParameterStore,
    source_geometry_hash: &str,
) -> CadResult<SweepFeatureResult> {
    let controls = op.resolve_controls(params)?;
    let mut stations = Vec::with_capacity((controls.segment_count + 1) as usize);

    for station_index in 0..=controls.segment_count {
        let t = f64::from(station_index) / f64::from(controls.segment_count);
        let center_mm = sample_path_point(&op.path_points_mm, t, controls.path_length_mm);
        let scale = controls.scale_start + t * (controls.scale_end - controls.scale_start);
        let twist_angle_rad = controls.twist_angle_rad * t;
        let geometry_hash = op.station_hash(
            source_geometry_hash,
            station_index,
            t,
            center_mm,
            scale,
            twist_angle_rad,
        );
        stations.push(SweepFeatureStation {
            station_index,
            t,
            center_mm,
            scale,
            twist_angle_rad,
            geometry_hash,
        });
    }

    Ok(SweepFeatureResult {
        feature_id: op.feature_id.clone(),
        source_feature_id: op.source_feature_id.clone(),
        geometry_hash: op.geometry_hash(source_geometry_hash, &controls, &stations),
        segment_count: controls.segment_count,
        path_length_mm: controls.path_length_mm,
        stations,
    })
}

#[derive(Clone, Debug)]
struct ResolvedSweepControls {
    twist_angle_rad: f64,
    scale_start: f64,
    scale_end: f64,
    segment_count: u32,
    path_length_mm: f64,
}

fn polyline_length_mm(path_points_mm: &[[f64; 3]]) -> f64 {
    path_points_mm
        .windows(2)
        .map(|pair| distance_mm(pair[0], pair[1]))
        .sum()
}

fn sample_path_point(path_points_mm: &[[f64; 3]], t: f64, total_length_mm: f64) -> [f64; 3] {
    if t <= 0.0 {
        return path_points_mm[0];
    }
    if t >= 1.0 || total_length_mm <= MIN_PATH_LENGTH_MM {
        return *path_points_mm
            .last()
            .expect("path should have at least one point");
    }

    let target_distance = total_length_mm * t;
    let mut traversed = 0.0;
    for pair in path_points_mm.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let segment_length = distance_mm(start, end);
        if segment_length <= MIN_PATH_LENGTH_MM {
            continue;
        }
        if traversed + segment_length >= target_distance {
            let local_t = (target_distance - traversed) / segment_length;
            return lerp_point(start, end, local_t);
        }
        traversed += segment_length;
    }

    *path_points_mm
        .last()
        .expect("path should have at least one endpoint")
}

fn distance_mm(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn lerp_point(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
