//! Claude Usage component - shows API usage stats

use wgpui::{Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad};
use wgpui::components::hud::{CornerConfig, Frame};

/// Session-level usage stats
#[derive(Default, Clone)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u32,
}

/// Usage limit info
#[derive(Clone)]
pub struct UsageLimit {
    pub name: String,
    pub percent_used: f64,
    pub resets_at: String,
}

/// Claude usage display component
pub struct ClaudeUsage {
    pub model: String,
    pub context_used: u64,
    pub context_total: u64,
    pub session: SessionUsage,
    pub limits: Vec<UsageLimit>,
    pub web_searches: u64,
}

impl ClaudeUsage {
    pub fn new() -> Self {
        Self {
            model: "opus-4-5".to_string(),
            context_used: 45_000,
            context_total: 200_000,
            session: SessionUsage {
                input_tokens: 125_000,
                output_tokens: 42_000,
                cache_read_tokens: 98_000,
                cache_creation_tokens: 15_000,
                total_cost_usd: 2.47,
                duration_ms: 847_000,
                duration_api_ms: 234_000,
                num_turns: 23,
            },
            limits: vec![
                UsageLimit {
                    name: "Weekly limit".to_string(),
                    percent_used: 34.0,
                    resets_at: "Jan 1".to_string(),
                },
            ],
            web_searches: 5,
        }
    }

    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub fn set_context(&mut self, used: u64, total: u64) {
        self.context_used = used;
        self.context_total = total;
    }

    pub fn set_session(&mut self, session: SessionUsage) {
        self.session = session;
    }
}

impl Default for ClaudeUsage {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ClaudeUsage {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw HUD frame
        let line_color = Hsla::new(0.0, 0.0, 0.4, 0.5);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0)
            .corner_config(CornerConfig::all())
            .square_size(4.0)
            .small_line_length(4.0)
            .large_line_length(12.0);
        frame.paint(bounds, cx);

        let padding = 12.0;
        let mut y = bounds.origin.y + padding;
        let x = bounds.origin.x + padding;
        let w = bounds.size.width - padding * 2.0;

        let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
        let value_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
        let muted_color = Hsla::new(0.0, 0.0, 0.4, 1.0);
        let green_color = Hsla::new(140.0, 0.6, 0.5, 1.0);
        let cyan_color = Hsla::new(180.0, 0.5, 0.6, 1.0);
        let orange_color = Hsla::new(30.0, 0.7, 0.5, 1.0);

        let font_size = 10.0;
        let line_height = 14.0;

        // Header
        let header = cx.text.layout("CLAUDE USAGE", Point::new(x, y), font_size, label_color);
        cx.scene.draw_text(header);
        y += line_height + 4.0;

        // Model
        let model_text = cx.text.layout(&self.model, Point::new(x, y), 11.0, value_color);
        cx.scene.draw_text(model_text);
        y += line_height + 8.0;

        // Context window progress bar
        let ctx_pct = if self.context_total > 0 {
            (self.context_used as f64 / self.context_total as f64) * 100.0
        } else {
            0.0
        };

        let ctx_label = cx.text.layout("Context", Point::new(x, y), font_size, muted_color);
        cx.scene.draw_text(ctx_label);
        let pct_text = format!("{:.0}%", ctx_pct);
        let pct_label = cx.text.layout(&pct_text, Point::new(x + w - 30.0, y), font_size, muted_color);
        cx.scene.draw_text(pct_label);
        y += line_height;

