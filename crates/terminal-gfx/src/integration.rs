//! Temporary startup files inject hooks without changing the user's
//! dotfiles. The files and their shells live in `terminal_core::integration`,
//! which the resident host shares.

pub use terminal_core::integration::Hooks as Integration;
