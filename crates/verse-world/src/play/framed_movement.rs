//! Confirmed movement intervals do not rewind world time, combat, or receipts.
use super::*;
use crate::{
    Command, Controller, Intent,
    movement::frames::{Clock, ExpiryOrigin, Frame},
};
use verse_engine::core::LifeId;
impl Game {
    /// Retains actual completed movement, independently of envelope admission.
    pub(super) fn record_movement_confirmations(&mut self) -> Result<(), String> {
        let lives: Vec<_> = std::iter::once(self.player_life())
            .chain(
                self.additional_players
                    .values()
                    .map(|p| p.admission.actor()),
            )
            .collect();
        for life in lives {
            let Some(baseline) = self.movement_baseline(life)? else {
                continue;
            };
            if baseline.profile != crate::movement::Profile::Frames {
                continue;
            }
            let clock = if life.actor == self.player_actor() {
                self.primary.frame_clock.as_mut()
            } else {
                self.additional_players
                    .get_mut(&life.actor)
                    .unwrap()
                    .frame_clock
                    .as_mut()
            }
            .unwrap();
            if clock
                .confirmations
                .back()
                .is_some_and(|b| b.applied_sequence == baseline.applied_sequence)
            {
                clock.confirmations.pop_back();
            }
            if clock.confirmations.len() == crate::movement::frames::MAX_QUEUED {
                clock.confirmations.pop_front();
            }
            clock.confirmations.push_back(baseline);
        }
        Ok(())
    }
    /// Committed poses of loose blocking props: the colliders whose motion
    /// a client cannot predict between scene snapshots (#10559).
    pub(crate) fn dynamic_poses(&self) -> Vec<crate::service::wire::ColliderPose> {
        self.spells
            .props
            .iter()
            .filter(|p| !p.removed && !p.spec.secured)
            .filter_map(|p| {
                let key = p.query_key();
                Some(crate::service::wire::ColliderPose {
                    key,
                    pose: self.query_scene.pose(key)?,
                })
            })
            .collect()
    }
    pub(crate) fn movement_confirmations(&self, life: LifeId) -> Vec<crate::movement::Baseline> {
        let Some(admission) = self.player_admission(life.actor) else {
            return Vec::new();
        };
        if admission.actor() != life {
            return Vec::new();
        }
        let clock = if life.actor == self.player_actor() {
            self.primary.frame_clock.as_ref()
        } else {
            self.additional_players[&life.actor].frame_clock.as_ref()
        };
        clock.map_or_else(Vec::new, |c| {
            c.confirmations
                .iter()
                .copied()
                .filter(|b| b.life == life && b.epoch == admission.epoch())
                .collect()
        })
    }

