//! Coder's source-equivalent presentation roles, independent of a renderer.

use rust_native::style::{Color, Style};

const fn rgb(hex: u32) -> Color {
    Color::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const BG_BASE: Color = rgb(0x0a0a0a);
pub const BG_LIGHT: Color = rgb(0x242424);
pub const BG_DARK: Color = rgb(0x1c1c1c);
pub const TEXT_PRIMARY: Color = rgb(0xe1e1e1);
pub const TEXT_SECONDARY: Color = rgb(0xc8c8c8);
pub const GRAY_DIM: Color = rgb(0x585858);
pub const GRAY: Color = rgb(0x6c6c6c);
pub const GRAY_BRIGHT: Color = rgb(0x787878);
pub const PROMPT_BORDER_ACTIVE: Color = rgb(0x505058);
pub const ACCENT_MODEL: Color = rgb(0x00ffff);
pub const ACCENT_DELEGATE: Color = rgb(0xff00ff);
pub const COMMAND: Color = rgb(0xffbf00);
pub const ACCENT_SKILL: Color = rgb(0x00ffff);
pub const ACCENT_SUCCESS: Color = rgb(0x00a645);
pub const PATH: Color = rgb(0xff6600);
pub const MD_CODE: Color = rgb(0x00ffff);
pub const DIFF_DELETE_FG: Color = rgb(0xff0000);
pub const DIFF_INSERT_FG: Color = rgb(0x00a645);
pub const DIFF_DELETE_BG: Color = rgb(0x3f0000);
pub const DIFF_INSERT_BG: Color = rgb(0x002911);

/// The reference profile uses 9 by 20 CSS pixel cells and a 15-point font.
pub fn style() -> Style {
    Style {
        foreground: Some(TEXT_SECONDARY),
        background: Some(BG_BASE),
        monospace: Some(true),
        text_size: Some(15),
        line_height: Some(20),
        gap_points: Some(0),
        padding_points: Some([0; 4]),
        ..Style::default()
    }
}

pub const TOKENS: &[(&str, Color)] = &[
    ("BG_BASE", BG_BASE),
    ("BG_LIGHT", BG_LIGHT),
    ("BG_DARK", BG_DARK),
    ("TEXT_PRIMARY", TEXT_PRIMARY),
    ("TEXT_SECONDARY", TEXT_SECONDARY),
    ("GRAY_DIM", GRAY_DIM),
    ("GRAY", GRAY),
    ("GRAY_BRIGHT", GRAY_BRIGHT),
    ("PROMPT_BORDER_ACTIVE", PROMPT_BORDER_ACTIVE),
    ("ACCENT_MODEL", ACCENT_MODEL),
    ("ACCENT_DELEGATE", ACCENT_DELEGATE),
    ("COMMAND", COMMAND),
    ("ACCENT_SKILL", ACCENT_SKILL),
    ("ACCENT_SUCCESS", ACCENT_SUCCESS),
    ("PATH", PATH),
    ("MD_CODE", MD_CODE),
    ("DIFF_DELETE_FG", DIFF_DELETE_FG),
    ("DIFF_INSERT_FG", DIFF_INSERT_FG),
    ("DIFF_DELETE_BG", DIFF_DELETE_BG),
    ("DIFF_INSERT_BG", DIFF_INSERT_BG),
];

/// Coder's source appearance profile for semantic DOM controls.
/// Scope this on the application-owned preview container, outside generic UI.
pub const WEB_PROFILE_CSS: &str = r#"
.coder-profile .rn-view{--rn-mono-font:"Paper Mono",monospace;--rn-default-font:"Paper Mono",monospace;background:#0a0a0a;font-size:15px;line-height:20px;font-variant-ligatures:none}
.coder-profile .rn-view .rn-node{font-variant-ligatures:none;flex-shrink:0}
.coder-profile .rn-view .rn-choice,.coder-profile .rn-view .rn-button{border:0;border-radius:0;padding:0;min-height:20px;line-height:20px;box-shadow:none}
.coder-profile .rn-view .rn-choice[aria-pressed=true]{outline:none}
.coder-profile .rn-view .rn-choice:focus-visible,.coder-profile .rn-view .rn-button:focus-visible{outline:1px solid #00ffff;outline-offset:-1px}
.coder-profile .rn-view .rn-field{gap:0}
.coder-profile .rn-view .rn-field[data-rn-node$="composer-draft"],.coder-profile .rn-view .rn-field[data-rn-node="rails-draft"]{flex:1;overflow:hidden}
.coder-profile .rn-view .rn-field[data-rn-node$="composer-draft"]>span,.coder-profile .rn-view .rn-field[data-rn-node="rails-draft"]>span{position:absolute;width:1px;height:1px;padding:0;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
.coder-profile .rn-view .rn-field[data-rn-node$="composer-draft"]>.rn-input,.coder-profile .rn-view .rn-field[data-rn-node="rails-draft"]>.rn-input{border:0;border-radius:0;padding:0;min-height:20px;line-height:20px;resize:none;outline:none;white-space:pre-wrap;overflow:auto;scrollbar-width:none;caret-color:#c8c8c8}
.coder-profile .rn-view .rn-field[data-rn-node$="composer-draft"]>.rn-input:focus-visible,.coder-profile .rn-view .rn-field[data-rn-node="rails-draft"]>.rn-input:focus-visible{outline:none}
.coder-profile .rn-view [data-rn-node="main-screen"]{overflow:hidden}
.coder-profile .rn-view [data-rn-node="main-slash"]{position:absolute;left:0;right:0;z-index:1;transform:translateY(-100%)}
.coder-profile .rn-view .rn-terminal{scrollbar-width:none}
.coder-profile .rn-view .rn-dialog{position:fixed;left:var(--coder-dialog-left,50vw);top:var(--coder-dialog-top,50vh);right:auto;bottom:auto;margin:0;transform:translate(-50%,-50%);width:var(--coder-dialog-width,396px);max-width:var(--coder-preview-width,calc(100vw - 36px));max-height:var(--coder-preview-height,calc(100vh - 40px));padding:0;background:#0a0a0a}
.coder-profile .rn-view .rn-dialog-close{position:absolute;width:1px;height:1px;padding:0;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
.coder-profile .rn-view .rn-field[data-rn-node="field-model-search"]{flex-direction:row;align-items:center;gap:0;height:20px;min-height:20px}
.coder-profile .rn-view .rn-field[data-rn-node="field-model-search"]>span{flex:0 0 auto;color:#787878;white-space:pre}
.coder-profile .rn-view .rn-field[data-rn-node="field-model-search"]>.rn-input{border:0;border-radius:0;padding:0;min-height:20px;height:20px;line-height:20px;outline:none;color:#e1e1e1}
"#;

/// Preserve the public source's imported syntax-color remapping.
pub fn remap(color: Color) -> Color {
    match (color.red, color.green, color.blue) {
        (0x42, 0x0e, 0x14) => DIFF_DELETE_BG,
        (0x06, 0x38, 0x06) => DIFF_INSERT_BG,
        (0x91, 0x4c, 0x54)
        | (0xdb, 0x4b, 0x4b)
        | (0xde, 0x59, 0x71)
        | (0xf7, 0x76, 0x8e)
        | (0xfc, 0x7b, 0x7b)
        | (0xff, 0x53, 0x70) => DIFF_DELETE_FG,
        (0x9e, 0xce, 0x6a) => ACCENT_SUCCESS,
        (0xff, 0x9e, 0x64) => PATH,
        (0x9a, 0xbd, 0xf5) | (0xc0, 0xce, 0xfc) | (0xe0, 0xaf, 0x68) | (0xff, 0xdb, 0x69) => {
            COMMAND
        }
        (0x9d, 0x7c, 0xd8) | (0xb2, 0x67, 0xe6) | (0xba, 0x3c, 0x97) | (0xbb, 0x9a, 0xf7) => {
            ACCENT_DELEGATE
        }
        (0x0d, 0xb9, 0xd7)
        | (0x1a, 0xbc, 0x9c)
        | (0x3a, 0x95, 0xab)
        | (0x41, 0xa6, 0xb5)
        | (0x44, 0x9d, 0xab)
        | (0x61, 0x83, 0xbb)
        | (0x61, 0xbd, 0xf2)
        | (0x6d, 0x91, 0xde)
        | (0x73, 0xda, 0xca)
        | (0x7a, 0xa2, 0xf7)
        | (0x7a, 0xa6, 0xda)
        | (0x7d, 0xcf, 0xff)
        | (0x89, 0xdd, 0xff)
        | (0xb4, 0xf9, 0xf8) => ACCENT_SKILL,
        (0x4e, 0x55, 0x79) | (0x51, 0x59, 0x7d) | (0x5a, 0x63, 0x8c) => GRAY_DIM,
        (0x64, 0x6e, 0x9c) | (0x74, 0x7c, 0xa1) | (0x9a, 0xa5, 0xce) => GRAY_BRIGHT,
        _ => color,
    }
}
