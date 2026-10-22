//! Every place to go on the Grid gets a colour and a picture (#12481): the
//! arches to Game Lab, Everglade, Lagrange 2, and the hosted games, and the
//! Gym's doorway. Each wears its colour along the inside of its opening and
//! a framed picture of the place above it, so from the spawn every portal
//! reads as somewhere different before its letters do. Within a few meters
//! a short line says what it is and what you can do there.
//!
//! The rest of the Grid stays monochrome: colour means a place to go. The
//! same colours are the ones a map or a world marker should use for these
//! places ([`Mark::colour`]).
//!
//! The marks are models built at start, like the Pylon Bay's, so they add
//! nothing to the pinned Grid pack or the download.

/// The picture's frame, m, in the mark's own coordinates: centered on the
/// arch, `x` across as a visitor approaching sees it, `y` up from the
/// ground. The frame sits over the arch's lettering.
pub const FRAME_X: f32 = 1.7;
pub const FRAME_Y: [f32; 2] = [5.7, 8.5];
/// The picture's box inside the frame, m: `x` across from its center, `y`
/// up from its lower edge.
pub const PICTURE: [f32; 2] = [1.3, 2.2];
/// How thick a picture's strokes and the frame's border are, m: thick
/// enough to read from the spawn.
pub const STROKE: f32 = 0.11;
/// The colour stripe's width up the inside of each pillar and under the
/// lintel, m. The arch's pillars stand 1.9 m either side of its middle and
/// its lintel's lower face is 4.32 m up (`verse::zones::arch_frame`).
pub const STRIPE: f32 = 0.16;
pub const PILLAR_INNER: f32 = 1.725;
pub const LINTEL_UNDER: f32 = 4.325;
/// How near, m, a visitor stands before the line shows.
pub const LINE_RANGE: f32 = 9.0;

/// What a place is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    /// A game controller.
    Controller,
    /// A tree on a hill.
    Tree,
    /// A rocket beside a moon.
    Rocket,
    /// An ice cream cone.
    Cone,
    /// A house with a crooked roof and a moon.
    House,
    /// A bar chart.
    Chart,
}

/// A place to go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// The model key's last part, and what the page calls the place.
    pub id: &'static str,
    /// The line shown within [`LINE_RANGE`].
    pub line: &'static str,
    /// Its colour, sRGB.
    pub colour: u32,
    pub picture: Picture,
}

pub const GAME_LAB: Mark = Mark {
    id: "game-lab",
    line: "Game Lab: change a game by chatting",
    colour: 0x00ff_4fd8,
    picture: Picture::Controller,
};
pub const EVERGLADE: Mark = Mark {
    id: "everglade",
    line: "Everglade: walk a town by the water",
    colour: 0x0040_f060,
    picture: Picture::Tree,
};
pub const LAGRANGE2: Mark = Mark {
    id: "lagrange-2",
    line: "Lagrange 2: build spaceships in orbit",
    colour: 0x0040_a8ff,
    picture: Picture::Rocket,
};
pub const ICE_CREAM_MAN: Mark = Mark {
    id: "icecream",
    line: "The Ice Cream Man: a horror game to play",
    colour: 0x00ff_e04a,
    picture: Picture::Cone,
};
pub const HAUNTED_HOUSE: Mark = Mark {
    id: "haunt",
    line: "The Haunted House: a horror game to play",
    colour: 0x00ff_7a1a,
    picture: Picture::House,
};
pub const GYM: Mark = Mark {
    id: "gym",
    line: "The Gym: see how agents score",
    colour: 0x009a_5cff,
    picture: Picture::Chart,
};

/// Every mark.
pub const ALL: [Mark; 6] = [
    GAME_LAB,
    EVERGLADE,
    LAGRANGE2,
    ICE_CREAM_MAN,
    HAUNTED_HOUSE,
    GYM,
];

