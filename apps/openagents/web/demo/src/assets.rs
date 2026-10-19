//! Fonts and icons, bundled into the module: a browser page has no system
//! fonts GPUI can reach, and no file system to load icons from.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

macro_rules! ui_asset {
    ($path:literal) => {
        include_bytes!(concat!("../../../crates/ui/assets/", $path))
    };
}

/// Every face the page draws: the desktop's Geist and Geist Mono, plus
/// Noto subsets for the CJK and emoji in the demo chats.
pub fn font_faces() -> Vec<Cow<'static, [u8]>> {
    let faces: [&'static [u8]; 12] = [
        ui_asset!("fonts/Geist.ttf"),
        ui_asset!("fonts/Geist-Italic.ttf"),
        ui_asset!("fonts/Geist-Medium.ttf"),
        ui_asset!("fonts/Geist-SemiBold.ttf"),
        ui_asset!("fonts/Geist-SemiBoldItalic.ttf"),
        ui_asset!("fonts/Geist-Bold.ttf"),
        ui_asset!("fonts/Geist-BoldItalic.ttf"),
        ui_asset!("fonts/GeistMono.ttf"),
        ui_asset!("fonts/GeistMono-Medium.ttf"),
        ui_asset!("fonts/GeistMono-SemiBold.ttf"),
        include_bytes!("../assets/fonts/NotoSansCJKjp-Regular-demo.otf"),
        include_bytes!("../assets/fonts/NotoColorEmoji-demo.ttf"),
    ];
    faces.into_iter().map(Cow::Borrowed).collect()
}

const ICONS: &[(&str, &[u8])] = &[
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
    ("icons/tuning.svg", ui_asset!("icons/tuning.svg")),
    ("icons/widget.svg", ui_asset!("icons/widget.svg")),
    ("icons/folder.svg", ui_asset!("icons/folder.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
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
