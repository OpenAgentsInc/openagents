//! The desktop's fixed icons, rasterized at build time and tinted by GPUI.

use gpui::{AnyElement, IntoElement, Refineable, SharedString, StyleRefinement, Styled};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub fn svg() -> Icon {
    Icon {
        path: SharedString::default(),
        style: StyleRefinement::default(),
    }
}

pub struct Icon {
    path: SharedString,
    style: StyleRefinement,
}

impl Icon {
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self
    }
}
impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl IntoElement for Icon {
    type Element = AnyElement;
    fn into_element(self) -> AnyElement {
        let mut image = openagents_gpui_web::mask_image(mask(&self.path).unwrap_or(&[]), 64);
        image.style().refine(&self.style);
        image.into_any_element()
    }
}

/// Draws the desktop views' icons (`openagents_chat_view::icons::icon`) from
/// the same masks.
pub fn paint(path: SharedString, style: StyleRefinement) -> AnyElement {
    let mut image = openagents_gpui_web::mask_image(mask(&path).unwrap_or(&[]), 64);
    image.style().refine(&style);
    image.into_any_element()
}
