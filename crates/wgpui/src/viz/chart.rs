use crate::{Bounds, Hsla, PaintContext, Point, Quad, theme};

use super::{panel, sampling, theme as viz_theme};

#[derive(Clone, Copy)]
pub struct HistoryChartSeries<'a> {
    pub label: &'a str,
    pub values: &'a [f32],
    pub color: Hsla,
    pub fill_alpha: f32,
    pub line_alpha: f32,
}

pub fn paint_history_chart_body(
    bounds: Bounds,
    accent: Hsla,
    phase: f32,
    header: Option<&str>,
    footer: Option<&str>,
    empty_state: &str,
    series: &[HistoryChartSeries<'_>],
    paint: &mut PaintContext,
) {
    panel::paint_texture(bounds, accent, phase, paint);
    let populated = series
        .iter()
        .filter(|series| !series.values.is_empty())
        .collect::<Vec<_>>();
    if populated.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            empty_state,
            Point::new(bounds.origin.x + 14.0, bounds.origin.y + 36.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    if let Some(header) = header {
        let header = truncate_text_to_width(paint, header, bounds.size.width - 32.0, 10.0, true);
        paint.scene.draw_text(paint.text.layout_mono(
            header.as_str(),
            Point::new(bounds.origin.x + 16.0, bounds.origin.y + 34.0),
            10.0,
            theme::text::PRIMARY,
        ));
    }

    let mut legend_x = bounds.origin.x + 16.0;
    for series in &populated {
        let label = series.label.to_ascii_uppercase();
        paint.scene.draw_text(paint.text.layout_mono(
            label.as_str(),
            Point::new(legend_x, bounds.origin.y + 46.0),
            9.0,
            series.color.with_alpha(0.92),
        ));
        let width = paint
            .text
            .layout_mono(label.as_str(), Point::ZERO, 9.0, theme::text::PRIMARY)
            .bounds()
            .size
            .width;
        legend_x += width + 12.0;
    }

    let chart_bounds = Bounds::new(
        bounds.origin.x + 16.0,
        bounds.origin.y + 60.0,
        bounds.size.width - 32.0,
        (bounds.size.height - 116.0).max(104.0),
    );
    paint.scene.draw_quad(
        Quad::new(chart_bounds)
            .with_background(viz_theme::surface::CHART_BG)
            .with_border(accent.with_alpha(0.16), 1.0)
            .with_corner_radius(8.0),
    );

    let min = populated
        .iter()
        .flat_map(|series| series.values.iter().copied())
        .fold(f32::INFINITY, f32::min);
    let max = populated
        .iter()
        .flat_map(|series| series.values.iter().copied())
        .fold(f32::NEG_INFINITY, f32::max);
    let min = if min.is_finite() { min } else { 0.0 };
    let max = if max.is_finite() { max } else { 1.0 };
    let span = (max - min).max(0.0001);

    for band in 0..=4 {
        let y = chart_bounds.origin.y + band as f32 * (chart_bounds.size.height / 4.0);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                chart_bounds.origin.x,
                y,
                chart_bounds.size.width,
                1.0,
            ))
            .with_background(theme::text::PRIMARY.with_alpha(0.08)),
        );
        let guide_value = max - span * (band as f32 / 4.0);
        paint.scene.draw_text(paint.text.layout_mono(
            format!("{guide_value:.2}").as_str(),
            Point::new(chart_bounds.origin.x + 8.0, y - 2.0),
            9.0,
            theme::text::MUTED,
        ));
    }

    let sample_count = ((chart_bounds.size.width / 4.0).floor() as usize).clamp(24, 160);
    let sampled = populated
        .iter()
        .map(|series| sampling::sample_history_series(series.values, sample_count))
        .collect::<Vec<_>>();
    let column_gap = 1.5;
    let column_width = ((chart_bounds.size.width
        - column_gap * sample_count.saturating_sub(1) as f32)
        / sample_count as f32)
        .max(1.0);

    for index in 0..sample_count {
        let x = chart_bounds.origin.x + index as f32 * (column_width + column_gap);
        let emphasis = if index + 1 == sample_count { 0.16 } else { 0.0 };
        for (series_index, series) in populated.iter().enumerate() {
            let value = sampled
                .get(series_index)
                .and_then(|series| series.get(index))
                .copied()
                .or_else(|| series.values.last().copied())
                .unwrap_or(min);
            let level = 1.0 - ((value - min) / span).clamp(0.0, 1.0);
            let y = chart_bounds.origin.y + level * chart_bounds.size.height;
            if series.fill_alpha > 0.0 {
                paint.scene.draw_quad(
                    Quad::new(Bounds::new(
                        x,
                        y.min(chart_bounds.max_y() - 2.0),
                        column_width,
                        (chart_bounds.max_y() - y).max(2.0),
                    ))
                    .with_background(series.color.with_alpha(series.fill_alpha + emphasis)),
                );
            }
            paint.scene.draw_quad(
                Quad::new(Bounds::new(
                    x,
                    y.clamp(chart_bounds.origin.y, chart_bounds.max_y() - 2.0),
                    column_width,
                    2.0,
                ))
                .with_background(series.color.with_alpha(series.line_alpha + emphasis)),
            );
        }
    }

    if let Some(footer) = footer {
        let footer = truncate_text_to_width(paint, footer, bounds.size.width - 32.0, 10.0, true);
        paint.scene.draw_text(paint.text.layout_mono(
            footer.as_str(),
            Point::new(bounds.origin.x + 16.0, bounds.max_y() - 12.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn truncate_text_to_width(
    paint: &mut PaintContext,
    value: &str,
    max_width: f32,
    font_size: f32,
    mono: bool,
) -> String {
    if value.is_empty() || max_width <= 0.0 {
        return String::new();
    }

    let measure = |candidate: &str, paint: &mut PaintContext| -> f32 {
        if mono {
            paint
                .text
                .layout_mono(candidate, Point::ZERO, font_size, theme::text::PRIMARY)
                .bounds()
                .size
                .width
        } else {
            paint
                .text
                .layout(candidate, Point::ZERO, font_size, theme::text::PRIMARY)
                .bounds()
                .size
                .width
        }
    };

    if measure(value, paint) <= max_width {
        return value.to_string();
    }
    if measure("...", paint) > max_width {
        return String::new();
    }

    let chars: Vec<char> = value.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high + 1) / 2;
        let candidate = format!("{}...", chars[..mid].iter().collect::<String>());
        if measure(candidate.as_str(), paint) <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    if low == 0 {
        String::from("...")
    } else {
        format!("{}...", chars[..low].iter().collect::<String>())
    }
}
