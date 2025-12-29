//! Bloomberg-style orderbook color theme

use wgpui::Hsla;

/// Orderbook-specific colors
pub mod orderbook {
    use super::*;

    /// Bid (buy) green - Bloomberg style (#00C853)
    /// H = 145°/360° ≈ 0.403, S = 100%, L = 39%
    pub const BID: Hsla = Hsla::new(0.403, 1.0, 0.39, 1.0);

    /// Bid dark variant for depth bars
    pub const BID_DARK: Hsla = Hsla::new(0.403, 0.8, 0.25, 1.0);

    /// Ask (sell) red - Bloomberg style (#D32F2F)
    /// H = 0°/360° = 0.0, S = 76%, L = 50%
    pub const ASK: Hsla = Hsla::new(0.0, 0.76, 0.50, 1.0);

    /// Ask dark variant for depth bars
    pub const ASK_DARK: Hsla = Hsla::new(0.0, 0.6, 0.30, 1.0);

    /// Depth bar background for bids (semi-transparent green)
    pub const BID_DEPTH: Hsla = Hsla::new(0.403, 0.5, 0.15, 0.3);

    /// Depth bar background for asks (semi-transparent red)
    pub const ASK_DEPTH: Hsla = Hsla::new(0.0, 0.5, 0.15, 0.3);

    /// Highlight for best bid/ask - Bloomberg yellow
    pub const HIGHLIGHT: Hsla = Hsla::new(0.117, 1.0, 0.5, 1.0);
}
