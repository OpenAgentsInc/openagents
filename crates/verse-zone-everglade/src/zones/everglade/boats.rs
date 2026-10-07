//! What floats on Everglade's water (`docs/verse/water.md`, phase W6): the
//! rowboats at Lantern Pond, Reed Pond, and the Thinking Pond, which a
//! player boards, rows, capsizes, rights, and breaks
//! ([`verse_world::rowboat`]), and the lily pads, which bob as movers pass.
//!
//! Both draw in the frame's figure after the creatures, posed on the CPU
//! from the pack's `generated/rowboat` and `generated/lily_pads` models. A
//! boat's triangles are split among its planks, so a whole boat poses them
//! with its hull and a broken one poses each with its plank's body. The
//! boathouse's rowboat stays a static prop.

use std::sync::Arc;

use glam::{DVec2, DVec3, Mat3, Mat4, Quat, Vec2, Vec3};
use verse_pbr::water::Source;
use verse_world::rowboat::{BOARD_REACH, Event, Fleet, HALF, Intent, Seat, State};
use verse_world::social::everglade_water as ew;
use verse_world::water::FLOAT_DEPTH;

use super::layout::Placement;
use super::scene::{Copied, copy_material};
use super::solids::Solids;
use crate::controller::{InputState, PlayerController};
use crate::pbr::textured::{Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex};
use crate::pbr::textured_bake::AmbientProbes;
use crate::zones::everglade_pack::ZonePack;

/// The local player's name aboard until a session names it.
pub const ME: &str = "me";
/// The boat model.
pub const ROWBOAT: &str = "generated/rowboat";
/// The lily pad model.
pub const LILY_PADS: &str = "generated/lily_pads";
/// How far a lily pad bobs as a mover passes, m, and how far away it feels
/// one, m.
const BOB: f32 = 0.03;
const BOB_REACH: f32 = 4.0;
/// How long a message about the boats stays in the log.
const MOST_LOG: usize = 6;

/// What one span of triangles follows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    /// Plank `plank` of boat `boat`, its triangles about the plank's center
    /// in the model's frame.
    Plank { boat: usize, plank: usize },
    /// Lily pad `pad`.
    Pad { pad: usize },
}

#[derive(Clone, Copy, Debug)]
struct Span {
    item: Item,
    start: usize,
    len: usize,
}

/// A lily pad on the water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub at: Vec2,
    pub yaw: f32,
    pub scale: f32,
    /// How high its origin sits over the surface, m.
    pub above: f32,
    /// Its own phase, so the pads don't bob together.
    pub phase: f32,
}

/// The boats and lily pads, their world, and their drawn triangles.
pub struct Afloat {
    pub fleet: Fleet,
    /// Who the local player is aboard: [`ME`] until a session names it.
    me: String,
    pads: Vec<Pad>,
    /// Each plank's center in the boat model's frame, m.
    plank_centers: [Vec3; 7],
    local: Vec<TexturedVertex>,
    spans: Vec<Span>,
    scene: Arc<TexturedScene>,
    posed: Vec<TexturedVertex>,
    combined: Option<(Arc<TexturedScene>, Arc<TexturedScene>)>,
    clock: f32,
    /// Movers near the pads this frame: where, and how fast.
    movers: Vec<(Vec2, f32)>,
    /// What happened, newest last.
    pub log: Vec<String>,
    events: Vec<Event>,
    /// The boat that dumped the local player in the water, until the
    /// zone puts the player there ([`Self::drop_in`]).
    dump: Option<usize>,
}

/// The rowboats' moorings by the jetties, and the lily pads, from
/// `floats` (`layout::floats`): each boat's place and heading.
#[must_use]
pub fn moorings(floats: &[Placement]) -> Vec<(DVec2, f64)> {
    floats
        .iter()
        .filter(|p| p.model == ROWBOAT)
        .map(|p| {
            (
                DVec2::new(f64::from(p.at[0]), f64::from(p.at[1])),
                f64::from(p.yaw),
            )
        })
        .collect()
}

