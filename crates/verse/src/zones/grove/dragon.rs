//! Shapechange in the Grove: the druid becomes a dragon, and back.
//!
//! Casting Shapechange starts a transformation ([`Morph`]): a rune circle
//! flares on the ground and a vortex of leaves and arcane motes rises and
//! closes on the druid, who spins, shrinks, and dissolves into it. At
//! [`SWAP`] a flash and a shockwave burst out, and the dragon grows out of
//! the light to its full size, about three times the druid's height,
//! roaring. Return to Form runs the same transformation the other way.
//!
//! As a dragon the druid keeps its spells (Shapechange keeps spellcasting)
//! and gains the dragon's five actions on row 2: a bite, a cone of Fire
//! Breath that leaves dummies burning, a Tail Sweep that knocks them down,
//! a Wing Buffet that throws them back, and a Roar that frightens them.
//! Each plays its own clip from the pack's dragon. The dice are an adult
//! red dragon's from the SRD, rolled behind the scenes like every other
//! roll in the Grove. The dragon walks, and with Jump it takes off and
//! flies faster than the eagle, beating its wings to climb and gliding
//! when it races level.

use super::kit::{Area, FT};
use super::*;
use crate::fx::{Handle, Spawn};

/// When the new shape bursts out, s after the cast.
pub const SWAP: f32 = 1.0;
/// When the druid's own shape returns after Return to Form, s.
pub const SWAP_BACK: f32 = 0.7;
/// How long the new shape takes to grow to its size after the swap, s.
pub const GROW: f32 = 0.55;
/// When the breath's fire leaves the jaws and when it ends, s after the
/// cast: the breath clip draws in first.
pub const BREATH_START: f32 = 0.55;
pub const BREATH_END: f32 = 1.6;
/// Where the jaws are in a breath, at the dragon's modeled size: ahead of
/// its feet and above them, m.
const MOUTH: [f32; 2] = [5.0, 3.1];
/// What each of Fire Breath's ten flame lines does to a structure it meets:
/// big, so a few breaths gut a wall or bring the tower down.
const BREATH_STRUCTURE_DAMAGE: i32 = 140;
/// How far the breath tilts down from level, radians, so it reaches the
/// ground about its length ahead.
const BREATH_TILT: f32 = 0.22;
/// Wing Buffet's throw, m.
const BUFFET_PUSH: f32 = 20.0 * FT;
/// Burning's damage each second: 1d4 fire.
const BURN: (u32, u32) = (1, 4);
/// How long the camera shakes after a roar, s.
const ROAR_SHAKE: f32 = 0.8;

/// A change of shape in progress: the shape it leaves (`None` for the
/// druid's own), the one it takes, when it began, and when the two swap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Morph {
    pub from: Option<Form>,
    pub to: Option<Form>,
    pub start: f32,
    pub swap: f32,
    /// Whether the swap has happened.
    pub swapped: bool,
}

impl Morph {
    /// Whether it has finished at `now`.
    #[must_use]
    pub fn done(&self, now: f32) -> bool {
        now - self.start >= self.swap + GROW
    }

    /// The shape drawn at `now` (`None` for the druid's own), its size as
    /// a share of full, and how far it has spun about its feet, radians.
    /// Before the swap the old shape spins faster and faster as it shrinks
    /// into the vortex; after it the new shape grows out, overshooting a
    /// little, as the spin unwinds.
    #[must_use]
    pub fn drawn(&self, now: f32) -> (Option<Form>, f32, f32) {
        let t = (now - self.start).max(0.0);
        if t < self.swap {
            let k = 1.0 - smooth((t - 0.45 * self.swap) / (0.55 * self.swap));
            let spin = 3.0 * std::f32::consts::PI * smooth(t / self.swap).powi(2);
            (self.from, k, spin)
        } else {
            let g = ((t - self.swap) / GROW).clamp(0.0, 1.0);
            let k = 0.1 + 0.9 * back_out(g);
            let spin = std::f32::consts::PI * (1.0 - smooth(g));
            (self.to, k, spin)
        }
    }
}

