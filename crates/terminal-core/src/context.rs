//! A removable, scrubbed preview of only the blocks explicitly attached to a request.

use crate::blocks::Block;
use crate::proposals::digest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub directory: Option<String>,
    pub git: Option<String>,
    pub blocks: Vec<Attached>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attached {
    pub id: u64,
    pub command: String,
    pub output: String,
    pub status: Option<i32>,
    pub truncated: bool,
}

impl Context {
    pub fn attach(&mut self, block: &Block, scrub: &dyn Fn(&str) -> String) {
        if self.blocks.iter().any(|attached| attached.id == block.id) || self.blocks.len() >= 8 {
            return;
        }
        self.blocks.push(Attached {
            id: block.id,
            command: scrub(&block.command),
            output: scrub(&block.output),
            status: block.status,
            truncated: block.truncated,
        });
    }

    pub fn remove_block(&mut self, id: u64) {
        self.blocks.retain(|block| block.id != id);
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn identity(&self) -> String {
        digest(self)
    }
    pub fn preview(&self) -> String {
        let mut text = String::new();
        if let Some(directory) = &self.directory {
            text.push_str(&format!("Directory: {directory}\n"));
        }
        if let Some(git) = &self.git {
            text.push_str(&format!("Git:\n{git}\n"));
        }
        for block in &self.blocks {
            text.push_str(&format!(
                "Block {} · exit {:?}\n$ {}\n{}{}\n",
                block.id,
                block.status,
                block.command,
                block.output,
                if block.truncated {
                    "\n[output truncated]"
                } else {
                    ""
                }
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_context_changes_approval_identity_and_sent_preview() {
        let mut context = Context {
            directory: Some("/tmp/project".into()),
            git: None,
            blocks: vec![Attached {
                id: 1,
                command: "test".into(),
                output: "failed".into(),
                status: Some(1),
                truncated: false,
            }],
        };
        let identity = context.identity();
        assert!(context.preview().contains("failed"));
        context.remove_block(1);
        assert_ne!(context.identity(), identity);
        assert!(!context.preview().contains("failed"));
        context.clear();
        assert!(context.preview().is_empty());
    }
}
