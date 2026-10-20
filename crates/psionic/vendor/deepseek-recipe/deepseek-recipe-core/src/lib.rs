//! Shared conversation, image, and tool types for protocol adapters and prompt
//! rendering.

// `prompt!` expands to paths under `deepseek_recipe_core`, which must
// also resolve inside this crate.
extern crate self as deepseek_recipe_core;

pub use deepseek_recipe_macros::prompt;

pub mod conversation;
pub mod json;
pub mod messages;
pub mod multimodal;
pub mod prompt;
pub mod special_tokens;
pub mod tools;