/// A breath of fire in progress: when it began, its particles once the
/// fire leaves the jaws, and whether its dice have rolled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    pub start: f32,
    pub fire: Option<Handle>,
    pub struck: bool,
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Eases out past one and back, for a shape that bursts to its size.
fn back_out(x: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    let x = x.clamp(0.0, 1.0) - 1.0;
    1.0 + c3 * x * x * x + c1 * x * x
}

impl Grove {
    /// Casts Shapechange: the transformation into the dragon begins.
    ///
    /// # Errors
    ///
    /// Returns why not: a transformation already under way, or the pack
    /// lacking the dragon.
    pub(super) fn shapechange(&mut self, player: &PlayerController) -> Result<(), String> {
        if self.morph.is_some() {
            return Err("Your shape is already changing".into());
        }
        if self.form() == Some(Form::Dragon) {
            return Err("You already wear the dragon's shape".into());
        }
        if self
            .beasts
            .get(Form::Dragon.index())
            .is_none_or(Option::is_none)
        {
            return Err("The pack has no dragon".into());
        }
        self.begin_morph(Some(Form::Dragon), SWAP, player.pos);
        self.say("Shapechange: leaves and light whirl around you".into());
        Ok(())
    }

    /// Return to Form from the dragon: the transformation runs back.
    pub(super) fn morph_back(&mut self, player: &PlayerController) {
        self.begin_morph(None, SWAP_BACK, player.pos);
        self.say("You call your own shape back from the dragon's".into());
    }

    fn begin_morph(&mut self, to: Option<Form>, swap: f32, feet: Vec3) {
        let now = self.time;
        self.stop_breath();
        self.morph = Some(Morph {
            from: self.form(),
            to,
            start: now,
            swap,
            swapped: false,
        });
        let ground = Vec3::new(feet.x, everglade::height(feet.x, feet.z) + 0.05, feet.z);
        let size = if to.is_some() { 1.0 } else { 0.8 };
        let _ = self
            .fx
            .start("grove_shapechange_rune", Spawn::at(ground).scaled(size));
        let _ = self.fx.start(
            "grove_shapechange_vortex",
            Spawn::at(ground).scaled(size * 1.1),
        );
    }

    /// Advances a transformation: at its swap the new shape takes over,
    /// with a flash, a shockwave, and, for the dragon, a roar.
    pub(super) fn tick_morph(&mut self, glade: &mut Everglade, player: &PlayerController) {
        let now = self.time;
        let Some(morph) = self.morph else {
            return;
        };
        if !morph.swapped && now - morph.start >= morph.swap {
            if let Some(m) = &mut self.morph {
                m.swapped = true;
            }
            self.wear(morph.to, player, glade);
            let at = player.pos + Vec3::Y * 1.5;
            let size = if morph.to.is_some() { 1.4 } else { 0.7 };
            let _ = self
                .fx
                .start("grove_shapechange_flash", Spawn::at(at).scaled(size));
            match morph.to {
                Some(form) => {
                    if let Some(shape) = &mut self.shape {
                        shape.clip = Some(("roar", now));
                    }
                    self.roared = now;
                    let ground = Vec3::new(player.pos.x, player.pos.y + 0.05, player.pos.z);
                    self.burst("grove_dragon_roar", ground);
                    self.say(format!(
                        "You take the shape of a {}",
                        form.name().to_lowercase()
                    ));
                }
                None => self.say("You stand in your own shape again".into()),
            }
        }
        if morph.done(now) {
            self.morph = None;
        }
    }

    /// Takes `form`'s shape at once, or the druid's own with `None`: a
    /// flying form that starts aloft takes to the air, and leaving a
    /// flying form for one that doesn't fly lands.
    pub(super) fn wear(
        &mut self,
        form: Option<Form>,
        player: &PlayerController,
        glade: &mut Everglade,
    ) {
        self.shape = form.map(|form| Shape {
            form,
            attack: None,
            clip: None,
        });
        let flies = form.is_some_and(Form::flies);
        if glade.levitating && !flies {
            glade.toggle_levitate(player);
        }
        if let Some(form) = form
            && form.starts_aloft()
            && !glade.levitating
        {
            glade.toggle_levitate(player);
            glade.altitude = player.pos.y + 3.0;
        }
        glade.set_lift(form.map_or(1.0, Form::lift));
    }

