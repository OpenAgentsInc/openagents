use maud::{Markup, html};

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LineType {
    User,
    Agent,
    Tool,
    Observation,
    Skill,
    Plan,
    Mode,
    Recall,
    Subagent,
    Mcp,
    Question,
    Comment,
    Lifecycle,
    Phase,
}

impl LineType {
    fn label(&self) -> &'static str {
        match self {
            LineType::User => "USER",
            LineType::Agent => "AGENT",
            LineType::Tool => "TOOL",
            LineType::Observation => "OBSERVATION",
            LineType::Skill => "SKILL",
            LineType::Plan => "PLAN",
            LineType::Mode => "MODE",
            LineType::Recall => "RECALL",
            LineType::Subagent => "SUBAGENT",
            LineType::Mcp => "MCP",
            LineType::Question => "QUESTION",
            LineType::Comment => "#",
            LineType::Lifecycle => "@",
            LineType::Phase => "\u{25D0}", // ◐
        }
    }
}

pub fn line_type_label(line_type: LineType) -> Markup {
    html! {
        span class="text-xs uppercase tracking-widest text-muted-foreground" {
            (line_type.label())
        }
    }
}
