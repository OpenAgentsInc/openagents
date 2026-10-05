//! Confirmed movement intervals do not rewind world time, combat, or receipts.
use super::*;
use crate::{
    Command, Controller, Intent,
    movement::frames::{Clock, Frame},
};
use verse_engine::core::LifeId;
impl Game {
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
            self.frame_clock.is_some()
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
            self.frame_clock = clock;
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
            || self.player_snapshot(frame.life)?.player.hp == 0
            || !self.unlocked()
        {
            return Err("Movement interval control is stale or unavailable".into());
        }
        let clock = if actor == self.player_actor() {
            self.frame_clock.as_ref()
        } else {
            self.additional_players[&actor].frame_clock.as_ref()
        }
        .ok_or("Movement intervals have not been started")?;
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
            self.admission
                .admit(sender, &command, self.authority_tick)
                .map_err(|e| format!("Movement interval refused: {e:?}"))?;
            self.frame_clock = Some(next);
        } else {
            let player = self.additional_players.get_mut(&actor).unwrap();
            player
                .admission
                .admit(sender, &command, self.authority_tick)
                .map_err(|e| format!("Movement interval refused: {e:?}"))?;
            player.frame_clock = Some(next);
        }
        Ok(())
    }
    pub(super) fn expire_primary_frames(&mut self) -> Result<(), String> {
        if self
            .frame_clock
            .as_ref()
            .is_some_and(|clock| clock.expired(self.physics_steps))
        {
            self.handoff_player(self.player_life(), self.admission.controller())?;
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
        assert!(g.casting.is_some());
        let mana = g.snapshot().player.mana;
        let f = frame(&g, life, false);
        g.submit_movement_frame(Controller(9), f).unwrap();
        ticks(&mut g, 1);
        assert!(g.casting.is_none());
        assert!(g.snapshot().player.mana >= mana);
        assert!(g.movement_baseline(life).unwrap().unwrap().applied_sequence >= 2);
    }
}
