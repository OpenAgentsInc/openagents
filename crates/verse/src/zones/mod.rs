//! Independently loaded places, with closed host-supported rules and assets.
//! A zone selects presentation and simulation; it never supplies executable code.

use serde::{Deserialize, Serialize};

pub mod assets;
mod forest;
pub mod hud;
pub mod rules;
mod runtime;
#[cfg(test)]
mod tests;

pub(crate) use forest::Forest;

/// Read on demand by native Settings, not repeated in each frame packet.
pub const CREDITS: &str = concat!(
    "Atlantis forest: a new Rust implementation using selected Ruins of Atlantis artwork.\n",
    "Geometry and original animation poses are baked with sampled colors and leaf cutouts.\n",
    "The original wizard/zombie upstream authors and separate asset licenses were not identified in the source.\n",
    "Source and modifications: https://github.com/OpenAgentsInc/openagents/tree/main/assets/verse/forest\n\n",
    include_str!("../../../../docs/verse/SRD-5.1-NOTICE.md"),
    "\n\nRetained source project notice (its revised SRD is distinct from this app's SRD 5.1 rules):\n",
    include_str!("../../../../assets/verse/forest/SOURCE_NOTICE"),
    "\n\nSource repository license:\n",
    include_str!("../../../../assets/verse/forest/SOURCE_LICENSE"),
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneId {
    #[default]
    Plaza,
    Forest,
}
impl ZoneId {
    pub const fn world_id(self) -> &'static str {
        match self {
            Self::Plaza => "verse-plaza",
            Self::Forest => "atlantis-forest-v1",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plaza => "Amber plaza",
            Self::Forest => "Atlantis forest",
        }
    }
    pub const fn half_extent(self) -> f32 {
        match self {
            Self::Plaza => crate::world::HALF,
            Self::Forest => 64.0,
        }
    }
    pub fn portal(self) -> glam::Vec3 {
        match self {
            Self::Plaza => glam::Vec3::new(-12.0, 0.0, 12.0),
            Self::Forest => glam::Vec3::new(0.0, 0.0, 18.0),
        }
    }
}

/// Linear-light scene values; the Coder application UI keeps its own palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub color: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
}
impl Atmosphere {
    pub fn validate(self) -> Result<Self, String> {
        if self
            .color
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !self.fog_start.is_finite()
            || !self.fog_end.is_finite()
            || !(0.0..=1_000.0).contains(&self.fog_start)
            || self.fog_end <= self.fog_start
            || self.fog_end > 2_000.0
        {
            return Err("Zone atmosphere exceeds its bounds".into());
        }
        Ok(self)
    }
}
pub fn atmosphere(zone: ZoneId) -> Atmosphere {
    match zone {
        ZoneId::Plaza => Atmosphere {
            color: crate::palette::field(),
            fog_start: crate::render::FOG_START,
            fog_end: crate::render::FOG_END,
        },
        ZoneId::Forest => Atmosphere {
            color: [0.045, 0.092, 0.079],
            fog_start: 24.0,
            fog_end: 82.0,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Enter,
    Return,
    Cancel,
    Retry,
    StartEncounter,
    Cast,
    EndTurn,
    ResetEncounter,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadState {
    #[default]
    Idle,
    Loading,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
pub struct Control {
    pub id: &'static str,
    pub label: String,
    pub action: Intent,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PortalProjection {
    pub near: bool,
    pub visible: bool,
    pub screen_x: f32,
    pub screen_y: f32,
    pub distance: f32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub id: ZoneId,
    pub label: &'static str,
    pub state: LoadState,
    /// Verified transfer progress, from zero to one. Decode follows transfer.
    pub progress: f32,
    pub error: Option<String>,
    pub portal: PortalProjection,
    pub controls: Vec<Control>,
    pub encounter: Option<rules::EncounterSnapshot>,
    pub caption: String,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            id: ZoneId::Plaza,
            label: ZoneId::Plaza.label(),
            state: LoadState::Idle,
            progress: 0.0,
            error: None,
            portal: PortalProjection {
                near: false,
                visible: false,
                screen_x: 0.5,
                screen_y: 0.5,
                distance: 0.0,
            },
            controls: vec![],
            encounter: None,
            caption: String::new(),
        }
    }
}

pub(crate) struct State {
    loader: Option<assets::Loader>,
    loading: LoadState,
    progress: f32,
    error: Option<String>,
    forest: Option<Forest>,
    plaza_pose: Option<(glam::Vec3, f32)>,
    elapsed: f32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            loader: None,
            loading: LoadState::Idle,
            progress: 0.0,
            error: None,
            forest: None,
            plaza_pose: None,
            elapsed: 0.0,
        }
    }
}

/// The first scene profile accepts reviewed assets and a closed ruleset only.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub world: String,
    pub ruleset: String,
    pub physics: String,
    pub asset_sha256: String,
    pub asset_bytes: u64,
}
impl Manifest {
    pub fn forest() -> Result<Self, String> {
        let manifest: Self =
            serde_json::from_str(include_str!("../../../../assets/verse/forest/zone.json"))
                .map_err(|_| "The installed forest definition is invalid".to_owned())?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "verse.zone.v1"
            || self.world != ZoneId::Forest.world_id()
            || self.ruleset != rules::RULESET_ID
            || self.physics != "verse.walk-flat.v1"
            || self.asset_sha256 != assets::PACK_SHA256
            || self.asset_bytes != assets::PACK_BYTES
        {
            return Err("Zone definition or ruleset is not supported by this host".into());
        }
        Ok(())
    }
}

/// A visible arch in the current zone. Only the plaza geometry is amber-bound.
pub fn portal_mesh(zone: ZoneId, elapsed: f32) -> crate::mesh::Mesh {
    use crate::mesh::{Mesh, Vertex};
    use coder_ui::theme::Intensity;
    use glam::{Mat4, Vec3};
    let at = zone.portal();
    let mut mesh = Mesh::default();
    for x in [-1.9, 1.9] {
        mesh.cube(
            Mat4::from_translation(at + Vec3::new(x, 2.25, 0.0))
                * Mat4::from_scale(Vec3::new(0.35, 4.5, 0.5)),
            Intensity::Full,
        );
    }
    mesh.cube(
        Mat4::from_translation(at + Vec3::new(0.0, 4.5, 0.0))
            * Mat4::from_scale(Vec3::new(4.2, 0.35, 0.5)),
        Intensity::Full,
    );
    crate::doors::scene_label(
        &mut mesh,
        if zone == ZoneId::Plaza {
            "ATLANTIS"
        } else {
            "PLAZA"
        },
        at + Vec3::new(0.0, 4.9, -0.3),
        0.45,
        Intensity::Full,
    );
    let color = if zone == ZoneId::Plaza {
        crate::palette::amber(Intensity::Half)
    } else {
        [0.13, 0.55, 0.34]
    };
    // Broken concentric arcs leave the destination visible through the opening.
    for ring in 0..3 {
        for i in 0..28 {
            if (i + ring * 3) % 7 == 0 {
                continue;
            }
            let angle =
                i as f32 / 28.0 * std::f32::consts::TAU + elapsed * (0.25 + ring as f32 * 0.1);
            let point = |a: f32| {
                at + Vec3::new(
                    a.cos() * (1.05 + ring as f32 * 0.18),
                    2.25 + a.sin() * (1.6 + ring as f32 * 0.1),
                    -0.3,
                )
            };
            for p in [point(angle), point(angle + 0.14)] {
                mesh.lines.push(Vertex {
                    pos: p.to_array(),
                    color,
                    fog: 1.0,
                });
            }
        }
    }
    mesh
}
