//! Text animation components.
//!
//! Provides animated text effects like character-by-character reveal
//! and scramble/decipher effects.

mod decipher;
mod sequence;

pub use decipher::TextDecipher;
pub use sequence::TextSequence;
