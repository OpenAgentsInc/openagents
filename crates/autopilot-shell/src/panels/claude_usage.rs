//! Claude Usage component - shows API usage stats
//!
//! Displays live usage data from Claude API response headers.
//! Rate limits come from headers like `x-codex-primary-used-percent`.

use autopilot_service::SdkSessionIds;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
};

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

/// Usage limit info from API response headers
#[derive(Clone)]
pub struct UsageLimit {
    pub name: String,
    pub percent_used: f64,
    pub resets_at: String,
}

/// Claude usage display component
///
/// Shows live usage data from API responses. All fields start at zero
/// and are updated via setter methods when real data is available.
pub struct ClaudeUsage {
    pub model: String,
    pub context_used: u64,
    pub context_total: u64,
    pub session: SessionUsage,
    pub limits: Vec<UsageLimit>,
    pub web_searches: u64,
    pub autopilot_session_id: Option<String>,
    pub sdk_session_ids: SdkSessionIds,
}

impl ClaudeUsage {
    pub fn new() -> Self {
        // Start with empty/zero state - NO placeholder data
        Self {
            model: String::new(),
            context_used: 0,
            context_total: 0,
            session: SessionUsage::default(),
            limits: Vec::new(),
            web_searches: 0,
            autopilot_session_id: None,
            sdk_session_ids: SdkSessionIds::default(),
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

    /// Update rate limits from API response headers
    pub fn set_limits(&mut self, limits: Vec<UsageLimit>) {
        self.limits = limits;
    }

    /// Update a single session stat
    #[allow(dead_code)]
    pub fn add_tokens(&mut self, input: u64, output: u64, cache_read: u64, cache_create: u64) {
        self.session.input_tokens += input;
        self.session.output_tokens += output;
        self.session.cache_read_tokens += cache_read;
        self.session.cache_creation_tokens += cache_create;
    }

    #[allow(dead_code)]
    pub fn increment_turns(&mut self) {
        self.session.num_turns += 1;
    }

    #[allow(dead_code)]
    pub fn set_web_searches(&mut self, count: u64) {
        self.web_searches = count;
    }

    pub fn set_session_ids(
        &mut self,
        autopilot_session_id: String,
        sdk_session_ids: SdkSessionIds,
    ) {
        if autopilot_session_id.is_empty() {
            self.autopilot_session_id = None;
        } else {
            self.autopilot_session_id = Some(autopilot_session_id);
        }
        self.sdk_session_ids = sdk_session_ids;
    }
}

impl Default for ClaudeUsage {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ClaudeUsage {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);

        // Divider line at top
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                1.0,
            ))
            .with_background(line_color),
        );

        let padding = 8.0;
        let mut y = bounds.origin.y + padding + 4.0;
        let x = bounds.origin.x;
        let w = bounds.size.width;

        let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
        let value_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
        let muted_color = Hsla::new(0.0, 0.0, 0.4, 1.0);
        // Hue is 0.0-1.0 range (not degrees): green=0.389, cyan=0.5, orange=0.083, yellow=0.125
        let green_color = Hsla::new(0.389, 0.7, 0.5, 1.0);
        let cyan_color = Hsla::new(0.5, 0.6, 0.6, 1.0);
        let orange_color = Hsla::new(0.083, 0.8, 0.55, 1.0);

        let font_size = 10.0;
        let line_height = 14.0;

        // Header
        let header = cx
            .text
            .layout("CLAUDE USAGE", Point::new(x, y), font_size, label_color);
        cx.scene.draw_text(header);
        y += line_height + 4.0;

        // Model
        let model_text = cx
            .text
            .layout(&self.model, Point::new(x, y), 11.0, value_color);
        cx.scene.draw_text(model_text);
        y += line_height + 8.0;

        // Context window progress bar
        let ctx_pct = if self.context_total > 0 {
            (self.context_used as f64 / self.context_total as f64) * 100.0
        } else {
            0.0
        };

        let ctx_label = cx
            .text
            .layout("Context", Point::new(x, y), font_size, muted_color);
        cx.scene.draw_text(ctx_label);
        let pct_text = format!("{:.0}%", ctx_pct);
        let pct_label = cx.text.layout(
            &pct_text,
            Point::new(x + w - 30.0, y),
            font_size,
            muted_color,
        );
        cx.scene.draw_text(pct_label);
        y += line_height;

        // Progress bar background
        let bar_h = 4.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y, w, bar_h)).with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
        );
        // Progress bar fill (hue 0-1: yellow=0.125, red=0.0)
        let bar_color = if ctx_pct < 50.0 {
            green_color
        } else if ctx_pct < 75.0 {
            Hsla::new(0.125, 0.8, 0.5, 1.0) // yellow
        } else {
            Hsla::new(0.0, 0.8, 0.5, 1.0) // red
        };
        let fill_w = (w * ctx_pct as f32 / 100.0).min(w);
        cx.scene
            .draw_quad(Quad::new(Bounds::new(x, y, fill_w, bar_h)).with_background(bar_color));
        y += bar_h + 2.0;

        // Context tokens
        let ctx_tokens = format!(
            "{} / {}",
            format_tokens(self.context_used),
            format_tokens(self.context_total)
        );
        let ctx_tok_label = cx
            .text
            .layout(&ctx_tokens, Point::new(x, y), 9.0, muted_color);
        cx.scene.draw_text(ctx_tok_label);
        y += line_height + 8.0;

        // Usage limits
        for limit in &self.limits {
            let limit_label = cx
                .text
                .layout(&limit.name, Point::new(x, y), font_size, muted_color);
            cx.scene.draw_text(limit_label);
            y += line_height;

            // Progress bar (hue 0-1: yellow=0.125, red=0.0)
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, y, w, bar_h))
                    .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
            );
            let limit_fill = (w * limit.percent_used as f32 / 100.0).min(w);
            let limit_color = if limit.percent_used < 50.0 {
                green_color
            } else if limit.percent_used < 75.0 {
                Hsla::new(0.125, 0.8, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.8, 0.5, 1.0)
            };
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, y, limit_fill, bar_h)).with_background(limit_color),
            );
            y += bar_h + 2.0;

            let limit_info = format!(
                "{:.0}% used · Resets {}",
                limit.percent_used, limit.resets_at
            );
            let limit_info_label = cx
                .text
                .layout(&limit_info, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(limit_info_label);
            y += line_height + 6.0;
        }

        // Session divider
        y += 4.0;

        // Session header
        let session_header = cx
            .text
            .layout("SESSION", Point::new(x, y), font_size, label_color);
        cx.scene.draw_text(session_header);
        y += line_height + 4.0;

        // Cost and turns
        let cost_text = format!("${:.4}", self.session.total_cost_usd);
        let cost_label = cx
            .text
            .layout(&cost_text, Point::new(x, y), 11.0, value_color);
        cx.scene.draw_text(cost_label);
        let turns_text = format!("{} turns", self.session.num_turns);
        let turns_label =
            cx.text
                .layout(&turns_text, Point::new(x + 70.0, y), font_size, muted_color);
        cx.scene.draw_text(turns_label);
        y += line_height + 4.0;

        // Tokens grid
        let col_w = w / 2.0;

        // Input tokens
        let in_text = format!("{} in", format_tokens(self.session.input_tokens));
        let in_label = cx
            .text
            .layout(&in_text, Point::new(x, y), font_size, muted_color);
        cx.scene.draw_text(in_label);

        // Output tokens
        let out_text = format!("{} out", format_tokens(self.session.output_tokens));
        let out_label = cx
            .text
            .layout(&out_text, Point::new(x + col_w, y), font_size, muted_color);
        cx.scene.draw_text(out_label);
        y += line_height;

        // Cache read
        let cache_read_text = format!("{} cached", format_tokens(self.session.cache_read_tokens));
        let cache_read_label =
            cx.text
                .layout(&cache_read_text, Point::new(x, y), font_size, green_color);
        cx.scene.draw_text(cache_read_label);

        // Cache written
        let cache_write_text = format!(
            "{} written",
            format_tokens(self.session.cache_creation_tokens)
        );
        let cache_write_label = cx.text.layout(
            &cache_write_text,
            Point::new(x + col_w, y),
            font_size,
            orange_color,
        );
        cx.scene.draw_text(cache_write_label);
        y += line_height + 4.0;

        // Duration
        let dur_text = format!("{} total", format_duration(self.session.duration_ms));
        let dur_label = cx
            .text
            .layout(&dur_text, Point::new(x, y), 9.0, muted_color);
        cx.scene.draw_text(dur_label);

        let api_dur_text = format!("{} api", format_duration(self.session.duration_api_ms));
        let api_dur_label =
            cx.text
                .layout(&api_dur_text, Point::new(x + col_w, y), 9.0, muted_color);
        cx.scene.draw_text(api_dur_label);
        y += line_height + 4.0;

        // Session IDs
        let session_header = cx
            .text
            .layout("SESSIONS", Point::new(x, y), font_size, label_color);
        cx.scene.draw_text(session_header);
        y += line_height + 4.0;

        let autopilot_id = self.autopilot_session_id.as_deref().unwrap_or("-");
        let autopilot_line = format!("Autopilot: {}", format_session_id(autopilot_id));
        let autopilot_label = cx
            .text
            .layout(&autopilot_line, Point::new(x, y), 9.0, muted_color);
        cx.scene.draw_text(autopilot_label);
        y += line_height;

        if let Some(ref plan_id) = self.sdk_session_ids.plan {
            let plan_line = format!("Plan: {}", format_session_id(plan_id));
            let plan_label = cx
                .text
                .layout(&plan_line, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(plan_label);
            y += line_height;
        }

        if let Some(ref exec_id) = self.sdk_session_ids.exec {
            let exec_line = format!("Exec: {}", format_session_id(exec_id));
            let exec_label = cx
                .text
                .layout(&exec_line, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(exec_label);
            y += line_height;
        }

        if let Some(ref review_id) = self.sdk_session_ids.review {
            let review_line = format!("Review: {}", format_session_id(review_id));
            let review_label = cx
                .text
                .layout(&review_line, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(review_label);
            y += line_height;
        }

        if let Some(ref fix_id) = self.sdk_session_ids.fix {
            let fix_line = format!("Fix: {}", format_session_id(fix_id));
            let fix_label = cx
                .text
                .layout(&fix_line, Point::new(x, y), 9.0, muted_color);
            cx.scene.draw_text(fix_label);
            y += line_height;
        }

        // Web searches
        if self.web_searches > 0 {
            let web_text = format!("{} web searches", self.web_searches);
            let web_label = cx
                .text
                .layout(&web_text, Point::new(x, y), font_size, cyan_color);
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

fn format_session_id(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return "-".to_string();
    }
    if trimmed.len() <= 16 {
        return trimmed.to_string();
    }
    let prefix = &trimmed[..8];
    let suffix = &trimmed[trimmed.len() - 4..];
    format!("{}...{}", prefix, suffix)
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
