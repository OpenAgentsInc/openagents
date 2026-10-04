//! Transport-independent command admission for owned Verse worlds.
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;

/// Identity resolved by a trusted local controller or authenticated transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Controller(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Intent<A> {
    Jump,
    Move {
        axes: [f32; 2],
        yaw: f32,
    },
    Cast {
        ability: A,
        target: Option<LifeId>,
        aim: [f32; 3],
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Command<A> {
    pub actor: LifeId,
    pub epoch: u64,
    pub sequence: u64,
    pub tick: u64,
    pub intent: Intent<A>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Refusal {
    NotController,
    StaleLife,
    StaleControl,
    DuplicateSequence,
    WrongTick,
    InvalidIntent,
    Exhausted,
}
/// Admission fences are state, independent of the transport carrying commands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Admission {
    actor: LifeId,
    controller: Controller,
    epoch: u64,
    last_sequence: u64,
}
impl Admission {
    pub fn new(actor: LifeId, controller: Controller) -> Self {
        Self {
            actor,
            controller,
            epoch: 0,
            last_sequence: 0,
        }
    }
    pub fn actor(&self) -> LifeId {
        self.actor
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn controller(&self) -> Controller {
        self.controller
    }
    /// Fences queued input even when ownership returns to the same controller.
    pub fn handoff(&mut self, controller: Controller) -> Result<(), Refusal> {
        let epoch = self.epoch.checked_add(1).ok_or(Refusal::Exhausted)?;
        self.controller = controller;
        self.epoch = epoch;
        self.last_sequence = 0;
        Ok(())
    }
    pub fn respawn(&mut self) -> Result<(), Refusal> {
        let life = self.actor.next().map_err(|_| Refusal::Exhausted)?;
        self.handoff(self.controller)?;
        self.actor = life;
        Ok(())
    }
    pub fn command<A>(&self, tick: u64, intent: Intent<A>) -> Result<Command<A>, Refusal> {
        Ok(Command {
            actor: self.actor,
            epoch: self.epoch,
            sequence: self
                .last_sequence
                .checked_add(1)
                .ok_or(Refusal::Exhausted)?,
            tick,
            intent,
        })
    }
    /// Performs identity and input validation before game-specific rule admission.
    /// Invalid requests do not consume sequence numbers. Accepted envelopes do,
    /// including those subsequently refused by resource or gameplay rules.
    pub fn admit<A>(
        &mut self,
        sender: Controller,
        command: &Command<A>,
        tick: u64,
    ) -> Result<(), Refusal> {
        if sender != self.controller {
            return Err(Refusal::NotController);
        }
        if command.actor != self.actor {
            return Err(Refusal::StaleLife);
        }
        if command.epoch != self.epoch {
            return Err(Refusal::StaleControl);
        }
        if command.sequence <= self.last_sequence {
            return Err(Refusal::DuplicateSequence);
        }
        if command.tick > tick || tick - command.tick > 6 {
            return Err(Refusal::WrongTick);
        }
        match &command.intent {
            Intent::Jump => {}
            Intent::Move { axes, yaw } => {
                if !yaw.is_finite() || axes.iter().any(|v| !v.is_finite() || v.abs() > 1.) {
                    return Err(Refusal::InvalidIntent);
                }
            }
            Intent::Cast { aim, target, .. } => {
                let length: f32 = aim.iter().map(|v| v * v).sum();
                if aim.iter().any(|v| !v.is_finite())
                    || !(0.99..=1.01).contains(&length)
                    || target.is_some_and(|t| t.instance != self.actor.instance)
                {
                    return Err(Refusal::InvalidIntent);
                }
            }
        }
        self.last_sequence = command.sequence;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn admission() -> Admission {
        Admission::new(
            LifeId {
                instance: 1,
                actor: 14,
                generation: 0,
            },
            Controller(7),
        )
    }
    fn movement(a: &Admission) -> Command<()> {
        a.command(
            10,
            Intent::Move {
                axes: [0., 1.],
                yaw: 0.,
            },
        )
        .unwrap()
    }
    #[test]
    fn controller_and_life_fences_precede_sequence_consumption() {
        let mut a = admission();
        let command = movement(&a);
        assert_eq!(
            a.admit(Controller(8), &command, 10),
            Err(Refusal::NotController)
        );
        a.admit(Controller(7), &command, 10).unwrap();
        assert_eq!(
            a.admit(Controller(7), &command, 10),
            Err(Refusal::DuplicateSequence)
        );
        a.respawn().unwrap();
        assert_eq!(
            a.admit(Controller(7), &command, 10),
            Err(Refusal::StaleLife)
        );
        assert_eq!(a.actor().generation, 1);
    }
    #[test]
    fn returning_controller_cannot_replay_queued_input() {
        let mut a = admission();
        let old = movement(&a);
        a.handoff(Controller(8)).unwrap();
        a.handoff(Controller(7)).unwrap();
        assert_eq!(a.admit(Controller(7), &old, 10), Err(Refusal::StaleControl));
        a.admit(Controller(7), &movement(&a), 10).unwrap();
    }
    #[test]
    fn bounded_ticks_and_invalid_intents_are_refused_without_mutation() {
        let mut a = admission();
        let mut command = movement(&a);
        assert_eq!(a.admit(Controller(7), &command, 9), Err(Refusal::WrongTick));
        assert_eq!(
            a.admit(Controller(7), &command, 17),
            Err(Refusal::WrongTick)
        );
        command.intent = Intent::Move {
            axes: [f32::NAN, 0.],
            yaw: 0.,
        };
        assert_eq!(
            a.admit(Controller(7), &command, 10),
            Err(Refusal::InvalidIntent)
        );
        command.intent = Intent::Cast {
            ability: (),
            target: None,
            aim: [0., 0., 100.],
        };
        assert_eq!(
            a.admit(Controller(7), &command, 10),
            Err(Refusal::InvalidIntent)
        );
        command.intent = Intent::Cast {
            ability: (),
            target: Some(LifeId {
                instance: 2,
                ..a.actor()
            }),
            aim: [0., 0., 1.],
        };
        assert_eq!(
            a.admit(Controller(7), &command, 10),
            Err(Refusal::InvalidIntent)
        );
        a.admit(Controller(7), &movement(&a), 10).unwrap();
    }
}

pub mod combat;
pub mod controls;
pub mod events;
pub mod play;
pub mod room;
pub mod rules;
pub mod utilities;