        // Progress bar background
        let bar_h = 4.0;
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, bar_h)).with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)));
        // Progress bar fill
        let bar_color = if ctx_pct < 50.0 {
            green_color
        } else if ctx_pct < 75.0 {
            Hsla::new(45.0, 0.8, 0.5, 1.0) // yellow
        } else {
            Hsla::new(0.0, 0.8, 0.5, 1.0) // red
        };
        let fill_w = (w * ctx_pct as f32 / 100.0).min(w);
        cx.scene.draw_quad(Quad::new(Bounds::new(x, y, fill_w, bar_h)).with_background(bar_color));
        y += bar_h + 2.0;

        // Context tokens
        let ctx_tokens = format!("{} / {}", format_tokens(self.context_used), format_tokens(self.context_total));
        let ctx_tok_label = cx.text.layout(&ctx_tokens, Point::new(x, y), 9.0, muted_color);
        cx.scene.draw_text(ctx_tok_label);
        y += line_height + 8.0;

        // Usage limits
        for limit in &self.limits {
            let limit_label = cx.text.layout(&limit.name, Point::new(x, y), font_size, muted_color);
            cx.scene.draw_text(limit_label);
            y += line_height;

            // Progress bar
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, bar_h)).with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)));
            let limit_fill = (w * limit.percent_used as f32 / 100.0).min(w);
            let limit_color = if limit.percent_used < 50.0 {
                green_color
            } else if limit.percent_used < 75.0 {
                Hsla::new(45.0, 0.8, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.8, 0.5, 1.0)
            };
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, limit_fill, bar_h)).with_background(limit_color));
            y += bar_h + 2.0;

            let limit_info = format!("{:.0}% used · Resets {}", limit.percent_used, limit.resets_at);
            let limit_info_label = cx.text.layout(&limit_info, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(limit_info_label);
            y += line_height + 6.0;
        }

        // Session divider
        y += 4.0;

        // Session header
        let session_header = cx.text.layout("SESSION", Point::new(x, y), font_size, label_color);
        cx.scene.draw_text(session_header);
        y += line_height + 4.0;

        // Cost and turns
        let cost_text = format!("${:.4}", self.session.total_cost_usd);
        let cost_label = cx.text.layout(&cost_text, Point::new(x, y), 11.0, value_color);
        cx.scene.draw_text(cost_label);
        let turns_text = format!("{} turns", self.session.num_turns);
        let turns_label = cx.text.layout(&turns_text, Point::new(x + 70.0, y), font_size, muted_color);
        cx.scene.draw_text(turns_label);
        y += line_height + 4.0;

        // Tokens grid
        let col_w = w / 2.0;

        // Input tokens
        let in_text = format!("{} in", format_tokens(self.session.input_tokens));
        let in_label = cx.text.layout(&in_text, Point::new(x, y), font_size, muted_color);
        cx.scene.draw_text(in_label);

        // Output tokens
        let out_text = format!("{} out", format_tokens(self.session.output_tokens));
        let out_label = cx.text.layout(&out_text, Point::new(x + col_w, y), font_size, muted_color);
        cx.scene.draw_text(out_label);
        y += line_height;

        // Cache read
        let cache_read_text = format!("{} cached", format_tokens(self.session.cache_read_tokens));
        let cache_read_label = cx.text.layout(&cache_read_text, Point::new(x, y), font_size, green_color);
        cx.scene.draw_text(cache_read_label);

        // Cache written
        let cache_write_text = format!("{} written", format_tokens(self.session.cache_creation_tokens));
        let cache_write_label = cx.text.layout(&cache_write_text, Point::new(x + col_w, y), font_size, orange_color);
        cx.scene.draw_text(cache_write_label);
        y += line_height + 4.0;

        // Duration
        let dur_text = format!("{} total", format_duration(self.session.duration_ms));
        let dur_label = cx.text.layout(&dur_text, Point::new(x, y), 9.0, muted_color);
        cx.scene.draw_text(dur_label);

        let api_dur_text = format!("{} api", format_duration(self.session.duration_api_ms));
        let api_dur_label = cx.text.layout(&api_dur_text, Point::new(x + col_w, y), 9.0, muted_color);
        cx.scene.draw_text(api_dur_label);
        y += line_height + 4.0;

        // Web searches
        if self.web_searches > 0 {
            let web_text = format!("{} web searches", self.web_searches);
            let web_label = cx.text.layout(&web_text, Point::new(x, y), font_size, cyan_color);
            cx.scene.draw_text(web_label);
        }
    }

    fn event(&mut self, _: &InputEvent, _: Bounds, _: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else if ms < 3_600_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m{}s", mins, secs)
    } else {
        let hours = ms / 3_600_000;
        let mins = (ms % 3_600_000) / 60_000;
        format!("{}h{}m", hours, mins)
    }
}
