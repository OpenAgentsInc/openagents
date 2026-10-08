//! The generated models' collision and doorways (`docs/verse/blender-pipeline.md`).
//!
//! `scripts/blender` builds the village buildings and the landmarks as whole
//! models, admitted into the pack's `generated` set. Each is drawn as one
//! placement that does not block by its bounds; instead its ground-level
//! boxes block walking, its gabled roofs are surfaces to land on, and its
//! front door has a step a walk leads to. The boxes come from each
//! building's `<name>.footprint.json` beside its glb in
//! `assets/verse/generated/buildings/`, and the roofs from its tiles'
//! extent.
//!
//! A model's frame is the glTF one: 1 unit = 1 m, +y up, +z out of the
//! front door, and the origin on the ground at the front wall's center (a
//! landmark's at its base's center).

use super::{Collision, Placement, height};
use crate::controller::Footprint;
use glam::{Quat, Vec3};
use verse_world::social::solids::Roof;

/// One gabled roof of a generated model, in its frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GableRoof {
    /// The roof's center, x and z, m.
    pub center: [f32; 2],
    /// Whether the slopes fall along z, with the ridge along x; otherwise
    /// they fall along x and the ridge runs along z.
    pub slopes_z: bool,
    /// Half extents across the ridge and along it, m.
    pub half: [f32; 2],
    /// Heights of the eaves and the ridge above the model's origin, m.
    pub eave: f32,
    pub ridge: f32,
}

/// A generated model's collision and front door, in its frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// The pack's model, such as `generated/library`.
    pub name: &'static str,
    /// Boxes that block walking: min x, max x, min z, max z, and top, m.
    pub blocks: &'static [[f32; 5]],
    pub roofs: &'static [GableRoof],
    /// Where its walk ends, just outside the front door, x and z, m.
    pub front: [f32; 2],
    /// A point inside past an open doorway, when the door opens (the
    /// market hall's arcade); the others' doors are closed.
    pub inside: Option<[f32; 2]>,
}

/// Solid boxes a carved model's triangles leave out, added to its
/// collision columns ([`super::super::demolition::carve::columns`]) and
/// breaking with the blocks they fall in: each model's boxes in its
/// frame, min x, max x, min z, max z, bottom, and top, m.
pub const PATCHES: [(&str, &[[f32; 6]]); 2] = [
    // The owner's house: the podium's top under the facade, which its
    // walls cover but its open doorway leaves as a slot a walker falls
    // into, 0.4 m deep and 1.3 m down from the portico.
    (
        "generated/greco_house",
        &[[-10.0, 10.0, -12.05, -11.55, 0.0, 1.6]],
    ),
    // The Civic Hall: the stepped walls beside its stair, solid up past
    // the podium's top, so no walker steps down onto them into the
    // pocket between a bowl and the step behind it.
    (
        "generated/civic_hall",
        &[
            [-9.64, -7.2, -4.84, 0.04, 0.0, 2.4],
            [7.2, 9.64, -4.84, 0.04, 0.0, 2.4],
        ],
    ),
];

/// [`PATCHES`]' boxes for model `name`.
#[must_use]
pub fn patches(name: &str) -> &'static [[f32; 6]] {
    PATCHES
        .iter()
        .find(|(model, _)| *model == name)
        .map_or(&[], |(_, boxes)| boxes)
}

const fn gable(
    center: [f32; 2],
    slopes_z: bool,
    half: [f32; 2],
    eave: f32,
    ridge: f32,
) -> GableRoof {
    GableRoof {
        center,
        slopes_z,
        half,
        eave,
        ridge,
    }
}

pub const LIBRARY: Model = Model {
    name: "generated/library",
    blocks: &[
        [-2.1, 2.1, -0.1, 2.1, 7.72],
        [-6.2, 6.2, -10.1, 0.1, 7.72],
        [6.0, 7.3, -6.0, -2.0, 4.2],
    ],
    roofs: &[gable([0.0, -5.0], true, [6.2, 7.0], 7.2, 14.5)],
    front: [0.0, 4.6],
    inside: None,
};

pub const COTTAGE_TOWER: Model = Model {
    name: "generated/cottage_tower",
    blocks: &[[-3.1, 3.1, -6.1, 0.1, 3.12], [-5.0, -1.6, -2.3, 1.1, 9.0]],
    roofs: &[gable([0.0, -3.0], false, [4.1, 3.9], 2.5, 8.0)],
    front: [0.0, 0.9],
    inside: None,
};

