//! Moves persistent resources and remaining cooldowns between simulation clocks.
use super::*;
#[derive(Clone)]
pub(crate) struct Portable {
    state: PlayerState,
    elapsed: f32,
}
impl Portable {
    pub(crate) fn defeated(&self) -> bool {
        self.state.resources.hp == 0
    }
}
impl Simulation {
    pub(crate) fn primary_absent(&self) -> bool {
        self.primary_absent
    }

    pub(crate) fn take_player(&mut self, id: u32) -> Result<Portable, String> {
        let state = if id == 0 {
            if self.primary_absent {
                return Err("Primary character is already absent".into());
            }
            self.primary_absent = true;
            let mut empty = PlayerState::default();
            empty.resources.hp = 0;
            empty.resources.mana = 0;
            let state = self
                .players
                .insert(0, empty)
                .ok_or("Transfer player is missing")?;
            let actor = self
                .actors
                .get_mut(&0)
                .ok_or("Primary simulation anchor is missing")?;
            actor.hp = 0;
            actor.max_hp = 200;
            actor.alive = false;
            state
        } else {
            let state = self
                .players
                .remove(&id)
                .ok_or("Transfer player is missing")?;
            self.actors.remove(&id);
            state
        };
        self.motion_starts.remove(&id);
        self.motion_paths.remove(&id);
        self.deaths.remove(&id);
        self.flights
            .retain(|f| f.view.caster != id && f.target != Some(id));
        self.burns.retain(|b| b.caster != id && b.actor != id);
        Ok(Portable {
            state,
            elapsed: self.elapsed,
        })
    }
    pub(crate) fn put_player(&mut self, id: u32, mut portable: Portable) -> Result<(), String> {
        if id == 0 || !self.players.contains_key(&id) {
            return Err("Transfer destination player is missing".into());
        }
        for at in portable.state.ready.values_mut() {
            *at = self.elapsed + (*at - portable.elapsed).max(0.);
        }
        portable.state.global_ready =
            self.elapsed + (portable.state.global_ready - portable.elapsed).max(0.);
        let actor = self
            .actors
            .get_mut(&id)
            .ok_or("Transfer destination actor is missing")?;
        actor.hp = portable.state.resources.hp;
        actor.max_hp = portable.state.resources.max_hp;
        actor.alive = portable.state.resources.hp > 0;
        self.players.insert(id, portable.state);
        self.validate()
    }
}
