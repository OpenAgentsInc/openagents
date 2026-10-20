//! Dropdown placement within a dialog, measured after layout.

use gpui::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, Position, SharedString, Size, Style, Window, div, point, prelude::*, px,
};
use web_time::Instant;

/// Which trigger edge the menu lines up with: `End` grows leftward from the
/// trigger's right edge, `Start` rightward from its left edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAlign {
    Start,
    End,
}

/// Mount from a relative trigger. The menu uses the available side of the
/// trigger instead of sliding over it when it approaches the dialog edge.
pub fn contained_menu(
    id: SharedString,
    content: gpui::Div,
    closing: Option<Instant>,
    trigger_height: f32,
    limits: Bounds<Pixels>,
    align: MenuAlign,
) -> AnyElement {
    let exit = closing.map(super::exit_progress);
    // Half the usable height guarantees room on at least one side, even
    // when the trigger sits in the middle of a small dialog.
    let max_height = ((f32::from(limits.size.height) - trigger_height) / 2.0 - 6.0)
        .max(1.0)
        .min(320.0);
    let card = content
        .id(SharedString::from(format!("{id}-scroll")))
        .debug_selector(|| "contained-menu-scroll".into())
        .max_h(px(max_height))
        .max_w(limits.size.width)
        .overflow_hidden()
        .into_any_element();
    let card = super::frosted_menu(exit, card);
    let content = super::menu_motion(
        id,
        exit,
        div()
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(card),
    );
    div()
        .absolute()
        .bottom_0()
        .map(|el| match align {
            MenuAlign::Start => el.left_0(),
            MenuAlign::End => el.right_0(),
        })
        .size_0()
        .child(
            gpui::deferred(ContainedMenu {
                content,
                limits,
                trigger_height,
                align,
            })
            .priority(1),
        )
        .into_any_element()
}

fn menu_origin(
    anchor: Point<Pixels>,
    size: Size<Pixels>,
    trigger_height: Pixels,
    limits: Bounds<Pixels>,
    align: MenuAlign,
) -> Point<Pixels> {
    let gap = px(6.0);
    let below = anchor.y + gap;
    let above = anchor.y - trigger_height - gap - size.height;
    let y = if below + size.height <= limits.bottom() {
        below
    } else {
        above
    };
    let x = match align {
        MenuAlign::Start => anchor.x,
        MenuAlign::End => anchor.x - size.width,
    };
    point(
        x.clamp(
            limits.left(),
            (limits.right() - size.width).max(limits.left()),
        ),
        y.clamp(
            limits.top(),
            (limits.bottom() - size.height).max(limits.top()),
        ),
    )
}

struct ContainedMenu {
    content: AnyElement,
    limits: Bounds<Pixels>,
    trigger_height: f32,
    align: MenuAlign,
}

impl IntoElement for ContainedMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ContainedMenu {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        let layout = window.request_layout(
            Style {
                position: Position::Absolute,
                ..Style::default()
            },
            [child],
            cx,
        );
        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*child).size;
        let origin = menu_origin(
            bounds.origin,
            size,
            px(self.trigger_height),
            self.limits,
            self.align,
        );
        window.with_element_offset(origin - bounds.origin, |window| {
            self.content.prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn dropdown_stays_in_dialog_without_covering_trigger() {
        let limits = Bounds::new(point(px(100.0), px(80.0)), size(px(800.0), px(600.0)));
        let menu = size(px(260.0), px(280.0));
        for y in [120.0, 350.0, 650.0] {
            let anchor = point(px(870.0), px(y));
            let origin = menu_origin(anchor, menu, px(34.0), limits, MenuAlign::End);
            let bounds = Bounds::new(origin, menu);
            assert!(bounds.left() >= limits.left() && bounds.right() <= limits.right());
            assert!(bounds.top() >= limits.top() && bounds.bottom() <= limits.bottom());
            assert!(bounds.top() >= anchor.y + px(6.0) || bounds.bottom() <= anchor.y - px(40.0));
        }
    }

    #[test]
    fn start_aligned_dropdown_grows_rightward_within_dialog() {
        let limits = Bounds::new(point(px(100.0), px(80.0)), size(px(800.0), px(600.0)));
        let menu = size(px(260.0), px(200.0));
        let origin = menu_origin(
            point(px(300.0), px(200.0)),
            menu,
            px(34.0),
            limits,
            MenuAlign::Start,
        );
        assert_eq!(origin.x, px(300.0));
        // Near the right edge it slides back inside rather than overflowing.
        let origin = menu_origin(
            point(px(800.0), px(200.0)),
            menu,
            px(34.0),
            limits,
            MenuAlign::Start,
        );
        assert_eq!(origin.x, limits.right() - menu.width);
    }
}
