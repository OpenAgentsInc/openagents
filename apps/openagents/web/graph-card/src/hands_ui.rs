//! `/lander9`'s hands on the page: the inbox the page's landmarker fills
//! (`hand_frame`, and `synth_hand` for the test hook), the hands drawn over
//! everything in the site's grays (bones, joints, brighter fingertips, and
//! a cursor at the index tip that fills while it pinches and rings while an
//! open palm is held), and the gesture legend.

use std::cell::RefCell;
use std::collections::VecDeque;

use gpui::{
    AnyElement, Bounds, FontWeight, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Styled,
    Window, canvas, div, fill, point, px, size,
};
use openagents_web_chat_view::theme;
use openagents_web_hands::synth::{Pose, hand};
use openagents_web_hands::{CONNECTIONS, Gesture, HandView, Mapping, RawFrame, RawHand, TIPS};

/// What the page has sent since the last look.
#[derive(Default)]
pub struct Inbox {
    pub frames: VecDeque<(RawFrame, f64)>,
    /// Tracking turned on or off.
    pub on: Option<bool>,
    /// The landmarker's last time for a frame, in milliseconds.
    pub infer_ms: f32,
}

/// The last numbers, for `hand_stats`.
#[derive(Default, Clone, Copy)]
pub struct Stats {
    pub fps: f32,
    pub frames: u64,
    pub hands: usize,
    pub infer_ms: f32,
}

thread_local! {
    pub static INBOX: RefCell<Inbox> = RefCell::new(Inbox::default());
    pub static STATS: RefCell<Stats> = RefCell::new(Stats::default());
}

/// Keeps the newest few frames: a slow page drops old ones rather than
/// falling behind.
const KEEP: usize = 4;

pub fn push_frame(data: &[f32], t_ms: f64, infer_ms: f32) -> bool {
    let Some(frame) = RawFrame::decode(data) else {
        return false;
    };
    INBOX.with(|inbox| {
        let mut inbox = inbox.borrow_mut();
        inbox.frames.push_back((frame, t_ms));
        while inbox.frames.len() > KEEP {
            inbox.frames.pop_front();
        }
        inbox.infer_ms = infer_ms;
    });
    true
}

pub fn set_on(on: bool) {
    INBOX.with(|inbox| inbox.borrow_mut().on = Some(on));
}

/// A made-up frame with a hand in each pose at each place, as fractions of
/// the page (`[pose, x, y]` per hand), for the test hook.
pub fn push_synth(hands: &[(&str, f32, f32)], t_ms: f64) -> bool {
    let aspect = 4.0 / 3.0;
    let map = Mapping::default();
    let mut frame = RawFrame {
        aspect,
        hands: Vec::new(),
    };
    for (pose, x, y) in hands {
        let Some(pose) = Pose::parse(pose) else {
            return false;
        };
        frame.hands.push(RawHand {
            score: 0.99,
            landmarks: hand(pose, *x, *y, aspect, &map),
        });
    }
    push_frame(&frame.encode(), t_ms, 0.0)
}

