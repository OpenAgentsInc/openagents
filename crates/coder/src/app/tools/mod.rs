pub(crate) mod dspy_stages;
pub(crate) mod parsing;
pub(crate) mod state;
pub(crate) mod visualization;

pub(crate) use dspy_stages::{DspyStageLayout, DspyStageVisualization};
pub(crate) use parsing::{build_simple_diff, format_tool_input, parse_diff_lines, parse_search_matches, tool_result_output, tool_type_for_name};
pub(crate) use state::ToolsState;
pub(crate) use visualization::{ToolDetail, ToolPanelBlock, ToolVisualization};