/// The beds and banks of the ponds the boats float on, as static boxes.
fn beds() -> &'static [(DVec3, DVec3)] {
    static BEDS: std::sync::OnceLock<Vec<(DVec3, DVec3)>> = std::sync::OnceLock::new();
    BEDS.get_or_init(|| {
        ew::bed_boxes(1.0, 1.5)
            .into_iter()
            // The boats keep to the three ponds with jetties; the run's
            // boxes would only cost the solver.
            .filter(|(c, _)| {
                ew::PONDS[..3]
                    .iter()
                    .any(|([x, z], r)| (c.x as f32 - x).hypot(c.z as f32 - z) < r + 3.0)
            })
            .collect()
    })
}

impl Afloat {
    /// The boats and pads `floats` places, drawn from `pack`.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a model.
    pub fn new(pack: &ZonePack, floats: &[Placement]) -> Result<Self, String> {
        let fleet = Fleet::new(ew::risen(), beds(), &moorings(floats));
        let pads: Vec<Pad> = floats
            .iter()
            .filter(|p| p.model == LILY_PADS)
            .enumerate()
            .map(|(i, p)| {
                let top = ew::surface(p.at[0], p.at[1]).unwrap_or(0.0);
                Pad {
                    at: Vec2::from(p.at),
                    yaw: p.yaw,
                    scale: p.scale,
                    above: (super::height(p.at[0], p.at[1]) + p.lift - top).clamp(0.0, 0.05),
                    phase: i as f32 * 1.7,
                }
            })
            .collect();
        let boat = pack
            .model(ROWBOAT)
            .ok_or_else(|| format!("The Everglade pack has no {ROWBOAT}"))?;
        let pad = pack
            .model(LILY_PADS)
            .ok_or_else(|| format!("The Everglade pack has no {LILY_PADS}"))?;
        // The model's hull to the physics hull: each plank's center in the
        // model's frame, scaled from the hull's box to the model's.
        let (lo, hi) = boat.bounds();
        let size = Vec3::from(hi) - Vec3::from(lo);
        let hull = (HALF * 2.0).as_vec3();
        let to_model = size / hull;
        let plank_centers = Fleet::planks().map(|(center, _)| center.as_vec3() * to_model);
        let mut scene = TexturedScene::default();
        let mut copied = Copied::default();
        // Every item's triangles by scene material.
        let mut by_material: std::collections::BTreeMap<usize, Vec<(Item, Vec<TexturedVertex>)>> =
            std::collections::BTreeMap::new();
        // The boat model's bow points along its -z; the hull's along +z.
        let turn = |p: [f32; 3], turned: bool| {
            if turned {
                Vec3::new(-p[0], p[1], -p[2])
            } else {
                Vec3::from(p)
            }
        };
        let vertex = |v: &crate::zones::everglade_pack::format::Vertex, at: Vec3, turned: bool| {
            TexturedVertex {
                pos: (turn(v.position, turned) - at).to_array(),
                normal: turn(v.normal, turned).to_array(),
                uv: v.uv,
                color: v.color,
                light: crate::pbr::textured::UNBAKED,
            }
        };
        for primitive in &boat.primitives {
            let material = copy_material(pack, primitive.material, &mut scene, &mut copied)?;
            let mut planks: Vec<Vec<TexturedVertex>> = vec![Vec::new(); 7];
            for tri in primitive.indices.chunks_exact(3) {
                let corners = [0, 1, 2].map(|k| &primitive.vertices[tri[k] as usize]);
                let centroid = corners.iter().map(|v| turn(v.position, true)).sum::<Vec3>() / 3.0;
                let nearest = nearest_plank(centroid / to_model);
                for v in corners {
                    planks[nearest].push(vertex(v, plank_centers[nearest], true));
                }
            }
            for k in 0..fleet.boats.len() {
                for (plank, vertices) in planks.iter().enumerate() {
                    if !vertices.is_empty() {
                        by_material
                            .entry(material)
                            .or_default()
                            .push((Item::Plank { boat: k, plank }, vertices.clone()));
                    }
                }
            }
        }
        for primitive in &pad.primitives {
            let material = copy_material(pack, primitive.material, &mut scene, &mut copied)?;
            let vertices: Vec<TexturedVertex> = primitive
                .indices
                .iter()
                .map(|&i| vertex(&primitive.vertices[i as usize], Vec3::ZERO, false))
                .collect();
            for p in 0..pads.len() {
                by_material
                    .entry(material)
                    .or_default()
                    .push((Item::Pad { pad: p }, vertices.clone()));
            }
        }
        let mut local = Vec::new();
        let mut spans = Vec::new();
        let mut primitives = Vec::new();
        for (material, entries) in by_material {
            let first = local.len();
            for (item, vertices) in entries {
                spans.push(Span {
                    item,
                    start: local.len(),
                    len: vertices.len(),
                });
                local.extend(vertices);
            }
            let vertices = local[first..].to_vec();
            let indices = (0..vertices.len() as u32).collect();
            primitives.push(Primitive {
                vertices,
                indices,
                material,
            });
        }
        scene.add_mesh(TexturedMesh { primitives });
        let mut afloat = Self {
            fleet,
            me: ME.to_owned(),
            pads,
            plank_centers,
            posed: local.clone(),
            local,
            spans,
            scene: Arc::new(scene),
            combined: None,
            clock: 0.0,
            movers: Vec::new(),
            log: Vec::new(),
            events: Vec::new(),
            dump: None,
        };
        afloat.pose();
        Ok(afloat)
    }

