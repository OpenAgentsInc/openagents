//! Desktop presentation of the separately admitted Gym board.
use super::{Rect, amber, field};
use crate::gym::BoardView;
use crate::ui::{Atlas, UiBatch};
use coder_ui::theme::Intensity;

/// Selection and retained observation for a Gym panel. Rendering performs no I/O.
pub struct GymPanel<'a> {
    pub view: Option<&'a BoardView>,
    pub recipes: bool,
    pub selected: usize,
    pub scroll: usize,
    pub notice: Option<&'a str>,
}

enum Row {
    Text(String, Intensity),
    Progress(u64, u64),
    Chart(Vec<gym_bridge::Point>),
}

/// Draw the board while its enclosing surface is inside the Gym.
#[must_use]
pub fn gym_panel(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    scale: f32,
    panel: &GymPanel<'_>,
) -> Rect {
    let pad = 12.0 * scale;
    let r = Rect {
        x: pad,
        y: size[1] * 0.12,
        w: (size[0] - 2.0 * pad).max(1.0),
        h: size[1] * 0.74,
    };
    ui.rect(atlas, r.x, r.y, r.w, r.h, field(0.98));
    ui.frame(
        atlas,
        r.x,
        r.y,
        r.w,
        r.h,
        scale.max(1.0),
        amber(Intensity::Full, 1.0),
    );
    let heading = if panel.recipes {
        "GYM · Recipes"
    } else {
        "GYM · Runs"
    };
    ui.text(
        atlas,
        r.x + pad,
        r.y + pad,
        heading,
        amber(Intensity::Full, 1.0),
    );
    let status = panel
        .notice
        .or_else(|| panel.view.and_then(|v| v.error.as_deref()))
        .unwrap_or_else(|| {
            panel
                .view
                .map_or("Gym identity unavailable. F5 retries.", |v| {
                    v.status.as_str()
                })
        });
    let width = r.w - 2.0 * pad;
    for (i, line) in atlas.wrap(status, width).iter().take(2).enumerate() {
        ui.text(
            atlas,
            r.x + pad,
            r.y + pad + (i + 1) as f32 * atlas.line,
            line,
            amber(Intensity::ThreeQuarters, 1.0),
        );
    }
    let top = r.y + pad + 4.0 * atlas.line;
    let bottom = r.y + r.h - pad - 2.0 * atlas.line;
    let split = r.x + r.w * 0.35;
    ui.rect(
        atlas,
        split,
        top,
        scale.max(1.0),
        (bottom - top).max(0.0),
        amber(Intensity::Quarter, 1.0),
    );
    let list_width = (split - r.x - 2.0 * pad).max(atlas.advance);
    let count = panel.view.map_or(0, |v| {
        if panel.recipes {
            v.recipes.len()
        } else {
            v.runs.len()
        }
    });
    let fit = ((bottom - top) / (2.0 * atlas.line)).floor().max(1.0) as usize;
    let selected = panel.selected.min(count.saturating_sub(1));
    let first = selected
        .saturating_sub(fit / 2)
        .min(count.saturating_sub(fit));
    if let Some(v) = panel.view {
        for i in first..(first + fit).min(count) {
            let text = if panel.recipes {
                v.recipes[i].title.clone()
            } else {
                format!("{} · {:?}", v.runs[i].title, v.runs[i].status)
            };
            let prefix = if i == selected { "> " } else { "  " };
            let intensity = if i == selected {
                Intensity::Full
            } else {
                Intensity::Half
            };
            for (j, line) in atlas
                .wrap(&format!("{prefix}{text}"), list_width)
                .iter()
                .take(2)
                .enumerate()
            {
                ui.text(
                    atlas,
                    r.x + pad,
                    top + ((i - first) * 2 + j) as f32 * atlas.line,
                    line,
                    amber(intensity, 1.0),
                );
            }
        }
    }
    if count == 0 {
        ui.text(
            atlas,
            r.x + pad,
            top,
            "No items available.",
            amber(Intensity::Half, 1.0),
        );
    }
    let right = split + pad;
    let rows = detail(panel, atlas, (r.x + r.w - pad - right).max(atlas.advance));
    let max_scroll = scroll_limit(&rows, (bottom - top) / atlas.line);
    let mut y = top;
    for row in rows.iter().skip(panel.scroll.min(max_scroll)) {
        let height = match row {
            Row::Chart(_) => 4.0 * atlas.line,
            _ => atlas.line,
        };
        if y + height > bottom {
            break;
        }
        match row {
            Row::Text(text, intensity) => {
                ui.text(atlas, right, y, text, amber(*intensity, 1.0));
            }
            Row::Progress(done, total) => {
                let w = r.x + r.w - pad - right;
                ui.rect(
                    atlas,
                    right,
                    y + 4.0 * scale,
                    w,
                    atlas.line - 8.0 * scale,
                    amber(Intensity::Quarter, 0.5),
                );
                if *total > 0 {
                    ui.rect(
                        atlas,
                        right,
                        y + 4.0 * scale,
                        w * (*done as f64 / *total as f64).clamp(0.0, 1.0) as f32,
                        atlas.line - 8.0 * scale,
                        amber(Intensity::ThreeQuarters, 1.0),
                    );
                }
            }
            Row::Chart(points) => chart(
                ui,
                atlas,
                Rect {
                    x: right,
                    y,
                    w: r.x + r.w - pad - right,
                    h: height,
                },
                points,
            ),
        }
        y += height;
    }
    for (i, text) in [
        "Tab lists · Up/Down choose · Enter open/confirm · Backspace back",
        "PgUp/PgDn details · Y retry · F5 reload · G/Esc close",
    ]
    .iter()
    .enumerate()
    {
        ui.text(
            atlas,
            r.x + pad,
            r.y + r.h - pad - (2 - i) as f32 * atlas.line,
            text,
            amber(Intensity::Half, 1.0),
        );
    }
    r
}

