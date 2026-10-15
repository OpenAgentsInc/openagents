//! Deterministic session observations and control barriers, separate from hooks.
//! A host authorizes controls before delivering them here. Playback executes
//! no tools, reads no files, and grants no permission.

use crate::observation::{Observation, Scope};
use crate::resources::{Input, Owner, Receipt, Reference};
use route_contract::{Digest, digest_of, pending::Reply, session::control::Native};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder-sdk.conversation.v1";
const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expected {
    Reply {
        reply: Reply,
    },
    Stop {
        scope: Scope,
    },
    Resume {
        scope: Scope,
        native: Native,
    },
    Input {
        owner: Owner,
        text: Digest,
        images: Vec<Digest>,
    },
    Read {
        reference: Digest,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Control {
    Reply {
        reply: Reply,
    },
    Stop {
        scope: Scope,
    },
    Resume {
        scope: Scope,
        native: Native,
    },
    Input {
        input: Input,
        receipt: Receipt,
        text: String,
    },
    Read {
        reference: Reference,
        generation: u64,
    },
}
impl Expected {
    fn accepts(&self, control: &Control) -> bool {
        match (self, control) {
            (Self::Reply { reply: a }, Control::Reply { reply: b }) => a == b,
            (Self::Stop { scope: a }, Control::Stop { scope: b }) => a == b,
            (
                Self::Resume {
                    scope: a,
                    native: x,
                },
                Control::Resume {
                    scope: b,
                    native: y,
                },
            ) => a == b && x == y,
            (
                Self::Input {
                    owner,
                    text,
                    images,
                },
                Control::Input {
                    input,
                    receipt,
                    text: prompt,
                },
            ) => {
                input.owner == *owner
                    && input.text == *text
                    && input.valid_for(&owner.task, prompt)
                    && receipt.valid_for(input)
                    && input
                        .resources
                        .iter()
                        .map(|resource| resource.digest.clone())
                        .collect::<Vec<_>>()
                        == *images
            }
            (
                Self::Read { reference: a },
                Control::Read {
                    reference: b,
                    generation,
                },
            ) => *generation > 0 && b.valid() && *a == b.id,
            _ => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Observations { events: Vec<Observation> },
    Await { expected: Expected },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub schema: String,
    pub owner: Owner,
    pub steps: Vec<Step>,
}
impl Conversation {
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("This session recording is too large.".into());
        }
        let recording: Self =
            serde_json::from_str(text).map_err(|_| "This session recording couldn't be read.")?;
        recording.validate()?;
        Ok(recording)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA || !self.owner.valid() || self.steps.len() > 512 {
            return Err("This session recording has an unsupported format.".into());
        }
        let mut count = 0usize;
        for step in &self.steps {
            match step {
                Step::Observations { events } => {
                    count = count.saturating_add(events.len());
                    if count > 4096
                        || events.iter().any(|event| {
                            !event.applies_to(&self.owner.host, &self.owner.task, event.scope.turn)
                                || event.scope.generation == 0
                        })
                    {
                        return Err(
                            "This session recording names another task or has too many events."
                                .into(),
                        );
                    }
                }
                Step::Await { expected } => {
                    let valid = match expected {
                        Expected::Reply { reply } => {
                            reply.scope.host == self.owner.host
                                && reply.scope.task == self.owner.task
                        }
                        Expected::Stop { scope } => {
                            scope.host == self.owner.host
                                && scope.task == self.owner.task
                                && scope.turn > 0
                                && scope.generation > 0
                        }
                        Expected::Resume { scope, native } => {
                            scope.host == self.owner.host
                                && scope.task == self.owner.task
                                && native.valid()
                                && native.host == self.owner.host
                                && native.task == self.owner.task
                        }
                        Expected::Input { owner, images, .. } => {
                            *owner == self.owner && images.len() <= crate::resources::MAX_INPUTS
                        }
                        Expected::Read { .. } => true,
                    };
                    if !valid {
                        return Err("This session recording names another task.".into());
                    }
                }
            }
        }
        let mut bound = Bounded(0);
        serde_json::to_writer(&mut bound, self)
            .map_err(|_| "This session recording is too large.")?;
        Ok(())
    }
    pub fn play(self) -> Result<Playback, String> {
        self.validate()?;
        Ok(Playback {
            recording: self,
            cursor: 0,
            delivered: vec![],
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Playback {
    recording: Conversation,
    cursor: usize,
    delivered: Vec<(usize, Digest)>,
}
impl Playback {
    pub fn next_observations(&mut self) -> Option<Vec<Observation>> {
        let Step::Observations { events } = self.recording.steps.get(self.cursor)? else {
            return None;
        };
        let events = events.clone();
        self.cursor += 1;
        Some(events)
    }
    pub fn waiting(&self) -> Option<&Expected> {
        match self.recording.steps.get(self.cursor)? {
            Step::Await { expected } => Some(expected),
            _ => None,
        }
    }
    /// Returns false for an already delivered control, without advancing again.
    pub fn deliver(&mut self, control: &Control) -> Result<bool, String> {
        let id = digest_of(control);
        if self.delivered.iter().any(|(_, known)| *known == id) {
            return Ok(false);
        }
        if !self
            .waiting()
            .is_some_and(|expected| expected.accepts(control))
        {
            return Err("This control doesn't match the recorded session.".into());
        }
        self.delivered.push((self.cursor, id));
        self.cursor += 1;
        Ok(true)
    }
    pub fn finished(&self) -> bool {
        self.cursor == self.recording.steps.len()
    }
    pub fn checkpoint(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|_| "This session recording couldn't be saved.".into())
    }
    pub fn restore(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("This session recording is too large.".into());
        }
        let playback: Self =
            serde_json::from_str(text).map_err(|_| "This session recording couldn't be read.")?;
        playback.recording.validate()?;
        if playback.cursor > playback.recording.steps.len()
            || playback.delivered.len() > 512
            || playback.delivered.iter().any(|(position, _)| {
                *position >= playback.cursor
                    || !matches!(
                        playback.recording.steps.get(*position),
                        Some(Step::Await { .. })
                    )
            })
        {
            return Err("This session recording has an invalid position.".into());
        }
        Ok(playback)
    }
}
struct Bounded(usize);
impl std::io::Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > MAX_BYTES {
            Err(std::io::ErrorKind::InvalidData.into())
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::{Body, Ending, Recorder};
    #[test]
    fn replay_waits_for_the_exact_control_and_checkpoints_do_not_repeat_it() {
        let scope = Scope {
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            generation: 1,
        };
        let mut recorder = Recorder::new(scope.clone());
        let recording = Conversation {
            schema: SCHEMA.into(),
            owner: Owner {
                host: "local".into(),
                task: "task".into(),
            },
            steps: vec![
                Step::Observations {
                    events: vec![recorder.next(
                        None,
                        Body::Started {
                            provider: "claude".into(),
                            model: "fixture".into(),
                        },
                    )],
                },
                Step::Await {
                    expected: Expected::Stop {
                        scope: scope.clone(),
                    },
                },
                Step::Observations {
                    events: vec![recorder.next(
                        None,
                        Body::Ended {
                            ending: Ending::Interrupted,
                            message: None,
                        },
                    )],
                },
            ],
        };
        let mut playback = Conversation::from_json(&serde_json::to_string(&recording).unwrap())
            .unwrap()
            .play()
            .unwrap();
        assert_eq!(playback.next_observations().unwrap().len(), 1);
        assert!(playback.next_observations().is_none());
        let wrong = Control::Stop {
            scope: Scope {
                generation: 2,
                ..scope.clone()
            },
        };
        assert!(playback.deliver(&wrong).is_err());
        let control = Control::Stop { scope };
        assert!(playback.deliver(&control).unwrap());
        let mut reopened = Playback::restore(&playback.checkpoint().unwrap()).unwrap();
        assert!(!reopened.deliver(&control).unwrap());
        assert_eq!(reopened.next_observations().unwrap().len(), 1);
        assert!(reopened.finished());
        let mut invalid = recording;
        invalid.owner.task = "other".into();
        assert!(invalid.play().is_err());
    }
}
