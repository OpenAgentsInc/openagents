//! Fonts and icons: a browser page has no system fonts GPUI can reach, and
//! no file system to load icons from, so the icons are compiled in (as
//! masks in the browser).

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

// In the browser every icon draws from its build-time mask (`crate::svg`),
// so the SVG sources stay native, where GPUI's own `svg` reads them.
#[cfg(not(target_family = "wasm"))]
macro_rules! ui_asset {
    ($path:literal) => {
        include_bytes!(concat!("../../../crates/ui/assets/", $path))
    };
}
#[cfg(target_family = "wasm")]
macro_rules! ui_asset {
    ($path:literal) => {
        &[]
    };
}

/// The desktop's Geist and Geist Mono faces the chat draws, by file name
/// under `apps/openagents/crates/ui/assets/fonts/`. The chat and the demo
/// load these as the site's shared font files (`/gpui-fonts/`), in this
/// order.
pub const FONT_FILES: [&str; 10] = [
    "Geist.ttf",
    "Geist-Italic.ttf",
    "Geist-Medium.ttf",
    "Geist-SemiBold.ttf",
    "Geist-SemiBoldItalic.ttf",
    "Geist-Bold.ttf",
    "Geist-BoldItalic.ttf",
    "GeistMono.ttf",
    "GeistMono-Medium.ttf",
    "GeistMono-SemiBold.ttf",
];

const ICONS: &[(&str, &[u8])] = &[
    // The desktop transcript's icons (openagents_chat_view::transcript).
    (
        "icons/alt-arrow-up.svg",
        ui_asset!("icons/alt-arrow-up.svg"),
    ),
    (
        "icons/arrow-up-right.svg",
        ui_asset!("icons/arrow-up-right.svg"),
    ),
    ("icons/bot.svg", ui_asset!("icons/bot.svg")),
    ("icons/checklist.svg", ui_asset!("icons/checklist.svg")),
    ("icons/command.svg", ui_asset!("icons/command.svg")),
    ("icons/copy.svg", ui_asset!("icons/copy.svg")),
    (
        "icons/document-add.svg",
        ui_asset!("icons/document-add.svg"),
    ),
    ("icons/eye.svg", ui_asset!("icons/eye.svg")),
    ("icons/file-code.svg", ui_asset!("icons/file-code.svg")),
    (
        "icons/folder-with-files.svg",
        ui_asset!("icons/folder-with-files.svg"),
    ),
    ("icons/gallery.svg", ui_asset!("icons/gallery.svg")),
    ("icons/global.svg", ui_asset!("icons/global.svg")),
    (
        "icons/magic-stick-3.svg",
        ui_asset!("icons/magic-stick-3.svg"),
    ),
    ("icons/terminal.svg", ui_asset!("icons/terminal.svg")),
    ("icons/wrap-text.svg", ui_asset!("icons/wrap-text.svg")),
    ("icons/file-image.svg", ui_asset!("icons/file-image.svg")),
    (
        "icons/alt-arrow-right.svg",
        ui_asset!("icons/alt-arrow-right.svg"),
    ),
    (
        "icons/alt-arrow-down.svg",
        ui_asset!("icons/alt-arrow-down.svg"),
    ),
    ("icons/arrow-up.svg", ui_asset!("icons/arrow-up.svg")),
    ("icons/check.svg", ui_asset!("icons/check.svg")),
    ("icons/close.svg", ui_asset!("icons/close.svg")),
    (
        "icons/danger-triangle.svg",
        ui_asset!("icons/danger-triangle.svg"),
    ),
    ("icons/document.svg", ui_asset!("icons/document.svg")),
    ("icons/git-branch.svg", ui_asset!("icons/git-branch.svg")),
    ("icons/laptop.svg", ui_asset!("icons/laptop.svg")),
    (
        "icons/openagents-logo.svg",
        ui_asset!("icons/openagents-logo.svg"),
    ),
    ("icons/claude-mark.svg", ui_asset!("icons/claude-mark.svg")),
    ("icons/openai-mark.svg", ui_asset!("icons/openai-mark.svg")),
    ("icons/paperclip.svg", ui_asset!("icons/paperclip.svg")),
    (
        "icons/pen-new-square.svg",
        ui_asset!("icons/pen-new-square.svg"),
    ),
    ("icons/pin.svg", ui_asset!("icons/pin.svg")),
    (
        "icons/pull-request.svg",
        ui_asset!("icons/pull-request.svg"),
    ),
    ("icons/stop.svg", ui_asset!("icons/stop.svg")),
    ("icons/microphone.svg", ui_asset!("icons/microphone.svg")),
    ("icons/tuning.svg", ui_asset!("icons/tuning.svg")),
    // The sidebar's Settings row (#12288).
    ("icons/settings.svg", ui_asset!("icons/settings.svg")),
    ("icons/widget.svg", ui_asset!("icons/widget.svg")),
    ("icons/folder.svg", ui_asset!("icons/folder.svg")),
    (
        "icons/chat-round-line.svg",
        ui_asset!("icons/chat-round-line.svg"),
    ),
    (
        "icons/archive-minimalistic.svg",
        ui_asset!("icons/archive-minimalistic.svg"),
    ),
    ("icons/magnifer.svg", ui_asset!("icons/magnifer.svg")),
    (
        "icons/more-horizontal.svg",
        ui_asset!("icons/more-horizontal.svg"),
    ),
    (
        "icons/trash-bin-minimalistic.svg",
        ui_asset!("icons/trash-bin-minimalistic.svg"),
    ),
    ("icons/refresh.svg", ui_asset!("icons/refresh.svg")),
    ("icons/pen.svg", ui_asset!("icons/pen.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if cfg!(target_family = "wasm") {
            return Ok(None);
        }
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod mask_tests {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));

    #[test]
    fn every_embedded_icon_has_a_visible_alpha_mask() {
        for (name, _) in super::ICONS {
            let alpha = mask(name).unwrap_or_else(|| panic!("missing mask: {name}"));
            assert_eq!(alpha.len(), 64 * 64, "{name}");
            assert!(alpha.iter().any(|alpha| *alpha > 0), "{name}");
        }
    }
}