    /// Starts interval movement from a stationary grounded authority pose.
    /// Repeated entry is idempotent and cannot renew the clock's lag budget.
    pub fn begin_movement_frames(
        &mut self,
        sender: Controller,
        life: LifeId,
    ) -> Result<(), String> {
        let admission = self
            .player_admission(life.actor)
            .ok_or("Unknown movement owner")?;
        if admission.actor() != life || admission.controller() != sender || sender.0 < 3 {
            return Err("Movement interval owner is stale or unsupported".into());
        }
        let active = if life.actor == self.player_actor() {
            self.primary.frame_clock.is_some()
        } else {
            self.additional_players[&life.actor].frame_clock.is_some()
        };
        if self.player_snapshot(life)?.player.hp == 0 || !self.unlocked() {
            return Err("Movement interval entry is unavailable".into());
        }
        if active {
            return Ok(());
        }
        let baseline = self
            .movement_baseline(life)?
            .ok_or("Movement interval entry is unavailable")?;
        if baseline.character.support.is_none()
            || baseline.held.axes(baseline.physics_step) != [0.; 2]
            || baseline.policy.walking_scale != 1.
            || !baseline.policy.jump_allowed
        {
            return Err(
                "Movement interval entry requires an unmodified stationary ground pose".into(),
            );
        }
        self.handoff_player(life, sender)?;
        let clock = Some(Clock::new(self.physics_steps, 0));
        if life.actor == self.player_actor() {
            self.primary.frame_clock = clock;
        } else {
            self.additional_players
                .get_mut(&life.actor)
                .unwrap()
                .frame_clock = clock;
        }
        Ok(())
    }
    /// Admits a contiguous bounded interval; envelope admission does not acknowledge physics.
    pub fn submit_movement_frame(
        &mut self,
        sender: Controller,
        frame: Frame,
    ) -> Result<(), String> {
        frame.validate()?;
        let actor = frame.life.actor;
        let admission = self
            .player_admission(actor)
            .ok_or("Unknown interval character")?;
        if admission.actor() != frame.life
            || admission.epoch() != frame.epoch
            || admission.controller() != sender
        {
            return Err("Movement interval control is stale or unavailable".into());
        }
        if self.player_snapshot(frame.life)?.player.hp == 0 || !self.unlocked() {
            let active = if actor == self.player_actor() {
                self.primary.frame_clock.is_some()
            } else {
                self.additional_players[&actor].frame_clock.is_some()
            };
            if active {
                self.handoff_player(frame.life, sender)?;
            }
            return Err("Movement interval control is stale or unavailable".into());
        }
        let clock = if actor == self.player_actor() {
            self.primary.frame_clock.as_ref()
        } else {
            self.additional_players[&actor].frame_clock.as_ref()
        }
        .ok_or("Movement intervals have not been started")?;
        if clock.expired(self.physics_steps) {
            self.movement_expiry.record(clock.expiry_sample(
                actor,
                frame.epoch,
                self.authority_tick,
                self.physics_steps,
                ExpiryOrigin::Admission,
            ));
            let message = format!(
                "Movement interval clock expired at world step {}, confirmed step {}",
                self.physics_steps, clock.step
            );
            self.handoff_player(frame.life, sender)?;
            return Err(message);
        }
        let mut next = clock.clone();
        next.admit(frame.clone(), self.physics_steps)?;
        if frame.tick > self.authority_tick
            || self.authority_tick - frame.tick > crate::movement::frames::ACK_TICKS
        {
            return Err("Movement interval acknowledgment clock is stale".into());
        }
        let first = &frame.segments[0];
        let command = Command {
            actor: frame.life,
            epoch: frame.epoch,
            sequence: frame.sequence,
            tick: self.authority_tick,
            intent: Intent::<Ability>::Move {
                axes: first.axes,
                yaw: first.yaw,
            },
        };
        if actor == self.player_actor() {
            self.primary
                .admission
                .admit(sender, &command, self.authority_tick)
                .map_err(|e| format!("Movement interval refused: {e:?}"))?;
            self.primary.frame_clock = Some(next);
        } else {
            let player = self.additional_players.get_mut(&actor).unwrap();
            player
                .admission
                .admit(sender, &command, self.authority_tick)
                .map_err(|e| format!("Movement interval refused: {e:?}"))?;
            player.frame_clock = Some(next);
        }
        self.clear_social_seat(frame.life);
        Ok(())
    }
    /// Retires unavailable control before checking the live interval deadline.
    pub(super) fn expire_primary_frames(&mut self, dead: bool) -> Result<(), String> {
        if self.primary.frame_clock.is_some() && (dead || !self.unlocked()) {
            self.handoff_player(self.player_life(), self.primary.admission.controller())?;
            return Ok(());
        }
        if self
            .frame_clock
            .as_ref()
            .is_some_and(|clock| clock.expired(self.physics_steps))
        {
            self.movement_expiry
                .record(self.primary.frame_clock.as_ref().unwrap().expiry_sample(
                    self.player_actor(),
                    self.primary.admission.epoch(),
                    self.authority_tick,
                    self.physics_steps,
                    ExpiryOrigin::PrimaryTick,
                ));
            self.handoff_player(self.player_life(), self.primary.admission.controller())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::{Profile, frames::Segment};
    use glam::Vec3;
    use verse_engine::director::Scene;
    fn world() -> Game {
        let mut scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        scene
            .actors
            .retain(|a| a.id == 1 || a.model == "adventurer");
        scene.cues.clear();
        scene.cut_at = 0.;
        scene.duration = 600.;
        let mut g = Game::new_in(scene, 700).unwrap();
        g.handoff_player(g.player_life(), Controller(9)).unwrap();
        g.tick(1. / 30., [0.; 2]).unwrap();
        g
    }
    fn frame(g: &Game, life: LifeId, jump: bool) -> Frame {
        let b = g.movement_baseline(life).unwrap().unwrap();
        let a = g.player_admission(life.actor).unwrap();
        Frame {
            life,
            epoch: a.epoch(),
            sequence: a.accepted_sequence() + 1,
            tick: g.authority_tick,
            start: b.physics_step,
            steps: 4,
            segments: vec![Segment {
                offset: 0,
                axes: [1., 0.],
                yaw: 0.,
                until: b.physics_step + 60,
                jump,
            }],
        }
    }
    fn ticks(g: &mut Game, n: usize) {
        for _ in 0..n {
            g.tick(1. / 30., [0.; 2]).unwrap();
        }
    }
    #[test]
    fn confirmations_require_applied_physics_are_bounded_and_retire_with_control() {
        for secondary in [false, true] {
            let mut g = world();
            let owner = Controller(if secondary { 10 } else { 9 });
            let life = if secondary {
                g.add_player(owner, Vec3::new(5., 0., -22.)).unwrap()
            } else {
                g.player_life()
            };
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, life).unwrap();
            for _ in 0..24 {
                let f = frame(&g, life, false);
                let sequence = f.sequence;
                let end = f.end().unwrap();
                g.submit_movement_frame(owner, f).unwrap();
                assert!(
                    g.movement_confirmations(life)
                        .iter()
                        .all(|b| b.applied_sequence < sequence)
                );
                ticks(&mut g, 1);
                let history = g.movement_confirmations(life);
                assert!(history.len() <= crate::movement::frames::MAX_QUEUED);
                let applied = history.last().unwrap();
                assert_eq!(applied.applied_sequence, sequence);
                assert_eq!(applied.physics_step, end);
                assert_eq!(
                    applied.character.feet,
                    g.movement_baseline(life).unwrap().unwrap().character.feet
                );
                applied.validate().unwrap();
            }
            let clock = if secondary {
                &g.additional_players[&life.actor].frame_clock
            } else {
                &g.primary.frame_clock
            };
            let encoded = serde_json::to_vec(clock.as_ref().unwrap()).unwrap();
            assert!(
                !std::str::from_utf8(&encoded)
                    .unwrap()
                    .contains("confirmations")
            );
            let restored: Clock = serde_json::from_slice(&encoded).unwrap();
            assert!(restored.confirmations.is_empty());
            g.handoff_player(life, Controller(0)).unwrap();
            assert!(g.movement_confirmations(life).is_empty());
        }
    }

    #[test]
    fn expired_admission_fences_control_before_the_next_simulation_tick() {
        for secondary in [false, true] {
            let mut g = world();
            let owner = Controller(if secondary { 10 } else { 9 });
            let life = if secondary {
                g.add_player(owner, Vec3::new(5., 0., -22.)).unwrap()
            } else {
                g.player_life()
            };
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, life).unwrap();
            ticks(&mut g, 9);
            g.submit_movement_frame(owner, frame(&g, life, false))
                .unwrap();
            ticks(&mut g, 1);
            let delayed = frame(&g, life, false);
            let epoch = g.player_admission(life.actor).unwrap().epoch();
            let error = g.submit_movement_frame(owner, delayed.clone()).unwrap_err();
            assert!(error.contains("Movement interval clock expired"), "{error}");
            assert_eq!(g.player_admission(life.actor).unwrap().epoch(), epoch + 1);
            assert_eq!(
                g.movement_baseline(life).unwrap().unwrap().profile,
                crate::movement::Profile::Arrival
            );
            assert!(g.submit_movement_frame(owner, delayed).is_err());
            assert_eq!(g.player_admission(life.actor).unwrap().epoch(), epoch + 1);
            assert_eq!(g.movement_expiry.total, 1);
            assert_eq!(g.movement_expiry.admission, 1);
            assert_eq!(g.movement_expiry.samples[0].actor, life.actor);
            assert_eq!(g.movement_expiry.samples[0].epoch, epoch);
            assert_eq!(g.movement_expiry.samples[0].applied_sequence, 1);
            let saved = g.checkpoint().unwrap();
            assert!(
                !std::str::from_utf8(&saved)
                    .unwrap()
                    .contains("movement_expiry")
            );
            assert_eq!(Game::restore(&saved).unwrap().movement_expiry.total, 0);
            g.begin_movement_frames(owner, life).unwrap();
            g.submit_movement_frame(owner, frame(&g, life, false))
                .unwrap();
        }
    }
    #[test]
    fn interval_admission_is_not_a_physics_ack_and_invalid_frames_do_not_consume_sequence() {
        let mut g = world();
        let life = g.player_life();
        g.begin_movement_frames(Controller(9), life).unwrap();
        let f = frame(&g, life, false);
        let before = g.checkpoint().unwrap();
        for mode in 0..5 {
            let mut bad = f.clone();
            match mode {
                0 => bad.start += 1,
                1 => bad.steps = 13,
                2 => bad.epoch += 1,
                3 => bad.life.generation += 1,
                _ => bad.segments[0].until += 100,
            }
            assert!(g.submit_movement_frame(Controller(9), bad).is_err());
            assert_eq!(g.checkpoint().unwrap(), before);
        }
        g.submit_movement_frame(Controller(9), f.clone()).unwrap();
        let b = g.movement_baseline(life).unwrap().unwrap();
        assert_eq!(b.applied_sequence, 0);
        assert_eq!(b.physics_step, f.start);
        assert_eq!(
            g.player_admission(life.actor).unwrap().accepted_sequence(),
            1
        );
        assert!(g.submit_movement_frame(Controller(9), f.clone()).is_err());
        let mut restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
        ticks(&mut g, 1);
        ticks(&mut restored, 1);
        assert_eq!(g.checkpoint().unwrap(), restored.checkpoint().unwrap());
        let b = g.movement_baseline(life).unwrap().unwrap();
        assert_eq!(b.applied_sequence, 1);
        assert_eq!(b.physics_step, f.end().unwrap());
        assert!(b.character.feet.x > 0.);
    }
    #[test]
    fn silent_death_retires_primary_and_secondary_intervals_without_expiry() {
        for secondary in [false, true] {
            let mut g = world();
            let owner = Controller(if secondary { 10 } else { 9 });
            let life = if secondary {
                g.add_player(owner, Vec3::new(5., 0., -22.)).unwrap()
            } else {
                g.player_life()
            };
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, life).unwrap();
            let queued = frame(&g, life, true);
            g.submit_movement_frame(owner, queued.clone()).unwrap();
            let position = if secondary {
                g.additional_players[&life.actor].player
            } else {
                g.primary.player
            };
            g.hostile_hit_player(life, 10_000).unwrap();
            ticks(&mut g, 1);
            assert!(
                if secondary {
                    g.additional_players[&life.actor].frame_clock.is_none()
                } else {
                    g.primary.frame_clock.is_none()
                },
                "Dead control must retire before its clock expires"
            );
            let fenced = g.player_admission(life.actor).unwrap().epoch();
            assert_eq!(fenced, queued.epoch + 1);
            assert_eq!(g.player_admission(life.actor).unwrap().controller(), owner);
            assert!(g.submit_movement_frame(owner, queued.clone()).is_err());
            ticks(&mut g, 12);
            assert_eq!(g.movement_expiry.total, 0);
            assert_eq!(g.player_admission(life.actor).unwrap().epoch(), fenced);
            assert_eq!(
                if secondary {
                    g.additional_players[&life.actor].player
                } else {
                    g.primary.player
                },
                position
            );
            let next = g.respawn_controlled_player(owner, life).unwrap();
            assert_ne!(next, life);
            assert!(g.submit_movement_frame(owner, queued).is_err());
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, next).unwrap();
        }
    }

    #[test]
    fn primary_and_secondary_lifecycle_changes_fence_delayed_intervals() {
        for secondary in [false, true] {
            let mut g = world();
            let (life, owner) = if secondary {
                (
                    g.add_player(Controller(10), Vec3::new(5., 0., -22.))
                        .unwrap(),
                    Controller(10),
                )
            } else {
                (g.player_life(), Controller(9))
            };
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, life).unwrap();
            let old = frame(&g, life, false);
            g.handoff_player(life, Controller(0)).unwrap();
            g.handoff_player(life, owner).unwrap();
            assert!(g.submit_movement_frame(owner, old).is_err());
            g.begin_movement_frames(owner, life).unwrap();
            let dead_frame = frame(&g, life, true);
            g.hostile_hit_player(life, 10000).unwrap();
            assert!(g.submit_movement_frame(owner, dead_frame.clone()).is_err());
            let fenced_epoch = g.player_admission(life.actor).unwrap().epoch();
            assert_eq!(fenced_epoch, dead_frame.epoch + 1);
            assert!(g.movement_baseline(life).unwrap().is_none());
            assert!(if secondary {
                g.additional_players[&life.actor].frame_clock.is_none()
            } else {
                g.primary.frame_clock.is_none()
            });
            assert!(g.submit_movement_frame(owner, dead_frame.clone()).is_err());
            assert_eq!(
                g.player_admission(life.actor).unwrap().epoch(),
                fenced_epoch
            );
            let next = g.respawn_controlled_player(owner, life).unwrap();
            assert_ne!(next, life);
            assert!(g.submit_movement_frame(owner, dead_frame).is_err());
            ticks(&mut g, 1);
            g.begin_movement_frames(owner, next).unwrap();
            let before_teleport = frame(&g, next, false);
            let command = Command {
                actor: next,
                epoch: before_teleport.epoch,
                sequence: 1,
                tick: g.authority_tick,
                intent: Intent::Cast {
                    ability: Ability::MistyStep,
                    target: None,
                    aim: [1., 0., 0.],
                },
            };
            g.submit(owner, command).unwrap();
            assert!(g.submit_movement_frame(owner, before_teleport).is_err());
            assert_eq!(
                g.movement_baseline(next).unwrap().unwrap().profile,
                Profile::Arrival
            );
        }
    }
    #[test]
    fn no_input_and_repeated_entry_cannot_freeze_a_character_or_renew_elapsed_time() {
        let mut g = world();
        let life = g.player_life();
        g.begin_movement_frames(Controller(9), life).unwrap();
        let original = frame(&g, life, true);
        let epoch = original.epoch;
        g.submit_movement_frame(Controller(9), original.clone())
            .unwrap();
        ticks(&mut g, 1);
        assert!(g.movement_baseline(life).unwrap().unwrap().character.feet.y > 0.);
        for _ in 0..8 {
            g.begin_movement_frames(Controller(9), life).unwrap();
            ticks(&mut g, 1);
        }
        ticks(&mut g, 1);
        let b = g.movement_baseline(life).unwrap().unwrap();
        assert_eq!(b.profile, Profile::Arrival);
        assert!(b.epoch > epoch);
        assert!(g.submit_movement_frame(Controller(9), original).is_err());
        ticks(&mut g, 60);
        assert!(
            g.movement_baseline(life)
                .unwrap()
                .unwrap()
                .character
                .support
                .is_some()
        );
    }
    #[test]
    fn interval_movement_interrupts_an_active_cast_without_rewinding_its_resources() {
        let mut g = world();
        let life = g.player_life();
        g.begin_movement_frames(Controller(9), life).unwrap();
        let b = g.movement_baseline(life).unwrap().unwrap();
        g.submit(
            Controller(9),
            Command {
                actor: life,
                epoch: b.epoch,
                sequence: 1,
                tick: g.authority_tick,
                intent: Intent::Cast {
                    ability: Ability::Fireball,
                    target: Some(g.actor_life(1).unwrap()),
                    aim: [0., 0., 1.],
                },
            },
        )
        .unwrap();
        assert!(g.primary.casting.is_some());
        let mana = g.snapshot().player.mana;
        let f = frame(&g, life, false);
        g.submit_movement_frame(Controller(9), f).unwrap();
        ticks(&mut g, 1);
        assert!(g.primary.casting.is_none());
        assert!(g.snapshot().player.mana >= mana);
        assert!(g.movement_baseline(life).unwrap().unwrap().applied_sequence >= 2);
    }

    #[test]
    fn an_obstructed_spawn_respawns_at_the_nearest_clear_cell() {
        // Battle soak run 2: the frontline player's respawn was refused while
        // others stood on its spawn, and it stayed dead for minutes (#10559).
        let mut g = world();
        let spawn = Vec3::new(5., 0., -22.);
        let life = g.add_player(Controller(10), spawn).unwrap();
        ticks(&mut g, 1);
        g.hostile_hit_player(life, 10_000).unwrap();
        ticks(&mut g, 1);
        // Another body's corpse box lies across the spawn.
        let mut blockers = g.blockers.clone();
        let at = spawn.as_dvec3();
        blockers
            .upsert(
                physics::queries::Life {
                    instance: life.instance,
                    entity: 900_000,
                    generation: 0,
                },
                at - glam::DVec3::new(0.35, 0., 0.8),
                at + glam::DVec3::new(0.35, 0.24, 0.8),
            )
            .unwrap();
        g.replace_blockers(blockers).unwrap();
        let next = g.respawn_controlled_player(Controller(10), life).unwrap();
        let p = &g.additional_players[&next.actor];
        let feet = p.character.feet;
        let away = glam::DVec2::new(feet.x - f64::from(spawn.x), feet.z - f64::from(spawn.z));
        assert!(
            away.length() > 0.5 && away.length() <= super::super::multiplayer::RESPAWN_RADIUS,
            "{feet:?}"
        );
        assert_eq!(p.player, feet.as_vec3());
        assert_eq!(p.spawn, spawn);
        assert_eq!(g.player_snapshot(next).unwrap().player.hp, 200);
        ticks(&mut g, 2);
        assert!(
            g.additional_players[&next.actor]
                .character
                .support
                .is_some()
        );
    }
}
