//! Everglade's water in play (`docs/verse/water.md`, phase W3): the four
//! ponds and Glade Run drawn with the shared water shader, the weir's
//! cascade, and the character's medium and breath.
//!
//! The bodies and their carved beds live in
//! [`verse_world::social::everglade_water`]; the rules in
//! [`verse_world::water`]. Here:
//!
//! - [`surface`]: each body baked by `verse_pbr::water::bake` over the carved
//!   ground (depth, shore distance, and current a vertex), the weir's
//!   falling sheet, and the plunge pool's foam where it lands.
//! - [`frame`]: the frame's bodies with the `pond` and `river` presets'
//!   optics, and [`see_from`], which marks the body the eye is in, so its
//!   surface shades from below.
//! - [`Swim`]: the player's medium each step, wading at half speed,
//!   swimming at half speed with the current carrying it, diving along the
//!   camera's pitch, buoyancy back to the float line, climbing out over a
//!   low lip, and breath with its Exhaustion levels, a defeated swimmer
//!   surfacing at the nearest bank.
//! - [`draw_breath`]: the HUD's breath bar and the Exhaustion debuff icon.

use glam::{DVec2, Vec2, Vec3};
use verse_pbr::water::{
    Body, Kind, Preset, Water, WaterPatch, WaterSurface, WaterVertex,
    bake::{self, Bake},
    frame::{RIPPLE_LIFE, Ripple},
};
use verse_world::social::everglade_water::{self as ew, PONDS, RUN};
use verse_world::water::{
    Breath, BreathEvent, CLIMB_LIP, FLOAT_DEPTH, Medium, Stroke, WADE_DEPTH, medium, pace,
};

use super::height;
use crate::controller::{InputState, PlayerController};
use crate::ui::{Atlas, UiBatch};

/// The player character's Constitution modifier: it holds its breath for
/// two minutes.
pub const CON_MODIFIER: i32 = 1;
/// A forward stroke with the camera pitched further down than this dives,
/// rad: the stroke's speed splits between ahead and down by the pitch.
pub const DIVE_PITCH: f32 = 0.5;
/// A forward stroke with the camera pitched further up than this rises,
/// rad (negative looks up).
pub const RISE_PITCH: f32 = -0.1;
/// The water's grid spacing, m: the high tier's, so the run's 2.2 m keeps
/// a few vertices across on every tier.
const SPACING: f32 = 0.35;
/// How far past each outline the grid reaches, m.
const PAD: f32 = 0.6;
/// How far inside a pond's rim Glade Run's own surface stops, m.
const POND_OVERLAP: f32 = 0.4;
/// Breath under which the bar warns, s.
pub const WARN: f32 = 10.0;
/// How long the medium's last word stays in the log, s.
const MOST_LOG: usize = 6;
/// How often a body in the water rings the surface, s: moving, and still.
const RIPPLE_MOVING: f32 = 0.35;
const RIPPLE_STILL: f32 = 1.2;
/// A ring's starting height, m: moving, and still.
const RIPPLE_STRENGTH: (f32, f32) = (0.018, 0.008);
/// The most rings a swimmer keeps, each living `RIPPLE_LIFE`.
const MOST_RIPPLES: usize = 12;

/// Index of Glade Run in the frame's bodies.
pub const RUN_BODY: usize = 4;

/// The zone's water at rest: each pond and Glade Run baked over the carved
/// ground, the weir's sheet, and the plunge pool's foam.
///
/// # Errors
///
/// Returns a message when a body would bake past the bake's bounds.
pub fn surface() -> Result<WaterSurface, String> {
    let bed = |x: f64, z: f64| f64::from(height(x as f32, z as f32));
    let set = ew::water();
    let mut patches = Vec::new();
    for (k, body) in set.bodies().iter().enumerate() {
        let kind = if body.id == RUN {
            Kind::Stream
        } else {
            Kind::Body(0.0)
        };
        let mut patch = bake::body(
            body,
            &bed,
            &Bake {
                spacing: SPACING,
                pad: PAD,
                kind,
                body: k,
                extent: None,
            },
        )?;
        if body.id == RUN {
            plunge_foam(&mut patch);
            under_ponds(&mut patch);
        }
        patches.push(patch);
    }
    patches.push(weir_sheet());
    let surface = WaterSurface { patches };
    surface.validate()?;
    Ok(surface)
}