    /// The shape's pace for `player`: the dragon's in the air while it
    /// flies, else its own.
    #[must_use]
    pub fn pace(&self, glade: &Everglade, player: &PlayerController) -> f32 {
        match self.form() {
            Some(form) if form.flies() && aloft(glade, player) => form.air_pace(),
            Some(form) => form.pace(),
            None => 1.0,
        }
    }

    /// How many times its usual distance the camera stands back, eased
    /// toward what the shape needs.
    #[must_use]
    pub fn camera(&self) -> f32 {
        self.pull
    }

    /// Eases the camera's pull toward the shape coming or worn, over `dt`.
    pub(super) fn tick_camera(&mut self, dt: f32) {
        let want = match self.morph {
            Some(m) => m.to.or(m.from).map_or(1.0, Form::camera),
            None => self.form().map_or(1.0, Form::camera),
        };
        let k = 1.0 - (-2.5 * dt.max(0.0)).exp();
        self.pull += (want - self.pull) * k;
    }

    /// The druid's own shape at `now`, while a transformation draws it: its
    /// size as a share of full, and its spin, radians. `None` draws it as
    /// it is.
    #[must_use]
    pub(super) fn druid_morph(&self) -> Option<(f32, f32)> {
        let (shown, k, spin) = self.morph?.drawn(self.time);
        shown.is_none().then_some((k, spin))
    }

    /// Poses the shape the druid wears when a transformation or the
    /// dragon's own clips decide its pose, and says whether it did; the
    /// other shapes pose as Wild Shape poses them.
    pub(super) fn pose_special(
        &mut self,
        dt: f32,
        glade: &Everglade,
        player: &PlayerController,
    ) -> bool {
        let now = self.time;
        let Some(shape) = self.shape else {
            return false;
        };
        let (k, spin) = match self.morph {
            Some(m) => {
                let (shown, k, spin) = m.drawn(now);
                if shown != Some(shape.form) {
                    return false;
                }
                (k, spin)
            }
            None => (1.0, 0.0),
        };
        if shape.form != Form::Dragon && self.morph.is_none() {
            return false;
        }
        let up = aloft(glade, player);
        let climbing = player.pos.y - self.last_y > 0.4 * dt;
        let scale = shape.form.scale() * k;
        let yaw = player.yaw + spin;
        let Some(Some(beast)) = self.beasts.get_mut(shape.form.index()) else {
            return false;
        };
        if shape.form != Form::Dragon {
            // A Wild Shape beast shrinking into Shapechange's vortex.
            beast.pose(
                player.pos,
                yaw,
                scale,
                crate::zones::everglade::player::Motion::Idle,
                now,
                dt,
            );
            return true;
        }
        let action = shape.clip.and_then(|(name, at)| {
            let t = now - at;
            (t < beast.clip_length(name).unwrap_or(0.0)).then_some((name, t))
        });
        if action.is_none()
            && let Some(s) = &mut self.shape
        {
            s.clip = None;
        }
        let posed = match action {
            Some((name, t)) => beast.pose_clip(player.pos, yaw, scale, name, t, false, dt),
            None if up => {
                // Wings beat to climb and to hold still, and now and then
                // in level flight; otherwise it glides.
                let glide = !climbing && player.speed > 6.0 && now.rem_euclid(6.0) > 1.6;
                let name = if glide { "glide" } else { "fly" };
                beast.pose_clip(player.pos, yaw, scale, name, now, true, dt)
            }
            None => false,
        };
        if !posed {
            beast.pose_walking(player, yaw, scale, dt);
        }
        true
    }

    /// The dragon's jaws for `player`: where a breath leaves them, and
    /// where its spells fly from.
    #[must_use]
    pub(super) fn mouth(&self, player: &PlayerController) -> Vec3 {
        let scale = Form::Dragon.scale();
        player.pos + player.forward() * (MOUTH[0] * scale) + Vec3::Y * (MOUTH[1] * scale)
    }

    /// Starts `name`'s clip on the dragon.
    fn play_clip(&mut self, name: &'static str) {
        let now = self.time;
        if let Some(shape) = &mut self.shape {
            shape.clip = Some((name, now));
        }
    }

