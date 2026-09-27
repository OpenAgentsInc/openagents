//! Destructible instance declarations in scene/zone data.
//!
//! The intent is to keep the format simple and deterministic. Each entry
//! references a mesh (by index or name) and supplies a transform. Local-space
//! AABBs are provided for the mesh to allow server-side world AABB assembly.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TransformDecl {
    pub translation: [f32; 3],
    /// Yaw in degrees around +Y.
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default = "unit3")]
    pub scale: [f32; 3],
}

#[inline]
fn unit3() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

#[derive(Debug, Clone, Deserialize)]
pub struct DestructibleDecl {
    /// Logical mesh id; resolved against mesh registry by server scene build.
    pub mesh_id: u32,
    /// Local-space AABB bounds for the mesh, used to compute world AABB.
    pub local_min: [f32; 3],
    pub local_max: [f32; 3],
    /// Instance transform in the world.
    pub transform: TransformDecl,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneDestructibles {
    pub destructibles: Vec<DestructibleDecl>,
}

impl SceneDestructibles {
    /// Parse a TOML string into a scene destructibles list.
    pub fn from_toml_str(s: &str) -> anyhow::Result<Self> {
        let v: SceneDestructibles = toml::from_str(s)?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_minimal_scene_toml() {
        let src = r#"
            [[destructibles]]
            mesh_id = 0
            local_min = [-1.0, -0.5, -1.0]
            local_max = [ 1.0,  0.5,  1.0]
            [destructibles.transform]
            translation = [10.0, 0.0, -3.0]
            yaw_deg = 90.0
            scale = [1.0, 1.0, 1.0]
        "#;
        let s = SceneDestructibles::from_toml_str(src).expect("parse");
        assert_eq!(s.destructibles.len(), 1);
        assert_eq!(s.destructibles[0].mesh_id, 0);
        assert!((s.destructibles[0].transform.yaw_deg - 90.0).abs() < 1e-5);
    }
}
