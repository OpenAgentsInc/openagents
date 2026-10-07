use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36LossMaskRole {
    System,
    User,
    Tool,
    Assistant,
    EmptyThinkBlock,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LossMaskSegment {
    pub role: Qwen36LossMaskRole,
    pub token_count: usize,
}

impl Qwen36LossMaskSegment {
    pub const fn new(role: Qwen36LossMaskRole, token_count: usize) -> Self {
        Self { role, token_count }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LossMaskOptions {
    pub assistant_only_loss: bool,
    pub ignore_empty_think_blocks: bool,
}

impl Default for Qwen36LossMaskOptions {
    fn default() -> Self {
        Self {
            assistant_only_loss: true,
            ignore_empty_think_blocks: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LossMask {
    pub train_on_token: Vec<bool>,
    pub ignored_token_count: usize,
    pub trained_token_count: usize,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum Qwen36LossMaskError {
    #[error("qwen36 loss mask has no tokens")]
    Empty,
}

pub fn build_qwen36_loss_mask(
    segments: &[Qwen36LossMaskSegment],
    options: &Qwen36LossMaskOptions,
) -> Result<Qwen36LossMask, Qwen36LossMaskError> {
    let total: usize = segments.iter().map(|segment| segment.token_count).sum();
    if total == 0 {
        return Err(Qwen36LossMaskError::Empty);
    }
    let mut train_on_token = Vec::with_capacity(total);
    for segment in segments {
        let train = match segment.role {
            Qwen36LossMaskRole::Assistant => true,
            Qwen36LossMaskRole::EmptyThinkBlock => !options.ignore_empty_think_blocks,
            Qwen36LossMaskRole::System | Qwen36LossMaskRole::User | Qwen36LossMaskRole::Tool => {
                !options.assistant_only_loss
            }
        };
        train_on_token.extend(std::iter::repeat_n(train, segment.token_count));
    }
    let trained_token_count = train_on_token.iter().filter(|value| **value).count();
    Ok(Qwen36LossMask {
        ignored_token_count: train_on_token.len() - trained_token_count,
        trained_token_count,
        train_on_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen36_loss_masks_train_only_assistant_tokens_by_default() {
        let mask = build_qwen36_loss_mask(
            &[
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::System, 2),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::User, 3),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::Tool, 2),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::Assistant, 4),
            ],
            &Qwen36LossMaskOptions::default(),
        )
        .expect("mask");

        assert_eq!(
            mask.train_on_token,
            vec![
                false, false, false, false, false, false, false, true, true, true, true,
            ]
        );
        assert_eq!(mask.trained_token_count, 4);
        assert_eq!(mask.ignored_token_count, 7);
    }

    #[test]
    fn qwen36_loss_masks_ignore_empty_think_block() {
        let mask = build_qwen36_loss_mask(
            &[
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::EmptyThinkBlock, 3),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::Assistant, 2),
            ],
            &Qwen36LossMaskOptions::default(),
        )
        .expect("mask");

        assert_eq!(mask.train_on_token, vec![false, false, false, true, true]);
        assert_eq!(mask.trained_token_count, 2);
    }

    #[test]
    fn qwen36_loss_masks_can_train_full_sequence_when_requested() {
        let mask = build_qwen36_loss_mask(
            &[
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::System, 1),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::User, 1),
                Qwen36LossMaskSegment::new(Qwen36LossMaskRole::Assistant, 1),
            ],
            &Qwen36LossMaskOptions {
                assistant_only_loss: false,
                ignore_empty_think_blocks: true,
            },
        )
        .expect("mask");

        assert_eq!(mask.train_on_token, vec![true, true, true]);
    }
}
