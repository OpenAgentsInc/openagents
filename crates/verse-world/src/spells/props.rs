//! Dynamic props: kinds, SRD size categories, reference masses, and materials.
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// SRD size categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
}
impl Size {
    /// Reference mass of a creature of this size for contact impulses, kg.
    pub fn creature_mass(self) -> f64 {
        match self {
            Self::Tiny => 4.,
            Self::Small => 20.,
            Self::Medium => 75.,
            Self::Large => 300.,
            Self::Huge => 1_500.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Material {
    Wood,
    Straw,
    Iron,
    Stone,
}
impl Material {
    /// Surface properties for the contact solver. Friction pairs combine by
    /// geometric mean, so a prop on the stone floor slides with
    /// `sqrt(prop * stone)`.
    pub fn physics(self) -> physics::Material {
        let (friction, restitution) = match self {
            Self::Wood => (0.5, 0.1),
            Self::Straw => (0.6, 0.05),
            Self::Iron => (0.4, 0.1),
            Self::Stone => (0.6, 0.05),
        };
        physics::Material {
            friction,
            torsional: 0.,
            restitution,
        }
    }
}

/// What a prop is, which selects its look and its reference values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKind {
    Crate,
    Barrel,
    TrainingDummy,
    Anvil,
    StoneBlock,
    /// A body a spell creates, such as a stone panel or a meteor.
    SpellBody,
    /// A rough boulder of the kind giants and siege engines hurl.
    Boulder,
    /// Loose, lightweight material: a sheet of paper or a mat of leaves.
    Sheet,
    /// A sheaf of loose paper, the lightest prop a wind carries off.
    Paper,
    /// An empty wicker basket.
    Basket,
}
impl PropKind {
    /// Pack model the renderer draws for this kind; `secured` props use a
    /// banded variant so they read differently on screen.
    pub fn model(self, secured: bool) -> &'static str {
        match self {
            Self::Crate if secured => "prop-crate-secured",
            Self::Crate => "prop-crate",
            Self::Barrel => "prop-barrel",
            Self::TrainingDummy => "prop-dummy",
            Self::Anvil => "prop-anvil",
            Self::StoneBlock | Self::SpellBody => "prop-stone",
            Self::Boulder => "prop-boulder",
            Self::Sheet => "prop-sheet",
            Self::Paper => "prop-paper",
            Self::Basket => "prop-basket",
        }
    }
}

/// Everything about a prop that does not change while it moves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropSpec {
    pub kind: PropKind,
    pub size: Size,
    /// Full edge lengths of the collision box, m.
    pub dimensions: DVec3,
    /// kg.
    pub mass: f64,
    pub material: Material,
    /// Welded to static geometry: spells and contacts never move it.
    pub secured: bool,
    pub flammable: bool,
    /// SRD object hit points, when a spell can damage it.
    pub hit_points: Option<i32>,
    /// Center of mass relative to the box center, m. A weighted base keeps a
    /// training dummy upright when it slides.
    pub center_of_mass: DVec3,
}

impl PropSpec {
    /// The reference prop of each kind: crate 20 kg (Small), barrel 60 kg
    /// (Small), training dummy 75 kg (Medium), anvil 250 kg (Small), and
    /// stone block 1,000 kg (Medium).
    pub fn reference(kind: PropKind) -> Self {
        let (size, dimensions, mass, material, flammable, center_of_mass) = match kind {
            PropKind::Crate => (
                Size::Small,
                DVec3::splat(0.6),
                20.,
                Material::Wood,
                true,
                DVec3::ZERO,
            ),
            PropKind::Barrel => (
                Size::Small,
                DVec3::new(0.6, 0.9, 0.6),
                60.,
                Material::Wood,
                true,
                DVec3::ZERO,
            ),
            PropKind::TrainingDummy => (
                Size::Medium,
                DVec3::new(0.5, 1.8, 0.5),
                75.,
                Material::Straw,
                true,
                DVec3::new(0., -0.55, 0.),
            ),
            PropKind::Anvil => (
                Size::Small,
                DVec3::new(0.7, 0.4, 0.35),
                250.,
                Material::Iron,
                false,
                DVec3::ZERO,
            ),
            // The lightest and thinnest prop the specification admits.
            PropKind::Paper => (
                Size::Tiny,
                DVec3::new(0.3, 0.05, 0.21),
                0.1,
                Material::Wood,
                true,
                DVec3::ZERO,
            ),
            PropKind::Basket => (
                Size::Tiny,
                DVec3::splat(0.4),
                1.,
                Material::Straw,
                true,
                DVec3::ZERO,
            ),
            PropKind::StoneBlock | PropKind::SpellBody => (
                Size::Medium,
                DVec3::splat(0.75),
                1_000.,
                Material::Stone,
                false,
                DVec3::ZERO,
            ),
            PropKind::Boulder => (
                Size::Small,
                DVec3::splat(0.9),
                150.,
                Material::Stone,
                false,
                DVec3::ZERO,
            ),
            PropKind::Sheet => (
                Size::Tiny,
                DVec3::new(0.6, 0.05, 0.45),
                0.5,
                Material::Straw,
                true,
                DVec3::ZERO,
            ),
        };
        Self {
            kind,
            size,
            dimensions,
            mass,
            material,
            secured: false,
            flammable,
            hit_points: None,
            center_of_mass,
        }
    }
    pub fn secured(mut self) -> Self {
        self.secured = true;
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.dimensions.is_finite()
            || self.dimensions.min_element() < 0.05
            || self.dimensions.max_element() > 20.
            || !self.mass.is_finite()
            || !(0.1..=1_000_000.).contains(&self.mass)
            || !self.center_of_mass.is_finite()
            || (self.center_of_mass.abs() - self.dimensions * 0.5)
                .max_element()
                .is_sign_positive()
            || self
                .hit_points
                .is_some_and(|hp| !(0..=10_000).contains(&hp))
        {
            return Err("Invalid prop specification".into());
        }
        Ok(())
    }
}

/// A prop in the spell world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    /// Shares the world's blocker identity space; never an actor's life.
    pub life: physics::queries::Life,
    pub name: String,
    pub spec: PropSpec,
    pub body: physics::BodyId,
    pub collider: physics::ColliderId,
    pub hit_points: Option<i32>,
    /// The cast that created it; the prop leaves when that spell ends.
    pub owner: Option<u64>,
    pub removed: bool,
}
impl Prop {
    pub fn query_key(&self) -> physics::queries::ColliderKey {
        physics::queries::ColliderKey {
            life: self.life,
            shape: 0,
        }
    }
}
