//! Moves persistent resources and remaining cooldowns between simulation clocks.
use super::*;
#[derive(Clone)]
pub(crate) struct Portable {
    state: PlayerState,
    elapsed: f32,
}
impl Simulation {
    pub(crate) fn take_player(&mut self, id: u32) -> Result<Portable, String> {
        if id == 0 {
            return Err(
                "Primary scene anchor requires character decoupling before transfer".into(),
            );
        }
        let state = self
            .players
            .remove(&id)
            .ok_or("Transfer player is missing")?;
        self.actors.remove(&id);
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
        if id == 0 || !self.players.contains_key(&id) || portable.state.resources.hp == 0 {
            return Err("Transfer requires a living destination player".into());
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
        actor.alive = true;
        self.players.insert(id, portable.state);
        self.validate()
    }
}
