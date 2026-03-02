use crate::hash::stable_hex_digest;
use crate::{CadError, CadResult};

/// One loft profile ring.
#[derive(Clone, Debug, PartialEq)]
pub struct LoftFeatureProfile {
    pub profile_id: String,
    pub vertices_mm: Vec<[f64; 3]>,
}

/// Feature op: loft between multiple profiles with optional closed tube mode.
#[derive(Clone, Debug, PartialEq)]
pub struct LoftFeatureOp {
    pub feature_id: String,
    pub source_feature_ids: Vec<String>,
    pub profiles: Vec<LoftFeatureProfile>,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoftFeatureResult {
    pub feature_id: String,
    pub closed: bool,
    pub profile_count: usize,
    pub vertices_per_profile: usize,
    pub transition_count: usize,
    pub lateral_patch_count: usize,
    pub cap_count: usize,
    pub profile_centroids_mm: Vec<[f64; 3]>,
    pub geometry_hash: String,
}

impl LoftFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "loft feature id must not be empty".to_string(),
            });
        }
        if self.profiles.len() < 2 {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "loft requires at least 2 profiles, got {}",
                    self.profiles.len()
                ),
            });
        }
        if self.source_feature_ids.len() != self.profiles.len() {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "loft source feature ids must match profile count ({} vs {})",
                    self.source_feature_ids.len(),
                    self.profiles.len()
                ),
            });
        }
        if self
            .source_feature_ids
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(CadError::InvalidPrimitive {
                reason: "loft source feature ids must not be empty".to_string(),
            });
        }

        let expected_vertices = self.profiles[0].vertices_mm.len();
        if expected_vertices < 3 {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "loft profile {} must contain at least 3 vertices",
                    self.profiles[0].profile_id
                ),
            });
        }

        for profile in &self.profiles {
            if profile.profile_id.trim().is_empty() {
                return Err(CadError::InvalidPrimitive {
                    reason: "loft profile id must not be empty".to_string(),
                });
            }
            if profile.vertices_mm.len() != expected_vertices {
                return Err(CadError::InvalidPrimitive {
                    reason: format!(
                        "loft profile {} vertex count mismatch: expected {}, got {}",
                        profile.profile_id,
                        expected_vertices,
                        profile.vertices_mm.len()
                    ),
                });
            }
            if profile.vertices_mm.iter().any(|point| {
                point[0].is_nan()
                    || point[1].is_nan()
                    || point[2].is_nan()
                    || !point[0].is_finite()
                    || !point[1].is_finite()
                    || !point[2].is_finite()
            }) {
                return Err(CadError::InvalidPrimitive {
                    reason: format!(
                        "loft profile {} must use finite vertex values",
                        profile.profile_id
                    ),
                });
            }
        }

        Ok(())
    }

    fn profile_centroids_mm(&self) -> Vec<[f64; 3]> {
        self.profiles
            .iter()
            .map(|profile| centroid_mm(&profile.vertices_mm))
            .collect()
    }

    fn transition_count(&self) -> usize {
        if self.closed {
            self.profiles.len()
        } else {
            self.profiles.len() - 1
        }
    }

    fn geometry_hash(
        &self,
        source_geometry_hashes: &[String],
        profile_centroids_mm: &[[f64; 3]],
    ) -> String {
        let sources_payload = source_geometry_hashes.join(",");
        let profile_payload = self
            .profiles
            .iter()
            .zip(profile_centroids_mm.iter())
            .map(|(profile, centroid)| {
                let vertices = profile
                    .vertices_mm
                    .iter()
                    .map(|point| format!("{:.6},{:.6},{:.6}", point[0], point[1], point[2]))
                    .collect::<Vec<_>>()
                    .join(";");
                format!(
                    "{}|cx={:.6}|cy={:.6}|cz={:.6}|{}",
                    profile.profile_id, centroid[0], centroid[1], centroid[2], vertices
                )
            })
            .collect::<Vec<_>>()
            .join("||");
        let payload = format!(
            "loft|feature={}|closed={}|sources={}|profiles={}",
            self.feature_id, self.closed, sources_payload, profile_payload
        );
        stable_hex_digest(payload.as_bytes())
    }
}

pub fn evaluate_loft_feature(
    op: &LoftFeatureOp,
    source_geometry_hashes: &[String],
) -> CadResult<LoftFeatureResult> {
    op.validate()?;
    if source_geometry_hashes.len() != op.profiles.len() {
        return Err(CadError::InvalidPrimitive {
            reason: format!(
                "loft source geometry hash count must match profile count ({} vs {})",
                source_geometry_hashes.len(),
                op.profiles.len()
            ),
        });
    }

    let profile_centroids_mm = op.profile_centroids_mm();
    let transition_count = op.transition_count();
    let vertices_per_profile = op.profiles[0].vertices_mm.len();
    let lateral_patch_count = transition_count * vertices_per_profile;
    let cap_count = if op.closed { 0 } else { 2 };
    let geometry_hash = op.geometry_hash(source_geometry_hashes, &profile_centroids_mm);

    Ok(LoftFeatureResult {
        feature_id: op.feature_id.clone(),
        closed: op.closed,
        profile_count: op.profiles.len(),
        vertices_per_profile,
        transition_count,
        lateral_patch_count,
        cap_count,
        profile_centroids_mm,
        geometry_hash,
    })
}

fn centroid_mm(vertices_mm: &[[f64; 3]]) -> [f64; 3] {
    let mut sum = [0.0_f64; 3];
    for point in vertices_mm {
        sum[0] += point[0];
        sum[1] += point[1];
        sum[2] += point[2];
    }
    let count = vertices_mm.len() as f64;
    [sum[0] / count, sum[1] / count, sum[2] / count]
}
