//! Original synthetic tool and plugin records.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Read,
    Search,
    Edit,
    Run,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolState {
    Complete,
    Running,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolCall {
    pub kind: ToolKind,
    pub input: &'static str,
    pub output: &'static str,
    pub state: ToolState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginCall {
    pub plugin: &'static str,
    pub operation: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub state: ToolState,
}
