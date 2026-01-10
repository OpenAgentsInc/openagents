use crate::components::atoms::ApmGauge;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Clone, Debug, Default)]
pub struct UsageSummary {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Clone, Debug, Default)]
pub struct LastPrSummary {
    pub url: Option<String>,
    pub title: Option<String>,
    pub merged: Option<bool>,
}

pub struct MetricsPane {
    id: Option<ComponentId>,
    apm: Option<f32>,
    queue_depth: Option<u64>,
    oldest_issue: Option<String>,
    last_pr: LastPrSummary,
    usage: Option<UsageSummary>,
}

impl MetricsPane {
    pub fn new() -> Self {
        Self {
            id: None,
            apm: None,
            queue_depth: None,
            oldest_issue: None,
            last_pr: LastPrSummary::default(),
            usage: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn set_apm(&mut self, apm: Option<f32>) {
        self.apm = apm;
    }

    pub fn set_queue(&mut self, depth: Option<u64>, oldest: Option<String>) {
        self.queue_depth = depth;
        self.oldest_issue = oldest;
    }

    pub fn set_last_pr(&mut self, summary: LastPrSummary) {
        self.last_pr = summary;
    }

    pub fn set_usage(&mut self, usage: Option<UsageSummary>) {
        self.usage = usage;
    }
}

impl Default for MetricsPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MetricsPane {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 24.0);
        let header_text = cx.text.layout_mono(
            "Metrics",
            Point::new(header_bounds.origin.x + 10.0, header_bounds.origin.y + 6.0),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(header_text);

        let mut y = bounds.origin.y + header_bounds.size.height + 8.0;
        let x = bounds.origin.x + 10.0;

        let apm_value = self.apm.unwrap_or(0.0);
        let apm_label = if self.apm.is_some() {
            format!("APM {:.1}", apm_value)
        } else {
            "APM --".to_string()
        };
        let apm_label_run = cx.text.layout_mono(
            &apm_label,
            Point::new(x, y),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(apm_label_run);
        y += 16.0;

        let gauge_bounds = Bounds::new(x, y, bounds.size.width - 20.0, 28.0);
        let mut gauge = ApmGauge::new(apm_value).compact(true).show_value(false);
        gauge.paint(gauge_bounds, cx);
        y += gauge_bounds.size.height + 10.0;

        let queue_text = match self.queue_depth {
            Some(depth) => format!("Queue depth {}", depth),
            None => "Queue depth --".to_string(),
        };
        let queue_run = cx.text.layout_mono(
            &queue_text,
            Point::new(x, y),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(queue_run);
        y += 14.0;

        if let Some(ref oldest) = self.oldest_issue {
            let oldest_run = cx.text.layout_mono(
                &format!("Oldest {}", oldest),
                Point::new(x, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(oldest_run);
            y += 14.0;
        }

        let last_pr_text = match self.last_pr.title.as_ref() {
            Some(title) if !title.is_empty() => format!("Last PR {}", title),
            _ => "Last PR --".to_string(),
        };
        let last_pr_run = cx.text.layout_mono(
            &last_pr_text,
            Point::new(x, y),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(last_pr_run);
        y += 14.0;

        if let Some(ref url) = self.last_pr.url {
            if !url.is_empty() {
                let url_run = cx.text.layout_mono(
                    url,
                    Point::new(x, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(url_run);
                y += 14.0;
            }
        }

        if let Some(ref usage) = self.usage {
            let usage_text = format!(
                "Usage {} in / {} out ${:.4}",
                usage.input_tokens, usage.output_tokens, usage.cost_usd
            );
            let usage_run = cx.text.layout_mono(
                &usage_text,
                Point::new(x, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(usage_run);
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}
