use serde_json::Value;
use wgpui::components::atoms::ToolStatus;

use crate::autopilot_loop::DspyStage;

use super::{DspyStageVisualization, ToolVisualization};
use super::parsing::{format_tool_input, tool_type_for_name};

pub(crate) struct ToolsState {
    pub(crate) current_tool_name: Option<String>,
    pub(crate) current_tool_input: String,
    pub(crate) current_tool_use_id: Option<String>,
    pub(crate) tool_history: Vec<ToolVisualization>,
    pub(crate) dspy_stages: Vec<DspyStageVisualization>,
}

impl ToolsState {
    pub(crate) fn new() -> Self {
        Self {
            current_tool_name: None,
            current_tool_input: String::new(),
            current_tool_use_id: None,
            tool_history: Vec::new(),
            dspy_stages: Vec::new(),
        }
    }

    pub(crate) fn start_tool_call(
        &mut self,
        name: String,
        tool_use_id: String,
        message_index: usize,
    ) {
        self.current_tool_name = Some(name.clone());
        self.current_tool_input.clear();
        self.current_tool_use_id = Some(tool_use_id.clone());

        let tool_type = tool_type_for_name(&name);
        if let Some(tool) = self
            .tool_history
            .iter_mut()
            .find(|tool| tool.tool_use_id == tool_use_id)
        {
            tool.name = name;
            tool.tool_type = tool_type;
            tool.status = ToolStatus::Running;
            tool.refresh_components();
            return;
        }

        let tool = ToolVisualization::new(tool_use_id, name, tool_type, message_index);
        self.tool_history.push(tool);
        if self.tool_history.len() > super::super::TOOL_HISTORY_LIMIT {
            let overflow = self.tool_history.len() - super::super::TOOL_HISTORY_LIMIT;
            self.tool_history.drain(0..overflow);
        }
    }

    pub(crate) fn finalize_tool_input(&mut self) {
        let Some(tool_use_id) = self.current_tool_use_id.clone() else {
            self.current_tool_input.clear();
            self.current_tool_name = None;
            return;
        };
        let input_json = std::mem::take(&mut self.current_tool_input);
        let input_value = serde_json::from_str::<Value>(&input_json).ok();

        if let Some(tool) = self
            .tool_history
            .iter_mut()
            .find(|tool| tool.tool_use_id == tool_use_id)
        {
            let display = format_tool_input(&tool.name, &input_json);
            tool.input = Some(display);
            tool.input_value = input_value;
            tool.refresh_components();
        }
        self.current_tool_name = None;
    }

    pub(crate) fn update_tool_progress(
        &mut self,
        tool_use_id: String,
        tool_name: String,
        elapsed_secs: f64,
        message_index: usize,
    ) {
        if self
            .tool_history
            .iter()
            .all(|tool| tool.tool_use_id != tool_use_id)
        {
            self.start_tool_call(tool_name.clone(), tool_use_id.clone(), message_index);
        }

        if let Some(tool) = self
            .tool_history
            .iter_mut()
            .find(|tool| tool.tool_use_id == tool_use_id)
        {
            tool.name = tool_name;
            tool.status = ToolStatus::Running;
            tool.elapsed_secs = Some(elapsed_secs);
            tool.refresh_card();
        }
    }

    pub(crate) fn apply_tool_result(
        &mut self,
        tool_use_id: Option<String>,
        content: String,
        is_error: bool,
        exit_code: Option<i32>,
        output_value: Option<Value>,
    ) {
        let mut resolved_id = tool_use_id.clone();
        if resolved_id.is_none() {
            resolved_id = self.current_tool_use_id.clone();
        }
        if resolved_id.is_none() {
            resolved_id = self
                .tool_history
                .iter()
                .rev()
                .find(|tool| matches!(tool.status, ToolStatus::Running | ToolStatus::Pending))
                .map(|tool| tool.tool_use_id.clone());
        }

        let Some(tool_id) = resolved_id else {
            return;
        };

        let status = if is_error || exit_code.map(|code| code != 0).unwrap_or(false) {
            ToolStatus::Error
        } else {
            ToolStatus::Success
        };

        if let Some(tool) = self
            .tool_history
            .iter_mut()
            .find(|tool| tool.tool_use_id == tool_id)
        {
            tool.status = status;
            tool.output = if content.trim().is_empty() {
                None
            } else {
                Some(content)
            };
            tool.output_value = output_value;
            tool.exit_code = exit_code;
            tool.refresh_components();
        }

        if self.current_tool_use_id.as_deref() == Some(&tool_id) {
            self.current_tool_use_id = None;
        }
    }

    pub(crate) fn push_dspy_stage(&mut self, stage: DspyStage, message_index: usize) {
        let viz = DspyStageVisualization::new(stage, message_index);
        self.dspy_stages.push(viz);
    }

    pub(crate) fn cancel_running_tools(&mut self) {
        for tool in &mut self.tool_history {
            if matches!(tool.status, ToolStatus::Running | ToolStatus::Pending) {
                tool.status = ToolStatus::Cancelled;
                tool.refresh_components();
            }
        }
    }

    pub(crate) fn has_running(&self) -> bool {
        self.tool_history
            .iter()
            .any(|tool| matches!(tool.status, ToolStatus::Running | ToolStatus::Pending))
    }
}
