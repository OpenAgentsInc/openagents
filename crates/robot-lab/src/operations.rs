//! Typed local operations shared by chat, terminal, and the Verse room.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::document::{Document, Reference, Step};
use crate::store::{Note, Selection, Store, Version};
use crate::{Error, Result, require};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    List,
    Open {
        document: String,
        version: Option<String>,
    },
    Create {
        document: String,
        content: Document,
        author: String,
        summary: String,
    },
    Edit {
        base: Reference,
        content: Document,
        author: String,
        summary: String,
    },
    Pose {
        base: Reference,
        robot: String,
        joints: BTreeMap<String, f64>,
        author: String,
        summary: String,
    },
    MoveObject {
        base: Reference,
        object: String,
        position: [f64; 3],
        author: String,
        summary: String,
    },
    Compare {
        a: Reference,
        b: Reference,
    },
    Versions {
        document: String,
    },
    Notes {
        subject: Reference,
    },
    AddNote {
        note: Note,
    },
    Select {
        chat: String,
        selection: Selection,
    },
    Restore {
        chat: String,
    },
    Inspect {
        episode: Reference,
        step: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Reply {
    Items {
        items: Vec<Reference>,
    },
    Opened {
        reference: Reference,
        version: Box<Version>,
    },
    Saved {
        reference: Reference,
        summary: String,
    },
    Compared {
        a: Reference,
        b: Reference,
        left: Box<Version>,
        right: Box<Version>,
    },
    Versions {
        versions: Vec<Version>,
    },
    Notes {
        notes: Vec<Note>,
    },
    Noted,
    Selection {
        selection: Option<Selection>,
    },
    Inspected {
        episode: Reference,
        step: Box<Step>,
    },
}

impl Store {
    /// Dispatch an already selected typed operation. `now_unix_ms` is supplied
    /// by the host; no model or keyword router runs inside the lab.
    pub fn execute(&self, operation: Operation, now_unix_ms: u64) -> Result<Reply> {
        let save = |document: &str,
                    base: Option<&Reference>,
                    content: Document,
                    author: &str,
                    summary: String|
         -> Result<Reply> {
            let reference = self.save(document, base, content, author, &summary, now_unix_ms)?;
            Ok(Reply::Saved { reference, summary })
        };
        match operation {
            Operation::List => Ok(Reply::Items {
                items: self.list()?,
            }),
            Operation::Open { document, version } => {
                let reference = match version {
                    Some(version) => Reference { document, version },
                    None => self.latest(&document)?,
                };
                let version = Box::new(self.get(&reference)?);
                Ok(Reply::Opened { reference, version })
            }
            Operation::Create {
                document,
                content,
                author,
                summary,
            } => save(&document, None, content, &author, summary),
            Operation::Edit {
                base,
                content,
                author,
                summary,
            } => save(&base.document, Some(&base), content, &author, summary),
            Operation::Pose {
                base,
                robot,
                joints,
                author,
                summary,
            } => {
                let Document::Scene(mut scene) = self.get(&base)?.content else {
                    return Err(Error::Invalid("Open a scene before moving a joint.".into()));
                };
                let instance = scene
                    .robots
                    .iter_mut()
                    .find(|r| r.id == robot)
                    .ok_or_else(|| Error::Invalid("Choose a robot in this scene.".into()))?;
                instance.joints.extend(joints);
                save(
                    &base.document,
                    Some(&base),
                    Document::Scene(scene),
                    &author,
                    summary,
                )
            }
            Operation::MoveObject {
                base,
                object,
                position,
                author,
                summary,
            } => {
                let Document::Scene(mut scene) = self.get(&base)?.content else {
                    return Err(Error::Invalid(
                        "Open a scene before moving an object.".into(),
                    ));
                };
                let object = scene
                    .objects
                    .iter_mut()
                    .find(|o| o.id == object)
                    .ok_or_else(|| Error::Invalid("Choose an object in this scene.".into()))?;
                object.geometry.pose.position = position;
                save(
                    &base.document,
                    Some(&base),
                    Document::Scene(scene),
                    &author,
                    summary,
                )
            }
            Operation::Compare { a, b } => {
                let left = Box::new(self.get(&a)?);
                let right = Box::new(self.get(&b)?);
                require(
                    left.content.kind() == right.content.kind(),
                    "Compare two items of the same type.",
                )?;
                Ok(Reply::Compared { a, b, left, right })
            }
            Operation::Versions { document } => Ok(Reply::Versions {
                versions: self.versions(&document)?,
            }),
            Operation::Notes { subject } => Ok(Reply::Notes {
                notes: self.notes(&subject)?,
            }),
            Operation::AddNote { note } => {
                self.add_note(note)?;
                Ok(Reply::Noted)
            }
            Operation::Select { chat, selection } => {
                self.select(&chat, &selection)?;
                Ok(Reply::Selection {
                    selection: Some(selection),
                })
            }
            Operation::Restore { chat } => Ok(Reply::Selection {
                selection: self.selection(&chat)?,
            }),
            Operation::Inspect { episode, step } => {
                let Document::Episode(value) = self.get(&episode)?.content else {
                    return Err(Error::Invalid("Choose a recorded episode.".into()));
                };
                let value = value
                    .steps
                    .get(step as usize)
                    .ok_or_else(|| Error::Invalid("Choose a step from this episode.".into()))?;
                Ok(Reply::Inspected {
                    episode,
                    step: Box::new(value.clone()),
                })
            }
        }
    }
}