/// The mark of a hosted arch's tenant, when the Grid knows it.
#[must_use]
pub fn hosted(tenant: &str) -> Option<Mark> {
    [ICE_CREAM_MAN, HAUNTED_HOUSE]
        .into_iter()
        .find(|m| m.id == tenant)
}

/// A stroke from one point to another in the picture's box.
pub type Stroke = ([f32; 2], [f32; 2]);

fn polyline(points: &[[f32; 2]], out: &mut Vec<Stroke>) {
    for pair in points.windows(2) {
        out.push((pair[0], pair[1]));
    }
}

fn circle(center: [f32; 2], r: f32, segments: usize, out: &mut Vec<Stroke>) {
    arc(center, r, 0.0, std::f32::consts::TAU, segments, out);
}

fn arc(center: [f32; 2], r: f32, from: f32, to: f32, segments: usize, out: &mut Vec<Stroke>) {
    let at = |i: usize| {
        let a = from + (to - from) * i as f32 / segments as f32;
        [center[0] + r * a.cos(), center[1] + r * a.sin()]
    };
    for i in 0..segments {
        out.push((at(i), at(i + 1)));
    }
}

impl Picture {
    /// Its strokes, inside the picture's box ([`PICTURE`]).
    #[must_use]
    pub fn strokes(self) -> Vec<Stroke> {
        let mut out = Vec::new();
        match self {
            Self::Controller => {
                polyline(
                    &[
                        [-1.1, 1.45],
                        [1.1, 1.45],
                        [1.25, 0.75],
                        [0.95, 0.5],
                        [0.55, 0.85],
                        [-0.55, 0.85],
                        [-0.95, 0.5],
                        [-1.25, 0.75],
                        [-1.1, 1.45],
                    ],
                    &mut out,
                );
                out.push(([-0.85, 1.15], [-0.45, 1.15]));
                out.push(([-0.65, 0.95], [-0.65, 1.35]));
                circle([0.5, 1.25], 0.1, 8, &mut out);
                circle([0.75, 1.05], 0.1, 8, &mut out);
            }
            Self::Tree => {
                out.push(([-1.2, 0.3], [1.2, 0.3]));
                out.push(([0.0, 0.3], [0.0, 0.8]));
                polyline(
                    &[[-0.8, 0.8], [0.0, 1.5], [0.8, 0.8], [-0.8, 0.8]],
                    &mut out,
                );
                polyline(&[[-0.6, 1.3], [0.0, 2.0], [0.6, 1.3]], &mut out);
            }
            Self::Rocket => {
                polyline(
                    &[
                        [-0.25, 0.6],
                        [-0.25, 1.4],
                        [0.0, 1.9],
                        [0.25, 1.4],
                        [0.25, 0.6],
                        [-0.25, 0.6],
                    ],
                    &mut out,
                );
                polyline(&[[-0.25, 0.9], [-0.5, 0.5], [-0.25, 0.6]], &mut out);
                polyline(&[[0.25, 0.9], [0.5, 0.5], [0.25, 0.6]], &mut out);
                polyline(&[[-0.12, 0.6], [0.0, 0.25], [0.12, 0.6]], &mut out);
                circle([0.0, 1.25], 0.1, 8, &mut out);
                circle([-0.85, 1.6], 0.28, 12, &mut out);
            }
            Self::Cone => {
                polyline(&[[-0.45, 1.2], [0.0, 0.2], [0.45, 1.2]], &mut out);
                out.push(([-0.22, 0.7], [0.3, 1.2]));
                out.push(([0.22, 0.7], [-0.3, 1.2]));
                arc(
                    [0.0, 1.35],
                    0.5,
                    -0.3,
                    std::f32::consts::PI + 0.3,
                    12,
                    &mut out,
                );
                out.push(([-0.48, 1.2], [0.48, 1.2]));
            }
            Self::House => {
                polyline(
                    &[
                        [-0.7, 0.3],
                        [-0.7, 1.1],
                        [-0.85, 1.05],
                        [-0.05, 1.75],
                        [0.85, 1.15],
                        [0.7, 1.1],
                        [0.7, 0.3],
                        [-0.7, 0.3],
                    ],
                    &mut out,
                );
                polyline(
                    &[[-0.15, 0.3], [-0.15, 0.75], [0.15, 0.75], [0.15, 0.3]],
                    &mut out,
                );
                polyline(
                    &[
                        [-0.5, 0.8],
                        [-0.3, 0.8],
                        [-0.3, 0.95],
                        [-0.5, 0.95],
                        [-0.5, 0.8],
                    ],
                    &mut out,
                );
                arc([0.85, 1.85], 0.25, 1.2, 5.1, 10, &mut out);
            }
            Self::Chart => {
                out.push(([-1.0, 0.3], [1.0, 0.3]));
                out.push(([-1.0, 0.3], [-1.0, 1.9]));
                for (x, h) in [(-0.6, 0.5), (-0.15, 0.95), (0.3, 0.7), (0.75, 1.35)] {
                    polyline(
                        &[
                            [x - 0.15, 0.3],
                            [x - 0.15, 0.3 + h],
                            [x + 0.15, 0.3 + h],
                            [x + 0.15, 0.3],
                        ],
                        &mut out,
                    );
                }
            }
        }
        out
    }
}

