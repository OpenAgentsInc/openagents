use crate::components::atoms::ToolStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::ThinkingBlock;
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexEventTone {
    Info,
    Success,
    Warning,
    Error,
    Neutral,
}

impl CodexEventTone {
    fn color(self) -> Hsla {
        match self {
            Self::Info => theme::accent::PRIMARY,
            Self::Success => theme::status::SUCCESS,
            Self::Warning => theme::status::WARNING,
            Self::Error => theme::status::ERROR,
            Self::Neutral => theme::text::MUTED,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexEventLine {
    pub label: String,
    pub value: String,
}

impl CodexEventLine {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

pub struct CodexEventCard {
    id: Option<ComponentId>,
    title: String,
    subtitle: Option<String>,
    tag: Option<String>,
    tone: CodexEventTone,
    lines: Vec<CodexEventLine>,
}

impl CodexEventCard {
    const TITLE_HEIGHT: f32 = 20.0;
    const LINE_HEIGHT: f32 = 16.0;

    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            title: title.into(),
            subtitle: None,
            tag: None,
            tone: CodexEventTone::Info,
            lines: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn tone(mut self, tone: CodexEventTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn line(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.lines.push(CodexEventLine::new(label, value));
        self
    }

    pub fn lines(mut self, lines: Vec<CodexEventLine>) -> Self {
        self.lines = lines;
        self
    }
}

impl Default for CodexEventCard {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for CodexEventCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        let accent_width = 3.0;
        let accent_color = self.tone.color();

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                accent_width,
                bounds.size.height,
            ))
            .with_background(accent_color),
        );

        let content_x = bounds.origin.x + padding + accent_width;
        let content_width = (bounds.size.width - padding * 2.0 - accent_width).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut title_text = Text::new(&self.title)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        title_text.paint(
            Bounds::new(content_x, y, content_width, Self::TITLE_HEIGHT),
            cx,
        );

        if let Some(tag) = &self.tag {
            let font_size = theme::font_size::XS;
            let tag_width = tag.len() as f32 * font_size * 0.6 + padding * 1.5;
            let tag_x = content_x + content_width - tag_width;
            let tag_bounds = Bounds::new(tag_x, y + 2.0, tag_width, 16.0);
            cx.scene.draw_quad(
                Quad::new(tag_bounds)
                    .with_background(accent_color.with_alpha(0.2))
                    .with_border(accent_color, 1.0),
            );
            let tag_run = cx.text.layout_mono(
                tag,
                Point::new(tag_x + padding * 0.5, y + 4.0),
                font_size,
                accent_color,
            );
            cx.scene.draw_text(tag_run);
        }

        y += Self::TITLE_HEIGHT + theme::spacing::XS;

        if let Some(subtitle) = &self.subtitle {
            let mut subtitle_text = Text::new(subtitle)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED)
                .no_wrap();
            subtitle_text.paint(
                Bounds::new(content_x, y, content_width, Self::LINE_HEIGHT),
                cx,
            );
            y += Self::LINE_HEIGHT + theme::spacing::XS;
        }

        for line in &self.lines {
            let line_text = format!("{}: {}", line.label, line.value);
            let mut text = Text::new(line_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::SECONDARY)
                .no_wrap();
            text.paint(
                Bounds::new(content_x, y, content_width, Self::LINE_HEIGHT),
                cx,
            );
            y += Self::LINE_HEIGHT;
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
        let padding = theme::spacing::SM;
        let mut height = padding * 2.0 + Self::TITLE_HEIGHT;
        if self.subtitle.is_some() {
            height += Self::LINE_HEIGHT + theme::spacing::XS;
        }
        if !self.lines.is_empty() {
            height += self.lines.len() as f32 * Self::LINE_HEIGHT;
        }
        (None, Some(height))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

impl CodexPlanStepStatus {
    fn color(self) -> Hsla {
        match self {
            Self::Pending => theme::text::MUTED,
            Self::InProgress => theme::status::WARNING,
            Self::Completed => theme::status::SUCCESS,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodexPlanStep {
    pub step: String,
    pub status: CodexPlanStepStatus,
}

impl CodexPlanStep {
    pub fn new(step: impl Into<String>, status: CodexPlanStepStatus) -> Self {
        Self {
            step: step.into(),
            status,
        }
    }
}

pub struct CodexPlanCard {
    id: Option<ComponentId>,
    explanation: Option<String>,
    steps: Vec<CodexPlanStep>,
}

impl CodexPlanCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const LINE_HEIGHT: f32 = 18.0;

    pub fn new() -> Self {
        Self {
            id: None,
            explanation: None,
            steps: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    pub fn steps(mut self, steps: Vec<CodexPlanStep>) -> Self {
        self.steps = steps;
        self
    }
}

impl Default for CodexPlanCard {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CodexPlanCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("Plan update")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        if let Some(explanation) = &self.explanation {
            let mut explanation_text = Text::new(explanation)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED)
                .no_wrap();
            explanation_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
            y += Self::LINE_HEIGHT + theme::spacing::SM;
        }

        for step in &self.steps {
            let dot_size = 6.0;
            let dot_y = y + (Self::LINE_HEIGHT - dot_size) / 2.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, dot_y, dot_size, dot_size))
                    .with_background(step.status.color())
                    .with_corner_radius(dot_size / 2.0),
            );

            let step_text = format!("{} ({})", step.step, step.status.label());
            let mut text = Text::new(step_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::SECONDARY)
                .no_wrap();
            text.paint(
                Bounds::new(x + dot_size + 8.0, y, content_width, Self::LINE_HEIGHT),
                cx,
            );
            y += Self::LINE_HEIGHT;
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
        let padding = theme::spacing::SM;
        let mut height = padding * 2.0 + Self::HEADER_HEIGHT;
        if self.explanation.is_some() {
            height += Self::LINE_HEIGHT + theme::spacing::SM;
        }
        height += self.steps.len() as f32 * Self::LINE_HEIGHT;
        (None, Some(height))
    }
}

pub struct CodexTokenUsageCard {
    id: Option<ComponentId>,
    input_tokens: i32,
    cached_input_tokens: i32,
    output_tokens: i32,
}

impl CodexTokenUsageCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const ROW_HEIGHT: f32 = 22.0;
    const BAR_HEIGHT: f32 = 6.0;

    pub fn new(input_tokens: i32, cached_input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            id: None,
            input_tokens,
            cached_input_tokens,
            output_tokens,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Default for CodexTokenUsageCard {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

impl Component for CodexTokenUsageCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("Token usage")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        let total =
            (self.input_tokens + self.cached_input_tokens + self.output_tokens).max(1) as f32;
        let rows = [
            ("Input", self.input_tokens, theme::accent::PRIMARY),
            ("Cached", self.cached_input_tokens, theme::accent::SECONDARY),
            ("Output", self.output_tokens, theme::status::SUCCESS),
        ];

        for (label, value, color) in rows {
            let value_text = format!("{}", value);
            let label_run = cx.text.layout_mono(
                label,
                Point::new(x, y + 4.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            let value_width = value_text.len() as f32 * theme::font_size::XS * 0.6;
            let value_run = cx.text.layout_mono(
                &value_text,
                Point::new(x + content_width - value_width, y + 4.0),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(value_run);

            let bar_y = y + Self::ROW_HEIGHT - Self::BAR_HEIGHT;
            let bar_bounds = Bounds::new(x, bar_y, content_width, Self::BAR_HEIGHT);
            cx.scene
                .draw_quad(Quad::new(bar_bounds).with_background(theme::bg::MUTED));
            let ratio = (value as f32 / total).min(1.0);
            let fill_bounds = Bounds::new(x, bar_y, content_width * ratio, Self::BAR_HEIGHT);
            cx.scene
                .draw_quad(Quad::new(fill_bounds).with_background(color));

            y += Self::ROW_HEIGHT;
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
        let padding = theme::spacing::SM;
        let height =
            padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS + Self::ROW_HEIGHT * 3.0;
        (None, Some(height))
    }
}

#[derive(Debug, Clone)]
pub struct CodexRateLimitWindow {
    pub label: String,
    pub used_percent: i32,
    pub resets_at: Option<String>,
}

impl CodexRateLimitWindow {
    pub fn new(label: impl Into<String>, used_percent: i32) -> Self {
        Self {
            label: label.into(),
            used_percent,
            resets_at: None,
        }
    }

    pub fn resets_at(mut self, value: impl Into<String>) -> Self {
        self.resets_at = Some(value.into());
        self
    }
}

pub struct CodexRateLimitCard {
    id: Option<ComponentId>,
    plan_label: Option<String>,
    credits_label: Option<String>,
    windows: Vec<CodexRateLimitWindow>,
}

impl CodexRateLimitCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const ROW_HEIGHT: f32 = 26.0;
    const BAR_HEIGHT: f32 = 6.0;

    pub fn new() -> Self {
        Self {
            id: None,
            plan_label: None,
            credits_label: None,
            windows: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn plan_label(mut self, plan: impl Into<String>) -> Self {
        self.plan_label = Some(plan.into());
        self
    }

    pub fn credits_label(mut self, credits: impl Into<String>) -> Self {
        self.credits_label = Some(credits.into());
        self
    }

    pub fn windows(mut self, windows: Vec<CodexRateLimitWindow>) -> Self {
        self.windows = windows;
        self
    }
}

impl Default for CodexRateLimitCard {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CodexRateLimitCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("Rate limits")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        if let Some(plan) = &self.plan_label {
            let mut plan_text = Text::new(format!("Plan: {}", plan))
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED)
                .no_wrap();
            plan_text.paint(Bounds::new(x, y, content_width, 16.0), cx);
            y += 16.0 + theme::spacing::XS;
        }

        if let Some(credits) = &self.credits_label {
            let mut credits_text = Text::new(format!("Credits: {}", credits))
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED)
                .no_wrap();
            credits_text.paint(Bounds::new(x, y, content_width, 16.0), cx);
            y += 16.0 + theme::spacing::XS;
        }

        for window in &self.windows {
            let label_run = cx.text.layout_mono(
                &window.label,
                Point::new(x, y + 4.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            let value_text = format!("{}%", window.used_percent);
            let value_width = value_text.len() as f32 * theme::font_size::XS * 0.6;
            let value_run = cx.text.layout_mono(
                &value_text,
                Point::new(x + content_width - value_width, y + 4.0),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(value_run);

            let bar_y = y + Self::ROW_HEIGHT - Self::BAR_HEIGHT;
            let bar_bounds = Bounds::new(x, bar_y, content_width, Self::BAR_HEIGHT);
            cx.scene
                .draw_quad(Quad::new(bar_bounds).with_background(theme::bg::MUTED));

            let ratio = (window.used_percent as f32 / 100.0).min(1.0);
            let color = if window.used_percent > 85 {
                theme::status::ERROR
            } else if window.used_percent > 65 {
                theme::status::WARNING
            } else {
                theme::status::SUCCESS
            };
            let fill_bounds = Bounds::new(x, bar_y, content_width * ratio, Self::BAR_HEIGHT);
            cx.scene
                .draw_quad(Quad::new(fill_bounds).with_background(color));

            if let Some(resets_at) = &window.resets_at {
                let reset_run = cx.text.layout_mono(
                    resets_at,
                    Point::new(x, y + Self::ROW_HEIGHT + 2.0),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(reset_run);
                y += Self::ROW_HEIGHT + 14.0;
            } else {
                y += Self::ROW_HEIGHT;
            }
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
        let padding = theme::spacing::SM;
        let mut height = padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS;
        if self.plan_label.is_some() {
            height += 16.0 + theme::spacing::XS;
        }
        if self.credits_label.is_some() {
            height += 16.0 + theme::spacing::XS;
        }
        for window in &self.windows {
            height += Self::ROW_HEIGHT;
            if window.resets_at.is_some() {
                height += 14.0;
            }
        }
        (None, Some(height))
    }
}

pub struct CodexReasoningCard {
    id: Option<ComponentId>,
    summary: Option<ThinkingBlock>,
    content: Option<ThinkingBlock>,
}

impl CodexReasoningCard {
    const HEADER_HEIGHT: f32 = 20.0;

    pub fn new(summary: Option<String>, content: Option<String>) -> Self {
        let summary_block = summary.map(|text| ThinkingBlock::new(text));
        let content_block = content.map(|text| ThinkingBlock::new(text));
        Self {
            id: None,
            summary: summary_block,
            content: content_block,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn summary_expanded(mut self, expanded: bool) -> Self {
        if let Some(block) = self.summary.as_mut() {
            block.set_expanded(expanded);
        }
        self
    }

    pub fn content_expanded(mut self, expanded: bool) -> Self {
        if let Some(block) = self.content.as_mut() {
            block.set_expanded(expanded);
        }
        self
    }

    fn layout_blocks(&self, bounds: Bounds) -> (Option<Bounds>, Option<Bounds>) {
        let padding = theme::spacing::SM;
        let x = bounds.origin.x + padding;
        let width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding + Self::HEADER_HEIGHT + theme::spacing::XS;

        let summary_bounds = self.summary.as_ref().map(|block| {
            let height = block.size_hint().1.unwrap_or(0.0);
            let bounds = Bounds::new(x, y, width, height);
            y += height + theme::spacing::SM;
            bounds
        });

        let content_bounds = self.content.as_ref().map(|block| {
            let height = block.size_hint().1.unwrap_or(0.0);
            Bounds::new(x, y, width, height)
        });

        (summary_bounds, content_bounds)
    }
}

impl Default for CodexReasoningCard {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Component for CodexReasoningCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let header_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            Self::HEADER_HEIGHT,
        );
        let mut header = Text::new("Reasoning")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(header_bounds, cx);

        let (summary_bounds, content_bounds) = self.layout_blocks(bounds);
        if let (Some(bounds), Some(block)) = (summary_bounds, self.summary.as_mut()) {
            block.paint(bounds, cx);
        }
        if let (Some(bounds), Some(block)) = (content_bounds, self.content.as_mut()) {
            block.paint(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let (summary_bounds, content_bounds) = self.layout_blocks(bounds);
        let mut handled = false;

        if let (Some(bounds), Some(block)) = (summary_bounds, self.summary.as_mut()) {
            handled |= matches!(block.event(event, bounds, cx), EventResult::Handled);
        }
        if let (Some(bounds), Some(block)) = (content_bounds, self.content.as_mut()) {
            handled |= matches!(block.event(event, bounds, cx), EventResult::Handled);
        }

        if handled {
            EventResult::Handled
        } else {
            EventResult::Ignored
        }
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let padding = theme::spacing::SM;
        let mut height = padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS;
        if let Some(block) = &self.summary {
            height += block.size_hint().1.unwrap_or(0.0) + theme::spacing::SM;
        }
        if let Some(block) = &self.content {
            height += block.size_hint().1.unwrap_or(0.0);
        }
        (None, Some(height))
    }
}

pub struct CodexMcpToolCallCard {
    id: Option<ComponentId>,
    server: String,
    tool: String,
    status: ToolStatus,
    message: Option<String>,
}

impl CodexMcpToolCallCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const LINE_HEIGHT: f32 = 16.0;

    pub fn new(server: impl Into<String>, tool: impl Into<String>) -> Self {
        Self {
            id: None,
            server: server.into(),
            tool: tool.into(),
            status: ToolStatus::Pending,
            message: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: ToolStatus) -> Self {
        self.status = status;
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    fn status_color(&self) -> Hsla {
        match self.status {
            ToolStatus::Pending => theme::text::MUTED,
            ToolStatus::Running => theme::accent::PRIMARY,
            ToolStatus::Success => theme::status::SUCCESS,
            ToolStatus::Error => theme::status::ERROR,
            ToolStatus::Cancelled => theme::text::MUTED,
        }
    }
}

impl Default for CodexMcpToolCallCard {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl Component for CodexMcpToolCallCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let header_text = format!("MCP {} / {}", self.server, self.tool);
        let mut header = Text::new(header_text)
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);

        let dot_size = 6.0;
        let dot_x = x + content_width - dot_size;
        let dot_y = y + (Self::HEADER_HEIGHT - dot_size) / 2.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(dot_x, dot_y, dot_size, dot_size))
                .with_background(self.status_color())
                .with_corner_radius(dot_size / 2.0),
        );
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        let status_line = format!("status: {:?}", self.status);
        let mut status_text = Text::new(status_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .no_wrap();
        status_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
        y += Self::LINE_HEIGHT + theme::spacing::XS;

        if let Some(message) = &self.message {
            let mut msg_text = Text::new(message)
                .font_size(theme::font_size::XS)
                .color(theme::text::SECONDARY)
                .no_wrap();
            msg_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
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
        let padding = theme::spacing::SM;
        let mut height =
            padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS + Self::LINE_HEIGHT;
        if self.message.is_some() {
            height += Self::LINE_HEIGHT + theme::spacing::XS;
        }
        (None, Some(height))
    }
}

pub struct CodexTerminalInteractionCard {
    id: Option<ComponentId>,
    process_id: String,
    stdin: String,
}

impl CodexTerminalInteractionCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const LINE_HEIGHT: f32 = 16.0;

    pub fn new(process_id: impl Into<String>, stdin: impl Into<String>) -> Self {
        Self {
            id: None,
            process_id: process_id.into(),
            stdin: stdin.into(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Default for CodexTerminalInteractionCard {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl Component for CodexTerminalInteractionCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("Terminal input")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        let pid_line = format!("process: {}", self.process_id);
        let mut pid_text = Text::new(pid_line)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED)
            .no_wrap();
        pid_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
        y += Self::LINE_HEIGHT + theme::spacing::XS;

        let mut stdin_text = Text::new(format!("stdin: {}", self.stdin))
            .font_size(theme::font_size::XS)
            .color(theme::text::SECONDARY)
            .no_wrap();
        stdin_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
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
        let padding = theme::spacing::SM;
        let height =
            padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS + Self::LINE_HEIGHT * 2.0;
        (None, Some(height))
    }
}

pub struct CodexRawResponseCard {
    id: Option<ComponentId>,
    payload: String,
}

impl CodexRawResponseCard {
    const HEADER_HEIGHT: f32 = 20.0;
    const LINE_HEIGHT: f32 = 16.0;

    pub fn new(payload: impl Into<String>) -> Self {
        Self {
            id: None,
            payload: payload.into(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Default for CodexRawResponseCard {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for CodexRawResponseCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let x = bounds.origin.x + padding;
        let content_width = (bounds.size.width - padding * 2.0).max(0.0);
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("Raw response item")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY)
            .no_wrap();
        header.paint(Bounds::new(x, y, content_width, Self::HEADER_HEIGHT), cx);
        y += Self::HEADER_HEIGHT + theme::spacing::XS;

        let mut payload_text = Text::new(&self.payload)
            .font_size(theme::font_size::XS)
            .color(theme::text::SECONDARY)
            .no_wrap();
        payload_text.paint(Bounds::new(x, y, content_width, Self::LINE_HEIGHT), cx);
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
        let padding = theme::spacing::SM;
        let height = padding * 2.0 + Self::HEADER_HEIGHT + theme::spacing::XS + Self::LINE_HEIGHT;
        (None, Some(height))
    }
}
