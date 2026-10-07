//! Independently loaded places, with closed host-supported rules and assets.
//! A zone selects presentation and simulation; it never supplies executable code.

use serde::{Deserialize, Serialize};

pub use verse_zone_crypt as crypt;
pub use verse_zone_everglade::zones::everglade;
pub use verse_zone_everglade::zones::everglade_pack;
pub use verse_zone_water as water;
pub mod gate;
pub use verse_zone_grove::zones::grove;
pub mod hud;
pub use verse_zone_lab as lab;
#[cfg(not(target_arch = "wasm32"))]
mod private_assets;
#[cfg(test)]
mod private_assets_tests;
pub use verse_zone_lagrange as lagrange;
#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod crypt_tests;
#[cfg(test)]
mod everglade_spell_tests;
#[cfg(test)]
mod everglade_tests;
#[cfg(test)]
mod grove_tests;
#[cfg(test)]
mod grove_tower_tests;
#[cfg(test)]
mod lab_tests;
pub mod meteor_stress;
pub mod operators;
mod runtime;
mod sight;
#[cfg(test)]
mod sight_tests;
#[cfg(test)]
mod studio_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod town_tests;

pub(crate) use everglade::Everglade;
pub use gate::Gate;
pub(crate) use lab::Lab;
pub use lab::{Kind as LabScenario, KnobView, Snapshot as LabSnapshot};
pub(crate) use lagrange::Lagrange;

/// Read on demand by native Settings, not repeated in each frame packet.
pub const CREDITS: &str = concat!(
    "Lagrange 1: Earth imagery from NASA Earth Observatory / NASA GSFC (Blue Marble Next Generation), ",
    "bathymetry from GEBCO; Moon maps from NASA SVS CGI Moon Kit (LRO LROC and LOLA); Milky Way from ",
    "NASA SVS Deep Star Maps 2020; stars from the Yale Bright Star Catalogue (Hoffleit and Warren, ",
    "NASA ADC / CDS). Sources: https://github.com/OpenAgentsInc/openagents/tree/main/crates/verse/assets/lagrange\n\n",
    "Spells and rules: this work includes material from the System Reference Document 5.2.1 ",
    "(\"SRD 5.2.1\") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. ",
    "The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, ",
    "available at https://creativecommons.org/licenses/by/4.0/legalcode.",
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneId {
    #[default]
    Plaza,
    Lagrange1,
    PhysicsLab,
    Everglade,
    /// The druid training field on Everglade's pack
    /// ([`grove`], `docs/verse/druid-demo.md`).
    Grove,
    /// The candlelit crypt lab ([`crypt`]), walked as Everglade's
    /// character.
    Crypt,
    MeteorStressTest,
    /// The Water Lab's cove ([`water`]), walked as Everglade's character.
    WaterLab,
}
impl ZoneId {
    pub const ALL: [Self; 8] = [
        Self::Plaza,
        Self::Lagrange1,
        Self::PhysicsLab,
        Self::Everglade,
        Self::Grove,
        Self::Crypt,
        Self::MeteorStressTest,
        Self::WaterLab,
    ];

    /// The zone a command line names, by world identifier or label
    /// (`everglade`, `lagrange-1`, `physics-lab`, `plaza`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let wanted = name.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        Self::ALL.into_iter().find(|zone| {
            zone.world_id() == wanted
                || zone.label().to_ascii_lowercase().replace(' ', "-") == wanted
                || zone.world_id().trim_start_matches("verse-") == wanted
                || zone.world_id().trim_end_matches("-v1") == wanted
        })
    }

    pub const fn world_id(self) -> &'static str {
        match self {
            Self::Plaza => "verse-plaza",
            Self::Lagrange1 => "verse-lagrange-1",
            Self::PhysicsLab => "physics-lab-v1",
            Self::Everglade => "verse-everglade",
            Self::Grove => "verse-grove",
            Self::Crypt => "verse-crypt",
            Self::MeteorStressTest => "verse-meteor-stress-test",
            Self::WaterLab => "verse-water-lab",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plaza => "Amber plaza",
            Self::Lagrange1 => "Lagrange 1",
            Self::PhysicsLab => "Physics Lab",
            Self::Everglade => "Everglade",
            Self::Grove => "Grove",
            Self::Crypt => "Crypt",
            Self::MeteorStressTest => "Meteor Stress Test",
            Self::WaterLab => "Water Lab",
        }
    }
    pub const fn half_extent(self) -> f32 {
        match self {
            Self::Plaza => crate::world::HALF,
            Self::Lagrange1 => 150.0,
            Self::PhysicsLab => lab::HALF_EXTENT,
            Self::Everglade | Self::Grove | Self::MeteorStressTest => everglade::HALF_EXTENT,
            Self::Crypt => crypt::HALF_EXTENT,
            Self::WaterLab => water::HALF_EXTENT,
        }
    }
    /// The zone's primary portal: the plaza's Lagrange 1 arch, or a zone's
    /// return. Everglade has none.
    pub fn portal(self) -> Option<glam::Vec3> {
        self.portals().first().map(|&(_, at)| at)
    }
    /// Every portal in this zone with its destination.
    pub fn portals(self) -> Vec<(ZoneId, glam::Vec3)> {
        match self {
            Self::Plaza => {
                let mut portals = vec![
                    (Self::Lagrange1, glam::Vec3::new(12.0, 0.0, 12.0)),
                    (Self::PhysicsLab, glam::Vec3::new(0.0, 0.0, -22.0)),
                    (Self::Everglade, glam::Vec3::new(-24.0, 0.0, -24.0)),
                    (Self::WaterLab, WATER_ARCH),
                ];
                // The crypt's arch, opposite Everglade's, in a build that
                // carries its models.
                if crypt::EMBEDDED {
                    portals.push((Self::Crypt, CRYPT_ARCH));
                }
                portals
            }
            Self::Lagrange1 => vec![(Self::Plaza, lagrange::RETURN_PORTAL)],
            Self::PhysicsLab => vec![(Self::Plaza, lab::RETURN_PORTAL)],
            // Everglade has no arch back; its zone panel's return control
            // leaves.
            Self::Everglade => Vec::new(),
            Self::Grove => vec![(Self::Plaza, grove::RETURN_PORTAL)],
            // The heavy door is the way out; it draws no arch.
            Self::Crypt => vec![(Self::Plaza, crypt::DOOR)],
            Self::MeteorStressTest => vec![(Self::Plaza, meteor_stress::RETURN_PORTAL)],
            // The lantern at the head of the beach is the way out; it
            // draws no arch.
            Self::WaterLab => vec![(Self::Plaza, water::EXIT)],
        }
    }
    /// Short arch lettering for a destination.
    pub(crate) const fn sign(self) -> &'static str {
        match self {
            Self::Plaza => "PLAZA",
            Self::Lagrange1 => "LAGRANGE 1",
            Self::PhysicsLab => "PHYSICS LAB",
            Self::Everglade => "EVERGLADE",
            Self::Grove => "GROVE",
            Self::Crypt => "CRYPT",
            Self::MeteorStressTest => "METEOR STRESS TEST",
            Self::WaterLab => "WATER LAB",
        }
    }
}