/// The marks in the Grid's world pass.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use glam::{Mat4, Quat, Vec3};
    use verse::imported::flat;
    use verse::runtime::WorldRuntime;
    use verse::ui::{Atlas, UiBatch};
    use verse_engine::assets::{Pack, Surface, Topology};
    use verse_engine::inventory::{Asset, Binding, fingerprint};
    use verse_engine::presentation::Instance;

    use super::{
        ALL, FRAME_X, FRAME_Y, LINE_RANGE, LINTEL_UNDER, Mark, PICTURE, PILLAR_INNER, STRIPE,
        STROKE,
    };

    /// How far in front of the arch's middle the marks stand, m: just
    /// proud of the pillars' faces, on the side a visitor walks up from.
    const PROUD: f32 = 0.27;
    /// The frame's dark backing, a little behind the strokes.
    const BACKING: [f32; 3] = [0.002; 3];
    const LINE_PAD: [f32; 2] = [10.0, 6.0];
    /// Where the line hangs, m: its top edge just under the lintel, above
    /// an arch's own prompt in the opening.
    const LINE_AT: f32 = 4.1;

    fn key(mark: &Mark) -> String {
        format!("mark/{}", mark.id)
    }

    /// A quad facing both ways.
    fn quad(surface: &mut Surface, corners: [Vec3; 4]) {
        flat::quad(surface, corners);
        let [a, b, c, d] = corners;
        flat::quad(surface, [d, c, b, a]);
    }

    /// A stroke from `a` to `b` on the plane `z`, `width` wide.
    fn stroke(surface: &mut Surface, a: [f32; 2], b: [f32; 2], z: f32, width: f32) {
        let (a, b) = (Vec3::new(a[0], a[1], z), Vec3::new(b[0], b[1], z));
        let along = (b - a).normalize_or_zero();
        let side = Vec3::new(-along.y, along.x, 0.0) * (width / 2.0);
        // Overlap the ends so joints close.
        let (a, b) = (a - along * width / 2.0, b + along * width / 2.0);
        quad(surface, [a - side, b - side, b + side, a + side]);
    }

    fn model(mark: &Mark, white: usize) -> verse_engine::assets::Model {
        let colour = verse::palette::linear(mark.colour);
        let mut lit = flat::surface(white, colour, Topology::Triangles);
        let z = -PROUD;
        // The stripes up the inside of the pillars and under the lintel.
        for side in [-1.0, 1.0] {
            let x = side * (PILLAR_INNER - STRIPE / 2.0);
            stroke(
                &mut lit,
                [x, 0.0],
                [x, LINTEL_UNDER - STRIPE / 2.0],
                z,
                STRIPE,
            );
        }
        let y = LINTEL_UNDER - STRIPE / 2.0;
        stroke(&mut lit, [-PILLAR_INNER, y], [PILLAR_INNER, y], z, STRIPE);
        // The frame, its posts down to the lintel, and the picture. A
        // visitor walking up looks along +z, so their right is -x.
        let [y0, y1] = FRAME_Y;
        for (a, b) in [
            ([-FRAME_X, y0], [FRAME_X, y0]),
            ([FRAME_X, y0], [FRAME_X, y1]),
            ([FRAME_X, y1], [-FRAME_X, y1]),
            ([-FRAME_X, y1], [-FRAME_X, y0]),
            ([-1.2, 4.7], [-1.2, y0]),
            ([1.2, 4.7], [1.2, y0]),
        ] {
            stroke(&mut lit, a, b, z, STROKE);
        }
        let bottom = y0 + (y1 - y0 - PICTURE[1]) / 2.0;
        for (a, b) in mark.picture.strokes() {
            stroke(
                &mut lit,
                [-a[0], bottom + a[1]],
                [-b[0], bottom + b[1]],
                z - 0.01,
                STROKE,
            );
        }
        let mut back = flat::surface(white, BACKING, Topology::Triangles);
        quad(
            &mut back,
            [
                Vec3::new(-FRAME_X, y0, z + 0.02),
                Vec3::new(FRAME_X, y0, z + 0.02),
                Vec3::new(FRAME_X, y1, z + 0.02),
                Vec3::new(-FRAME_X, y1, z + 0.02),
            ],
        );
        flat::model(&key(mark), vec![back, lit], y1)
    }

    /// Adds every mark's model to the Grid's `pack`, whose white texel is
    /// slot `white` and listed as `texture`.
    pub fn add(pack: &mut Pack, white: usize, texture: &str) -> Result<(), String> {
        let inventory = pack
            .inventory
            .as_mut()
            .ok_or("The Grid's inventory is missing")?;
        let texture = inventory
            .assets
            .iter()
            .find(|a| a.id.as_str() == texture)
            .ok_or("The Grid's white texture is not listed")?
            .id
            .clone();
        for mark in &ALL {
            let model = model(mark, white);
            let (sha256, bytes) = fingerprint(&model)?;
            let key = key(mark);
            inventory.assets.push(Asset {
                id: verse::imported::inventory::id(&format!("verse:model:{key}"))?,
                binding: Binding::Model { key: key.clone() },
                sha256,
                bytes,
                dependencies: vec![texture.clone()],
            });
            pack.models.insert(key, model);
        }
        Ok(())
    }

    /// Where each mark stands on the Grid now: its arch's foot and turn.
    fn placed(runtime: &WorldRuntime) -> Vec<(Mark, Vec3, f32)> {
        let mut out = vec![(super::GAME_LAB, verse::zones::game_place::POSITION, 0.0)];
        if let Some(gate) = runtime.everglade_gate() {
            out.push((super::EVERGLADE, gate.at, gate.yaw));
        }
        if let Some(gate) = runtime.lagrange2_gate() {
            out.push((super::LAGRANGE2, gate.at, gate.yaw));
        }
        for (portal, gate) in runtime.hosted_gates() {
            if let Some(mark) = super::hosted(&portal.tenant) {
                out.push((mark, gate.at, gate.yaw));
            }
        }
        let door = verse::world::GymSite::GRID.point(verse::world::GYM_ENTRANCE);
        out.push((super::GYM, Vec3::new(door.x, 0.0, door.z), 0.0));
        out
    }

    /// This frame's mark instances.
    pub fn instances(runtime: &WorldRuntime, out: &mut Vec<Instance>) {
        for (mark, at, yaw) in placed(runtime) {
            out.push(verse::grid_frame::fixed(
                &key(&mark),
                Mat4::from_rotation_translation(Quat::from_rotation_y(yaw), at),
            ));
        }
    }

    /// The line of the nearest mark within [`LINE_RANGE`], over its frame.
    pub fn draw(
        ui: &mut UiBatch,
        atlas: &Atlas,
        size: [f32; 2],
        view_proj: Mat4,
        runtime: &WorldRuntime,
    ) {
        let player = runtime.player.pos;
        let near = placed(runtime)
            .into_iter()
            .map(|(mark, at, yaw)| {
                let front = at + Quat::from_rotation_y(yaw) * Vec3::new(0.0, 0.0, -2.0);
                (
                    mark,
                    at,
                    front.distance(Vec3::new(player.x, at.y, player.z)),
                )
            })
            .filter(|(_, _, d)| *d <= LINE_RANGE)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((mark, at, _)) = near else {
            return;
        };
        let Some([x, y]) = verse::hud::project(view_proj, size, at + Vec3::Y * LINE_AT) else {
            return;
        };
        let w = atlas.measure(mark.line) + LINE_PAD[0] * 2.0;
        let h = atlas.line + LINE_PAD[1] * 2.0;
        let (left, top) = (x - w / 2.0, y);
        ui.rect(
            atlas,
            left,
            top,
            w,
            h,
            crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.92),
        );
        ui.frame(
            atlas,
            left,
            top,
            w,
            h,
            1.0,
            crate::theme::linear(mark.colour, 1.0),
        );
        ui.text(
            atlas,
            left + LINE_PAD[0],
            top + LINE_PAD[1],
            mark.line,
            crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: u32) -> [f32; 3] {
        [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff].map(|v| v as f32 / 255.0)
    }

    #[test]
    fn every_place_has_its_own_strong_colour() {
        for (i, a) in ALL.iter().enumerate() {
            let [r, g, b] = rgb(a.colour);
            let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
            // Saturated and bright: never a grey the Grid already uses.
            assert!(hi > 0.9 && hi - lo > 0.5, "{}", a.id);
            for other in &ALL[i + 1..] {
                let [s, t, u] = rgb(other.colour);
                let apart = ((r - s).powi(2) + (g - t).powi(2) + (b - u).powi(2)).sqrt();
                assert!(apart > 0.3, "{} and {}", a.id, other.id);
                assert_ne!(a.picture, other.picture);
                assert_ne!(a.id, other.id);
            }
        }
    }

    #[test]
    fn pictures_fit_their_frame_and_the_frame_clears_the_lettering() {
        for mark in ALL {
            let strokes = mark.picture.strokes();
            assert!(strokes.len() >= 4, "{}", mark.id);
            for (a, b) in strokes {
                for p in [a, b] {
                    assert!(p[0].abs() <= PICTURE[0], "{}: {p:?}", mark.id);
                    assert!((0.0..=PICTURE[1]).contains(&p[1]), "{}: {p:?}", mark.id);
                }
            }
        }
        // The picture box sits inside the frame.
        assert!(PICTURE[0] + STROKE < FRAME_X);
        assert!(FRAME_Y[0] + 0.2 + PICTURE[1] + STROKE < FRAME_Y[1]);
        // The arch's lettering tops out near 5.4 m.
        assert!(FRAME_Y[0] > 5.5);
        // The stripes stay inside the opening.
        assert!(PILLAR_INNER - STRIPE > 1.5);
    }

    #[test]
    fn the_known_hosted_arches_have_marks() {
        assert_eq!(hosted("icecream"), Some(ICE_CREAM_MAN));
        assert_eq!(hosted("haunt"), Some(HAUNTED_HOUSE));
        assert_eq!(hosted("someone-else"), None);
    }

    #[test]
    fn the_lines_are_plain_and_short() {
        for mark in ALL {
            assert!(
                oa_copy::violations(mark.line, &[]).is_empty(),
                "{}",
                mark.line
            );
            assert!(mark.line.len() <= 44, "{}", mark.line);
            assert!(mark.line.is_ascii());
        }
    }
}