/// The observatory on its hill: a drum tower with a door at its front.
pub const OBSERVATORY: Model = Model {
    name: "generated/observatory",
    blocks: &[[-2.4, 2.4, -2.6, 2.4, 5.0]],
    roofs: &[],
    front: [0.0, 3.4],
    inside: None,
};

/// The Fountain Plaza's fountain: the medieval kit's basin. Its blockers
/// match the kit piece's box (`kit/fountain`, about 6.2 m across and 2.1 m
/// tall), so a walker turns at the rim the same way the drawn fountain does.
/// The stand is past the rim, clear of the walker's radius; the old
/// generated basin's stand at 3.4 m now sits inside this wider box.
pub const FOUNTAIN: Model = Model {
    name: "kit/fountain",
    blocks: &[[-3.09, 3.09, -3.09, 3.09, 2.14]],
    roofs: &[],
    front: [0.0, 4.1],
    inside: None,
};

/// The bandshell on the commons: a raised stage under a shell, with its
/// steps at the front.
pub const BANDSHELL: Model = Model {
    name: "generated/bandshell",
    blocks: &[[-4.5, 4.5, -3.39, 1.0, 0.9], [-4.8, 4.8, -3.39, -2.6, 5.7]],
    roofs: &[],
    front: [0.0, 3.9],
    inside: None,
};

/// The Lantern Quarter's Music Hall: an octagon of tall arched windows
/// under a tiled cone, blocked as three crossing boxes.
pub const MUSIC_HALL: Model = Model {
    name: "generated/music_hall",
    blocks: &[
        [-5.55, 5.55, -7.85, -3.25, 5.18],
        [-2.3, 2.3, -11.11, 0.0, 5.18],
        [-4.35, 4.35, -9.91, -1.2, 5.18],
    ],
    roofs: &[],
    front: [0.0, 1.9],
    inside: None,
};

/// The boathouse: its arch faces the water and its door is on its west
/// side.
pub const BOATHOUSE: Model = Model {
    name: "generated/boathouse",
    blocks: &[[-3.2, 3.2, -8.2, 0.2, 4.09]],
    roofs: &[gable([0.0, -4.0], false, [3.55, 4.55], 3.62, 7.64)],
    front: [-4.0, -3.0],
    inside: None,
};

/// A Boardwalk Café; its deck in front does not block.
pub const BOARDWALK_CAFE: Model = Model {
    name: "generated/boardwalk_cafe",
    blocks: &[[-4.1, 4.1, -6.1, 0.1, 3.27]],
    roofs: &[gable([0.0, -3.0], true, [3.7, 4.3], 3.27, 8.11)],
    front: [1.0, 1.6],
    inside: None,
};

/// The smithy and its open forge on its east side (+x). The stand is past
/// the front wall and clear of the walker's radius: the lot places the
/// model's origin on the wall line, so a stand at 0.9 sits inside the wall.
pub const SMITHY: Model = Model {
    name: "generated/smithy",
    blocks: &[[-4.1, 4.1, -8.1, 0.1, 3.12], [4.0, 7.5, -7.8, -0.2, 2.4]],
    roofs: &[gable([0.0, -4.0], false, [4.7, 4.3], 3.12, 9.03)],
    front: [-1.0, 1.6],
    inside: None,
};

/// The windmill's tower; its sails turn high over the ground in front.
pub const WINDMILL: Model = Model {
    name: "generated/windmill",
    blocks: &[[-3.45, 3.45, -6.01, 0.1, 9.0]],
    roofs: &[],
    front: [0.0, 1.0],
    inside: None,
};

pub const GREENHOUSE: Model = Model {
    name: "generated/greenhouse",
    blocks: &[[-2.25, 2.25, -8.05, 0.1, 2.4]],
    roofs: &[gable([0.0, -4.0], false, [2.2, 4.0], 2.4, 3.6)],
    front: [0.0, 0.9],
    inside: None,
};

/// The clock tower at the front and the hall behind it.
pub const CLOCK_TOWER: Model = Model {
    name: "generated/clock_tower",
    blocks: &[[-2.4, 2.4, -4.6, 0.2, 15.1], [-3.1, 3.1, -12.5, -4.4, 4.16]],
    roofs: &[gable([0.0, -8.72], false, [3.6, 4.3], 4.16, 8.99)],
    front: [0.0, 1.6],
    inside: None,
};