    /// The local player's name aboard, such as its public key.
    #[must_use]
    pub fn me(&self) -> &str {
        &self.me
    }

    /// Names the local player aboard, moving any seat it holds.
    pub fn set_me(&mut self, me: &str) {
        if me == self.me {
            return;
        }
        for boat in &mut self.fleet.boats {
            for seat in boat.seats.iter_mut().flatten() {
                if seat.who == self.me {
                    me.clone_into(&mut seat.who);
                }
            }
            if boat.owner.as_deref() == Some(self.me.as_str()) {
                boat.owner = Some(me.to_owned());
            }
        }
        me.clone_into(&mut self.me);
    }

    /// The lily pads.
    #[must_use]
    pub fn pads(&self) -> &[Pad] {
        &self.pads
    }

    /// Takes the fleet's events since the last call, for effects.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > MOST_LOG {
            self.log.remove(0);
        }
    }

    /// The local player's seat: boat, seat, and whether seated (rather than
    /// climbing in).
    #[must_use]
    pub fn seat(&self) -> Option<(usize, Seat, bool)> {
        self.fleet.seat_of(&self.me)
    }

    /// The interact key for `player`, who is `swimming` or not: leaves the
    /// boat it sits in, rights a capsized boat beside a swimmer, or boards
    /// the boat in reach. Returns what happened, or `None` when no boat is
    /// in reach.
    pub fn interact(
        &mut self,
        player: &mut PlayerController,
        swimming: bool,
        solids: &Solids,
    ) -> Option<String> {
        if let Some((k, _, _)) = self.seat() {
            self.fleet.leave(&self.me.clone());
            let line = match self.landing(k, solids) {
                Some(land) => {
                    player.pos = land;
                    player.set_surface_height(land.y);
                    player.set_vertical_speed(0.0);
                    "You step out of the rowboat".to_owned()
                }
                None => {
                    self.into_water(k, player);
                    "You slip over the side into the water".to_owned()
                }
            };
            self.say(line.clone());
            return Some(line);
        }
        let k = self.fleet.near(player.pos)?;
        let line = if self.fleet.boats[k].state == State::Capsized {
            if !swimming {
                "Swim to the boat to right it".to_owned()
            } else {
                match self.fleet.right(k, player.pos) {
                    Ok(()) => "You right the rowboat".to_owned(),
                    Err(e) => e.to_string(),
                }
            }
        } else {
            let me = self.me.clone();
            match self.fleet.board(k, &me, player.pos, swimming) {
                Ok(Seat::Rower) if swimming => "You climb into the rowboat".to_owned(),
                Ok(Seat::Rower) => "You take the oars".to_owned(),
                Ok(Seat::Passenger) if swimming => "You climb into the stern".to_owned(),
                Ok(Seat::Passenger) => "You sit in the stern".to_owned(),
                Err(e) => e.to_string(),
            }
        };
        self.say(line.clone());
        Some(line)
    }

    /// Whether a boat is in the interact key's reach of `at`, or the
    /// player sits in one.
    #[must_use]
    pub fn in_reach(&self, at: Vec3) -> bool {
        self.seat().is_some() || self.fleet.near(at).is_some()
    }

    /// Where a player leaving boat `k` steps: the nearest jetty or bank
    /// within [`BOARD_REACH`] of the hull that stands at or over the water,
    /// if any.
    fn landing(&self, k: usize, solids: &Solids) -> Option<Vec3> {
        let (keel, rot) = self.fleet.frame(k);
        let keel = keel.as_vec3();
        let top = ew::surface(keel.x, keel.z).unwrap_or(keel.y);
        let mut best: Option<(f32, Vec3)> = None;
        for side in [-1.0_f32, 1.0] {
            for along in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
                for out in [0.4_f32, 0.9, 1.4, 1.9] {
                    let local = DVec3::new(
                        f64::from(side) * (HALF.x + f64::from(out)),
                        0.0,
                        HALF.z * f64::from(along),
                    );
                    let p = keel + (rot * local).as_vec3();
                    let floor = solids.floor(p.x, p.z, top + 1.6);
                    let water = ew::surface(p.x, p.z);
                    let dry = water.is_none_or(|w| floor >= w - 0.05);
                    if dry && floor < top + 1.5 && self.fleet.reach(k, p) <= BOARD_REACH {
                        let d = out + along.abs() * 0.3;
                        if best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, Vec3::new(p.x, floor, p.z)));
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// The boat that dumped the local player in the water since the last
    /// call, if one did.
    pub fn take_dump(&mut self) -> Option<usize> {
        self.dump.take()
    }

    /// Puts `player` in the water beside boat `k`, swimming, as a capsize
    /// or a break throws it.
    pub fn drop_in(&self, k: usize, player: &mut PlayerController) {
        self.into_water(k, player);
    }

    /// Puts `player` in the water beside boat `k`, swimming.
    fn into_water(&self, k: usize, player: &mut PlayerController) {
        let (keel, rot) = self.fleet.frame(k);
        let side = (rot * DVec3::new(HALF.x + 0.7, 0.0, 0.0)).as_vec3();
        let p = keel.as_vec3() + side;
        let top = ew::surface(p.x, p.z).unwrap_or(p.y);
        player.pos = Vec3::new(p.x, top - FLOAT_DEPTH as f32, p.z);
        player.set_surface_height(super::height(p.x, p.z).min(player.pos.y));
        player.hold_altitude(player.pos.y);
        player.set_vertical_speed(0.0);
    }

    /// Steps a seated player: the keys row and turn the boat, and the
    /// player rides its seat. A player climbing in clings to the side.
    /// Returns whether the boat moved the player (so it doesn't walk).
    pub fn ride(&mut self, player: &mut PlayerController, input: &InputState) -> bool {
        let Some((k, seat, seated)) = self.seat() else {
            return false;
        };
        let me = self.me.clone();
        if seated && seat == Seat::Rower {
            let ahead = f32::from(u8::from(input.forward)) - f32::from(u8::from(input.backward));
            let turn = f32::from(u8::from(input.right || input.strafe_right))
                - f32::from(u8::from(input.left || input.strafe_left));
            self.fleet.row(&me, Intent { ahead, turn });
        }
        let (keel, rot) = self.fleet.frame(k);
        if seated {
            // Standing on the boards by the seat, facing the bow.
            let local = seat.local();
            let feet = keel + rot * DVec3::new(local.x, 0.08, local.z);
            player.pos = feet.as_vec3();
            player.yaw = self.fleet.yaw(k) as f32;
        } else {
            let side = (rot * DVec3::new(HALF.x + 0.35, 0.0, seat.local().z)).as_vec3();
            let p = keel.as_vec3() + side;
            player.pos = Vec3::new(p.x, keel.y as f32 - 0.2, p.z);
        }
        player.set_surface_height(player.pos.y);
        player.hold_altitude(player.pos.y);
        player.set_vertical_speed(0.0);
        true
    }

    /// Advances the boats and the pads by `dt`. The occupants a capsize or
    /// a break dumps fall in the water; for the local player among them,
    /// [`Self::take_dump`] names the boat.
    pub fn tick(&mut self, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        self.clock = (self.clock + dt) % 3600.0;
        self.fleet.tick(dt);
        for event in self.fleet.take_events() {
            match &event {
                Event::Capsized { boat, dumped } | Event::Broken { boat, dumped }
                    if dumped.iter().any(|w| *w == self.me) =>
                {
                    self.dump = Some(*boat);
                    let line = if matches!(event, Event::Broken { .. }) {
                        "The rowboat breaks apart under you"
                    } else {
                        "The rowboat capsizes and throws you in"
                    };
                    self.say(line.to_owned());
                }
                Event::Boarded { who, .. } if *who == self.me => {
                    self.say("You are aboard".to_owned());
                }
                _ => {}
            }
            self.events.push(event);
        }
        if self.events.len() > 64 {
            self.events.drain(..self.events.len() - 64);
        }
        self.pose();
    }

    /// Movers near the pads, for their bobbing: each where it is and how
    /// fast it moves.
    pub fn set_movers(&mut self, movers: Vec<(Vec2, f32)>) {
        self.movers = movers;
    }

    /// The boats' and floating planks' wakes for the ripple field.
    #[must_use]
    pub fn sources(&self) -> Vec<Source> {
        self.fleet
            .wakes()
            .into_iter()
            .filter(|(at, _, _)| ew::surface(at.x, at.z).is_some())
            .map(|(at, v, r)| Source::mover(Vec2::new(at.x, at.z), Vec2::new(v.x, v.z), r))
            .collect()
    }

    /// Lily pad `pad`'s bob now, m.
    fn bob(&self, pad: &Pad) -> f32 {
        let breeze = 0.003 * (self.clock * 1.3 + pad.phase).sin();
        let passing: f32 = self
            .movers
            .iter()
            .map(|(at, speed)| {
                let d = at.distance(pad.at);
                if d > BOB_REACH {
                    return 0.0;
                }
                let k = (1.0 - d / BOB_REACH).powi(2) * speed.min(2.0) * 0.5;
                k * (5.0 * d - 7.0 * self.clock).sin()
            })
            .sum();
        breeze + BOB * passing.clamp(-1.0, 1.0)
    }

    /// Where lily pad `pad` floats now: pushed aside by any hull over it.
    fn pad_at(&self, pad: &Pad) -> Vec2 {
        let mut at = pad.at;
        for k in 0..self.fleet.boats.len() {
            if self.fleet.boats[k].state == State::Broken {
                continue;
            }
            let (keel, _) = self.fleet.frame(k);
            let center = Vec2::new(keel.x as f32, keel.z as f32);
            let d = at - center;
            let reach = HALF.z as f32 + 0.4;
            if d.length() < reach {
                at = center + d.normalize_or(Vec2::X) * reach;
            }
        }
        at
    }

    /// The frame each item's triangles are posed by.
    fn item_pose(&self, item: Item) -> Mat4 {
        match item {
            Item::Plank { boat, plank } => {
                let b = &self.fleet.boats[boat];
                if b.state == State::Broken {
                    let (pos, rot) = self.fleet.plank_pose(b.planks[plank]);
                    Mat4::from_rotation_translation(rot.as_quat(), pos.as_vec3())
                } else {
                    let (keel, rot) = self.fleet.frame(boat);
                    Mat4::from_rotation_translation(rot.as_quat(), keel.as_vec3())
                        * Mat4::from_translation(self.plank_centers[plank])
                }
            }
            Item::Pad { pad } => {
                let p = &self.pads[pad];
                let at = self.pad_at(p);
                let top = ew::surface(at.x, at.y).unwrap_or(0.0);
                let bob = self.bob(p);
                let tilt = Quat::from_rotation_x(bob * 2.0) * Quat::from_rotation_z(-bob * 1.5);
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(p.scale),
                    Quat::from_rotation_y(p.yaw) * tilt,
                    Vec3::new(at.x, top + p.above + bob, at.y),
                )
            }
        }
    }

    /// Poses every item's triangles for this frame.
    fn pose(&mut self) {
        for span in &self.spans {
            let m = self.item_pose(span.item);
            let turn = Mat3::from_mat4(m).transpose().inverse().transpose();
            let turn = Mat3::from_cols(
                turn.x_axis.normalize_or_zero(),
                turn.y_axis.normalize_or_zero(),
                turn.z_axis.normalize_or_zero(),
            );
            for i in span.start..span.start + span.len {
                let v = self.local[i];
                self.posed[i] = TexturedVertex {
                    pos: m.transform_point3(Vec3::from(v.pos)).to_array(),
                    normal: (turn * Vec3::from(v.normal)).normalize_or_zero().to_array(),
                    ..v
                };
            }
        }
    }

    /// Joins the boats and pads to the characters' `scene` once per scene.
    pub fn prepare(&mut self, scene: &Arc<TexturedScene>) {
        if self
            .combined
            .as_ref()
            .is_some_and(|(cast, _)| Arc::ptr_eq(cast, scene))
        {
            return;
        }
        let joined = super::demolition::join(scene, &self.scene);
        self.combined = Some((scene.clone(), Arc::new(joined)));
    }

    /// The scene of the characters, the boats, and the pads, once prepared
    /// for the characters' `scene`.
    #[must_use]
    pub fn joined(&self, scene: &Arc<TexturedScene>) -> Option<Arc<TexturedScene>> {
        self.combined
            .as_ref()
            .filter(|(cast, _)| Arc::ptr_eq(cast, scene))
            .map(|(_, joined)| joined.clone())
    }

    /// The frame's figure: the characters' `cast` figure followed by the
    /// boats and pads, lit by `probes` when the bake has them.
    #[must_use]
    pub fn figure(&self, cast: Figure, probes: Option<&AmbientProbes>) -> Figure {
        let Some(joined) = self.joined(&cast.scene) else {
            return cast;
        };
        let mut vertices = Vec::with_capacity(cast.vertices.len() + self.posed.len());
        vertices.extend_from_slice(&cast.vertices);
        let start = vertices.len();
        vertices.extend_from_slice(&self.posed);
        if let Some(probes) = probes {
            probes.shade(&mut vertices[start..]);
        }
        Figure {
            scene: joined,
            vertices: Arc::new(vertices),
        }
    }

    /// The hulls over the water, for masking the water out of them: each
    /// boat's middle, heading, half beam and half length, and gunwale.
    #[must_use]
    pub fn hulls(&self) -> Vec<verse_pbr::water::frame::Hull> {
        (0..self.fleet.boats.len())
            .filter(|&k| self.fleet.boats[k].state == State::Upright && self.fleet.tilt(k) < 0.6)
            .map(|k| {
                let (keel, rot) = self.fleet.frame(k);
                let ahead = (rot * DVec3::Z).as_vec3();
                verse_pbr::water::frame::Hull {
                    center: [keel.x as f32, keel.z as f32],
                    dir: Vec2::new(ahead.x, ahead.z).normalize_or(Vec2::Y).to_array(),
                    half: [HALF.x as f32 - 0.04, HALF.z as f32 - 0.04],
                    top: keel.y as f32 + 2.0 * HALF.y as f32,
                }
            })
            .collect()
    }

    /// Puts every boat back at its mooring.
    pub fn reset(&mut self) {
        self.fleet.reset();
        self.fleet.take_events();
        self.pose();
    }
}

/// The plank nearest `p`, a point in the physics hull's frame.
fn nearest_plank(p: Vec3) -> usize {
    let p = p.as_dvec3();
    Fleet::planks()
        .iter()
        .enumerate()
        .map(|(i, (center, half))| {
            let d = ((p - *center).abs() - *half).max(DVec3::ZERO).length();
            (i, d)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(i, _)| i)
}

#[cfg(test)]
#[path = "boats_tests.rs"]
mod tests;
