//! Transcript text selection on the web, as the desktop has it (#12305):
//! Cmd/Ctrl+C copies the selected transcript text, and dragging a selection
//! past the transcript's top or bottom edge scrolls it.

use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, Context, ListState, MouseMoveEvent, Pixels, Point, Task, px,
};

/// How often the transcript steps while a drag holds at an edge.
pub const SCROLL_TICK_MS: u64 = 24;
const SCROLL_EDGE_PX: f32 = 36.0;
const SCROLL_MAX_STEP_PX: f32 = 24.0;

/// Binds Cmd/Ctrl+C to [`crate::composer::Copy`] everywhere, so a
/// transcript selection copies even when the composer doesn't have focus;
/// the app's root handles the action with [`copy`].
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("cmd-c", crate::composer::Copy, None),
        gpui::KeyBinding::new("ctrl-c", crate::composer::Copy, None),
    ]);
}

/// Copies the transcript's selected text, if any; whether it did.
pub fn copy(cx: &mut App) -> bool {
    match openagents_chat_view::markdown::selection::selected_text() {
        Some(text) => {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            true
        }
        None => false,
    }
}

/// Whether a transcript selection drag is in flight.
pub fn dragging() -> bool {
    openagents_chat_view::markdown::selection::is_dragging()
}

/// Moves the drag's head to `position` against this frame's text; whether
/// the selection changed.
pub fn drag_to(position: Point<Pixels>) -> bool {
    openagents_chat_view::markdown::render::update_drag_at(position)
}

/// Ends the drag, if one is in flight.
pub fn end_drag() {
    openagents_chat_view::markdown::selection::end_active_drag();
}

/// A transcript's selection drag: where it is, and the timer that scrolls
/// the transcript's list while it holds near the top or bottom edge. Keep
/// one in the view that owns the list; call [`DragScroll::moved`] from the
/// transcript's mouse-move handler and [`DragScroll::released`] from its
/// mouse-up handlers.
#[derive(Default)]
pub struct DragScroll {
    at: Option<Point<Pixels>>,
    task: Option<Task<()>>,
}

impl DragScroll {
    /// The pointer moved; `this` finds this value in the view `T`.
    pub fn moved<T: 'static>(
        &mut self,
        event: &MouseMoveEvent,
        list: &ListState,
        this: fn(&mut T) -> &mut DragScroll,
        cx: &mut Context<T>,
    ) {
        if !event.dragging() || !dragging() {
            self.at = None;
            self.task = None;
            return;
        }
        self.at = Some(event.position);
        if drag_to(event.position) {
            cx.notify();
        }
        self.schedule(list, this, cx);
    }

    /// The button came up: the drag ends.
    pub fn released<T: 'static>(&mut self, cx: &mut Context<T>) {
        self.at = None;
        self.task = None;
        if dragging() {
            end_drag();
            cx.notify();
        }
    }

    fn schedule<T: 'static>(
        &mut self,
        list: &ListState,
        this: fn(&mut T) -> &mut DragScroll,
        cx: &mut Context<T>,
    ) {
        let Some(position) = self.at else {
            return;
        };
        if self.task.is_some() || scroll_step(list.viewport_bounds(), position) == 0.0 {
            return;
        }
        let list = list.clone();
        self.task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(SCROLL_TICK_MS))
                .await;
            view.update(cx, |view, cx| {
                let drag = this(view);
                drag.task = None;
                let Some(position) = drag.at.filter(|_| dragging()) else {
                    return;
                };
                let step = scroll_step(list.viewport_bounds(), position);
                if step == 0.0 {
                    return;
                }
                // Extend over what the last step brought in, then move again.
                drag_to(position);
                list.scroll_by(px(step));
                cx.notify();
                this(view).schedule(&list, this, cx);
            })
            .ok();
        }));
    }
}

/// The signed scroll step (pixels toward the bottom) for a drag at
/// `position` near an edge of `bounds`, as the desktop transcript steps: a
/// quadratic ramp over the 36 pixels nearest the edge, up to 24 a tick.
pub fn scroll_step(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let height = f32::from(bounds.size.height);
    let edge = SCROLL_EDGE_PX.min(height / 3.0);
    if edge <= 0.0 {
        return 0.0;
    }
    let y = f32::from(position.y);
    let (top, bottom) = (f32::from(bounds.top()), f32::from(bounds.bottom()));
    let scaled = |penetration: f32| {
        let t = (penetration / edge).clamp(0.0, 1.0);
        SCROLL_MAX_STEP_PX * t * t
    };
    if y < top + edge {
        -scaled(top + edge - y)
    } else if y > bottom - edge {
        scaled(y - (bottom - edge))
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn a_drag_scrolls_only_near_an_edge_and_faster_at_it() {
        let bounds = Bounds::new(point(px(0.0), px(100.0)), size(px(400.0), px(600.0)));
        assert_eq!(scroll_step(bounds, point(px(10.0), px(400.0))), 0.0);
        assert!(scroll_step(bounds, point(px(10.0), px(110.0))) < 0.0);
        assert!(scroll_step(bounds, point(px(10.0), px(690.0))) > 0.0);
        assert_eq!(scroll_step(bounds, point(px(10.0), px(800.0))), 24.0);
        assert_eq!(scroll_step(bounds, point(px(10.0), px(0.0))), -24.0);
        let near = scroll_step(bounds, point(px(10.0), px(680.0)));
        let at = scroll_step(bounds, point(px(10.0), px(699.0)));
        assert!(0.0 < near && near < at);
    }
}