/// Where the weir's water lands in the plunge pool.
#[must_use]
pub fn landing() -> Vec3 {
    let run = ew::run();
    let along = run.weir + 0.5;
    let [x, z] = run.point_at(along);
    Vec3::new(x, run.level_at(along), z)
}

/// Dries the run's vertices inside a pond, where the run starts in Reed
/// Pond: the pond's own surface draws there, and two surfaces at one level
/// would add their reflections twice.
fn under_ponds(patch: &mut WaterPatch) {
    for v in &mut patch.vertices {
        let inside = PONDS
            .iter()
            .any(|([cx, cz], r)| (v.pos[0] - cx).hypot(v.pos[2] - cz) < r - POND_OVERLAP);
        if inside {
            v.depth = v.depth.min(-POND_OVERLAP);
        }
    }
}

/// White water where the weir's sheet lands, spreading into the pool.
fn plunge_foam(patch: &mut WaterPatch) {
    let land = landing();
    for v in &mut patch.vertices {
        if v.depth <= 0.0 {
            continue;
        }
        let d = Vec2::new(v.pos[0] - land.x, v.pos[2] - land.z).length();
        v.foam = v.foam.max((1.0 - d / 2.4).clamp(0.0, 1.0) * 0.9);
    }
}

/// The weir's falling sheet: from the stones' lip, a hand's height over
/// the water above, along the water's ballistic path down to the pool.
fn weir_sheet() -> WaterPatch {
    let run = ew::run();
    let ([lx, lz], [tx, tz], half) = run.weir_lip();
    let along = Vec2::new(tx, tz);
    let across = Vec2::new(-tz, tx);
    let top = run.level_at(run.weir - 0.5) + 0.04;
    let bottom = run.level_at(run.weir + 0.5);
    // Over a broad-crested weir the water leaves the lip at about the
    // critical speed, sqrt(g h) for the head h over it (Henderson, *Open
    // Channel Flow*, 1966, §6.5); a few centimeters of head give about
    // 0.6 m/s.
    let speed = 0.6;
    let drop = (top - bottom).max(0.05);
    let fall = (2.0 * drop / verse_pbr::water::frame::GRAVITY).sqrt();
    let (cols, rows) = (9_u32, 7_u32);
    let start = Vec2::new(lx, lz) - along * 0.12;
    let mut vertices = Vec::with_capacity((cols * rows) as usize);
    for j in 0..rows {
        let s = j as f32 / (rows - 1) as f32;
        let t = fall * s;
        let c = start + along * speed * t;
        let y = top - 0.5 * verse_pbr::water::frame::GRAVITY * t * t;
        for i in 0..cols {
            let a = i as f32 / (cols - 1) as f32 * 2.0 - 1.0;
            let p = c + across * a * half * 0.92;
            let mut v =
                WaterVertex::new(Vec3::new(p.x, y.max(bottom) + 0.015, p.y), 1.0, Kind::Fall)
                    .in_body(RUN_BODY);
            v.flow = [tx, tz];
            // Ragged edges, whiter as it falls.
            v.foam = (0.35 + 0.6 * s - a.abs().powi(4) * 0.5).clamp(0.0, 1.0);
            vertices.push(v);
        }
    }
    WaterPatch {
        cols,
        rows,
        vertices,
        decimate: false,
        dry: verse_pbr::water::frame::DRY,
    }
}

/// The zone's water bodies as a frame draws them, at water clock `time`:
/// the ponds with the `pond` preset and Glade Run with `river`, still but
/// for the fine detail the wind raises.
#[must_use]
pub fn frame(time: f32) -> Water {
    let preset = |name| Preset::named(name).cloned().unwrap_or_default();
    let (pond, river) = (preset("pond"), preset("river"));
    let mut water = Water::calm(ew::pond_level(0));
    water.count = 0;
    for body in ew::water().bodies() {
        let look = if body.id == RUN { &river } else { &pond };
        water.add(Body::from_physics(body, look));
    }
    // A light breeze from the south-west over the town.
    water.set_detail(0.6, 0.08, 1.6, 0.012);
    water.caustics = 0.6;
    water.time = time;
    water
}

/// Marks the body `eye` is in, so its surface shades from below.
pub fn see_from(water: &mut Water, eye: Vec3) {
    let p = DVec2::new(f64::from(eye.x), f64::from(eye.z));
    for (k, body) in ew::water().bodies().iter().enumerate().take(water.count) {
        let inside = body
            .surface()
            .sample(p.x, p.y, 0)
            .is_some_and(|s| f64::from(eye.y) < s.height);
        water.bodies[k].eye_inside = inside;
    }
}