/// The hands, over everything.
pub fn overlay(views: Vec<HandView>, still: bool) -> AnyElement {
    canvas(
        move |_, _, _| views,
        move |bounds: Bounds<Pixels>, views: Vec<HandView>, window: &mut Window, _| {
            let o = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
            for v in &views {
                draw_hand(window, o, v, still);
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

fn circle(window: &mut Window, o: (f32, f32), at: (f32, f32), r: f32, color: Hsla) {
    window.paint_quad(
        fill(
            Bounds::new(
                point(px(o.0 + at.0 - r), px(o.1 + at.1 - r)),
                size(px(2. * r), px(2. * r)),
            ),
            color,
        )
        .corner_radii(px(r)),
    );
}

fn ring(window: &mut Window, o: (f32, f32), at: (f32, f32), r: f32, width: f32, color: Hsla) {
    window.paint_quad(
        fill(
            Bounds::new(
                point(px(o.0 + at.0 - r), px(o.1 + at.1 - r)),
                size(px(2. * r), px(2. * r)),
            ),
            gpui::transparent_black(),
        )
        .corner_radii(px(r))
        .border_widths(px(width))
        .border_color(color),
    );
}

fn white(a: f32) -> Hsla {
    gpui::hsla(0., 0., 1., a)
}

fn draw_hand(window: &mut Window, o: (f32, f32), v: &HandView, still: bool) {
    let at = |p: (f32, f32)| point(px(o.0 + p.0), px(o.1 + p.1));
    let main = if v.primary { 1.0 } else { 0.7 };
    // Bones: a wide faint glow under a thin bright line.
    for (width, alpha) in [(10.0, 0.06), (5.0, 0.12), (2.0, 0.55)] {
        let mut path = PathBuilder::stroke(px(width));
        for [a, b] in CONNECTIONS {
            path.move_to(at(v.points[*a]));
            path.line_to(at(v.points[*b]));
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, white(alpha * main));
        }
    }
    // Nearer joints (more negative depth) a little larger.
    let near = |i: usize| (1.0 - v.depth[i] * 4.0).clamp(0.75, 1.6);
    for i in 0..21 {
        if TIPS.contains(&i) {
            continue;
        }
        let k = near(i);
        circle(window, o, v.points[i], 7.0 * k, white(0.07 * main));
        circle(window, o, v.points[i], 3.0 * k, white(0.75 * main));
    }
    for &i in &TIPS {
        let k = near(i);
        circle(window, o, v.points[i], 13.0 * k, white(0.08 * main));
        circle(window, o, v.points[i], 8.0 * k, white(0.16 * main));
        circle(window, o, v.points[i], 4.2 * k, white(main));
    }
    // The cursor.
    let c = v.cursor;
    if v.grabbing {
        circle(window, o, c, 16.0, white(0.18 * main));
        circle(window, o, c, 9.0, white(0.9 * main));
        ring(window, o, c, 14.0, 2.0, white(0.9 * main));
    } else {
        circle(window, o, c, 4.5, white(main));
        ring(window, o, c, 11.0, 1.5, white(0.7 * main));
    }
    // An open palm's hold, as an arc filling around the cursor.
    if v.palm > 0.0 && !still {
        let r = 22.0;
        let steps = (48.0 * v.palm).ceil() as usize;
        let mut path = PathBuilder::stroke(px(3.0));
        for s in 0..=steps {
            let a = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * v.palm * s as f32 / steps.max(1) as f32;
            let p = at((c.0 + r * a.cos(), c.1 + r * a.sin()));
            if s == 0 {
                path.move_to(p);
            } else {
                path.line_to(p);
            }
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, white(0.9));
        }
    }
}

/// The gestures, and what each does.
pub const LEGEND: [(&str, &str); 6] = [
    ("Point", "Show a part"),
    ("Pinch", "Select, or type a message"),
    ("Pinch and hold, or a fist", "Move a part or the chat"),
    ("Drop a tool on the chat", "Start a demo turn"),
    ("Pinch with both hands", "Spread the parts"),
    ("Hold an open palm", "Let go"),
];

/// The hint while tracking is off.
pub const MOUSE_HINT: &str = "Drag a tool onto the chat to start a demo turn.";

/// The legend's last line: how tracking is going.
pub fn status_line(stats: Stats, gesture: Option<Gesture>) -> String {
    let hands = match stats.hands {
        0 => "No hand in view".to_owned(),
        1 => format!("1 hand: {}", gesture.unwrap_or_default().word()),
        n => format!("{n} hands"),
    };
    if stats.fps > 0.0 {
        format!("{hands} · {:.0} fps · {:.0} ms", stats.fps, stats.infer_ms)
    } else {
        hands
    }
}

pub fn legend(on: bool, status: String, x: f32, bottom: f32) -> AnyElement {
    let mut el = div()
        .absolute()
        .right(px(x))
        .bottom(px(bottom))
        .w(px(312.))
        .flex()
        .flex_col()
        .gap(px(5.))
        .p(px(12.))
        .rounded(px(12.))
        .bg(gpui::hsla(0., 0., 0.05, 0.88))
        .border_1()
        .border_color(theme::border())
        .text_size(px(11.5))
        .line_height(px(16.))
        .text_color(theme::text_muted());
    if !on {
        return el.child(MOUSE_HINT).into_any_element();
    }
    el = el.child(
        div()
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::text())
            .child("Hand gestures"),
    );
    for (gesture, does) in LEGEND {
        el = el.child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(140.))
                        .flex_none()
                        .text_color(theme::text())
                        .child(gesture),
                )
                .child(div().flex_1().min_w_0().child(does)),
        );
    }
    el.child(
        div()
            .pt(px(4.))
            .font_family(theme::MONO)
            .text_size(px(10.5))
            .text_color(theme::text_faint())
            .child(status),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_legend_reads_as_plain_words() {
        for (a, b) in LEGEND {
            assert!(oa_copy::violations(a, &[]).is_empty(), "{a}");
            assert!(oa_copy::violations(b, &[]).is_empty(), "{b}");
        }
        assert!(oa_copy::violations(MOUSE_HINT, &[]).is_empty());
    }

    #[test]
    fn the_test_hook_frames_decode() {
        assert!(push_synth(&[("pinch", 0.4, 0.5), ("open", 0.6, 0.5)], 10.0));
        assert!(!push_synth(&[("wave", 0.4, 0.5)], 20.0));
        let frames = INBOX.with(|i| i.borrow().frames.len());
        assert_eq!(frames, 1);
        assert!(!push_frame(&[1.0], 0.0, 0.0));
    }
}
