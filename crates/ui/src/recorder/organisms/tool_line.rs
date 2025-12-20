use maud::{Markup, html};

use super::styles::{LINE_CARD_CLASS, LINE_HEADER_CLASS};
use super::super::atoms::{CallType, LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::{LineMeta, ResultType, result_display};

pub struct ToolLine {
    pub tool_name: String,
    pub args: String,
    pub result: ResultType,
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub call_id: Option<String>,
    pub latency_ms: Option<u32>,
    pub content_preview: Option<String>,
    pub expanded: bool,
}

impl ToolLine {
    pub fn new(tool_name: &str, args: &str, result: ResultType) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            args: args.to_string(),
            result,
            step: None,
            elapsed: None,
            call_id: None,
            latency_ms: None,
            content_preview: None,
            expanded: false,
        }
    }

    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn elapsed(mut self, h: u8, m: u8, s: u8) -> Self {
        self.elapsed = Some((h, m, s));
        self
    }

    pub fn call_id(mut self, id: &str) -> Self {
        self.call_id = Some(id.to_string());
        self
    }

    pub fn latency(mut self, ms: u32) -> Self {
        self.latency_ms = Some(ms);
        self
    }

    pub fn preview(mut self, content: &str) -> Self {
        self.content_preview = Some(content.to_string());
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn build(self) -> Markup {
        let mut meta = LineMeta::new();
        if let Some(s) = self.step {
            meta = meta.step(s);
        }
        if let Some((h, m, s)) = self.elapsed {
            meta = meta.elapsed(h, m, s);
        }
        if let Some(ref id) = self.call_id {
            meta = meta.call_id(id, CallType::Tool);
        }
        if let Some(ms) = self.latency_ms {
            meta = meta.latency(ms);
        }

        let status = match &self.result {
            ResultType::Error(_) => StatusState::Error,
            ResultType::Pending => StatusState::Pending,
            _ => StatusState::Success,
        };

        let border_class = match status {
            StatusState::Error => "border-l-2 border-red",
            StatusState::Pending => "border-l-2 border-yellow",
            _ => "border-l-2 border-border",
        };

        let details_id = self.call_id.as_deref().unwrap_or(&self.tool_name);

        html! {
            details
                class={ (LINE_CARD_CLASS) " " (border_class) }
                id={ "tool-" (details_id) }
                open[self.expanded]
                ontoggle="localStorage.setItem(this.id, this.open)" {
                summary class="cursor-pointer list-none" {
                    div class={ "flex items-center gap-2 " (LINE_HEADER_CLASS) } {
                        (status_dot(status))
                        (line_type_label(LineType::Tool))
                        span class="text-foreground font-medium" { (self.tool_name) }
                        span class="flex-1" {}
                        (meta.build())
                    }
                    div class="pl-6 text-xs text-muted-foreground mt-1" {
                        (self.args)
                    }
                }
                div class="px-3 py-3 border-t border-border" {
                    @if let Some(ref preview) = self.content_preview {
                        pre class="mb-2 p-2 bg-background text-xs text-muted-foreground overflow-x-auto whitespace-pre-wrap" {
                            (preview)
                        }
                    }
                    (result_display(self.result))
                }
            }
            script {
                "(function() {"
                    "const el = document.getElementById('tool-" (details_id) "');"
                    "const saved = localStorage.getItem('tool-" (details_id) "');"
                    "if (saved !== null) el.open = saved === 'true';"
                "})();"
            }
        }
    }
}