/// The plaza's arch to the Water Lab, opposite Lagrange 1's.
pub const WATER_ARCH: glam::Vec3 = glam::Vec3::new(-12.0, 0.0, 12.0);

/// The plaza's arch to the crypt, opposite Everglade's.
pub const CRYPT_ARCH: glam::Vec3 = glam::Vec3::new(24.0, 0.0, -24.0);

pub use verse_core::zone::Atmosphere;
pub fn atmosphere(zone: ZoneId) -> Atmosphere {
    match zone {
        ZoneId::Plaza => Atmosphere {
            color: crate::palette::field(),
            fog_start: crate::render::FOG_START,
            fog_end: crate::render::FOG_END,
            height_fog: None,
        },
        // Vacuum: no scattering. Fog only fades the edge of the 2 km sky shell.
        ZoneId::Lagrange1 => Atmosphere {
            color: [0.0, 0.0, 0.004],
            fog_start: 1_000.0,
            fog_end: 2_000.0,
            height_fog: None,
        },
        // A dark blueprint hall; fog only softens the far floor grid.
        ZoneId::PhysicsLab => Atmosphere {
            color: [0.006, 0.01, 0.018],
            fog_start: 30.0,
            fog_end: 90.0,
            height_fog: None,
        },
        ZoneId::Everglade => everglade::ATMOSPHERE,
        // The Grove's dusk haze, glowing toward the low Sun.
        ZoneId::Grove | ZoneId::MeteorStressTest => grove::light::ATMOSPHERE,
        // The cove's warm sea haze.
        ZoneId::WaterLab => Atmosphere {
            color: water::sea::HAZE,
            fog_start: water::sea::FOG_START,
            fog_end: water::sea::FOG_END,
            height_fog: Some(water::sea::HEIGHT_FOG),
        },
        // The candlelit hall's near-black air and low fog.
        ZoneId::Crypt => Atmosphere {
            color: crypt::FIELD,
            fog_start: crypt::FOG_START,
            fog_end: crypt::FOG_END,
            height_fog: Some(crypt::HEIGHT_FOG),
        },
    }
}

pub use verse_core::zone::Intent;
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
    /// Lagrange 1 physics and construction state.
    pub station: Option<verse_lagrange::Snapshot>,
    /// Physics Lab scenario, knobs, and readouts.
    pub lab: Option<lab::Snapshot>,
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
            station: None,
            lab: None,
            caption: String::new(),
        }
    }
}

