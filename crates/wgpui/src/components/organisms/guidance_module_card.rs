use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult, Text};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone)]
pub struct GuidanceField {
    pub label: String,
    pub value: String,
}

impl GuidanceField {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidanceGoal {
    pub intent: String,
    pub success_criteria: Vec<String>,
}

impl GuidanceGoal {
    pub fn new(intent: impl Into<String>) -> Self {
        Self {
            intent: intent.into(),
            success_criteria: Vec::new(),
        }
    }

    pub fn success_criteria(mut self, criteria: Vec<String>) -> Self {
        self.success_criteria = criteria;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceNetwork {
    None,
    Scoped,
    Full,
}

impl GuidanceNetwork {
    pub fn label(self) -> &'static str {
        match self {
            GuidanceNetwork::None => "None",
            GuidanceNetwork::Scoped => "Scoped",
            GuidanceNetwork::Full => "Full",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidancePermissions {
    pub can_exec: bool,
    pub can_write: bool,
    pub network: GuidanceNetwork,
}

impl GuidancePermissions {
    pub fn new(can_exec: bool, can_write: bool, network: GuidanceNetwork) -> Self {
        Self {
            can_exec,
            can_write,
            network,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidanceState {
    pub turn_count: u32,
    pub no_progress_count: u32,
    pub tokens_remaining: Option<u64>,
    pub time_remaining_ms: Option<u64>,
}

impl GuidanceState {
    pub fn new(turn_count: u32, no_progress_count: u32) -> Self {
        Self {
            turn_count,
            no_progress_count,
            tokens_remaining: None,
            time_remaining_ms: None,
        }
    }

    pub fn tokens_remaining(mut self, tokens: Option<u64>) -> Self {
        self.tokens_remaining = tokens;
        self
    }

    pub fn time_remaining_ms(mut self, ms: Option<u64>) -> Self {
        self.time_remaining_ms = ms;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceAction {
    Continue,
    Pause,
    Stop,
    Review,
}

impl GuidanceAction {
    pub fn label(self) -> &'static str {
        match self {
            GuidanceAction::Continue => "Continue",
            GuidanceAction::Pause => "Pause",
            GuidanceAction::Stop => "Stop",
            GuidanceAction::Review => "Review",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidanceDecision {
    pub action: GuidanceAction,
    pub reason: String,
    pub confidence: f32,
    pub next_input: Option<String>,
    pub tags: Vec<String>,
}

impl GuidanceDecision {
    pub fn new(action: GuidanceAction, reason: impl Into<String>, confidence: f32) -> Self {
        Self {
            action,
            reason: reason.into(),
            confidence: confidence.clamp(0.0, 1.0),
            next_input: None,
            tags: Vec::new(),
        }
    }

    pub fn next_input(mut self, input: impl Into<String>) -> Self {
        self.next_input = Some(input.into());
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceGuardrailStatus {
    Clear,
    Triggered,
}

impl GuidanceGuardrailStatus {
    pub fn label(self) -> &'static str {
        match self {
            GuidanceGuardrailStatus::Clear => "Clear",
            GuidanceGuardrailStatus::Triggered => "Triggered",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GuidanceGuardrail {
    pub name: String,
    pub status: GuidanceGuardrailStatus,
    pub detail: Option<String>,
}

impl GuidanceGuardrail {
    pub fn new(name: impl Into<String>, status: GuidanceGuardrailStatus) -> Self {
        Self {
            name: name.into(),
            status,
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct GuidanceAudit {
    pub input_hash: String,
    pub output_hash: String,
    pub model: String,
    pub package: String,
}

impl GuidanceAudit {
    pub fn new(
        input_hash: impl Into<String>,
        output_hash: impl Into<String>,
        model: impl Into<String>,
        package: impl Into<String>,
    ) -> Self {
        Self {
            input_hash: input_hash.into(),
            output_hash: output_hash.into(),
            model: model.into(),
            package: package.into(),
        }
    }
}

pub struct GuidanceModuleCard {
    id: Option<ComponentId>,
    title: String,
    goal: Option<GuidanceGoal>,
    summary: Vec<GuidanceField>,
    state: Option<GuidanceState>,
    permissions: Option<GuidancePermissions>,
    decision: Option<GuidanceDecision>,
    guardrails: Vec<GuidanceGuardrail>,
    audit: Option<GuidanceAudit>,
}

impl GuidanceModuleCard {
    const HEADER_HEIGHT: f32 = 32.0;
    const SECTION_TITLE_HEIGHT: f32 = 18.0;
    const ROW_GAP: f32 = 8.0;
    const SECTION_GAP: f32 = 18.0;
    const LABEL_COL_WIDTH: f32 = 130.0;
    const MIN_ROW_HEIGHT: f32 = 20.0;

    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: None,
            title: title.into(),
            goal: None,
            summary: Vec::new(),
            state: None,
            permissions: None,
            decision: None,
            guardrails: Vec::new(),
            audit: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn goal(mut self, goal: GuidanceGoal) -> Self {
        self.goal = Some(goal);
        self
    }

    pub fn summary(mut self, summary: Vec<GuidanceField>) -> Self {
        self.summary = summary;
        self
    }

    pub fn state(mut self, state: GuidanceState) -> Self {
        self.state = Some(state);
        self
    }

    pub fn permissions(mut self, permissions: GuidancePermissions) -> Self {
        self.permissions = Some(permissions);
        self
    }

    pub fn decision(mut self, decision: GuidanceDecision) -> Self {
        self.decision = Some(decision);
        self
    }

    pub fn guardrails(mut self, guardrails: Vec<GuidanceGuardrail>) -> Self {
        self.guardrails = guardrails;
        self
    }

    pub fn audit(mut self, audit: GuidanceAudit) -> Self {
        self.audit = Some(audit);
        self
    }

    fn action_color(action: GuidanceAction) -> Hsla {
        match action {
            GuidanceAction::Continue => theme::status::SUCCESS,
            GuidanceAction::Pause => theme::status::WARNING,
            GuidanceAction::Stop => theme::status::ERROR,
            GuidanceAction::Review => theme::status::INFO,
        }
    }

    fn guardrail_color(status: GuidanceGuardrailStatus) -> Hsla {
        match status {
            GuidanceGuardrailStatus::Clear => theme::text::SECONDARY,
            GuidanceGuardrailStatus::Triggered => theme::status::WARNING,
        }
    }

    fn yes_no(value: bool) -> &'static str {
        if value { "Yes" } else { "No" }
    }

    fn format_time(ms: u64) -> String {
        let total_sec = ms / 1000;
        let minutes = total_sec / 60;
        let seconds = total_sec % 60;
        if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    fn draw_section_title(cx: &mut PaintContext, title: &str, x: f32, y: f32, width: f32) -> f32 {
        let mut text = Text::new(title)
            .font_size(theme::font_size::SM)
            .color(theme::text::SECONDARY)
            .no_wrap();
        text.paint(Bounds::new(x, y, width, Self::SECTION_TITLE_HEIGHT), cx);
        y + Self::SECTION_TITLE_HEIGHT + Self::ROW_GAP
    }

    fn draw_text_block(
        cx: &mut PaintContext,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        color: Hsla,
    ) -> f32 {
        let mut block = Text::new(text).font_size(theme::font_size::SM).color(color);
        let (_, height) = block.size_hint_with_width(width);
        let height = height.unwrap_or(Self::MIN_ROW_HEIGHT);
        block.paint(Bounds::new(x, y, width, height), cx);
        height
    }

    fn draw_key_value(
        cx: &mut PaintContext,
        label: &str,
        value: &str,
        x: f32,
        y: f32,
        width: f32,
        value_color: Hsla,
    ) -> f32 {
        let label_width = Self::LABEL_COL_WIDTH.min(width * 0.4).max(90.0);
        let label_run = cx.text.layout_mono(
            label,
            Point::new(x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(label_run);

        let value_x = x + label_width;
        let value_width = (width - label_width).max(0.0);
        let height = Self::draw_text_block(cx, value, value_x, y, value_width, value_color);
        height.max(Self::MIN_ROW_HEIGHT)
    }

    fn draw_tags_line(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(decision) = &self.decision else {
            return 0.0;
        };
        if decision.tags.is_empty() {
            return 0.0;
        }
        let tags = decision.tags.join(", ");
        Self::draw_key_value(cx, "Tags", &tags, x, y, width, theme::text::SECONDARY) + Self::ROW_GAP
    }

    fn draw_goal_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(goal) = &self.goal else {
            return y;
        };
        let mut cursor = Self::draw_section_title(cx, "Goal", x, y, width);
        cursor += Self::draw_key_value(
            cx,
            "Intent",
            &goal.intent,
            x,
            cursor,
            width,
            theme::text::PRIMARY,
        ) + Self::ROW_GAP;

        if !goal.success_criteria.is_empty() {
            let criteria = goal
                .success_criteria
                .iter()
                .map(|c| format!("• {}", c))
                .collect::<Vec<_>>()
                .join("\n");
            cursor += Self::draw_key_value(
                cx,
                "Criteria",
                &criteria,
                x,
                cursor,
                width,
                theme::text::SECONDARY,
            ) + Self::ROW_GAP;
        }
        cursor + Self::SECTION_GAP
    }

    fn draw_summary_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        if self.summary.is_empty() {
            return y;
        }
        let mut cursor = Self::draw_section_title(cx, "Turn Summary", x, y, width);
        for field in &self.summary {
            cursor += Self::draw_key_value(
                cx,
                &field.label,
                &field.value,
                x,
                cursor,
                width,
                theme::text::SECONDARY,
            ) + Self::ROW_GAP;
        }
        cursor + Self::SECTION_GAP
    }

    fn draw_state_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(state) = &self.state else {
            return y;
        };
        let mut cursor = Self::draw_section_title(cx, "State", x, y, width);
        cursor += Self::draw_key_value(
            cx,
            "Turns",
            &state.turn_count.to_string(),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "No progress",
            &state.no_progress_count.to_string(),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        let tokens = state
            .tokens_remaining
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string());
        cursor += Self::draw_key_value(
            cx,
            "Tokens",
            &tokens,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        let time = state
            .time_remaining_ms
            .map(Self::format_time)
            .unwrap_or_else(|| "—".to_string());
        cursor += Self::draw_key_value(
            cx,
            "Time left",
            &time,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor + Self::SECTION_GAP
    }

    fn draw_permissions_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(permissions) = &self.permissions else {
            return y;
        };
        let mut cursor = Self::draw_section_title(cx, "Permissions", x, y, width);
        cursor += Self::draw_key_value(
            cx,
            "Exec",
            Self::yes_no(permissions.can_exec),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Write",
            Self::yes_no(permissions.can_write),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Network",
            permissions.network.label(),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor + Self::SECTION_GAP
    }

    fn draw_decision_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(decision) = &self.decision else {
            return y;
        };
        let mut cursor = Self::draw_section_title(cx, "Decision", x, y, width);
        cursor += Self::draw_key_value(
            cx,
            "Action",
            decision.action.label(),
            x,
            cursor,
            width,
            theme::text::PRIMARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Confidence",
            &format!("{:.0}%", decision.confidence * 100.0),
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Reason",
            &decision.reason,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        if let Some(next_input) = &decision.next_input {
            cursor += Self::draw_key_value(
                cx,
                "Next input",
                next_input,
                x,
                cursor,
                width,
                theme::text::SECONDARY,
            ) + Self::ROW_GAP;
        }
        cursor += self.draw_tags_line(cx, x, cursor, width);
        cursor + Self::SECTION_GAP
    }

    fn draw_guardrails_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        if self.guardrails.is_empty() {
            return y;
        }
        let mut cursor = Self::draw_section_title(cx, "Guardrails", x, y, width);
        for guardrail in &self.guardrails {
            let mut value = guardrail.status.label().to_string();
            if let Some(detail) = &guardrail.detail {
                value = format!("{} · {}", value, detail);
            }
            cursor += Self::draw_key_value(
                cx,
                &guardrail.name,
                &value,
                x,
                cursor,
                width,
                Self::guardrail_color(guardrail.status),
            ) + Self::ROW_GAP;
        }
        cursor + Self::SECTION_GAP
    }

    fn draw_audit_section(&self, cx: &mut PaintContext, x: f32, y: f32, width: f32) -> f32 {
        let Some(audit) = &self.audit else {
            return y;
        };
        let mut cursor = Self::draw_section_title(cx, "Audit", x, y, width);
        cursor += Self::draw_key_value(
            cx,
            "Input hash",
            &audit.input_hash,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Decision hash",
            &audit.output_hash,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Model",
            &audit.model,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor += Self::draw_key_value(
            cx,
            "Package",
            &audit.package,
            x,
            cursor,
            width,
            theme::text::SECONDARY,
        ) + Self::ROW_GAP;
        cursor + Self::SECTION_GAP
    }
}

impl Component for GuidanceModuleCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::LG;
        let content = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            (bounds.size.width - padding * 2.0).max(0.0),
            (bounds.size.height - padding * 2.0).max(0.0),
        );

        cx.scene.push_clip(bounds);

        let mut title_text = Text::new(&self.title)
            .font_size(theme::font_size::LG)
            .color(theme::text::PRIMARY)
            .no_wrap();
        title_text.paint(
            Bounds::new(
                content.origin.x,
                content.origin.y,
                content.size.width,
                Self::HEADER_HEIGHT,
            ),
            cx,
        );

        if let Some(decision) = &self.decision {
            let label = format!(
                "{} · {:.0}%",
                decision.action.label(),
                decision.confidence * 100.0
            );
            let font_size = theme::font_size::XS;
            let padding_x = 10.0;
            let padding_y = 4.0;
            let text_width = label.chars().count() as f32 * font_size * 0.6;
            let badge_width = text_width + padding_x * 2.0;
            let badge_height = font_size + padding_y * 2.0;
            let badge_x = content.origin.x + content.size.width - badge_width;
            let badge_y = content.origin.y + 2.0;
            let color = Self::action_color(decision.action);
            cx.scene.draw_quad(
                Quad::new(Bounds::new(badge_x, badge_y, badge_width, badge_height))
                    .with_background(color.with_alpha(0.2))
                    .with_border(color, 1.0),
            );
            let label_run = cx.text.layout_mono(
                &label,
                Point::new(badge_x + padding_x, badge_y + padding_y + 1.0),
                font_size,
                color,
            );
            cx.scene.draw_text(label_run);
        }

        let start_y = content.origin.y + Self::HEADER_HEIGHT + theme::spacing::SM;
        let use_columns = content.size.width >= 960.0;

        if use_columns {
            let column_gap = theme::spacing::LG;
            let column_width = (content.size.width - column_gap).max(0.0) / 2.0;
            let left_x = content.origin.x;
            let right_x = content.origin.x + column_width + column_gap;

            let mut left_y = start_y;
            left_y = self.draw_goal_section(cx, left_x, left_y, column_width);
            left_y = self.draw_summary_section(cx, left_x, left_y, column_width);
            left_y = self.draw_state_section(cx, left_x, left_y, column_width);
            let _ = self.draw_permissions_section(cx, left_x, left_y, column_width);

            let mut right_y = start_y;
            right_y = self.draw_decision_section(cx, right_x, right_y, column_width);
            right_y = self.draw_guardrails_section(cx, right_x, right_y, column_width);
            let _ = self.draw_audit_section(cx, right_x, right_y, column_width);
        } else {
            let mut y = start_y;
            y = self.draw_goal_section(cx, content.origin.x, y, content.size.width);
            y = self.draw_summary_section(cx, content.origin.x, y, content.size.width);
            y = self.draw_state_section(cx, content.origin.x, y, content.size.width);
            y = self.draw_permissions_section(cx, content.origin.x, y, content.size.width);
            y = self.draw_decision_section(cx, content.origin.x, y, content.size.width);
            y = self.draw_guardrails_section(cx, content.origin.x, y, content.size.width);
            let _ = self.draw_audit_section(cx, content.origin.x, y, content.size.width);
        }

        cx.scene.pop_clip();
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
        let section_count = 6;
        let rows = self.summary.len() + self.guardrails.len().max(1) + 12;
        let height = theme::spacing::LG * 2.0
            + Self::HEADER_HEIGHT
            + (section_count as f32 * Self::SECTION_GAP)
            + (rows as f32 * (Self::MIN_ROW_HEIGHT + Self::ROW_GAP));
        (None, Some(height))
    }
}