    /// One of the dragon's five actions, `spell`, for `player`.
    ///
    /// # Errors
    ///
    /// Returns why the action was refused: a bite with no dummy at the
    /// jaws.
    pub(super) fn dragon_act(
        &mut self,
        spell: Spell,
        player: &mut PlayerController,
    ) -> Result<(), String> {
        let now = self.time;
        let def = spell.def();
        let reach = match def.area {
            Area::Cone(length) => length,
            Area::Burst(radius) => radius,
            _ => def.range,
        };
        if let Some(i) = self.target(player, reach) {
            let to = self.dummies[i].pos - player.pos;
            player.yaw = to.x.atan2(to.z);
        }
        let forward = player.forward();
        let feet = player.pos;
        let ground = |p: Vec3| Vec3::new(p.x, everglade::height(p.x, p.z) + 0.05, p.z);
        match spell {
            Spell::DragonBite => {
                let i = self.target(player, def.range).ok_or_else(|| {
                    format!("{} needs a dummy within {:.0} m", def.label, def.range)
                })?;
                self.play_clip("bite");
                let at = self.dummies[i].center();
                self.burst("grove_flame_hit", at);
                self.strike(spell, i);
            }
            Spell::FireBreath => {
                self.stop_breath();
                self.play_clip("breath");
                self.breath = Some(Breath {
                    start: now,
                    fire: None,
                    struck: false,
                });
            }
            Spell::TailSweep => {
                self.play_clip("sweep");
                let center = feet - forward * 1.0;
                self.burst("grove_tail_sweep", ground(center));
                for i in self.within(center + Vec3::Y * 0.9, reach) {
                    self.strike(spell, i);
                }
            }
            Spell::WingBuffet => {
                self.play_clip("fly");
                let center = feet + forward * 1.5;
                let _ = self.fx.start(
                    "grove_wing_buffet",
                    Spawn::at(ground(feet + forward * 3.0)).along(forward),
                );
                for i in self.within(center + Vec3::Y * 0.9, reach) {
                    if self.strike(spell, i).takes_hold() {
                        let away = self.dummies[i].pos - feet;
                        self.dummies[i].push(away, BUFFET_PUSH, now);
                    }
                }
            }
            Spell::Roar => {
                self.play_clip("roar");
                self.roared = now;
                self.burst("grove_dragon_roar", ground(feet + forward * 3.0));
                for i in self.within(feet + Vec3::Y * 0.9, reach) {
                    self.strike(spell, i);
                }
            }
            _ => return Err(format!("{} is not the dragon's", def.label)),
        }
        Ok(())
    }

    /// Ends a breath under way, its fire lingering out.
    pub(super) fn stop_breath(&mut self) {
        if let Some(breath) = self.breath.take()
            && let Some(fire) = breath.fire
        {
            self.fx.stop(fire);
        }
    }

    /// Advances a breath: the fire leaves the jaws, rolls its dice against
    /// every dummy in the cone, follows the head, and ends.
    pub(super) fn tick_breath(&mut self, player: &PlayerController) {
        let now = self.time;
        let Some(breath) = self.breath else {
            return;
        };
        let age = now - breath.start;
        if self.form() != Some(Form::Dragon) || age >= BREATH_END {
            self.stop_breath();
            return;
        }
        if age < BREATH_START {
            return;
        }
        let mouth = self.mouth(player);
        let forward = player.forward();
        let along = (forward * BREATH_TILT.cos() - Vec3::Y * BREATH_TILT.sin()).normalize();
        match breath.fire {
            Some(fire) => self.fx.place(fire, mouth, Vec3::ZERO),
            None => {
                let fire = self
                    .fx
                    .start("grove_dragon_breath", Spawn::at(mouth).along(along));
                if let Some(b) = &mut self.breath {
                    b.fire = fire;
                }
            }
        }
        if !breath.struck {
            if let Some(b) = &mut self.breath {
                b.struck = true;
            }
            self.breathe(player.pos, forward);
        }
    }

