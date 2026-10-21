//! The load screen's elements (#12438): a full-window cover over the
//! canvas with the place's name, a progress bar, and what the page is
//! doing, built through the DOM style API so it works under every Grid
//! page's policy. [`crate::load_screen`] holds its state.

use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlElement};

use crate::load_screen::{FADE_MS, Load};

/// The cover's element ID.
const ID: &str = "verse-load";

pub struct LoadView {
    cover: HtmlElement,
    title: HtmlElement,
    bar: HtmlElement,
    fill: HtmlElement,
    detail: HtmlElement,
}

fn element(document: &Document, tag: &str, css: &str) -> Option<HtmlElement> {
    let element: HtmlElement = document.create_element(tag).ok()?.dyn_into().ok()?;
    element.style().set_css_text(css);
    Some(element)
}

impl LoadView {
    /// The page's cover, created hidden the first time.
    pub fn new(document: &Document) -> Option<Self> {
        if let Some(old) = document.get_element_by_id(ID) {
            old.remove();
        }
        let cover = element(
            document,
            "div",
            "position:fixed;inset:0;z-index:3;display:none;opacity:0;\
             transition:opacity .2s ease;background:#0a0a0a;color:#f5f5f5;\
             font-family:inherit;align-items:center;justify-content:center;\
             pointer-events:auto;touch-action:none",
        )?;
        cover.set_id(ID);
        let _ = cover.set_attribute("role", "status");
        let _ = cover.set_attribute("aria-live", "polite");
        let column = element(
            document,
            "div",
            "display:flex;flex-direction:column;align-items:center;gap:14px;\
             width:min(320px,calc(100vw - 32px))",
        )?;
        let title = element(
            document,
            "p",
            "margin:0;font-size:18px;font-weight:600;line-height:24px;text-align:center",
        )?;
        let bar = element(
            document,
            "div",
            "width:100%;height:4px;border-radius:2px;overflow:hidden;\
             background:rgba(255,255,255,.14)",
        )?;
        let _ = bar.set_attribute("role", "progressbar");
        let _ = bar.set_attribute("aria-valuemin", "0");
        let _ = bar.set_attribute("aria-valuemax", "100");
        let fill = element(
            document,
            "div",
            "width:0;height:100%;border-radius:2px;background:#f5f5f5;\
             transition:width .3s ease",
        )?;
        let detail = element(
            document,
            "p",
            "margin:0;font-size:13px;line-height:18px;color:#9a9a9a;text-align:center",
        )?;
        bar.append_child(&fill).ok()?;
        column.append_child(&title).ok()?;
        column.append_child(&bar).ok()?;
        column.append_child(&detail).ok()?;
        cover.append_child(&column).ok()?;
        document.body()?.append_child(&cover).ok()?;
        Some(Self {
            cover,
            title,
            bar,
            fill,
            detail,
        })
    }

    /// Puts the cover up for `load`, at once, so the next paint hides the
    /// canvas.
    pub fn show(&self, load: &Load) {
        let style = self.cover.style();
        let _ = style.set_property("transition", "none");
        let _ = style.set_property("display", "flex");
        let _ = style.set_property("opacity", "1");
        let _ = self.fill.style().set_property("transition", "none");
        self.update(load);
        // Reading layout commits the jump; later changes animate.
        let _ = self.cover.offset_width();
        let _ = style.set_property("transition", &format!("opacity {FADE_MS}ms ease"));
        let _ = self
            .fill
            .style()
            .set_property("transition", "width .3s ease");
    }

    /// The bar and the lines for `load`'s step.
    pub fn update(&self, load: &Load) {
        self.title.set_text_content(Some(&load.title()));
        self.detail.set_text_content(Some(&load.detail()));
        let _ = self
            .fill
            .style()
            .set_property("width", &format!("{:.1}%", load.fraction() * 100.0));
        let _ = self
            .bar
            .set_attribute("aria-valuenow", &load.percent().to_string());
        let _ = self.bar.set_attribute("aria-label", &load.title());
    }

    /// Fades the cover out; [`Self::hide`] takes it away once faded.
    pub fn fade(&self) {
        let _ = self.cover.style().set_property("opacity", "0");
    }

    pub fn hide(&self) {
        let _ = self.cover.style().set_property("display", "none");
    }
}