/// The player's breath for the HUD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreathBar {
    /// Breath left and the full hold, s.
    pub left: f32,
    pub limit: f32,
    /// Exhaustion levels from suffocating.
    pub levels: u8,
    /// Whether the eye is under water now.
    pub under: bool,
}

/// The player's medium and breath in Everglade's water.
#[derive(Clone, Debug)]
pub struct Swim {
    pub medium: Medium,
    pub breath: Breath,
    /// Whether the eye was under the surface at the last step.
    pub under: bool,
    /// Rising after a jump until the swimmer is back at the float line.
    rising: bool,
    /// The camera's pitch, which steers a stroke up or down.
    pitch: f32,
    /// What happened, newest last.
    pub log: Vec<String>,
    /// How many times breath ran out to the sixth level.
    pub defeats: u32,
    /// The swimmer's own clock, s, which its ripples start on.
    clock: f32,
    /// Until the next ring on the surface, s.
    ripple_wait: f32,
    /// Where the feet were last step, for whether the body moves.
    last: Option<Vec2>,
    /// Rings the body made where it crosses the surface, on [`Self::clock`].
    ripples: Vec<Ripple>,
}

impl Default for Swim {
    fn default() -> Self {
        Self::new(CON_MODIFIER)
    }
}

impl Swim {
    /// On dry land with full breath for a Constitution modifier of `con`.
    #[must_use]
    pub fn new(con: i32) -> Self {
        Self {
            medium: Medium::Ground,
            breath: Breath::new(con),
            under: false,
            rising: false,
            pitch: 0.28,
            log: Vec::new(),
            defeats: 0,
            clock: 0.0,
            ripple_wait: 0.0,
            last: None,
            ripples: Vec::new(),
        }
    }

    /// Puts the rings the body made on `water`, whose clock reads `now`.
    pub fn ring(&self, water: &mut Water, now: f32) {
        for (slot, r) in water.ripples.iter_mut().zip(&self.ripples) {
            *slot = Ripple {
                start: now - (self.clock - r.start),
                ..*r
            };
        }
    }

    /// Rings the surface where the body crosses it: often while it moves,
    /// now and then while it floats still. Wakes are W6's.
    fn ripple(&mut self, feet: Vec3, dt: f32) {
        self.clock += dt;
        let clock = self.clock;
        self.ripples.retain(|r| clock - r.start < RIPPLE_LIFE);
        let at = Vec2::new(feet.x, feet.z);
        let moved = self.last.map_or(0.0, |last| (at - last).length());
        self.last = Some(at);
        let crossing = ew::surface(feet.x, feet.z)
            .is_some_and(|top| feet.y < top && feet.y + medium::EYE_HEIGHT as f32 > top);
        if !crossing {
            self.ripple_wait = 0.0;
            return;
        }
        self.ripple_wait -= dt;
        if self.ripple_wait > 0.0 {
            return;
        }
        let moving = moved > 0.2 * dt;
        let (wait, strength) = if moving {
            (RIPPLE_MOVING, RIPPLE_STRENGTH.0)
        } else {
            (RIPPLE_STILL, RIPPLE_STRENGTH.1)
        };
        self.ripple_wait = wait;
        if self.ripples.len() >= MOST_RIPPLES {
            self.ripples.remove(0);
        }
        self.ripples.push(Ripple {
            at: at.to_array(),
            start: clock,
            strength,
        });
    }

    /// Sets the camera's pitch, rad, positive looking down.
    pub fn set_pitch(&mut self, pitch: f32) {
        if pitch.is_finite() {
            self.pitch = pitch;
        }
    }

    /// The speed multiplier the medium and Exhaustion give: half wading or
    /// swimming (SRD 5.2.1), less a sixth a level. A stroke pitched down
    /// or up spends the rest of its speed going that way.
    #[must_use]
    pub fn pace(&self) -> f32 {
        let pace = pace(self.medium, false, false, self.breath.levels());
        if self.medium.afloat() && self.steered() {
            pace * self.pitch.cos()
        } else {
            pace
        }
    }

    /// Whether the camera's pitch steers a forward stroke down or up.
    fn steered(&self) -> bool {
        self.pitch > DIVE_PITCH || self.pitch < RISE_PITCH
    }

