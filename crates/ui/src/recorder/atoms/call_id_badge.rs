use maud::{Markup, html};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    Tool,
    Mcp,
    Subagent,
    Skill,
}

impl CallType {
    fn class(&self) -> &'static str {
        match self {
            CallType::Tool => "text-yellow",
            CallType::Mcp => "text-cyan",
            CallType::Subagent => "text-red",
            CallType::Skill => "text-magenta",
        }
    }
}

pub fn call_id_badge(call_id: &str, call_type: CallType) -> Markup {
    html! {
        span
            title={ "Call ID: " (call_id) }
            class={ "text-xs cursor-pointer " (call_type.class()) }
        {
            (call_id)
        }
    }
}