/// The lookout tower: four legs a walker passes between, under the
/// platform.
pub const LOOKOUT: Model = Model {
    name: "generated/lookout",
    blocks: &[
        [-1.55, -1.05, -0.45, 0.05, 7.0],
        [-1.55, -1.05, -3.05, -2.55, 7.0],
        [1.05, 1.55, -0.45, 0.05, 7.0],
        [1.05, 1.55, -3.05, -2.55, 7.0],
    ],
    roofs: &[],
    front: [0.0, 1.0],
    inside: Some([0.0, -1.5]),
};

/// A log cabin with its chimney and porch posts.
pub const LOG_CABIN: Model = Model {
    name: "generated/log_cabin",
    blocks: &[
        [-3.2, 3.2, -5.2, 0.2, 2.67],
        [-4.05, -3.05, -3.1, -1.9, 2.4],
        [-2.92, -2.68, 1.48, 1.72, 2.3],
        [2.68, 2.92, 1.48, 1.72, 2.3],
    ],
    roofs: &[gable([0.0, -2.5], false, [3.6, 3.1], 2.23, 4.87)],
    front: [-1.1, 1.0],
    inside: None,
};

/// The open gazebo: eight posts round a floor, open at the front.
pub const GAZEBO: Model = Model {
    name: "generated/gazebo",
    blocks: &[
        [0.88, 1.11, -0.42, -0.18, 2.6],
        [2.28, 2.52, -1.83, -1.59, 2.6],
        [2.28, 2.52, -3.82, -3.58, 2.6],
        [0.88, 1.11, -5.22, -4.98, 2.6],
        [-1.11, -0.88, -5.22, -4.98, 2.6],
        [-2.52, -2.28, -3.82, -3.58, 2.6],
        [-2.52, -2.28, -1.83, -1.59, 2.6],
        [-1.11, -0.88, -0.42, -0.18, 2.6],
    ],
    roofs: &[],
    front: [0.0, 0.8],
    inside: Some([0.0, -2.7]),
};

/// The thatched farmhouse; its thatch reaches well past the walls.
pub const FARMHOUSE: Model = Model {
    name: "generated/farmhouse",
    blocks: &[[-5.1, 5.1, -6.1, 0.1, 3.12]],
    roofs: &[gable([0.0, -3.0], true, [3.8, 5.8], 2.7, 6.62)],
    front: [0.0, 0.9],
    inside: None,
};

pub const COTTAGE_THATCH: Model = Model {
    name: "generated/cottage_thatch",
    blocks: &[[-3.1, 3.1, -5.1, 0.1, 3.12]],
    roofs: &[gable([0.0, -2.5], true, [3.3, 3.8], 2.7, 6.02)],
    front: [-1.0, 0.9],
    inside: None,
};

/// The gambrel barn. Its landing surface is the shallow upper roof between
/// the knees, and a block under the steep lower slopes stops a lander near
/// their surface.
pub const GAMBREL_BARN: Model = Model {
    name: "generated/gambrel_barn",
    blocks: &[[-5.2, 5.2, -12.2, 0.2, 3.6], [-4.4, 4.4, -12.2, 0.2, 4.9]],
    roofs: &[gable([0.0, -6.0], false, [3.3, 6.35], 5.9, 7.1)],
    front: [2.6, 1.0],
    inside: None,
};

/// The gambrel house, its gable to the street. Its landing surface is the
/// shallow upper roof between the knees; its porch blocks low.
pub const GAMBREL_HOUSE: Model = Model {
    name: "generated/gambrel_house",
    blocks: &[
        [-4.12, 4.12, -8.12, 0.12, 3.4],
        [-1.4, 1.4, 0.0, 1.6, 0.6],
        [-3.4, 3.4, -8.12, 0.12, 4.9],
    ],
    roofs: &[gable([0.0, -4.0], false, [2.5, 4.45], 5.8, 7.0)],
    front: [0.0, 2.4],
    inside: None,
};

/// The stone cottage under its hipped roof, with its chimney stack on its
/// east side (+x).
pub const STONE_COTTAGE: Model = Model {
    name: "generated/stone_cottage",
    blocks: &[[-4.1, 4.1, -7.1, 0.1, 3.1], [4.0, 4.7, -4.0, -2.4, 5.55]],
    roofs: &[gable([0.0, -3.5], true, [4.05, 1.1], 3.15, 6.15)],
    front: [-0.4, 0.9],
    inside: None,
};