    /// Fire Breath's dice against every dummy in its cone from `feet`
    /// along `forward`: a cone as wide as half its length at its end,
    /// starting at the jaws.
    fn breathe(&mut self, feet: Vec3, forward: Vec3) {
        let Area::Cone(length) = Spell::FireBreath.def().area else {
            return;
        };
        let half = 0.5f32.atan();
        let reach = length + MOUTH[0];
        // The fire also scorches the tower: lines fanned across the cone at
        // the jaws' height and a little lower each blast what they meet.
        let scale = Form::Dragon.scale();
        let jaws = feet + forward * (MOUTH[0] * scale) + Vec3::Y * (MOUTH[1] * scale);
        for k in -2..=2 {
            for pitch in [0.0f32, -0.12] {
                let yaw = glam::Quat::from_rotation_y(half * 0.9 * k as f32 / 2.0);
                let dir = (yaw * forward + Vec3::Y * pitch).normalize_or(forward);
                self.chips.push(super::Chip::Line {
                    from: jaws,
                    toward: dir,
                    length: length,
                    damage: BREATH_STRUCTURE_DAMAGE,
                });
            }
        }
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let to = d.center() - feet;
            let flat = Vec3::new(to.x, 0.0, to.z);
            if d.down() || flat.length() > reach || flat.length() < 0.5 {
                continue;
            }
            // The cone's apex sits behind the jaws, so it is a jaw's width
            // across where the fire leaves them.
            let apex = feet - forward * 1.5;
            let from_apex = Vec3::new(d.center().x - apex.x, 0.0, d.center().z - apex.z);
            if forward.angle_between(from_apex) <= half {
                self.strike(Spell::FireBreath, i);
            }
        }
    }

    /// Burning's fire each second, and its flames on each burning dummy,
    /// put out when it ends.
    pub(super) fn tick_burning(&mut self) {
        let now = self.time;
        if now >= self.next_burn {
            self.next_burn = now + 1.0;
            for i in 0..self.dummies.len() {
                if self.dummies[i].has(Condition::Burning, now) && !self.dummies[i].down() {
                    let amount = self.dice.sum(BURN.0, BURN.1) as f32;
                    let dealt = self.dummies[i].damage(amount, Damage::Fire, now);
                    self.float(i, dealt.to_string(), Damage::Fire.color());
                }
            }
        }
        let mut burns = std::mem::take(&mut self.burns);
        burns.retain(|&(i, handle)| {
            let lit = self
                .dummies
                .get(i)
                .is_some_and(|d| d.has(Condition::Burning, now) && !d.down());
            if lit {
                let at = self.dummies[i].center();
                self.fx.place(handle, at, Vec3::ZERO);
            } else {
                self.fx.stop(handle);
            }
            lit
        });
        for i in 0..self.dummies.len() {
            if self.dummies[i].has(Condition::Burning, now)
                && !self.dummies[i].down()
                && !burns.iter().any(|(b, _)| *b == i)
                && let Some(handle) = self
                    .fx
                    .start("grove_burning", Spawn::at(self.dummies[i].center()))
            {
                burns.push((i, handle));
            }
        }
        self.burns = burns;
    }

    /// The camera's jolt from the dragon's newest roar, m.
    #[must_use]
    pub(super) fn roar_shake(&self) -> Vec3 {
        let age = self.time - self.roared;
        if (0.0..ROAR_SHAKE).contains(&age) {
            draw::shake(age * 0.5) * 1.6
        } else {
            Vec3::ZERO
        }
    }

    /// Moves the druid's own vertices, the first `count` of `vertices`,
    /// as a transformation draws them: spun `spin` about the druid's
    /// middle above `feet`, shrunk to `k` of their size toward it, and
    /// lifted into the vortex as they shrink.
    pub(super) fn dissolve(
        vertices: &mut [TexturedVertex],
        count: usize,
        feet: Vec3,
        k: f32,
        spin: f32,
    ) {
        let middle = feet + Vec3::Y * 0.9;
        let turn = glam::Quat::from_rotation_y(spin);
        let rise = Vec3::Y * (1.0 - k) * 1.2;
        for v in vertices.iter_mut().take(count) {
            let p = Vec3::from(v.pos);
            v.pos = (middle + turn * (p - middle) * k + rise).to_array();
            v.normal = (turn * Vec3::from(v.normal)).to_array();
        }
    }
}

/// Whether the druid's flying shape is in the air above `player`: aloft
/// and clear of the ground.
#[must_use]
pub(super) fn aloft(glade: &Everglade, player: &PlayerController) -> bool {
    glade.levitating && player.pos.y > everglade::height(player.pos.x, player.pos.z) + 0.8
}

use crate::pbr::textured::TexturedVertex;