pub(crate) struct State {
    everglade_loader: Option<everglade_pack::Loader>,
    loading: LoadState,
    progress: f32,
    error: Option<String>,
    lagrange: Option<Lagrange>,
    lab: Option<Lab>,
    everglade: Option<Everglade>,
    /// The Grove's training field. The Grove also fills `everglade`, whose
    /// movement, spells, and character it walks with.
    grove: Option<grove::Grove>,
    /// The crypt's light and effects. The crypt also fills `everglade`,
    /// whose movement, spells, and character it walks with.
    crypt: Option<crypt::Crypt>,
    /// The Water Lab's sea, floats, and spells. The lab also fills
    /// `everglade`, whose movement and character it walks with.
    water: Option<Box<water::WaterLab>>,
    /// Open Everglade as the demolition yard (`verse --demolition`).
    demolition: bool,
    /// Meteor Swarm and the sledgehammer on Everglade's hotbar, a local test
    /// of destruction (`verse --dev-destruction`). Only a `dev-destruction`
    /// build can set it ([`crate::runtime::WorldRuntime::set_dev_destruction`]).
    dev_destruction: bool,
    /// The Agent Studio Everglade draws. It keeps its source across visits
    /// and observes only while the player is in Everglade.
    studio: everglade::studio::Studio,
    /// What Everglade's caption leads with, such as that no coding agent
    /// can sign in ([`crate::runtime::WorldRuntime::set_studio_notice`]).
    studio_notice: Option<String>,
    /// Whether this window's own host answers for the workshop agent, so
    /// she talks to this player: only her owner's windows do.
    workshop_owner: bool,
    destination: ZoneId,
    plaza_pose: Option<(glam::Vec3, f32)>,
    elapsed: f32,
    /// Seconds before a walk-in portal admits another crossing.
    gate_cooldown: f32,
    /// Whether the Grid shows its portal; see [`gate::GRID_PORTAL_OPEN`].
    grid_portal: bool,
    /// The pinned chamber the Grid's RITUAL arch joins, when there is one.
    ritual: Option<std::path::PathBuf>,
    /// A RITUAL crossing the application has not taken yet.
    ritual_crossed: bool,
    /// The tops of the world's blockers the camera sees ([`sight`]), and the
    /// zone revision and blocker count they were measured for.
    sight_tops: (u64, usize, Vec<f32>),
    /// Verse's home, where the owner's private placements and cache live,
    /// when the desktop configured it, and Everglade's background load of
    /// them (`private_assets`).
    #[cfg(not(target_arch = "wasm32"))]
    private_home: Option<std::path::PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    private_loader: Option<everglade_pack::private_assets::PrivateLoader>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            everglade_loader: None,
            loading: LoadState::Idle,
            progress: 0.0,
            error: None,
            lagrange: None,
            lab: None,
            everglade: None,
            grove: None,
            crypt: None,
            water: None,
            demolition: false,
            dev_destruction: false,
            studio: everglade::studio::Studio::default(),
            studio_notice: None,
            workshop_owner: false,
            destination: ZoneId::Everglade,
            plaza_pose: None,
            elapsed: 0.0,
            gate_cooldown: 0.0,
            grid_portal: gate::GRID_PORTAL_OPEN,
            ritual: None,
            ritual_crossed: false,
            sight_tops: (u64::MAX, 0, Vec::new()),
            #[cfg(not(target_arch = "wasm32"))]
            private_home: None,
            #[cfg(not(target_arch = "wasm32"))]
            private_loader: None,
        }
    }
}

/// A visible arch in the current zone. Only the plaza geometry is amber-bound.
pub fn portal_mesh(zone: ZoneId, elapsed: f32) -> crate::mesh::Mesh {
    let mut mesh = crate::mesh::Mesh::default();
    if matches!(zone, ZoneId::Crypt | ZoneId::WaterLab) {
        return mesh;
    }
    for (destination, at) in zone.portals() {
        arch(&mut mesh, zone, destination.sign(), at, elapsed);
    }
    mesh
}

pub(crate) fn arch(
    mesh: &mut crate::mesh::Mesh,
    zone: ZoneId,
    sign: &str,
    at: glam::Vec3,
    elapsed: f32,
) {
    use crate::mesh::Vertex;
    use coder_ui::theme::Intensity;
    use glam::{Mat4, Vec3};
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
        mesh,
        sign,
        at + Vec3::new(0.0, 4.9, -0.3),
        if sign.len() > 6 { 0.3 } else { 0.45 },
        Intensity::Full,
    );
    let color = match zone {
        ZoneId::Plaza => crate::palette::amber(Intensity::Half),
        ZoneId::Lagrange1 => [0.35, 0.7, 1.0],
        ZoneId::PhysicsLab => [0.3, 0.85, 1.0],
        ZoneId::Everglade | ZoneId::Grove | ZoneId::MeteorStressTest => [0.95, 0.85, 0.4],
        ZoneId::Crypt => [1.0, 0.56, 0.24],
        ZoneId::WaterLab => [0.35, 0.8, 1.0],
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
}
