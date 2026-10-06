//! Casting in the Grove: what each ability does when its slot is pressed.
//!
//! The glade's spells (Wind Wall, Wall of Stone, Reverse Gravity) go
//! through Everglade's rules, and Thunderwave, Gust of Wind, Fire Bolt,
//! Fireball, Misty Step, and Web keep the shapes the Grove first gave them.
//! Every other spell lands by its [`Area`]: a projectile, the target alone,
//! a burst, a cone, a line, or a lasting zone or wall ([`super::aura`]).
//! Each roll goes through [`Grove::strike`]; each effect starts its
//! particles (`assets/verse/fx/effects/grove_*.toml`) and its line-drawn
//! shape ([`super::draw`]).

use super::kit::{Area, Damage, FT};
use super::*;
use crate::fx::Spawn;
use glam::DVec3;

/// How long a web holds, s (two rounds).
const WEB_ROOT: f32 = 12.0;
/// Bolt speeds, m/s.
const BOLT_SPEED: f32 = 32.0;
const FIREBALL_SPEED: f32 = 24.0;
/// Gust of Wind's push, m.
const GUST_PUSH: f32 = 15.0 * 0.3048;
/// Wind Wall's lift on a dummy that fails its save, m/s.
const WIND_LIFT: f32 = 8.0;
/// How far Wall of Stone shoves a dummy past its face, m.
const SHOVE_CLEARANCE: f64 = 0.5;
/// A dummy's reach into an area beyond its center, m.
const BODY: f32 = 0.35;
/// How far ahead a spell lands with no dummy to aim at, m.
const AHEAD: f32 = 10.0;
/// Ice Knife's burst: a 5-foot radius, 2d6 cold on a failed Dexterity
/// save.
const ICE_BURST: f32 = 5.0 * FT;

