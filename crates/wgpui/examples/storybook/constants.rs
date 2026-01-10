use wgpui::components::hud::{DotShape, DotsOrigin, DrawDirection, FrameAnimation, FrameStyle, LineDirection};
use wgpui::Hsla;

pub(crate) const MARGIN: f32 = 24.0;
pub(crate) const HEADER_HEIGHT: f32 = 48.0;
pub(crate) const NAV_WIDTH: f32 = 220.0;
pub(crate) const NAV_ITEM_HEIGHT: f32 = 36.0;
pub(crate) const GAP: f32 = 20.0;
pub(crate) const PANEL_PADDING: f32 = 12.0;
pub(crate) const SECTION_GAP: f32 = 24.0;
pub(crate) const FRAME_TILE_W: f32 = 170.0;
pub(crate) const FRAME_TILE_H: f32 = 110.0;
pub(crate) const FRAME_TILE_GAP: f32 = 12.0;
pub(crate) const FRAME_VARIANT_W: f32 = 160.0;
pub(crate) const FRAME_VARIANT_H: f32 = 100.0;
pub(crate) const BG_TILE_W: f32 = 300.0;
pub(crate) const BG_TILE_H: f32 = 200.0;
pub(crate) const BG_TILE_GAP: f32 = 16.0;
pub(crate) const TEXT_TILE_W: f32 = 240.0;
pub(crate) const TEXT_TILE_H: f32 = 80.0;
pub(crate) const TEXT_TILE_GAP: f32 = 12.0;
pub(crate) const ILLUMINATOR_TILE_W: f32 = 200.0;
pub(crate) const ILLUMINATOR_TILE_H: f32 = 140.0;
pub(crate) const ILLUMINATOR_TILE_GAP: f32 = 12.0;
pub(crate) const LIGHT_DEMO_FRAMES_INNER_H: f32 = 320.0;
pub(crate) const LIGHT_DEMO_HERO_INNER_H: f32 = 280.0;
pub(crate) const TOOLCALL_DEMO_INNER_H: f32 = 520.0;
pub(crate) const SECTION_OVERVIEW: usize = 0;
pub(crate) const SECTION_ATOMS: usize = 1;
pub(crate) const SECTION_MOLECULES: usize = 2;
pub(crate) const SECTION_ORGANISMS: usize = 3;
pub(crate) const SECTION_INTERACTIONS: usize = 4;
pub(crate) const SECTION_ARWES_FRAMES: usize = 5;
pub(crate) const SECTION_ARWES_BACKGROUNDS: usize = 6;
pub(crate) const SECTION_ARWES_TEXT: usize = 7;
pub(crate) const SECTION_ARWES_ILLUMINATOR: usize = 8;
pub(crate) const SECTION_HUD_WIDGETS: usize = 9;
pub(crate) const SECTION_LIGHT_DEMO: usize = 10;
pub(crate) const SECTION_TOOLCALL_DEMO: usize = 11;
pub(crate) const SECTION_SYSTEM_UI: usize = 12;
pub(crate) const SECTION_CHAT_THREADS: usize = 13;
pub(crate) const SECTION_BITCOIN_WALLET: usize = 14;
pub(crate) const SECTION_NOSTR_PROTOCOL: usize = 15;
pub(crate) const SECTION_GITAFTER: usize = 16;
pub(crate) const SECTION_SOVEREIGN_AGENTS: usize = 17;
pub(crate) const SECTION_MARKETPLACE: usize = 18;
pub(crate) const SECTION_AUTOPILOT: usize = 19;
pub(crate) const SECTION_THREAD_COMPONENTS: usize = 20;
pub(crate) const SECTION_SESSIONS: usize = 21;
pub(crate) const SECTION_PERMISSIONS: usize = 22;
pub(crate) const SECTION_APM_METRICS: usize = 23;
pub(crate) const SECTION_WALLET_FLOWS: usize = 24;
pub(crate) const SECTION_GITAFTER_FLOWS: usize = 25;
pub(crate) const SECTION_MARKETPLACE_FLOWS: usize = 26;
pub(crate) const SECTION_NOSTR_FLOWS: usize = 27;
pub(crate) const SECTION_SOVEREIGN_AGENT_FLOWS: usize = 28;
pub(crate) const HOT_RELOAD_POLL_MS: u64 = 500;

#[derive(Clone, Copy)]
pub(crate) struct GlowPreset {
    pub(crate) short: &'static str,
    pub(crate) color: Hsla,
}

pub(crate) const GLOW_PRESETS: [GlowPreset; 8] = [
    GlowPreset {
        short: "Wht",
        color: Hsla::new(0.0, 0.0, 1.0, 0.6),
    },
    GlowPreset {
        short: "Cyn",
        color: Hsla::new(180.0, 1.0, 0.7, 0.5),
    },
    GlowPreset {
        short: "Pur",
        color: Hsla::new(280.0, 1.0, 0.7, 0.5),
    },
    GlowPreset {
        short: "Grn",
        color: Hsla::new(120.0, 1.0, 0.6, 0.5),
    },
    GlowPreset {
        short: "C2",
        color: Hsla::new(0.5, 1.0, 0.6, 0.8),
    },
    GlowPreset {
        short: "Org",
        color: Hsla::new(0.125, 1.0, 0.5, 0.9),
    },
    GlowPreset {
        short: "Red",
        color: Hsla::new(0.0, 1.0, 0.5, 1.0),
    },
    GlowPreset {
        short: "G2",
        color: Hsla::new(0.389, 1.0, 0.5, 0.8),
    },
];

pub(crate) const FRAME_STYLES: [FrameStyle; 9] = [
    FrameStyle::Corners,
    FrameStyle::Lines,
    FrameStyle::Octagon,
    FrameStyle::Underline,
    FrameStyle::Nefrex,
    FrameStyle::Kranox,
    FrameStyle::Nero,
    FrameStyle::Header,
    FrameStyle::Circle,
];
pub(crate) const FRAME_ANIMATIONS: [FrameAnimation; 4] = [
    FrameAnimation::Fade,
    FrameAnimation::Draw,
    FrameAnimation::Flicker,
    FrameAnimation::Assemble,
];
pub(crate) const FRAME_DIRECTIONS: [DrawDirection; 6] = [
    DrawDirection::LeftToRight,
    DrawDirection::RightToLeft,
    DrawDirection::TopToBottom,
    DrawDirection::BottomToTop,
    DrawDirection::CenterOut,
    DrawDirection::EdgesIn,
];
pub(crate) const DOT_SHAPES: [DotShape; 3] = [DotShape::Box, DotShape::Circle, DotShape::Cross];
pub(crate) const DOT_ORIGINS: [DotsOrigin; 6] = [
    DotsOrigin::Left,
    DotsOrigin::Right,
    DotsOrigin::Top,
    DotsOrigin::Bottom,
    DotsOrigin::Center,
    DotsOrigin::Point(0.25, 0.75),
];
pub(crate) const LINE_DIRECTIONS: [LineDirection; 4] = [
    LineDirection::Right,
    LineDirection::Left,
    LineDirection::Down,
    LineDirection::Up,
];
