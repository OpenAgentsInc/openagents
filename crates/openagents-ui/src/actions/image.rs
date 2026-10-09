//! Image, ported from Apps SDK UI `src/components/Image` (MIT).
//! Styles: `static/components/image.css`.

use maud::{Markup, Render, html};

use super::html::{Attrs, Tag, safe_url};

/// An `<img class="oa-image">`, not draggable by default. React fades the
/// image in on load by toggling `data-loaded`; server-rendered images carry
/// `data-loaded` from the start so they show without a script. An empty
/// `src` renders nothing, as in React.
#[derive(Clone, Debug)]
pub struct Image {
    src: String,
    alt: String,
    width: Option<u32>,
    height: Option<u32>,
    lazy: bool,
    draggable: bool,
    attrs: Attrs,
}

impl Image {
    /// `alt` is required; pass "" for a decorative image.
    pub fn new(src: impl Into<String>, alt: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            alt: alt.into(),
            width: None,
            height: None,
            lazy: false,
            draggable: false,
            attrs: Attrs::default(),
        }
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// `loading="lazy" decoding="async"`.
    pub fn lazy(mut self) -> Self {
        self.lazy = true;
        self
    }

    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }
}

impl_attrs!(Image);

impl Render for Image {
    fn render(&self) -> Markup {
        if self.src.trim().is_empty() {
            return html! {};
        }
        let width = self.width.map(|w| w.to_string());
        let height = self.height.map(|h| h.to_string());
        let mut tag = Tag::new("img", "oa-image", &self.attrs)
            .attr("src", &safe_url(&self.src))
            .attr("alt", &self.alt)
            .attr_opt("width", width.as_deref())
            .attr_opt("height", height.as_deref());
        if self.lazy {
            tag = tag.attr("loading", "lazy").attr("decoding", "async");
        }
        tag.flag("data-loaded", true)
            .attr("draggable", if self.draggable { "true" } else { "false" })
            .extra(&self.attrs)
            .void()
    }
}