    /// A swimmer's stroke speed, m/s: half its run, less Exhaustion.
    fn stroke_speed(&self) -> f32 {
        crate::controller::RUN_SPEED * pace(Medium::Swimming, false, false, self.breath.levels())
    }

    /// The HUD's breath bar, while the eye is under or breath is short.
    #[must_use]
    pub fn bar(&self) -> Option<BreathBar> {
        (self.under || !self.breath.full()).then(|| BreathBar {
            left: self.breath.left(),
            limit: self.breath.limit(),
            levels: self.breath.levels(),
            under: self.under,
        })
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > MOST_LOG {
            self.log.remove(0);
        }
    }

    /// The medium after the controller's step moved `player` over `solids`
    /// with `input` from feet at `before`: holds a swimmer at its stroke's
    /// height (the water, not gravity, holds it up), lets the current
    /// carry it, climbs it out over a low lip, and spends or refills its
    /// breath.
    pub fn after_step(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        before: f32,
        solids: &super::solids::Solids,
        dt: f32,
    ) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let (x, z) = (player.pos.x, player.pos.z);
        let top = ew::surface(x, z);
        let bed = solids.floor(x, z, player.pos.y);
        let mut now = medium::classify(top.map(f64::from), f64::from(bed), f64::from(player.pos.y));
        if let (Some(top), true) = (top, now.afloat()) {
            let speed = self.stroke_speed();
            if input.jump {
                self.rising = true;
            }
            let vertical = if self.rising {
                speed
            } else if input.forward && self.steered() {
                // Down when the camera looks down, up when it looks up.
                -speed * self.pitch.sin()
            } else {
                0.0
            };
            let stroke = Stroke {
                vertical: f64::from(vertical),
                held: false,
            };
            // A swimmer afloat last step ignores the step's gravity.
            let feet = if self.medium.afloat() {
                before.max(bed)
            } else {
                player.pos.y
            };
            let y = medium::swim_height(
                f64::from(feet),
                f64::from(top),
                f64::from(bed),
                stroke,
                f64::from(dt),
            ) as f32;
            if self.rising && y >= top - FLOAT_DEPTH as f32 - 0.02 {
                self.rising = false;
            }
            // The current carries the swimmer at the water's own speed.
            let [fx, fz] = ew::current(x, z);
            player.pos.x += fx * dt;
            player.pos.z += fz * dt;
            player.set_surface_height(bed.min(y));
            player.hold_altitude(y);
            if input.forward {
                self.climb_out(player, top, solids);
            }
            let (x, z) = (player.pos.x, player.pos.z);
            let bed = solids.floor(x, z, player.pos.y);
            now = medium::classify(
                ew::surface(x, z).map(f64::from),
                f64::from(bed),
                f64::from(player.pos.y),
            );
        } else {
            self.rising = false;
        }
        if now != self.medium && now != Medium::Ground && !self.medium.wet() {
            let place = place_name(player.pos.x, player.pos.z);
            self.say(format!("You wade into {place}"));
        }
        self.medium = now;
        self.ripple(player.pos, dt);
        let eye = player.pos.y + medium::EYE_HEIGHT as f32;
        self.under = ew::surface(player.pos.x, player.pos.z).is_some_and(|top| eye < top);
        for event in self.breath.tick(dt, self.under, false) {
            match event {
                BreathEvent::OutOfBreath => self.say("You are out of breath".into()),
                BreathEvent::Exhausted(level) => {
                    self.say(format!("Suffocating: Exhaustion level {level}"));
                }
                BreathEvent::Recovered(levels) => {
                    self.say(format!(
                        "You breathe again; {levels} Exhaustion level{} gone",
                        if levels == 1 { "" } else { "s" }
                    ));
                }
                BreathEvent::Defeated => {
                    // Nothing dies in Everglade: the swimmer comes to on the
                    // nearest bank.
                    self.defeats += 1;
                    let [bx, bz] = ew::nearest_bank(player.pos.x, player.pos.z);
                    let ground = solids.floor(bx, bz, height(bx, bz) + 1.0);
                    player.pos = Vec3::new(bx, ground, bz);
                    player.set_surface_height(ground);
                    player.set_vertical_speed(0.0);
                    self.medium = Medium::Ground;
                    self.under = false;
                    self.rising = false;
                    self.say("You black out, and come to on the bank".into());
                }
            }
        }
    }

    /// Climbs a swimmer pressing forward out onto a lip ahead no more than
    /// [`CLIMB_LIP`] over the water at `top`, with no check (Ours).
    fn climb_out(&self, player: &mut PlayerController, top: f32, solids: &super::solids::Solids) {
        let ahead = player.pos + player.forward() * (crate::controller::RADIUS + 0.35);
        let lip = solids.floor(
            ahead.x,
            ahead.z,
            top + CLIMB_LIP as f32 - super::solids::STEP,
        );
        let deep = ew::surface(ahead.x, ahead.z).is_some_and(|t| t - lip > WADE_DEPTH as f32);
        if !deep
            && medium::can_climb_out(f64::from(top), f64::from(lip))
            && lip > player.pos.y + super::solids::STEP
        {
            player.pos = Vec3::new(ahead.x, lip, ahead.z);
            player.set_surface_height(lip);
            player.set_vertical_speed(0.0);
        }
    }
}