impl Grove {
    /// Casts `spell` for `player`, who turns to face its target, through
    /// `glade` for the glade's spells and the eagle's flight.
    ///
    /// # Errors
    ///
    /// Returns why the cast was refused: no target, a beast's attack out of
    /// its shape, or a glade spell's own rule. No mana or cooldown ever
    /// refuses one.
    pub fn cast(
        &mut self,
        spell: Spell,
        player: &mut PlayerController,
        glade: &mut Everglade,
    ) -> Result<(), String> {
        let now = self.time;
        match spell {
            Spell::LongRest => {
                self.long_rest(player, glade);
                return Ok(());
            }
            Spell::Shapechange => return self.shapechange(player),
            Spell::ReturnToForm => {
                if self.morph.is_some() {
                    return Err("Your shape is already changing".into());
                }
                let shape = self.shape.ok_or("You already stand in your own shape")?;
                if shape.form == Form::Dragon {
                    self.morph_back(player);
                    return Ok(());
                }
                self.end_shape(player, glade);
                self.add(draw::Effect::Shift {
                    at: player.pos,
                    start: now,
                });
                self.say(format!("You drop the {}'s shape", shape.form.name()));
                return Ok(());
            }
            Spell::ChooseLand => {
                self.land = self.land.next();
                let names: Vec<&str> = self.land.spells().iter().map(|s| s.def().label).collect();
                self.say(format!(
                    "Choose Land: {}, with {}",
                    self.land.name(),
                    names.join(", ")
                ));
                return Ok(());
            }
            Spell::SpeakWithAnimals => {
                let line = BIRDS[(self.dice.roll(BIRDS.len() as u32) - 1) as usize];
                self.burst("grove_cast_burst", player.pos + Vec3::Y * 1.6);
                self.say(format!("A sparrow says: {line}"));
                return Ok(());
            }
            _ => {}
        }
        if let Some(form) = Form::of(spell) {
            return self.take_shape(form, player, glade);
        }
        // Meteor Swarm and the Thunderbolt aim with the cursor at a wall
        // or the ground, and a click calls them down.
        if let Some(strike) = spell.strike() {
            return glade.target_strike(strike, player);
        }
        if spell.beast() {
            let shape = self.shape.ok_or_else(|| {
                format!(
                    "{} is a beast's attack: Wild Shape first",
                    spell.def().label
                )
            })?;
            if !shape.form.attacks().contains(&spell) {
                return Err(format!(
                    "A {} can't use {}",
                    shape.form.name(),
                    spell.def().label
                ));
            }
        }
        if Form::Dragon.attacks().contains(&spell) {
            return self.dragon_act(spell, player);
        }
        // Pressing a live concentration spell's slot casts it again: the
        // old one ends and the new one rises where the druid faces now.
        // Reverse Gravity only ends, so what it lifted falls back down.
        let mut ended = false;
        if let Some(glade_spell) = spell.glade()
            && glade.spell_active(glade_spell)
        {
            glade.cast_spell(glade_spell, player)?;
            ended = true;
        }
        let def = spell.def();
        // A melee strike reaches only as far as the fangs or the staff.
        let reach = if def.area == Area::Single && def.range < 6.0 {
            def.range
        } else {
            def.range.max(6.0)
        };
        let target = self.target(player, reach);
        if spell.needs_target() && target.is_none() {
            return Err(format!(
                "{} needs a dummy in front within {:.0} m",
                def.label, reach
            ));
        }
        if let Some(i) = target {
            let to = self.dummies[i].pos - player.pos;
            player.yaw = to.x.atan2(to.z);
        }
        let feet = player.pos;
        let forward = player.forward();
        // A dragon's spells leave its jaws.
        let hand = if self.form() == Some(Form::Dragon) {
            self.mouth(player)
        } else {
            feet + Vec3::Y * 1.4 + forward * 0.3
        };
        // Where an area lands: the target, or ahead with none.
        let point = target.map_or_else(
            || {
                let mut at = feet + forward * def.range.min(AHEAD).max(3.0);
                at.y = everglade::height(at.x, at.z);
                at
            },
            |i| self.dummies[i].pos,
        );
        if spell.beast() {
            self.attack();
        }
        match spell {
            Spell::FireBolt | Spell::Fireball => {
                let i = target.ok_or("no target")?;
                let distance = hand.distance(self.dummies[i].center());
                let speed = if spell == Spell::Fireball {
                    FIREBALL_SPEED
                } else {
                    BOLT_SPEED
                };
                self.add(draw::Effect::Bolt {
                    from: hand,
                    target: i,
                    start: now,
                    flight: distance / speed,
                    spell,
                    trail: None,
                });
            }
            Spell::Thunderwave => self.thunderwave(feet, forward),
            Spell::GustOfWind => {
                let line = verse_world::gust::Line::new(feet.as_dvec3(), forward.as_dvec3())
                    .ok_or("Gust of Wind needs a direction")?;
                for i in 0..self.dummies.len() {
                    let d = &self.dummies[i];
                    let radius = f64::from(BODY * d.kind.scale());
                    if d.down() || !line.touches_sphere(d.center().as_dvec3(), radius) {
                        continue;
                    }
                    if self.strike(spell, i).takes_hold() {
                        self.dummies[i].push(forward, GUST_PUSH, now);
                    }
                }
                self.add(draw::Effect::Gust {
                    origin: feet,
                    forward,
                    start: now,
                });
            }
            Spell::WindWall | Spell::WallOfStone => {
                let ahead = target.map_or(f64::from(everglade::spells::AHEAD), |i| {
                    let to = self.dummies[i].pos - feet;
                    f64::from(to.x.hypot(to.z))
                });
                let glade_spell = spell.glade().ok_or("not a glade spell")?;
                glade.cast_spell_ahead(glade_spell, player, ahead)?;
                if spell == Spell::WindWall {
                    self.wind_wall(glade);
                } else {
                    self.shove(glade, feet);
                }
            }
            Spell::ReverseGravity if ended => {
                self.say("Reverse Gravity ends: everything comes back down".into());
            }
            Spell::ReverseGravity => {
                glade.cast_spell(everglade::spells::Spell::ReverseGravity, player)?;
                self.say("Reverse Gravity: everything nearby falls upward".into());
            }
            Spell::MistyStep | Spell::TreeStride => {
                let reach = def.range;
                let step = target.map_or(reach, |i| {
                    let to = self.dummies[i].pos - feet;
                    (to.x.hypot(to.z) - 2.0).clamp(0.0, reach)
                });
                let mut to = feet + forward * step;
                let r = to.x.hypot(to.z);
                if r > MEADOW_RADIUS {
                    to.x *= MEADOW_RADIUS / r;
                    to.z *= MEADOW_RADIUS / r;
                }
                to.y = everglade::height(to.x, to.z);
                let look = if spell == Spell::TreeStride {
                    "grove_vines"
                } else {
                    "grove_cast_burst"
                };
                self.add(draw::Effect::Mist {
                    at: feet,
                    start: now,
                });
                self.add(draw::Effect::Mist { at: to, start: now });
                self.burst(look, feet);
                self.burst(look, to);
                player.pos = to;
                player.set_surface_height(to.y);
                player.set_vertical_speed(0.0);
                if spell == Spell::TreeStride {
                    self.say("Tree Stride: you step through the trees".into());
                }
            }
            Spell::Web => {
                let i = target.ok_or("no target")?;
                let at = self.dummies[i].pos;
                // A 20-foot cube centered on the target.
                let half = 10.0 * 0.3048;
                for j in 0..self.dummies.len() {
                    let d = &self.dummies[j];
                    let off = d.pos - at;
                    if d.down() || off.x.abs() > half || off.z.abs() > half || off.y.abs() > half {
                        continue;
                    }
                    if self.strike(spell, j).takes_hold() {
                        self.afflict(j, Condition::Restrained, WEB_ROOT, "Web");
                    }
                }
                self.add(draw::Effect::Web {
                    at,
                    start: now,
                    until: now + WEB_ROOT,
                });
            }
            Spell::Elementalism => {
                const ELEMENTS: [(&str, &str); 3] = [
                    ("grove_cast_burst", "a gust stirs the grass"),
                    ("grove_flame_hit", "embers dance in the air"),
                    ("grove_thorns", "the ground trembles"),
                ];
                let (look, what) = ELEMENTS[(self.dice.roll(3) - 1) as usize];
                self.burst(look, point + Vec3::Y * 0.4);
                self.say(format!("Elementalism: {what}"));
            }
            _ if spell.placeholder() => {
                self.burst("grove_cast_burst", point + Vec3::Y * 1.0);
                self.decal(point, 2.0);
                self.say(format!(
                    "{}: not built in the Grove yet, so a labeled burst",
                    def.label
                ));
            }
            _ => match def.area {
                Area::Bolt => {
                    let i = target.ok_or("no target")?;
                    let distance = hand.distance(self.dummies[i].center());
                    let trail = self.fx.start(bolt_trail(spell), Spawn::at(hand));
                    self.add(draw::Effect::Bolt {
                        from: hand,
                        target: i,
                        start: now,
                        flight: distance / BOLT_SPEED,
                        spell,
                        trail,
                    });
                }
                Area::Single => {
                    let i = target.ok_or("no target")?;
                    let at = self.dummies[i].center();
                    self.burst(hit_look(spell), at);
                    if spell == Spell::ShockingGrasp {
                        self.add(draw::Effect::Strike {
                            from: hand,
                            to: at,
                            start: now,
                        });
                    }
                    self.strike(spell, i);
                }
                Area::Burst(radius) => {
                    let center = point + Vec3::Y * 0.9;
                    self.burst(hit_look(spell), point + Vec3::Y * burst_height(spell));
                    self.decal(point, radius);
                    for i in self.within(center, radius) {
                        self.strike(spell, i);
                    }
                }
                Area::Cone(length) => {
                    let _ = self
                        .fx
                        .start(hit_look(spell), Spawn::at(hand).along(forward));
                    // A cone as wide as it is long at its end.
                    let half = 0.5f32.atan();
                    for i in 0..self.dummies.len() {
                        let d = &self.dummies[i];
                        let to = d.center() - feet;
                        let flat = Vec3::new(to.x, 0.0, to.z);
                        if d.down() || flat.length() > length + BODY {
                            continue;
                        }
                        if flat.length() < 1.0 || forward.angle_between(flat) <= half {
                            self.strike(spell, i);
                        }
                    }
                }
                Area::Line(length, width) => {
                    let _ = self
                        .fx
                        .start(hit_look(spell), Spawn::at(hand).along(forward));
                    // Lightning chips the tower where the line meets it,
                    // with the dice's average.
                    if def.kind == Damage::Lightning {
                        let (count, sides) = def.dice;
                        let (from, length) = super::from_body(feet, hand, forward, length);
                        self.chips.push(super::Chip::Line {
                            from,
                            toward: forward,
                            length,
                            damage: (count * (sides + 1) / 2) as i32,
                        });
                    }
                    for i in 0..self.dummies.len() {
                        let d = &self.dummies[i];
                        let to = d.center() - feet;
                        let along = to.dot(forward);
                        let across = (to - forward * along).with_y(0.0).length();
                        if !d.down()
                            && (0.0..=length).contains(&along)
                            && across <= width / 2.0 + BODY
                        {
                            self.strike(spell, i);
                        }
                    }
                }
                Area::Zone(_) | Area::Wall(_) => {
                    self.raise(spell, point, forward);
                }
                Area::Caster => {
                    self.burst("grove_cast_burst", feet + Vec3::Y * 1.2);
                    self.say(def.label.to_string());
                }
            },
        }
        Ok(())
    }

