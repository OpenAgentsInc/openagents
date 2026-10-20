//! Markdown drawn by the desktop's own renderer
//! (`openagents_chat_view::markdown`, #12216), over the block tree every
//! OpenAgents frontend shares.
//!
//! While a reply streams its source goes through
//! [`openagents_markdown::parse_streaming`], which holds back an unfinished
//! tail (a table header waiting for its delimiter row, a fence's opening
//! line, a bare `**`), so raw syntax never flashes (#12246). New text fades
//! in through the desktop's veil: alpha multiplied into the text color at
//! paint time, so shaping and wrapping never change while a reply streams.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, SharedString, Window,
};
use openagents_chat_view::markdown::render::{self, RenderOptions};
use openagents_chat_view::markdown::veil::RowVeil;
use openagents_markdown::{parse_full, parse_streaming};

use crate::theme;

/// The fade state of one streaming text part. Keep one per live answer and
/// draw every frame with it; the fade advances as the text grows.
pub type Veil = Rc<RefCell<RowVeil>>;

/// A fresh veil for a reply that starts streaming now.
pub fn new_veil() -> Veil {
    Rc::default()
}

/// A veil for a reply already partly on screen: the text it first draws
/// stays as it is, and only what arrives later fades in.
pub fn seeded_veil() -> Veil {
    Rc::new(RefCell::new(RowVeil::seeded()))
}

/// Whether any text drawn with `veil` is still fading in.
pub fn is_fading(veil: &Veil) -> bool {
    veil.borrow().is_fading()
}

/// Paint this first in a transcript, before any markdown: it clears the
/// frame's text-selection registry, so each frame holds exactly the text it
/// paints, in paint order.
pub fn selection_reset() -> impl IntoElement {
    render::selection_frame_reset()
}

pub struct Markdown {
    /// Prefix for element ids, unique per text part.
    pub id: SharedString,
}

impl Markdown {
    /// Draws `source`; `veil` fades in what a streaming reply adds.
    pub fn render(&self, source: &str, streaming: bool, veil: Option<&Veil>) -> AnyElement {
        let tree = if streaming {
            parse_streaming(source)
        } else {
            parse_full(source)
        };
        let mut options = RenderOptions::settled(self.id.clone());
        options.veil = veil.cloned();
        WithWindow(Some(Box::new(move |window, _| {
            let element =
                render::render_tree(&tree, &options, &theme::desktop(), window, &|_| None);
            // Text already on screen when a seeded veil attached stays put;
            // only what arrives from the next frame on fades in.
            if let Some(veil) = &options.veil {
                veil.borrow_mut().finish_seeding();
            }
            element
        })))
        .into_any_element()
    }
}

/// Builds its child when layout starts, where the renderer can measure text
/// with the window (table columns size to their content).
struct WithWindow(Option<Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>>);

impl IntoElement for WithWindow {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for WithWindow {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let build = self.0.take().expect("laid out once");
        let mut child = build(window, cx);
        (child.request_layout(window, cx), child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Render, TestAppContext, div, prelude::*, px, size};

    struct Harness {
        source: &'static str,
        streaming: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(390.))
                .child(render::selection_frame_reset())
                .child(Markdown { id: "md".into() }.render(self.source, self.streaming, None))
        }
    }

    #[gpui::test]
    fn draws_a_table_and_a_streaming_tail(cx: &mut TestAppContext) {
        cx.update(theme::install);
        for (source, streaming) in [
            ("| a | b |\n|---|---|\n| 1 | 2 |\n", false),
            ("Intro\n\n| a | b |\n", true),
            ("**bold", true),
        ] {
            let (_, cx) = cx.add_window_view(|_, _| Harness { source, streaming });
            cx.simulate_resize(size(px(390.), px(844.)));
            cx.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear();
            });
        }
    }
}
