//! Coder Light: the light GUI theme, Apps SDK UI's light values from the
//! shared token table (`oa-tokens`), the same values the web's
//! `data-theme="light"` paints.
//!
//! GUI surfaces only (desktop and mobile). Terminal UIs and terminal panels
//! stay [`crate::coder_noir`] in every theme, so there are no terminal
//! values here.

/// Coder Light's GUI role palette (the light side of the token table).
pub const PALETTE: oa_tokens::Palette = oa_tokens::Palette::LIGHT;
