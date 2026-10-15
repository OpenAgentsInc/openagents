//! Read-only child activity. References identify owner records, never files.

use crate::{Digest, digest_of, observation::Scope};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.children.v1";
pub const MAX_CHILDREN: usize = 64;
pub const PAGE_BYTES: usize = 16 * 1024;
pub const TRANSCRIPT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Native,
    Delegation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Completed,
    Failed,
    Interrupted,
    #[serde(other)]
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub schema: String,
    pub id: Digest,
    pub scope: Scope,
    pub source: Source,
    pub child: String,
}
impl Reference {
    pub fn new(scope: Scope, source: Source, child: String) -> Self {
        let mut reference = Self {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            scope,
            source,
            child,
        };
        reference.id = reference.identity();
        reference
    }
    fn identity(&self) -> Digest {
        digest_of(&(&self.schema, &self.scope, self.source, &self.child))
    }
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && self.scope.turn > 0
            && self.scope.generation > 0
            && valid_id(&self.scope.host)
            && valid_id(&self.scope.task)
            && valid_id(&self.child)
            && self.id == self.identity()
    }
}
pub fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Child {
    pub reference: Reference,
    pub parent: Option<String>,
    pub title: String,
    pub harness: String,
    pub model: Option<String>,
    pub status: Status,
    pub summary: String,
    pub transcript: bool,
    pub incomplete: bool,
}
impl Child {
    pub fn valid(&self) -> bool {
        self.reference.valid()
            && self.title.len() <= 512
            && self.harness.len() <= 128
            && self.summary.len() <= 2048
            && self.model.as_ref().is_none_or(|model| model.len() <= 256)
            && self
                .parent
                .as_ref()
                .is_none_or(|parent| valid_id(parent) && parent != &self.reference.child)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: String,
    pub task: String,
    pub revision: u64,
    pub children: Vec<Child>,
    pub truncated: bool,
}
impl Snapshot {
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && valid_id(&self.task)
            && self.children.len() <= MAX_CHILDREN
            && self.children.iter().enumerate().all(|(index, child)| {
                child.valid()
                    && child.reference.scope.task == self.task
                    && !self.children[..index]
                        .iter()
                        .any(|earlier| earlier.reference.id == child.reference.id)
            })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub offset: usize,
    pub prefix: Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Read {
    pub reference: Reference,
    pub cursor: Option<Cursor>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub reference: Digest,
    pub offset: usize,
    pub text: String,
    pub next: Option<Cursor>,
    pub truncated: bool,
    pub running: bool,
}
impl Page {
    pub fn valid_for(&self, read: &Read) -> bool {
        read.reference.valid()
            && self.reference == read.reference.id
            && self.offset == read.cursor.as_ref().map_or(0, |cursor| cursor.offset)
            && self.offset <= TRANSCRIPT_BYTES
            && self.text.len() <= PAGE_BYTES
            && self.offset + self.text.len() <= TRANSCRIPT_BYTES
            && self.next.as_ref().is_none_or(|cursor| {
                cursor.offset == self.offset + self.text.len()
                    && cursor.offset <= TRANSCRIPT_BYTES
                    && !self.text.is_empty()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> Reference {
        Reference::new(
            Scope {
                host: "local".into(),
                task: "task".into(),
                turn: 1,
                generation: 1,
            },
            Source::Native,
            "child".into(),
        )
    }
    #[test]
    fn child_references_and_page_bounds_reject_changed_identity_or_overflow() {
        let reference = reference();
        assert!(reference.valid());
        let mut other = reference.clone();
        other.scope.generation += 1;
        assert!(!other.valid());
        let read = Read {
            reference,
            cursor: None,
        };
        let mut page = Page {
            reference: read.reference.id.clone(),
            offset: 0,
            text: "hello".into(),
            next: None,
            truncated: false,
            running: true,
        };
        assert!(page.valid_for(&read));
        page.next = Some(Cursor {
            offset: 4,
            prefix: Digest::of_bytes(b"hell"),
        });
        assert!(!page.valid_for(&read));
        page.next = None;
        page.offset = TRANSCRIPT_BYTES;
        let read = Read {
            cursor: Some(Cursor {
                offset: TRANSCRIPT_BYTES,
                prefix: Digest::of_bytes(b""),
            }),
            ..read
        };
        assert!(!page.valid_for(&read));
    }
}
