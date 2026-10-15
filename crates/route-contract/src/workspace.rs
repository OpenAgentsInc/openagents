//! Bounded workspace reads. A reference describes authority; the host verifies it.

use crate::{Digest, digest_of, pending::Authority};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.workspace.v1";
pub const MAX_ROWS: usize = 128;
pub const MAX_TEXT: usize = 64 * 1024;
pub const MAX_PATH: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub schema: String,
    pub id: Digest,
    pub host: String,
    pub task: String,
    pub turn: usize,
    pub authority: Authority,
}
impl Reference {
    fn identity(&self) -> Digest {
        digest_of(&(
            &self.schema,
            &self.host,
            &self.task,
            self.turn,
            &self.authority,
        ))
    }
    pub fn seal(&mut self) {
        self.id = self.identity();
    }
    pub fn valid(&self) -> bool {
        self.schema == SCHEMA
            && self.turn > 0
            && !self.host.is_empty()
            && self.host.len() <= 256
            && !self.task.is_empty()
            && self.task.len() <= 256
            && !self.authority.checkout.is_empty()
            && self.authority.checkout.len() <= MAX_PATH
            && !self.authority.target.is_empty()
            && self.authority.target.len() <= 256
            && !self.authority.intent.is_empty()
            && self.authority.intent.len() <= 256
            && !self.authority.grant.is_empty()
            && self.authority.grant.len() <= 256
            && self.id == self.identity()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    List { path: String },
    Read { path: String },
    Search { path: String, query: String },
}
impl Operation {
    pub fn path(&self) -> &str {
        match self {
            Self::List { path } | Self::Read { path } | Self::Search { path, .. } => path,
        }
    }
    pub fn valid(&self) -> bool {
        let path = self.path();
        path.len() <= MAX_PATH
            && !path.contains(['\\', '\0'])
            && !path.starts_with('/')
            && !path.chars().any(char::is_control)
            && !path.contains(':')
            && (path.is_empty()
                || path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".." && p != ".git"))
            && !matches!(self, Self::Read {path} if path.is_empty())
            && match self {
                Self::Search { query, .. } => {
                    !query.is_empty() && query.len() <= 256 && !query.contains(['\0', '\n', '\r'])
                }
                _ => true,
            }
    }
    pub fn id(&self) -> Digest {
        digest_of(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub reference: Reference,
    pub operation: Operation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Directory,
    Match,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub id: Digest,
    pub path: String,
    pub kind: Kind,
    pub line: Option<usize>,
    pub text: String,
}
impl Row {
    pub fn new(path: String, kind: Kind, line: Option<usize>, text: String) -> Self {
        let id = digest_of(&(&path, kind, line, &text));
        Self {
            id,
            path,
            kind,
            line,
            text,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub reference: Digest,
    pub operation: Digest,
    pub rows: Vec<Row>,
    pub text: Option<String>,
    pub content: Option<Digest>,
    pub truncated: bool,
}
impl Output {
    pub fn valid_for(&self, request: &Request) -> bool {
        request.reference.valid()
            && request.operation.valid()
            && self
                .text
                .as_ref()
                .is_none_or(|text| text.len() <= MAX_TEXT && !text.contains('\0'))
            && match (&request.operation, &self.text, &self.content) {
                (Operation::Read { .. }, Some(text), Some(content)) => {
                    self.rows.is_empty() && content == &Digest::of_bytes(text.as_bytes())
                }
                (Operation::Read { .. }, _, _) => false,
                (_, None, None) => true,
                _ => false,
            }
            && self.reference == request.reference.id
            && self.operation == request.operation.id()
            && self.rows.len() <= MAX_ROWS
            && self.text.as_ref().is_none_or(|text| text.len() <= MAX_TEXT)
            && self.rows.iter().all(|row| {
                Operation::Read {
                    path: row.path.clone(),
                }
                .valid()
                    && row.line.is_none_or(|line| line > 0)
                    && match request.operation {
                        Operation::Search { .. } => row.kind == Kind::Match && row.line.is_some(),
                        Operation::List { .. } => row.kind != Kind::Match && row.line.is_none(),
                        Operation::Read { .. } => false,
                    }
                    && row.text.len() <= 1024
                    && row.id == digest_of(&(&row.path, row.kind, row.line, &row.text))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_literal_queries_are_relative_and_bounded() {
        for path in [
            "../a",
            "a/../b",
            "a//b",
            ".git/config",
            "a/.git/config",
            "/a",
            "C:/a",
            "a\\b",
            "a\0b",
        ] {
            assert!(!Operation::Read { path: path.into() }.valid(), "{path}");
        }
        assert!(
            Operation::List {
                path: String::new()
            }
            .valid()
        );
        assert!(
            Operation::Read {
                path: "src/lib.rs".into()
            }
            .valid()
        );
        assert!(
            !Operation::Search {
                path: String::new(),
                query: String::new()
            }
            .valid()
        );
        assert!(
            !Operation::Search {
                path: String::new(),
                query: "x".repeat(257)
            }
            .valid()
        );
        assert!(
            Operation::Search {
                path: "src".into(),
                query: "a.*b".into()
            }
            .valid()
        );
    }
    #[test]
    fn result_identity_changes_with_text_and_rejects_a_stale_query() {
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            authority: Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/fixture".into(),
                target: "inode".into(),
            },
        };
        reference.seal();
        let request = Request {
            reference,
            operation: Operation::Search {
                path: String::new(),
                query: "needle".into(),
            },
        };
        let row = Row::new("a.txt".into(), Kind::Match, Some(1), "needle".into());
        let mut output = Output {
            reference: request.reference.id.clone(),
            operation: request.operation.id(),
            rows: vec![row.clone()],
            text: None,
            content: None,
            truncated: false,
        };
        assert!(output.valid_for(&request));
        output.rows[0].text = "changed".into();
        assert!(!output.valid_for(&request));
        output.rows[0] = row;
        let mut other = request.clone();
        other.operation = Operation::Search {
            path: String::new(),
            query: "other".into(),
        };
        assert!(!output.valid_for(&other));
    }
}