    /// A burst's area on the ground: a faint ring of light spreading out to
    /// `radius` m around `at`, drawn by particles rather than lines.
    pub(super) fn decal(&mut self, at: Vec3, radius: f32) {
        let ground = Vec3::new(at.x, everglade::height(at.x, at.z) + 0.05, at.z);
        let _ = self
            .fx
            .start("grove_area_ring", Spawn::at(ground).scaled(radius.max(0.5)));
    }

    /// The standing dummies whose bodies reach within `radius` of `center`.
    pub(super) fn within(&self, center: Vec3, radius: f32) -> Vec<usize> {
        (0..self.dummies.len())
            .filter(|&i| {
                let d = &self.dummies[i];
                !d.down() && d.center().distance(center) <= radius + BODY * d.kind.scale() + 0.5
            })
            .collect()
    }

    /// Thunderwave's cube from the caster: damage and a push.
    fn thunderwave(&mut self, feet: Vec3, forward: Vec3) {
        let now = self.time;
        let cube = verse_world::spells::thunderwave::Cube::new(feet.as_dvec3(), forward.as_dvec3());
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let probe = d.pos.as_dvec3() + DVec3::Y * 0.9;
            if d.down() || !cube.contains(probe) {
                continue;
            }
            let away = cube.away(probe).as_vec3();
            if self.strike(Spell::Thunderwave, i).takes_hold() {
                let push = verse_world::spells::thunderwave::PUSH as f32;
                self.dummies[i].push(away, push, now);
            }
        }
        self.add(draw::Effect::Wave {
            origin: cube.origin.as_vec3(),
            forward: cube.forward.as_vec3(),
            start: now,
        });
    }

    /// Wind Wall's damage and lift on every dummy the new wall stands in.
    fn wind_wall(&mut self, glade: &Everglade) {
        let Some(wall) = glade.spells().wind_wall().cloned() else {
            return;
        };
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let scale = f64::from(d.kind.scale());
            if d.down() || !wall.in_area(d.pos.as_dvec3(), 0.35 * scale, 1.8 * scale) {
                continue;
            }
            if self.strike(Spell::WindWall, i).takes_hold() {
                self.dummies[i].lift(WIND_LIFT, self.time);
            }
        }
    }

    /// Wall of Stone's shove: a dummy where a panel rose moves out past
    /// the panel's far face, away from the caster at `from`.
    fn shove(&mut self, glade: &Everglade, from: Vec3) {
        let Some(panels) = glade.spells().stone_panels().map(<[_]>::to_vec) else {
            return;
        };
        let now = self.time;
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let radius = 0.35 * f64::from(d.kind.scale());
            let center = d.pos.as_dvec3();
            let Some((normal, depth)) = panels.iter().find_map(|panel| {
                let half = panel.half();
                let local = panel.orientation.inverse() * (center - panel.center);
                (local.x.abs() <= half.x + radius && local.z.abs() <= half.z + radius).then(|| {
                    let normal = panel.normal();
                    let side = (center - from.as_dvec3()).dot(normal).signum();
                    let side = if side == 0.0 { 1.0 } else { side };
                    let past = half.z + radius + SHOVE_CLEARANCE - local.z * side;
                    (normal * side, past)
                })
            }) else {
                continue;
            };
            if self.dummies[i].push(normal.as_vec3(), depth as f32, now) {
                let name = self.dummies[i].kind.name();
                self.say(format!("Wall of Stone shoves {name} aside"));
            }
        }
    }

    /// Lands every bolt whose flight is over: its strike, its burst, and
    /// its trail's end.
    pub(super) fn land_bolts(&mut self) {
        let now = self.time;
        let mut landed = Vec::new();
        let mut kept = Vec::with_capacity(self.effects.len());
        for effect in std::mem::take(&mut self.effects) {
            match effect {
                draw::Effect::Bolt {
                    target,
                    start,
                    flight,
                    spell,
                    trail,
                    from,
                } => {
                    if now - start >= flight {
                        if let Some(trail) = trail {
                            self.fx.stop(trail);
                        }
                        landed.push((target, spell));
                    } else {
                        if let (Some(trail), Some(d)) = (trail, self.dummies.get(target)) {
                            let k = ((now - start) / flight.max(1e-3)).clamp(0.0, 1.0);
                            let at = from.lerp(d.center(), k);
                            let velocity = (d.center() - from) / flight.max(1e-3);
                            self.fx.place(trail, at, velocity);
                        }
                        kept.push(draw::Effect::Bolt {
                            target,
                            start,
                            flight,
                            spell,
                            trail,
                            from,
                        });
                    }
                }
                other if !other.done(now) => kept.push(other),
                _ => {}
            }
        }
        self.effects = kept;
        for (target, spell) in landed {
            if target >= self.dummies.len() {
                continue;
            }
            let at = self.dummies[target].center();
            match spell {
                Spell::Fireball => {
                    let radius = 20.0 * 0.3048;
                    for i in 0..self.dummies.len() {
                        if self.dummies[i].center().distance(at) <= radius + BODY {
                            self.strike(Spell::Fireball, i);
                        }
                    }
                    let _ = self.fx.start("grove_flame_hit", Spawn::at(at).scaled(2.2));
                    self.add_flash(super::light::Flash {
                        at: at + Vec3::Y * 0.8,
                        start: self.time,
                        life: 0.8,
                        color: [1.0, 0.5, 0.16],
                        peak: 650_000.0,
                        range: 22.0,
                        crackle: false,
                    });
                    self.decal(at, radius);
                }
                Spell::FireBolt => {
                    self.strike(Spell::FireBolt, target);
                }
                Spell::IceKnife => {
                    self.burst("grove_ice_shatter", at);
                    self.strike(Spell::IceKnife, target);
                    let burst = Def {
                        label: "Ice Knife",
                        dice: (2, 6),
                        bonus: 0,
                        kind: Damage::Cold,
                        delivery: Delivery::Save {
                            ability: kit::Ability::Dexterity,
                            half: false,
                        },
                        ..Spell::IceKnife.def()
                    };
                    for i in self.within(at, ICE_BURST) {
                        self.strike_def(&burst, i);
                    }
                }
                _ => {
                    self.burst(hit_look(spell), at);
                    self.strike(spell, target);
                }
            }
        }
    }
}