fn scroll_limit(rows: &[Row], visible_lines: f32) -> usize {
    let height = |row: &Row| {
        if matches!(row, Row::Chart(_)) {
            4.0
        } else {
            1.0
        }
    };
    let mut hidden = (rows.iter().map(height).sum::<f32>() - visible_lines.max(1.0)).max(0.0);
    let mut first = 0;
    while hidden > 0.0 && first + 1 < rows.len() {
        hidden -= height(&rows[first]);
        first += 1;
    }
    first
}

fn detail(panel: &GymPanel<'_>, atlas: &Atlas, width: f32) -> Vec<Row> {
    let mut lines: Vec<(String, Intensity)> = Vec::new();
    let mut graphics = Vec::new();
    let Some(view) = panel.view else {
        return vec![Row::Text(
            "Press F5 to retry identity loading.".into(),
            Intensity::Half,
        )];
    };
    lines.push((
        if view.stale {
            "STALE · last retained observation".into()
        } else {
            format!(
                "Observed {} · host report",
                view.observed_at.map_or("unknown".into(), |t| t.to_string())
            )
        },
        Intensity::Half,
    ));
    if !view.configured {
        lines.push((
            "Authorize this public key on a Gym host:".into(),
            Intensity::Full,
        ));
        lines.push((view.public_key.clone(), Intensity::ThreeQuarters));
        lines.push(("Start Verse with --gym-connection FILE. After the host writes the signed connection code, press F5 here.".into(),Intensity::Half));
    } else if panel.recipes {
        if let Some(recipe) = &view.selected_recipe {
            lines.push((format!("Confirm: {}", recipe.title), Intensity::Full));
            lines.push((
                format!("Recipe {} · {}", recipe.id, recipe.revision),
                Intensity::Half,
            ));
            lines.push((
                format!(
                    "Wall limit {} seconds · at most {} starts",
                    recipe.budget.wall_ms / 1000,
                    recipe.budget.max_starts
                ),
                Intensity::ThreeQuarters,
            ));
            lines.push((spend_label(&recipe.budget), Intensity::ThreeQuarters));
            lines.push((recipe.detail.clone(), Intensity::Half));
            lines.push(("Enter explicitly requests this exact recipe. Selecting or entering the Gym never starts a run.".into(),Intensity::Full));
        } else {
            lines.push(("Choose a recipe and press Enter to review its exact revision and bounds before confirming.".into(),Intensity::Half));
        }
    } else if let Some(run) = &view.selected_run {
        lines.push((run.title.clone(), Intensity::Full));
        lines.push((
            format!("{:?} · {:?}", run.category, run.status),
            Intensity::ThreeQuarters,
        ));
        lines.push((measurements(run), Intensity::Half));
        if let Some((done, total)) = run.completed.zip(run.total) {
            lines.push((format!("Progress {done}/{total}"), Intensity::Half));
            graphics.push(Row::Progress(done, total));
        }
        lines.push((format!("Source: {}", run.source), Intensity::Half));
        lines.push((run.provenance.clone(), Intensity::Quarter));
        for metric in &run.metrics {
            graphics.push(Row::Text(
                format!(
                    "{} ({}) · {} points",
                    metric.name,
                    metric.unit,
                    metric.points.len()
                ),
                Intensity::Half,
            ));
            if metric.points.is_empty() {
                graphics.push(Row::Text("No observations.".into(), Intensity::Quarter));
            } else {
                let min = metric
                    .points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::INFINITY, f64::min);
                let max = metric
                    .points
                    .iter()
                    .map(|p| p.value)
                    .fold(f64::NEG_INFINITY, f64::max);
                graphics.push(Row::Text(
                    format!("Range {min:.4} to {max:.4}; horizontal axis is step"),
                    Intensity::Quarter,
                ));
                graphics.push(Row::Chart(metric.points.clone()));
            }
        }
    } else {
        lines.push((
            "Choose a run and press Enter to inspect retained progress and measurements.".into(),
            Intensity::Half,
        ));
    }
    if let Some(launch) = &view.launch {
        lines.push((
            format!("Launch {} · {}", launch.phase, launch.request_id),
            Intensity::Full,
        ));
        if let Some(receipt) = &launch.receipt {
            lines.push((
                format!(
                    "Host receipt: {:?} · run {}",
                    receipt.status, receipt.run_id
                ),
                Intensity::Half,
            ));
        }
        if let Some(error) = &launch.error {
            lines.push((error.clone(), Intensity::ThreeQuarters));
        }
        lines.push(("Accepted means the host admitted a start; it does not mean the run passed. Uncertain retries reuse the same launch ID.".into(),Intensity::Half));
    }
    lines.extend(
        view.notices
            .iter()
            .cloned()
            .map(|s| (s, Intensity::Quarter)),
    );
    let mut rows: Vec<Row> = lines
        .into_iter()
        .flat_map(|(text, step)| {
            atlas
                .wrap(&text, width)
                .into_iter()
                .map(move |line| Row::Text(line, step))
        })
        .collect();
    for graphic in graphics {
        match graphic {
            Row::Text(text, step) => rows.extend(
                atlas
                    .wrap(&text, width)
                    .into_iter()
                    .map(|line| Row::Text(line, step)),
            ),
            other => rows.push(other),
        }
    }
    rows
}

