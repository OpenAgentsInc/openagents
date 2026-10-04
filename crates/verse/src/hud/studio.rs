//! The Agent Studio's goal bar and waiting badge, top center while the
//! player is in Everglade: the goal the atrium board shows, a progress bar
//! with its task count, and, while decisions wait, a badge that names the
//! count and the key that opens them. The badge is also a tap target.

use super::{Rect, amber, field};
use crate::ui::{Atlas, UiBatch};
use crate::zones::everglade::signals::{Summary, badge};
use coder_ui::theme::Intensity;

/// Where the goal bar and its badge landed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StudioStrip {
    /// The goal bar.
    pub bar: Rect,
    /// The waiting badge, while decisions wait: a click or tap there opens
    /// the decisions.
    pub badge: Option<Rect>,
}

/// `text` cut to fit `width` at `atlas`'s size, with an ellipsis when cut.
fn fit(atlas: &Atlas, text: &str, width: f32) -> String {
    if atlas.measure(text) <= width {
        return text.to_owned();
    }
    // More characters than fit at a quarter of the advance each, so the
    // trimming below measures a bounded string however long the goal is.
    let budget = (width / (atlas.advance * 0.25).max(0.5)) as usize + 1;
    let mut out: String = text.chars().take(budget).collect();
    while !out.is_empty() && atlas.measure(&format!("{out}…")) > width {
        out.pop();
    }
    format!("{}…", out.trim_end())
}

/// Draws the goal bar for `summary`, with the sound's state when `muted`.
#[must_use]
pub fn studio_strip(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    scale: f32,
    summary: &Summary,
    muted: bool,
) -> StudioStrip {
    let s = scale;
    let margin = 12.0 * s;
    let pad = 6.0 * s;
    let width = (size[0] * 0.46)
        .max(atlas.advance * 30.0)
        .min(size[0] - 2.0 * margin)
        .max(1.0);
    let bar_h = 6.0 * s;
    let height = 2.0 * atlas.line + 2.0 * pad;
    let bar = Rect {
        x: (size[0] - width) / 2.0,
        y: margin,
        w: width,
        h: height,
    };
    ui.rect(atlas, bar.x, bar.y, bar.w, bar.h, field(0.9));
    ui.frame(
        atlas,
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
    let inner = bar.w - 2.0 * pad;
    let label = "GOAL  ";
    let x = bar.x + pad;
    let label_w = ui.text(atlas, x, bar.y + pad, label, amber(Intensity::Half, 1.0));
    let goal = fit(
        atlas,
        &summary.text.replace(['\n', '\r', '\t'], " "),
        inner - label_w,
    );
    ui.text(
        atlas,
        x + label_w,
        bar.y + pad,
        &goal,
        amber(Intensity::Full, 1.0),
    );
    let mut counts = summary.counts();
    if muted {
        counts.push_str(" · muted (V)");
    }
    let counts_w = atlas.measure(&counts);
    let row = bar.y + pad + atlas.line;
    let track_w = (inner - counts_w - 8.0 * s).max(0.0);
    let track_y = row + (atlas.line - bar_h) / 2.0;
    ui.rect(
        atlas,
        x,
        track_y,
        track_w,
        bar_h,
        amber(Intensity::Quarter, 0.5),
    );
    ui.rect(
        atlas,
        x,
        track_y,
        track_w * summary.progress(),
        bar_h,
        amber(Intensity::Full, 1.0),
    );
    ui.text(
        atlas,
        bar.x + bar.w - pad - counts_w,
        row,
        &counts,
        amber(Intensity::ThreeQuarters, 1.0),
    );
    let badge = badge(summary.waiting).map(|text| {
        let w = atlas.measure(&text) + 16.0 * s;
        let r = Rect {
            x: (size[0] - w) / 2.0,
            y: bar.y + bar.h + 4.0 * s,
            w,
            h: atlas.line + 8.0 * s,
        };
        ui.rect(atlas, r.x, r.y, r.w, r.h, amber(Intensity::Quarter, 0.7));
        ui.frame(
            atlas,
            r.x,
            r.y,
            r.w,
            r.h,
            s.max(1.0),
            amber(Intensity::Full, 1.0),
        );
        ui.text(
            atlas,
            r.x + 8.0 * s,
            r.y + 4.0 * s,
            &text,
            amber(Intensity::Full, 1.0),
        );
        r
    });
    StudioStrip { bar, badge }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::GoalStatus;

    fn summary(waiting: usize) -> Summary {
        Summary {
            text: "Add a dark mode to the settings page".into(),
            status: GoalStatus::Running,
            done: 2,
            total: 5,
            waiting,
        }
    }

    #[test]
    fn the_bar_sits_top_center_and_the_badge_shows_only_while_decisions_wait() {
        let atlas = Atlas::new(14.0);
        let size = [1280.0, 800.0];
        let mut ui = UiBatch::default();
        let quiet = studio_strip(&mut ui, &atlas, size, 1.0, &summary(0), false);
        assert!(quiet.badge.is_none());
        assert!(!ui.vertices.is_empty());
        let center = quiet.bar.x + quiet.bar.w / 2.0;
        assert!((center - size[0] / 2.0).abs() < 0.5);
        assert!(quiet.bar.x >= 0.0 && quiet.bar.x + quiet.bar.w <= size[0]);
        let mut ui = UiBatch::default();
        let waiting = studio_strip(&mut ui, &atlas, size, 1.0, &summary(2), true);
        let badge = waiting.badge.expect("a badge while decisions wait");
        assert!(badge.y >= waiting.bar.y + waiting.bar.h);
        assert!(badge.contains(badge.x + badge.w / 2.0, badge.y + badge.h / 2.0));
        // A narrow screen keeps the bar inside it.
        let mut ui = UiBatch::default();
        let narrow = studio_strip(&mut ui, &atlas, [200.0, 400.0], 1.0, &summary(1), false);
        assert!(narrow.bar.x >= 0.0 && narrow.bar.x + narrow.bar.w <= 200.0 + 0.5);
    }

    #[test]
    fn a_long_goal_is_cut_to_fit() {
        let atlas = Atlas::new(14.0);
        let long = "word ".repeat(80);
        let cut = fit(&atlas, &long, 200.0);
        assert!(atlas.measure(&cut) <= 200.0);
        assert!(cut.ends_with('…'));
        assert_eq!(fit(&atlas, "short", 200.0), "short");
    }
}
