//! The graph's colors: the site's dark palette (`tokens-semantic.css` and
//! `tokens-primitive.css` in `openagents-ui`), and the desktop's fonts.

use agent_graph::Accent;
use gpui::{Hsla, hsla, rgb};

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// The canvas behind the graph.
pub fn bg() -> Hsla {
    c(0x070707)
}
/// The dot grid on the canvas.
pub fn grid() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.07)
}
/// The side panel.
pub fn panel() -> Hsla {
    c(0x0d0d0d)
}
/// A unit's body.
pub fn unit() -> Hsla {
    c(0x121212)
}
/// A graph unit's body, behind its children.
pub fn group() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.025)
}
pub fn border() -> Hsla {
    c(0x2c2c2c)
}
pub fn border_strong() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.2)
}
pub fn text() -> Hsla {
    c(0xe5e5e5)
}
pub fn text_muted() -> Hsla {
    c(0x818181)
}
pub fn text_faint() -> Hsla {
    c(0x5c5c5c)
}
pub fn chip() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.06)
}

/// `--color-text-{info,discovery,warning,success,danger}` in dark mode, and
/// the agent itself in the site's near-white.
pub fn accent(accent: Accent) -> Hsla {
    match accent {
        Accent::Core => c(0xededed),
        Accent::Info => c(0x66b5ff),
        Accent::Discovery => c(0xad7bf9),
        Accent::Warning => c(0xe25507),
        Accent::Success => c(0x04b84c),
        Accent::Danger => c(0xff6764),
    }
}