/// The name of the water at `(x, z)`.
#[must_use]
pub fn place_name(x: f32, z: f32) -> &'static str {
    let near = PONDS
        .iter()
        .position(|([cx, cz], r)| (x - cx).hypot(z - cz) < r + 0.5);
    near.map_or(ew::STREAM_NAME, |k| ew::POND_NAMES[k])
}

/// The breath bar's frame in logical points for a screen of `size`, just
/// over Everglade's hotbar raised `bottom` points.
#[must_use]
pub fn breath_frame(size: [f32; 2], bottom: f32) -> [f32; 4] {
    let u = super::hotbar::unit(size);
    let tray = super::hotbar::frame(size, bottom);
    let (w, h) = (180.0 * u, 10.0 * u);
    [(size[0] - w) * 0.5, tray[1] - h - 18.0 * u, w, h]
}

/// The HUD's breath bar over the hotbar, with the seconds left, amber
/// under ten seconds, and the Exhaustion debuff icon with its level beside
/// it once suffocation has begun.
pub fn draw_breath(
    batch: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    bottom: f32,
    bar: &BreathBar,
) {
    let [x, y, w, h] = breath_frame(size, bottom);
    let u = super::hotbar::unit(size);
    let share = (bar.left / bar.limit.max(1e-3)).clamp(0.0, 1.0);
    let warn = bar.left < WARN;
    batch.rect(
        atlas,
        x - 2.0,
        y - 2.0,
        w + 4.0,
        h + 4.0,
        [0.02, 0.03, 0.05, 0.75],
    );
    let fill = if warn {
        [0.95, 0.55, 0.15, 0.95]
    } else {
        [0.35, 0.72, 0.98, 0.95]
    };
    batch.rect(atlas, x, y, w * share, h, fill);
    let label = if bar.left > 0.0 {
        format!("Breath {:.0} s", bar.left.ceil())
    } else {
        "Out of breath".to_owned()
    };
    let text = if warn {
        [1.0, 0.7, 0.35, 1.0]
    } else {
        [0.9, 0.95, 1.0, 1.0]
    };
    batch.text(atlas, x, y - atlas.line - 2.0 * u, &label, text);
    if bar.levels > 0 {
        let side = 26.0 * u;
        let (ix, iy) = (x + w + 8.0 * u, y + h * 0.5 - side * 0.5);
        if atlas.sprites.contains_key(EXHAUSTION_ICON) {
            batch.image(atlas, EXHAUSTION_ICON, ix, iy, side, side, [1.0; 4]);
        } else {
            batch.rect(atlas, ix, iy, side, side, [0.55, 0.18, 0.1, 0.95]);
        }
        batch.text(
            atlas,
            ix + side + 4.0 * u,
            iy + side * 0.5 - atlas.line * 0.5,
            &format!("Exhaustion {}", bar.levels),
            [1.0, 0.6, 0.4, 1.0],
        );
    }
}

/// The Exhaustion debuff icon's sprite.
pub const EXHAUSTION_ICON: &str = "exhaustion-icon";

/// Adds the Exhaustion icon to `atlas`.
///
/// # Errors
///
/// Returns a message when the icon cannot rasterize.
pub fn add_sprites(atlas: &mut Atlas) -> Result<(), String> {
    use crate::imported::icons;
    let icon = icons::icon(EXHAUSTION_ICON).ok_or("no Exhaustion icon")?;
    atlas.add_sprite(
        EXHAUSTION_ICON,
        icons::SIZE,
        icons::SIZE,
        &icons::rasterize(icon)?,
    )
}

#[cfg(test)]
#[path = "water_tests.rs"]
mod tests;