fn measurements(run: &gym_bridge::Run) -> String {
    let cost = run
        .cost_usd
        .map_or("unknown".into(), |v| format!("${v:.4}"));
    let time = run
        .elapsed_ms
        .map_or("unknown".into(), |v| format!("{:.1}s", v as f64 / 1000.0));
    format!("Cost {cost} · elapsed {time}")
}
fn spend_label(budget: &gym_bridge::Budget) -> String {
    match (budget.spend_limit_usd, budget.spend_enforced) {
        (Some(limit), true) => format!("Enforced spend limit ${limit:.4}"),
        (Some(limit), false) => format!("Declared spend ${limit:.4}; NOT enforced"),
        (None, _) => {
            "No monetary limit is enforced; inspect the host recipe before starting.".into()
        }
    }
}
fn chart(ui: &mut UiBatch, atlas: &Atlas, r: Rect, points: &[gym_bridge::Point]) {
    let min = points.iter().map(|p| p.value).fold(f64::INFINITY, f64::min);
    let max = points
        .iter()
        .map(|p| p.value)
        .fold(f64::NEG_INFINITY, f64::max);
    let Some(first) = points.first() else {
        return;
    };
    let last = points.last().expect("the first point exists");
    ui.rect(
        atlas,
        r.x,
        r.y + r.h - 1.0,
        r.w,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
    let magnitude = min.abs().max(max.abs()).max(1.0);
    let span = max / magnitude - min / magnitude;
    for point in points {
        let x = if first.step == last.step {
            0.5
        } else {
            (point.step - first.step) as f64 / (last.step - first.step) as f64
        };
        let y = if min == max {
            0.5
        } else {
            (point.value / magnitude - min / magnitude) / span
        };
        ui.rect(
            atlas,
            r.x + (r.w - 3.0) * x as f32,
            r.y + (r.h - 3.0) * (1.0 - y as f32),
            3.0,
            3.0,
            amber(Intensity::Full, 1.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tall_charts_can_scroll_even_when_the_number_of_rows_is_small() {
        let rows = [
            Row::Text("heading".into(), Intensity::Half),
            Row::Chart(vec![]),
            Row::Chart(vec![]),
        ];
        assert_eq!(scroll_limit(&rows, 5.0), 2);
        assert_eq!(scroll_limit(&rows, 9.0), 0);
    }

    #[test]
    fn missing_measurements_and_unenforced_spend_are_not_free_or_bounded() {
        let run = gym_bridge::Run {
            id: "0".repeat(64),
            title: "Synthetic".into(),
            category: gym_bridge::Category::Agent,
            status: gym_bridge::Status::Unknown,
            completed: None,
            total: None,
            cost_usd: None,
            elapsed_ms: None,
            metrics: vec![],
            source: "fixture".into(),
            provenance: "synthetic".into(),
        };
        assert_eq!(measurements(&run), "Cost unknown · elapsed unknown");
        assert!(
            spend_label(&gym_bridge::Budget {
                wall_ms: 1000,
                max_starts: 1,
                spend_limit_usd: None,
                spend_enforced: false
            })
            .contains("No monetary limit")
        );
        assert!(
            spend_label(&gym_bridge::Budget {
                wall_ms: 1000,
                max_starts: 1,
                spend_limit_usd: Some(1.0),
                spend_enforced: false
            })
            .contains("NOT enforced")
        );
    }
}
