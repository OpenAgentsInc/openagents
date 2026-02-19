use serde_json::Value;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::organisms::{DiffToolCall, SearchToolCall, TerminalToolCall, ToolCallCard};
use wgpui::components::{Component, EventContext, EventResult, PaintContext};
use wgpui::{Bounds, InputEvent};

use super::parsing::{build_simple_diff, parse_diff_lines, parse_search_matches};

pub(crate) enum ToolDetail {
    None,
    Search(SearchToolCall),
    Terminal(TerminalToolCall),
    Diff(DiffToolCall),
}

impl ToolDetail {
    pub(crate) fn height(&self) -> f32 {
        match self {
            ToolDetail::None => 0.0,
            ToolDetail::Search(detail) => detail.size_hint().1.unwrap_or(0.0),
            ToolDetail::Terminal(detail) => detail.size_hint().1.unwrap_or(0.0),
            ToolDetail::Diff(detail) => detail.size_hint().1.unwrap_or(0.0),
        }
    }

    pub(crate) fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        match self {
            ToolDetail::None => {}
            ToolDetail::Search(detail) => detail.paint(bounds, cx),
            ToolDetail::Terminal(detail) => detail.paint(bounds, cx),
            ToolDetail::Diff(detail) => detail.paint(bounds, cx),
        }
    }

    pub(crate) fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        match self {
            ToolDetail::None => EventResult::Ignored,
            ToolDetail::Search(detail) => detail.event(event, bounds, cx),
            ToolDetail::Terminal(detail) => detail.event(event, bounds, cx),
            ToolDetail::Diff(detail) => detail.event(event, bounds, cx),
        }
    }
}

pub(crate) struct ToolVisualization {
    pub(crate) tool_use_id: String,
    pub(crate) name: String,
    pub(crate) tool_type: ToolType,
    pub(crate) status: ToolStatus,
    pub(crate) input: Option<String>,
    pub(crate) input_value: Option<Value>,
    pub(crate) output: Option<String>,
    pub(crate) output_value: Option<Value>,
    pub(crate) elapsed_secs: Option<f64>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) card_expanded: bool,
    pub(crate) card: ToolCallCard,
    pub(crate) detail: ToolDetail,
    /// Index of the message this tool is associated with (for inline rendering).
    pub(crate) message_index: usize,
}

impl ToolVisualization {
    pub(crate) fn new(
        tool_use_id: String,
        name: String,
        tool_type: ToolType,
        message_index: usize,
    ) -> Self {
        let card = ToolCallCard::new(tool_type, name.clone());
        let mut tool = Self {
            tool_use_id,
            name,
            tool_type,
            status: ToolStatus::Running,
            input: None,
            input_value: None,
            output: None,
            output_value: None,
            elapsed_secs: None,
            exit_code: None,
            card_expanded: false,
            card,
            detail: ToolDetail::None,
            message_index,
        };
        tool.refresh_components();
        tool
    }

    pub(crate) fn refresh_components(&mut self) {
        self.refresh_card();
        self.refresh_detail();
    }

    pub(crate) fn refresh_card(&mut self) {
        // For Task tools, combine name and description into the display name.
        let display_name = if self.tool_type == ToolType::Task {
            if let Some(input) = &self.input {
                format!("{} {}", self.name, input)
            } else {
                self.name.clone()
            }
        } else {
            self.name.clone()
        };

        let mut card = ToolCallCard::new(self.tool_type, display_name)
            .status(self.status)
            .expanded(self.card_expanded);
        // For Task tools, don't repeat input since it's already in the name.
        if self.tool_type != ToolType::Task {
            if let Some(input) = &self.input {
                card = card.input(input.clone());
            }
        }
        if let Some(output) = &self.output {
            card = card.output(output.clone());
        }
        if let Some(elapsed) = self.elapsed_secs {
            card = card.elapsed_secs(elapsed);
        }
        self.card = card;
    }

    pub(crate) fn refresh_detail(&mut self) {
        self.detail = build_tool_detail(self);
    }

    pub(crate) fn sync_expanded_from_card(&mut self) -> bool {
        let expanded = self.card.is_expanded();
        if expanded != self.card_expanded {
            self.card_expanded = expanded;
            self.refresh_detail();
            true
        } else {
            false
        }
    }
}

pub(crate) struct ToolPanelBlock {
    pub(crate) index: usize,
    pub(crate) card_bounds: Bounds,
    pub(crate) detail_bounds: Option<Bounds>,
}

fn build_tool_detail(tool: &ToolVisualization) -> ToolDetail {
    if !tool.card_expanded {
        return ToolDetail::None;
    }

    let status = tool.status;

    if tool.tool_type == ToolType::Bash {
        let command = tool
            .input_value
            .as_ref()
            .and_then(|value| {
                value
                    .get("command")
                    .or_else(|| value.get("cmd"))
                    .or_else(|| value.get("bash_id"))
                    .or_else(|| value.get("shell_id"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("bash")
            .to_string();
        let mut detail = TerminalToolCall::new(command).status(status).expanded(true);
        if let Some(output) = tool.output.as_ref() {
            if !output.is_empty() {
                detail = detail.output(output.clone());
            }
        }
        if let Some(code) = tool.exit_code {
            detail = detail.exit_code(code);
        }
        return ToolDetail::Terminal(detail);
    }

    if matches!(
        tool.tool_type,
        ToolType::Glob | ToolType::Grep | ToolType::Search
    ) {
        let query = tool
            .input_value
            .as_ref()
            .and_then(|value| {
                value
                    .get("pattern")
                    .or_else(|| value.get("query"))
                    .or_else(|| value.get("regex"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("")
            .to_string();
        let output_text = tool.output.as_deref().unwrap_or("");
        let matches = parse_search_matches(tool.output_value.as_ref(), output_text);
        let detail = SearchToolCall::new(query)
            .matches(matches)
            .status(status)
            .expanded(true);
        return ToolDetail::Search(detail);
    }

    if tool.tool_type == ToolType::Edit {
        let file_path = tool
            .input_value
            .as_ref()
            .and_then(|value| value.get("file_path").and_then(|v| v.as_str()))
            .unwrap_or("file")
            .to_string();
        let mut output_text = tool.output.as_deref();
        let mut output_storage = None::<String>;
        if output_text.map(|text| text.is_empty()).unwrap_or(true) {
            if let Some(value) = tool.output_value.as_ref() {
                if let Some(diff) = value
                    .get("diff")
                    .or_else(|| value.get("patch"))
                    .or_else(|| value.get("content"))
                    .and_then(|v| v.as_str())
                {
                    output_storage = Some(diff.to_string());
                } else if let Some(text) = value.as_str() {
                    output_storage = Some(text.to_string());
                }
            }
        }
        if output_text.map(|text| text.is_empty()).unwrap_or(true) {
            output_text = output_storage.as_deref();
        }
        let mut diff_lines = parse_diff_lines(output_text.unwrap_or(""));
        if diff_lines.is_empty() {
            if let Some(value) = tool.input_value.as_ref() {
                let old_text = value
                    .get("old_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_text = value
                    .get("new_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !old_text.is_empty() || !new_text.is_empty() {
                    diff_lines = build_simple_diff(old_text, new_text);
                }
            }
        }
        if diff_lines.is_empty() {
            return ToolDetail::None;
        }
        let detail = DiffToolCall::new(file_path)
            .lines(diff_lines)
            .status(status)
            .expanded(true);
        return ToolDetail::Diff(detail);
    }

    ToolDetail::None
}
