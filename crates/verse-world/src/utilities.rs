//! Real-time chamber adaptations of the SRD utility and control spells.
//! Mana, cooldowns, and short control durations are MMO tuning, not tabletop rules.
use crate::rules::Simulation;
use glam::Vec3;
use std::collections::BTreeMap;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Utility {
    MistyStep,
    Thunderwave,
    Web,
    Grease,
    Light,
    Shield,
}
impl Utility {
    pub fn cost(self) -> i32 {
        match self {
            Self::Shield => 1,
            Self::Light => 0,
            Self::MistyStep => 2,
            Self::Thunderwave => 3,
            Self::Web => 3,
            Self::Grease => 2,
        }
    }
    pub fn cooldown(self) -> f32 {
        match self {
            Self::Shield => 8.0,
            Self::Light => 1.0,
            Self::MistyStep => 6.0,
            Self::Thunderwave => 4.0,
            Self::Web => 8.0,
            Self::Grease => 5.0,
        }
    }
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Area {
    pub kind: Utility,
    pub position: Vec3,
    pub until: f32,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Controls {
    pub areas: Vec<Area>,
    pub light: Option<Vec3>,
    pub shield: i32,
    pub shield_until: f32,
    light_until: f32,
    ready: BTreeMap<Utility, f32>,
    offsets: BTreeMap<u32, Vec3>,
    roots: BTreeMap<u32, Vec3>,
}
impl Controls {
    pub fn validate(&self) -> Result<(), String> {
        if !(0..=18).contains(&self.shield)
            || !self.shield_until.is_finite()
            || self.shield_until < 0.
            || !self.light_until.is_finite()
            || self.light_until < 0.
            || self.light.is_some_and(|p| !p.is_finite())
            || self.areas.len() > 128
            || self
                .areas
                .iter()
                .any(|a| !a.position.is_finite() || !a.until.is_finite() || a.until < 0.)
            || self.ready.values().any(|at| !at.is_finite() || *at < 0.)
            || self.offsets.len() > 256
            || self.roots.len() > 256
            || self
                .offsets
                .values()
                .chain(self.roots.values())
                .any(|p| !p.is_finite())
        {
            return Err("Invalid control-effect checkpoint".into());
        }
        Ok(())
    }
    /// Clears control effects that belong to a removed actor's previous life.
    pub fn forget_actor(&mut self, id: u32) {
        self.offsets.remove(&id);
        self.roots.remove(&id);
    }

    pub fn cooldown(&self, spell: Utility, time: f32) -> f32 {
        (self.ready.get(&spell).copied().unwrap_or(0.0) - time).max(0.0)
    }
    pub fn position(&mut self, id: u32, authored: Vec3, time: f32) -> Vec3 {
        self.areas.retain(|a| a.until > time);
        if time >= self.light_until {
            self.light = None;
        }
        let position = authored + self.offsets.get(&id).copied().unwrap_or(Vec3::ZERO);
        if self.areas.iter().any(|a| {
            matches!(a.kind, Utility::Web | Utility::Grease)
                && square_distance(self.roots.get(&id).copied().unwrap_or(position), a.position)
                    < if a.kind == Utility::Web { 3.048 } else { 1.524 }
        }) {
            *self.roots.entry(id).or_insert(position)
        } else {
            self.roots.remove(&id);
            position
        }
    }
    /// Moves an actor's authored place by `delta`, as a shove does, and
    /// frees it from any root.
    pub fn displace(&mut self, id: u32, delta: Vec3) {
        if delta.is_finite() {
            *self.offsets.entry(id).or_default() += delta;
        }
        self.roots.remove(&id);
    }
    pub fn held(&self, id: u32) -> bool {
        self.roots.contains_key(&id)
    }
    pub fn prone(&self, position: Vec3, time: f32) -> bool {
        self.areas.iter().any(|a| {
            a.kind == Utility::Grease
                && a.until > time
                && square_distance(position, a.position) < 1.524
        })
    }
    /// Absorbs incoming damage while a shield has remaining strength and duration.
    pub fn absorb(&mut self, damage: i32, time: f32) -> i32 {
        if time >= self.shield_until {
            self.shield = 0;
        }
        let absorbed = damage.min(self.shield).max(0);
        self.shield -= absorbed;
        absorbed
    }
    /// Executes an admitted spell against the shared combat state.
    pub fn cast(
        &mut self,
        simulation: &mut Simulation,
        spell: Utility,
        time: f32,
        player: Vec3,
        direction: Vec3,
        target: Option<Vec3>,
    ) -> Result<Vec3, String> {
        self.cast_with_teleport(simulation, spell, time, player, direction, target, None)
    }
    /// Uses a caller-admitted teleport destination before spending resources.
    pub fn cast_with_teleport(
        &mut self,
        simulation: &mut Simulation,
        spell: Utility,
        time: f32,
        player: Vec3,
        direction: Vec3,
        target: Option<Vec3>,
        teleport: Option<Vec3>,
    ) -> Result<Vec3, String> {
        self.cast_with_visibility(
            simulation,
            spell,
            time,
            player,
            direction,
            target,
            teleport,
            |_| true,
        )
    }
    /// Filters directional damage through the caller's world visibility query.
    pub fn cast_with_visibility(
        &mut self,
        simulation: &mut Simulation,
        spell: Utility,
        time: f32,
        player: Vec3,
        direction: Vec3,
        target: Option<Vec3>,
        teleport: Option<Vec3>,
        visible: impl Fn(Vec3) -> bool,
    ) -> Result<Vec3, String> {
        self.cast_with_visibility_for(
            simulation, 0, spell, time, player, direction, target, teleport, visible,
        )
    }
    /// Uses the admitted caster's resources and this caster's effect state.
    pub(crate) fn cast_with_visibility_for(
        &mut self,
        simulation: &mut Simulation,
        caster: u32,
        spell: Utility,
        time: f32,
        player: Vec3,
        direction: Vec3,
        target: Option<Vec3>,
        teleport: Option<Vec3>,
        visible: impl Fn(Vec3) -> bool,
    ) -> Result<Vec3, String> {
        if teleport.is_some_and(|p| !p.is_finite() || spell != Utility::MistyStep)
            || !time.is_finite()
            || time < 0.0
            || !player.is_finite()
            || !direction.is_finite()
            || !direction.length_squared().is_finite()
            || Vec3::new(direction.x, 0.0, direction.z).length_squared() < 0.001
            || target.is_some_and(|p| !p.is_finite())
        {
            return Err("Invalid utility spell input".into());
        }
        if self.cooldown(spell, time) > 0.0 {
            return Err("Spell is cooling down".into());
        }
        let center = if matches!(spell, Utility::Web | Utility::Grease) {
            let p = target.ok_or("Select a living target")?;
            if horizontal(player, p) > 18.288 {
                return Err("Target is out of range".into());
            }
            p
        } else {
            player
        };
        let direction = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
        let mut destination = player;
        if spell == Utility::MistyStep {
            destination = teleport.unwrap_or_else(|| {
                let mut p = player + direction * 9.144;
                p.x = p.x.clamp(-12.0, 12.0);
                p.z = p.z.clamp(-25.0, 12.0);
                p
            });
            if simulation
                .snapshot()
                .actors
                .iter()
                .any(|a| a.alive && a.id != caster && horizontal(destination, a.pos.into()) < 1.0)
            {
                return Err("Teleport destination is occupied".into());
            }
        }
        self.areas.retain(|a| a.until > time);
        if self.areas.len() + if spell == Utility::MistyStep { 2 } else { 1 } > 128 {
            return Err("Utility effect budget exceeded".into());
        }
        simulation.spend_mana_for(caster, spell.cost())?;
        self.ready.insert(spell, time + spell.cooldown());
        match spell {
            Utility::Shield => {
                self.shield = 18;
                self.shield_until = time + 4.0;
            }
            Utility::MistyStep => {
                self.areas.push(Area {
                    kind: spell,
                    position: player,
                    until: time + 0.7,
                });
                self.areas.push(Area {
                    kind: spell,
                    position: destination,
                    until: time + 0.7,
                });
            }
            Utility::Thunderwave => {
                // Saves, damage, and pushes resolve in the spell physics
                // layer (`crate::spells::thunderwave`); this records the cue.
                let _ = &visible;
                self.areas.push(Area {
                    kind: spell,
                    position: player + direction * 2.286,
                    until: time + 0.7,
                });
            }
            Utility::Web => {
                self.areas.retain(|a| a.kind != Utility::Web);
                self.roots.clear();
                self.areas.push(Area {
                    kind: spell,
                    position: center,
                    until: time + 12.0,
                });
            }
            Utility::Grease => {
                self.areas.push(Area {
                    kind: spell,
                    position: center,
                    until: time + 10.0,
                });
            }
            Utility::Light => {
                self.light_until = time + 3600.0;
                self.light = Some(player + direction * 1.5 + Vec3::Y * 0.3);
            }
        }
        Ok(destination)
    }
}
fn square_distance(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}
fn horizontal(a: Vec3, b: Vec3) -> f32 {
    Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_casts_do_not_spend_mana_or_start_cooldowns() {
        let (mut s, _) = Simulation::chamber([0.0; 3], &[([9.144, 0.0, 0.0], 100)]).unwrap();
        let mut c = Controls::default();
        assert!(
            c.cast(&mut s, Utility::MistyStep, 0.0, Vec3::ZERO, Vec3::X, None)
                .is_err()
        );
        assert_eq!(s.snapshot().player.mana, 20);
        s.spend_chamber_mana(20).unwrap();
        assert!(
            c.cast(
                &mut s,
                Utility::Web,
                0.0,
                Vec3::ZERO,
                Vec3::Z,
                Some(Vec3::Z)
            )
            .is_err()
        );
        assert_eq!(c.cooldown(Utility::Web, 0.0), 0.0);
        assert!(c.areas.is_empty());
        c.cast(&mut s, Utility::Light, 1.0, Vec3::ZERO, Vec3::Z, None)
            .unwrap();
        c.cast(&mut s, Utility::Light, 2.0, Vec3::ZERO, Vec3::X, None)
            .unwrap();
        assert!(c.light.unwrap().x > 1.0);
        c.position(1, Vec3::ZERO, 3603.0);
        assert!(c.light.is_none());
    }
    #[test]
    fn teleport_wave_and_resource_gates() {
        let (mut s, ids) = Simulation::chamber([0.0; 3], &[([0.0, 0.0, 2.0], 100)]).unwrap();
        let mut c = Controls::default();
        c.cast(&mut s, Utility::Thunderwave, 0.0, Vec3::ZERO, Vec3::Z, None)
            .unwrap();
        assert_eq!(s.snapshot().player.hp, 200);
        assert_eq!(s.snapshot().player.mana, 17);
        assert!(
            c.cast(&mut s, Utility::Thunderwave, 1.0, Vec3::ZERO, Vec3::Z, None)
                .is_err()
        );
        c.displace(ids[0], Vec3::Z * 3.048);
        assert!(
            c.position(ids[0], Vec3::Z * 2.0, 2.0)
                .distance(Vec3::Z * 5.048)
                < 0.001
        );
        let destination = c
            .cast(&mut s, Utility::MistyStep, 2.0, Vec3::ZERO, Vec3::X, None)
            .unwrap();
        assert!((destination.x - 9.144).abs() < 0.001);
    }
    #[test]
    fn control_expires_and_light_does_not_need_a_target() {
        let (mut s, ids) = Simulation::chamber([0.0; 3], &[([0.0, 0.0, 2.0], 100)]).unwrap();
        let mut c = Controls::default();
        c.cast(
            &mut s,
            Utility::Web,
            0.0,
            Vec3::ZERO,
            Vec3::Z,
            Some(Vec3::Z * 2.0),
        )
        .unwrap();
        assert_eq!(c.position(ids[0], Vec3::Z * 2.0, 1.0), Vec3::Z * 2.0);
        assert_eq!(c.position(ids[0], Vec3::Z * 3.0, 2.0), Vec3::Z * 2.0);
        assert_eq!(c.position(ids[0], Vec3::Z * 3.0, 13.0), Vec3::Z * 3.0);
        c.cast(
            &mut s,
            Utility::Grease,
            13.0,
            Vec3::ZERO,
            Vec3::Z,
            Some(Vec3::Z * 2.0),
        )
        .unwrap();
        assert!(c.prone(Vec3::Z * 2.0, 14.0));
        assert!(!c.prone(Vec3::Z * 2.0, 24.0));
        c.cast(&mut s, Utility::Light, 24.0, Vec3::ZERO, Vec3::Z, None)
            .unwrap();
        assert!(c.light.is_some());
        assert!(
            c.cast(
                &mut s,
                Utility::Web,
                24.0,
                Vec3::ZERO,
                Vec3::Z,
                Some(Vec3::Z * 30.0)
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod shield_tests {
    use super::*;
    #[test]
    fn shield_spends_mana_absorbs_only_its_capacity_and_expires() {
        let (mut simulation, _) = Simulation::chamber([0.0; 3], &[]).unwrap();
        let mut controls = Controls::default();
        controls
            .cast(
                &mut simulation,
                Utility::Shield,
                0.0,
                Vec3::ZERO,
                Vec3::Z,
                None,
            )
            .unwrap();
        assert_eq!(simulation.snapshot().player.mana, 19);
        assert_eq!(controls.absorb(12, 1.0), 12);
        assert_eq!(controls.absorb(12, 2.0), 6);
        simulation.chamber_player_damage(6).unwrap();
        assert_eq!(simulation.snapshot().player.hp, 194);
        assert!(
            controls
                .cast(
                    &mut simulation,
                    Utility::Shield,
                    2.0,
                    Vec3::ZERO,
                    Vec3::Z,
                    None
                )
                .is_err()
        );
        controls
            .cast(
                &mut simulation,
                Utility::Shield,
                8.0,
                Vec3::ZERO,
                Vec3::Z,
                None,
            )
            .unwrap();
        assert_eq!(controls.absorb(9, 12.0), 0);
        assert_eq!(controls.shield, 0);
        assert!(simulation.chamber_player_damage(-1).is_err());
    }
}
