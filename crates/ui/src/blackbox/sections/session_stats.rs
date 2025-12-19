use maud::{Markup, html};

pub struct SessionStats {
    pub lines: u32,
    pub duration: String,
    pub cost: f64,
    pub user_msgs: u32,
    pub agent_msgs: u32,
    pub tool_calls: u32,
    pub mcp_calls: u32,
    pub subagents: u32,
    pub questions: u32,
    pub phases: u32,
    pub blobs: u32,
    pub redacted: u32,
}

impl SessionStats {
    pub fn new() -> Self {
        Self {
            lines: 0,
            duration: String::new(),
            cost: 0.0,
            user_msgs: 0,
            agent_msgs: 0,
            tool_calls: 0,
            mcp_calls: 0,
            subagents: 0,
            questions: 0,
            phases: 0,
            blobs: 0,
            redacted: 0,
        }
    }

    pub fn build(self) -> Markup {
        html! {
            details class="bg-card border border-border" open {
                summary class="px-3 py-2 cursor-pointer list-none flex items-center gap-2" {
                    span class="text-xs text-muted-foreground tracking-widest" { "STATISTICS" }
                    span class="flex-1" {}
                    span class="text-xs text-muted-foreground" { "[\u{25BE}]" }
                }
                div class="px-3 py-3 border-t border-border" {
                    div class="grid grid-cols-[1fr,auto] gap-x-4 gap-y-1 text-xs" {
                        span class="text-muted-foreground" { "Lines:" }
                        span class="text-muted-foreground text-right" { (self.lines) }

                        span class="text-muted-foreground" { "Duration:" }
                        span class="text-muted-foreground text-right" { (self.duration) }

                        span class="text-muted-foreground" { "Cost:" }
                        span class="text-green text-right" { "$" (format!("{:.2}", self.cost)) }
                    }

                    div class="border-t border-border mt-2 pt-2" {
                        div class="grid grid-cols-[1fr,auto] gap-x-4 gap-y-1 text-xs" {
                            span class="text-muted-foreground" { "User msgs:" }
                            span class="text-muted-foreground text-right" { (self.user_msgs) }

                            span class="text-muted-foreground" { "Agent msgs:" }
                            span class="text-muted-foreground text-right" { (self.agent_msgs) }

                            span class="text-muted-foreground" { "Tool calls:" }
                            span class="text-muted-foreground text-right" { (self.tool_calls) }

                            span class="text-muted-foreground" { "MCP calls:" }
                            span class="text-muted-foreground text-right" { (self.mcp_calls) }

                            span class="text-muted-foreground" { "Subagents:" }
                            span class="text-muted-foreground text-right" { (self.subagents) }

                            span class="text-muted-foreground" { "Questions:" }
                            span class="text-muted-foreground text-right" { (self.questions) }

                            span class="text-muted-foreground" { "Phases:" }
                            span class="text-muted-foreground text-right" { (self.phases) }
                        }
                    }

                    div class="border-t border-border mt-2 pt-2" {
                        div class="grid grid-cols-[1fr,auto] gap-x-4 gap-y-1 text-xs" {
                            span class="text-muted-foreground" { "Blobs:" }
                            span class="text-muted-foreground text-right" { (self.blobs) }

                            span class="text-muted-foreground" { "Redacted:" }
                            span class="text-muted-foreground text-right" { (self.redacted) }
                        }
                    }
                }
            }
        }
    }
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}