/// The particles a bolt trails.
fn bolt_trail(spell: Spell) -> &'static str {
    match spell {
        Spell::StarryWisp => "grove_star_mote",
        Spell::RayOfFrost | Spell::IceKnife => "grove_cold_cone",
        Spell::RayOfSickness => "grove_poison_puff",
        _ => "grove_flame_bolt",
    }
}

/// The particles a spell's landing or area makes.
fn hit_look(spell: Spell) -> &'static str {
    match spell {
        Spell::ProduceFlame | Spell::FireStorm => "grove_flame_hit",
        Spell::StarryWisp | Spell::FaerieFire => "grove_star_hit",
        Spell::PoisonSpray | Spell::RayOfSickness => "grove_poison_puff",
        Spell::HealingWord | Spell::MassCureWounds => "grove_heal",
        Spell::IceStorm => "grove_hail",
        Spell::IceKnife | Spell::RayOfFrost => "grove_ice_shatter",
        Spell::Sunburst => "grove_sunburst",
        Spell::Sunbeam => "grove_sunbeam",
        Spell::Blight | Spell::LandsAid => "grove_necrotic",
        Spell::AcidSplash => "grove_acid",
        Spell::ConeOfCold => "grove_cold_cone",
        Spell::BurningHands => "grove_fire_cone",
        Spell::LightningBolt => "grove_lightning_line",
        Spell::ShockingGrasp => "grove_lightning",
        Spell::Sleep | Spell::HoldPerson | Spell::Polymorph | Spell::ConjureAnimals => {
            "grove_cast_burst"
        }
        Spell::Shillelagh => "grove_vines",
        _ => "grove_cast_burst",
    }
}

/// How high above the ground a burst's particles start, m.
fn burst_height(spell: Spell) -> f32 {
    match spell {
        // The hail falls from above.
        Spell::IceStorm => 8.0,
        Spell::MassCureWounds | Spell::Sleep => 0.3,
        _ => 0.9,
    }
}

/// What the meadow's birds say to a druid who speaks with animals.
const BIRDS: [&str; 6] = [
    "the straw ones never flinch, and never fly away either",
    "the armored one is hollow, and it rings when the stones hit it",
    "the one on the pole has a nest of ours behind its ear, so mind the fire",
    "the big one was here before the trees, or says it was",
    "the warded one smells of smoke but has never burned",
    "they all stand back up when nobody is looking",
];
