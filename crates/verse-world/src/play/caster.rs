//! Actor-scoped state lookup and captured effect-construction context.
use super::*;
use verse_engine::core::LifeId;
impl Game {
    pub(super) fn prune_caster_dice(&mut self) {
        let retained: std::collections::BTreeSet<_> = self
            .additional_players
            .keys()
            .copied()
            .chain(
                self.spells
                    .meteors
                    .iter()
                    .filter(|e| !e.swarm.finished())
                    .map(|e| e.caster),
            )
            .collect();
        self.spells
            .caster_dice
            .retain(|actor, _| retained.contains(actor));
    }
    pub fn actor_state(&self, life: LifeId) -> Option<&ActorState> {
        if life == self.primary.admission.actor() {
            return self.primary_resident.then_some(&self.primary);
        }
        self.additional_players
            .get(&life.actor)
            .filter(|state| state.admission.actor() == life)
    }
    pub(crate) fn actor_state_mut(&mut self, life: LifeId) -> Option<&mut ActorState> {
        if life == self.primary.admission.actor() {
            return self.primary_resident.then_some(&mut self.primary);
        }
        self.additional_players
            .get_mut(&life.actor)
            .filter(|state| state.admission.actor() == life)
    }
    /// Captures a trusted constructor context. Player commands use `submit`.
    pub fn caster_context(&self, life: LifeId) -> Result<crate::spells::Caster, String> {
        let state = self.actor_state(life).ok_or("Spell caster life is stale")?;
        Ok(crate::spells::Caster {
            life,
            source: state.source,
            feet: state.player,
            yaw: state.yaw,
            selected: state.selected,
            save_dc: state.definition.save_dc,
            tick: self.authority_tick,
        })
    }
    pub fn catalog_cooldown(&self, life: LifeId, slot: u8) -> Option<f32> {
        self.actor_state(life).map(|state| {
            state
                .catalog_ready
                .get(&slot)
                .map_or(0., |at| (*at - self.time).max(0.))
        })
    }
    /// Applies trusted character content. This grants no controller authority.
    pub fn configure_character(
        &mut self,
        life: LifeId,
        definition: crate::content::Character,
    ) -> Result<(), String> {
        definition.validate()?;
        let state = self.actor_state(life).ok_or("Character life is stale")?;
        if state.casting.is_some() || !state.catalog_ready.is_empty() {
            return Err("Character tuning requires an idle uncast actor".into());
        }
        let source = state.source;
        self.simulation
            .equipment_limits(source, definition.health, definition.mana)?;
        let resources = self.simulation.snapshot_for(source)?.player;
        if resources.hp < resources.max_hp || resources.mana < resources.max_mana {
            self.simulation.recover_resources(
                source,
                definition.health as u32,
                definition.mana as u32,
            )?;
        }
        self.actor_state_mut(life).unwrap().definition = definition;
        Ok(())
    }
    pub(super) fn activate_catalog(
        &mut self,
        context: crate::spells::Caster,
        ability: Ability,
    ) -> Result<(), String> {
        context.validate(self)?;
        if self.social.is_some()
            || !self.unlocked()
            || self.simulation.snapshot_for(context.source)?.player.hp == 0
        {
            return Err("The caster cannot act in the current world state".into());
        }
        if self
            .actor_state(context.life)
            .ok_or("Spell caster life is stale")?
            .casting
            .is_some()
            && !matches!(
                ability,
                Ability::Spell(3)
                    | Ability::SpellCommand(crate::spells::command::Command::Targets {
                        slot: 3,
                        ..
                    })
            )
        {
            return Err("A spell is already being cast".into());
        }
        if let Ability::SpellCommand(crate::spells::command::Command::EndConcentration) = ability {
            return self.spells.end_concentration(context.life.actor);
        }
        if let Ability::SpellCommand(crate::spells::command::Command::Hand(point)) = ability {
            let caster = context.life.actor;
            let effect = self
                .spells
                .telekinesis
                .iter_mut()
                .find(|e| e.caster == caster && e.grip.grip.is_some())
                .ok_or("No Telekinesis grip")?;
            effect.aim = glam::DVec3::from_array(point.map(|mm| f64::from(mm) / 1000.));
            return Ok(());
        }
        if let Ability::SpellCommand(crate::spells::command::Command::Release) = ability {
            let caster = context.life.actor;
            let effect = self
                .spells
                .telekinesis
                .iter_mut()
                .find(|e| e.caster == caster && e.grip.grip.is_some())
                .ok_or("No Telekinesis grip")?;
            effect
                .grip
                .release(&mut self.spells.world, crate::telekinesis::Reason::Let);
            return Ok(());
        }
        if let Ability::SpellCommand(crate::spells::command::Command::Altitude(mm)) = ability {
            let caster = context.life.actor;
            let effect = self
                .spells
                .levitations
                .iter_mut()
                .find(|e| e.caster == caster && e.state.holding())
                .ok_or("No held Levitate target")?;
            effect
                .state
                .command(f64::from(mm) / 1000., self.time as f64)
                .map_err(|e| format!("Altitude command refused: {e:?}"))?;
            return Ok(());
        }
        if let Ability::SpellCommand(crate::spells::command::Command::Wind(aim)) = ability {
            let caster = context.life.actor;
            let effect = self
                .spells
                .gusts
                .iter_mut()
                .find(|e| e.caster == caster && e.gust.active(self.time as f64))
                .ok_or("No active Gust of Wind")?;
            return effect.gust.reaim(
                glam::DVec3::from_array(aim.map(|v| f64::from(v) / 1000.)),
                self.time as f64,
            );
        }
        if let Ability::SpellCommand(crate::spells::command::Command::Escape) = ability {
            return crate::spells::black_tentacles::escape(self, context.life.actor);
        }
        if let Some(slot) = match ability {
            Ability::Spell(slot) => Some(slot),
            Ability::SpellCommand(command) => Some(command.slot()),
            _ => None,
        } {
            let spell = ability.catalog().ok_or("No spell in this slot")?;
            let tuning = self
                .actor_state(context.life)
                .ok_or("Spell caster life is stale")?
                .definition
                .catalog
                .get(&slot)
                .ok_or("Ability is absent from character definition")?
                .clone();
            if self
                .actor_state(context.life)
                .ok_or("Spell caster life is stale")?
                .catalog_ready
                .get(&slot)
                .is_some_and(|at| *at > self.time)
            {
                return Err("Spell is cooling down".into());
            }
            if self.simulation.snapshot_for(context.source)?.player.mana < tuning.cost {
                return Err("Not enough mana".into());
            }
            let retained = match slot {
                0 => self.spells.telekinesis.len(),
                1 => self.spells.walls.len(),
                2 => self.spells.levitations.len(),
                3 => self.spells.feather_falls.len(),
                4 => self.spells.gusts.len(),
                5 => self.spells.wind_walls.len(),
                6 => self.spells.tentacles.len(),
                7 => self.spells.meteors.len(),
                8 => self.spells.reversed.len(),
                _ => 0,
            };
            if retained >= 64 {
                return Err("Retained spell effect budget exceeded".into());
            }
            match ability {
                Ability::SpellCommand(crate::spells::command::Command::Targets {
                    slot,
                    targets,
                    count,
                }) => {
                    if count == 0 || usize::from(count) > targets.len() {
                        return Err("Invalid spell target count".into());
                    }
                    match slot {
                        3 => {
                            let chosen = targets[..usize::from(count)]
                                .iter()
                                .map(|id| {
                                    u32::try_from(*id).map_err(|_| {
                                        "Feather Fall target ID is too large".to_string()
                                    })
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            crate::spells::feather_fall::cast_on(self, context, &chosen)?;
                        }
                        0 | 2 if count == 1 => {
                            let target = if targets[0] >= crate::spells::PROP_ENTITY_BASE {
                                crate::spells::Target::Prop(
                                    self.spells
                                        .props
                                        .iter()
                                        .position(|p| p.life.entity == targets[0] && !p.removed)
                                        .ok_or("Unknown spell prop")?,
                                )
                            } else {
                                crate::spells::Target::Actor(targets[0])
                            };
                            if slot == 0 {
                                crate::spells::telekinesis::cast_on(self, context, target)?;
                            } else {
                                crate::spells::levitate::cast_on(self, context, target)?;
                            }
                        }
                        _ => return Err("This spell does not accept these targets".into()),
                    }
                }
                Ability::SpellCommand(crate::spells::command::Command::Point { slot, point }) => {
                    let point = glam::DVec3::from_array(point.map(|v| f64::from(v) / 1000.));
                    match slot {
                        6 => crate::spells::black_tentacles::cast_at(self, context, point)?,
                        8 => crate::spells::reverse_gravity::cast_at(self, context, point)?,
                        _ => return Err("This spell does not accept a ground point".into()),
                    }
                }
                Ability::SpellCommand(crate::spells::command::Command::WindWall {
                    points,
                    count,
                }) => {
                    if !(2..=8).contains(&count) {
                        return Err("Wind Wall needs 2 to 8 path points".into());
                    }
                    crate::spells::wind_wall::cast_path(
                        self,
                        context,
                        points[..usize::from(count)]
                            .iter()
                            .map(|p| glam::DVec2::from_array(p.map(|v| f64::from(v) / 1000.)))
                            .collect(),
                    )?;
                }
                Ability::SpellCommand(crate::spells::command::Command::Meteors(points)) => {
                    crate::spells::meteor_swarm::cast_at(
                        self,
                        context,
                        points.map(|p| glam::DVec3::from_array(p.map(|v| f64::from(v) / 1000.))),
                    )?;
                }
                Ability::SpellCommand(crate::spells::command::Command::Stone {
                    shape,
                    from,
                    to,
                    count,
                    thin,
                }) => {
                    crate::spells::wall_of_stone::authored(
                        self, context, shape, from, to, count, thin,
                    )?;
                }
                _ => (spell.cast)(self, context)?,
            }
            self.simulation
                .spend_mana_for(context.source, tuning.cost)?;
            let ready_at = self.time + tuning.cooldown;
            self.actor_state_mut(context.life)
                .unwrap()
                .catalog_ready
                .insert(slot, ready_at);
            self.record_ability(ability);
            let at = self.time;
            self.actor_state_mut(context.life).unwrap().last_cast = Some((ability, at));
            self.message = format!("{}: {}", spell.label, spell.description);
            return Ok(());
        }
        Err("Ability is not a catalog spell or command".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Controller, Intent, spells::command::Command as SpellCommand};
    fn fixture() -> (Game, LifeId, LifeId) {
        let mut game = crate::playground::Run::new(crate::spells::scenarios::area(5))
            .unwrap()
            .game;
        let primary = game.player_life();
        let extra = game
            .add_player(Controller(42), Vec3::new(6., 0., -8.))
            .unwrap();
        game.tick(0.01, [0.; 2]).unwrap();
        (game, primary, extra)
    }
    fn cast(
        game: &mut Game,
        life: LifeId,
        ability: Ability,
        target: Option<LifeId>,
    ) -> Result<(), String> {
        let admission = game.player_admission(life.actor).unwrap();
        let controller = admission.controller();
        let command = admission
            .command(
                game.authority_tick,
                Intent::Cast {
                    ability,
                    target,
                    aim: if ability == Ability::MistyStep {
                        [if life == game.player_life() { -1. } else { 1. }, 0., 0.]
                    } else {
                        [0., 0., 1.]
                    },
                },
            )
            .unwrap();
        game.submit(controller, command)
    }
    #[test]
    fn every_shipped_ability_admits_two_independent_casters_and_restores() {
        for ability in Ability::ALL.into_iter().chain(
            crate::spells::CATALOG
                .iter()
                .map(|s| Ability::Spell(s.slot)),
        ) {
            let (mut game, a, b) = fixture();
            let ta = game.actor_life(101).unwrap();
            let tb = game.actor_life(103).unwrap();
            for (life, target, other) in [(a, ta, b), (b, tb, a)] {
                if ability == Ability::Spell(3) {
                    let character = game.npc_characters.get_mut(&target.actor).unwrap();
                    *character = physics::character::Character::new(character.feet);
                    character.vertical_speed = -2.;
                }
                if matches!(ability, Ability::Spell(0) | Ability::Spell(2)) {
                    game.spells
                        .dice_for(life.actor)
                        .force_save(target.actor, 1)
                        .unwrap();
                }
                let before = game.player_snapshot(other).unwrap().player;
                cast(&mut game, life, ability, Some(target))
                    .unwrap_or_else(|e| panic!("{ability:?}, {life:?}: {e}"));
                let after = game.player_snapshot(other).unwrap().player;
                assert_eq!(
                    before.mana, after.mana,
                    "{ability:?} spent another caster's mana"
                );
                let state = game.actor_state(life).unwrap();
                assert_eq!(state.last_cast.unwrap().0, ability);
                if let Ability::Spell(slot) = ability {
                    assert!(state.catalog_ready.contains_key(&slot));
                }
                assert!(game.events.iter().any(|e|e.actor==Some(life) && matches!(&e.kind,crate::events::Kind::Ability{label} if label==ability.label())));
                let bytes = game
                    .checkpoint()
                    .unwrap_or_else(|e| panic!("{ability:?} checkpoint: {e}"));
                assert_eq!(
                    bytes,
                    Game::restore(&bytes).unwrap().checkpoint().unwrap(),
                    "{ability:?} checkpoint"
                );
            }
            if ability == Ability::Spell(2) {
                let height_a = game
                    .spells
                    .levitations
                    .iter()
                    .find(|e| e.caster == a.actor)
                    .unwrap()
                    .state
                    .target_height();
                cast(
                    &mut game,
                    b,
                    Ability::SpellCommand(SpellCommand::Altitude(-1000)),
                    None,
                )
                .unwrap();
                assert_eq!(
                    height_a,
                    game.spells
                        .levitations
                        .iter()
                        .find(|e| e.caster == a.actor)
                        .unwrap()
                        .state
                        .target_height()
                );
            }
            let mut replay = Game::restore(&game.checkpoint().unwrap()).unwrap();
            for step in 0..26 {
                game.tick(0.05, [0.; 2])
                    .unwrap_or_else(|e| panic!("{ability:?}: {e}"));
                replay.tick(0.05, [0.; 2]).unwrap();
                let actual = game.checkpoint().unwrap();
                let restored = replay.checkpoint().unwrap();
                assert!(actual == restored, "{ability:?} live replay at step {step}");
            }
            assert_ne!(
                game.actor_state(a).unwrap().character.feet,
                game.actor_state(b).unwrap().character.feet
            );
        }
    }
    #[test]
    fn character_content_scopes_saves_tuning_streams_and_commands() {
        let (mut game, a, b) = fixture();
        let mut definition = crate::content::Character::default();
        definition.key = "storm-warden".into();
        definition.health = 300;
        definition.mana = 30;
        definition.save_dc = 19;
        definition.catalog.get_mut(&4).unwrap().cost = 1;
        definition.catalog.get_mut(&4).unwrap().cooldown = 2.;
        game.configure_character(b, definition.clone()).unwrap();
        assert_eq!(game.player_snapshot(a).unwrap().player.max_mana, 20);
        assert_eq!(game.player_snapshot(b).unwrap().player.max_mana, 30);
        let ta = game.actor_life(101);
        let tb = game.actor_life(103);
        for (life, target) in [(a, ta), (b, tb)] {
            game.spells
                .dice_for(life.actor)
                .force_save(target.unwrap().actor, 16)
                .unwrap();
            cast(&mut game, life, Ability::Spell(0), target).unwrap();
        }
        let saves: Vec<_> = game
            .spells
            .log
            .iter()
            .filter_map(|r| r.save.as_ref())
            .filter(|s| s.ability == "Strength")
            .collect();
        assert!(
            saves
                .iter()
                .any(|s| s.target == 101 && s.roll == 16 && s.dc == 15 && s.success)
        );
        assert!(
            saves
                .iter()
                .any(|s| s.target == 103 && s.roll == 16 && s.dc == 19 && !s.success)
        );
        // A command for the extra caster changes only that caster's retained grip.
        cast(
            &mut game,
            b,
            Ability::SpellCommand(SpellCommand::Hand([4000, 6000, -3000])),
            None,
        )
        .unwrap();
        assert_eq!(
            game.spells
                .telekinesis
                .iter()
                .find(|e| e.caster == b.actor)
                .unwrap()
                .aim,
            glam::DVec3::new(4., 6., -3.)
        );
        cast(
            &mut game,
            b,
            Ability::SpellCommand(SpellCommand::Release),
            None,
        )
        .unwrap();
        let primary_rolls = game.spells.dice.rolled;
        game.spells.dice_for(b.actor).roll(20);
        assert_eq!(primary_rolls, game.spells.dice.rolled);
        let bytes = game.checkpoint().unwrap();
        let mut replay = Game::restore(&bytes).unwrap();
        assert_eq!(replay.actor_state(b).unwrap().definition, definition);
        for g in [&mut game, &mut replay] {
            cast(g, a, Ability::Spell(4), None).unwrap();
            cast(g, b, Ability::Spell(4), None).unwrap();
            assert_eq!(g.actor_state(a).unwrap().catalog_ready[&4], g.time + 6.);
            assert_eq!(g.actor_state(b).unwrap().catalog_ready[&4], g.time + 2.);
            assert!(g.spells.concentration.contains_key(&a.actor));
            assert!(g.spells.concentration.contains_key(&b.actor));
            cast(
                g,
                b,
                Ability::SpellCommand(SpellCommand::EndConcentration),
                None,
            )
            .unwrap();
            assert!(g.spells.concentration.contains_key(&a.actor));
            assert!(!g.spells.concentration.contains_key(&b.actor));
        }
        assert_eq!(game.checkpoint().unwrap(), replay.checkpoint().unwrap());
    }
    #[test]
    fn moving_remote_gust_follows_its_owner_without_moving_the_other_line() {
        let (mut game, a, b) = fixture();
        cast(&mut game, a, Ability::Spell(4), None).unwrap();
        cast(&mut game, b, Ability::Spell(4), None).unwrap();
        let before = game.actor_state(b).unwrap().player;
        let admission = game.player_admission(b.actor).unwrap();
        let command = admission
            .command(
                game.authority_tick,
                Intent::Move {
                    axes: [0.5, 0.],
                    yaw: std::f32::consts::PI,
                },
            )
            .unwrap();
        game.submit(Controller(42), command).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        assert!(game.actor_state(b).unwrap().player.distance(before) > 0.01);
        for life in [a, b] {
            let origin = game
                .spells
                .gusts
                .iter()
                .find(|e| e.caster == life.actor)
                .unwrap()
                .gust
                .line
                .origin;
            assert!(
                origin.distance(game.actor_state(life).unwrap().previous_player.as_dvec3()) < 1e-6
            );
        }
        assert!(
            game.spells.gusts[0]
                .gust
                .line
                .origin
                .distance(game.spells.gusts[1].gust.line.origin)
                > 5.
        );
    }
    #[test]
    fn character_tuning_and_catalog_cooldowns_survive_world_transfer() {
        let (mut source, _a, b) = fixture();
        let mut definition = crate::content::Character::default();
        definition.key = "storm-warden".into();
        definition.health = 300;
        definition.mana = 30;
        source.configure_character(b, definition.clone()).unwrap();
        cast(&mut source, b, Ability::Spell(4), None).unwrap();
        source.spells.dice_for(b.actor).roll(20);
        let remaining = source.catalog_cooldown(b, 4).unwrap();
        let resources = source.player_snapshot(b).unwrap().player;
        let portable = source.take_transfer_player(b.actor).unwrap();
        let (mut destination, _a, _b) = fixture();
        let life = destination
            .add_player(Controller(45), Vec3::new(-6., 0., -8.))
            .unwrap();
        destination.put_transfer_player(life, portable).unwrap();
        assert_eq!(
            destination.actor_state(life).unwrap().definition,
            definition
        );
        assert_eq!(destination.catalog_cooldown(life, 4).unwrap(), remaining);
        assert_eq!(
            destination.player_snapshot(life).unwrap().player.mana,
            resources.mana
        );
        let bytes = destination.checkpoint().unwrap();
        assert_eq!(bytes, Game::restore(&bytes).unwrap().checkpoint().unwrap());
    }
    #[test]
    fn previous_checkpoint_layout_migrates_primary_and_additional_records() {
        let (mut game, a, b) = fixture();
        cast(&mut game, a, Ability::Spell(4), None).unwrap();
        let mut old: serde_json::Value =
            serde_json::from_slice(&game.checkpoint().unwrap()).unwrap();
        old["rules_revision"] = "verse-chamber-owned-v23".into();
        let world = old["world"].as_object_mut().unwrap();
        let ready = world.remove("catalog_ready").unwrap();
        world.get_mut("spells").unwrap()["ready"] = ready;
        for key in ["definition", "source", "spawn"] {
            world.remove(key);
        }
        for player in world
            .get_mut("additional_players")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            let player = player.as_object_mut().unwrap();
            player.remove("definition");
            player.remove("catalog_ready");
            for (new, old) in [
                ("player", "position"),
                ("previous_player", "previous"),
                ("pending_movement", "pending_move"),
                ("held_movement", "held_move"),
            ] {
                let value = player.remove(new).unwrap();
                player.insert(old.into(), value);
            }
        }
        let restored = Game::restore(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(restored.catalog_cooldown(a, 4), game.catalog_cooldown(a, 4));
        assert_eq!(
            restored.actor_state(b).unwrap().player,
            game.actor_state(b).unwrap().player
        );
        assert_eq!(
            restored.actor_state(a).unwrap().definition,
            crate::content::Character::default()
        );
    }
    #[test]
    fn refused_cast_restores_gameplay_and_retains_the_consumed_envelope() {
        let (mut game, _a, b) = fixture();
        let before = game.checkpoint().unwrap();
        assert!(
            cast(
                &mut game,
                b,
                Ability::SpellCommand(SpellCommand::WindWall {
                    points: [[0; 2]; 8],
                    count: 1
                }),
                None
            )
            .is_err()
        );
        let mut expected = Game::restore(&before).unwrap();
        let admission = game.player_admission(b.actor).unwrap().clone();
        assert_eq!(admission.accepted_sequence(), 1);
        expected.actor_state_mut(b).unwrap().admission = admission;
        let after = game.checkpoint().unwrap();
        assert_eq!(expected.checkpoint().unwrap(), after);
        let mut definition = crate::content::Character::default();
        definition.catalog.get_mut(&4).unwrap().cooldown = f32::NAN;
        assert!(game.configure_character(b, definition).is_err());
        assert_eq!(after, game.checkpoint().unwrap());
        let stale = game.caster_context(b).unwrap();
        game.handoff_player(b, Controller(43)).unwrap();
        // Contexts retain life and pose; the controller must still use admission.
        assert!(stale.validate(&game).is_ok());
        let admission = game.player_admission(b.actor).unwrap();
        let command = admission
            .command(
                game.authority_tick,
                Intent::Cast {
                    ability: Ability::Light,
                    target: None,
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        assert!(game.submit(Controller(42), command.clone()).is_err());
        game.submit(Controller(43), command).unwrap();
    }
}