/// The brownstone: a flat roof behind its cornice, and its stoop, which
/// climbs 3 m out from the front wall, blocking low.
pub const BROWNSTONE: Model = Model {
    name: "generated/brownstone",
    blocks: &[[-4.1, 4.1, -8.1, 0.1, 10.7], [1.1, 3.3, 0.0, 3.0, 1.4]],
    roofs: &[gable([0.0, -4.0], true, [3.8, 3.8], 10.82, 10.83)],
    front: [2.2, 3.6],
    inside: None,
};

/// A narrow row house of three storeys under a front gable, for Brownstone
/// Row's terraces, two to a place.
pub const NARROW_HOUSE: Model = Model {
    name: "generated/narrow_house",
    blocks: &[[-2.05, 2.05, -8.05, 0.05, 8.85]],
    roofs: &[gable([0.0, -3.65], false, [2.45, 4.75], 8.87, 12.47)],
    front: [0.85, 1.1],
    inside: None,
};

/// The eighth round's wayside chapel (`scripts/blender/town_houses.py`)
/// beside the north trail: stone walls with buttresses down each side, a
/// steep roof, and a bell cote over its front gable.
pub const CHAPEL: Model = Model {
    name: "generated/chapel",
    blocks: &[
        [-2.95, 2.95, -9.15, 0.15, 4.2],
        [-3.25, 3.25, -8.9, -0.1, 2.6],
        [-1.0, 1.0, 0.0, 0.6, 0.4],
    ],
    roofs: &[gable([0.0, -4.5], false, [3.15, 4.85], 4.2, 8.0)],
    front: [0.0, 1.2],
    inside: None,
};

/// One generated model placed in the town.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub name: &'static str,
    pub model: &'static Model,
    /// The model's origin, x and z, m.
    pub at: [f32; 2],
    /// As the controller's yaw: the model's +z faces
    /// `controller::forward(yaw)`.
    pub yaw: f32,
    pub scale: f32,
}

impl Instance {
    pub const fn new(name: &'static str, model: &'static Model, at: [f32; 2], yaw: f32) -> Self {
        Self {
            name,
            model,
            at,
            yaw,
            scale: 1.0,
        }
    }

    /// A point of the model's frame on the ground, x and z, m.
    #[must_use]
    pub fn world(&self, local: [f32; 2]) -> [f32; 2] {
        let p = Quat::from_rotation_y(self.yaw)
            * Vec3::new(local[0] * self.scale, 0.0, local[1] * self.scale);
        [self.at[0] + p.x, self.at[1] + p.z]
    }

    /// The model's drawn placement, which blocks nothing by its bounds.
    #[must_use]
    pub fn placement(&self) -> Placement {
        Placement::new(self.model.name, self.at, self.yaw, Collision::None).scale(self.scale)
    }

    fn base(&self) -> f32 {
        height(self.at[0], self.at[1])
    }

    /// The boxes that block walking, with their tops, m.
    #[must_use]
    pub fn blocks(&self) -> Vec<(Footprint, f32)> {
        self.model
            .blocks
            .iter()
            .map(|&[x0, x1, z0, z1, top]| {
                let corners = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]].map(|c| self.world(c));
                let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
                for [x, z] in corners {
                    min = [min[0].min(x), min[1].min(z)];
                    max = [max[0].max(x), max[1].max(z)];
                }
                (Footprint { min, max }, self.base() + top * self.scale)
            })
            .collect()
    }

    /// The roofs to land on.
    #[must_use]
    pub fn roofs(&self) -> Vec<Roof> {
        self.model
            .roofs
            .iter()
            .map(|r| {
                let axis = if r.slopes_z { Vec3::Z } else { Vec3::X };
                let across = Quat::from_rotation_y(self.yaw) * axis;
                Roof {
                    center: self.world(r.center),
                    across: [across.x, across.z],
                    half: r.half.map(|h| h * self.scale),
                    eave: self.base() + r.eave * self.scale,
                    ridge: self.base() + r.ridge * self.scale,
                }
            })
            .collect()
    }

    /// Where its walk ends, outside the front door, m.
    #[must_use]
    pub fn front(&self) -> [f32; 2] {
        self.world(self.model.front)
    }

    /// The model's outward front, as a ground unit vector.
    #[must_use]
    pub fn outward(&self) -> [f32; 2] {
        let f = crate::controller::forward(self.yaw);
        [f.x, f.z]
    }
}
