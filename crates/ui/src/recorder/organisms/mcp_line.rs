use maud::{Markup, html};

use super::styles::LINE_CARD_CLASS;
use super::super::atoms::{CallType, LineType, StatusState, line_type_label, status_dot};
use super::super::molecules::{LineMeta, ResultType, result_display};

pub struct McpLine {
    pub server_method: String,
    pub args: String,
    pub result: ResultType,
    pub step: Option<u32>,
    pub call_id: Option<String>,
    pub result_items: Vec<String>,
}

impl McpLine {
    pub fn new(server_method: &str, args: &str, result: ResultType) -> Self {
        Self {
            server_method: server_method.to_string(),
            args: args.to_string(),
            result,
            step: None,
            call_id: None,
            result_items: vec![],
        }
    }

    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn call_id(mut self, id: &str) -> Self {
        self.call_id = Some(id.to_string());
        self
    }

    pub fn items(mut self, items: Vec<&str>) -> Self {
        self.result_items = items.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn build(self) -> Markup {
        let mut meta = LineMeta::new();
        if let Some(s) = self.step {
            meta = meta.step(s);
        }
        if let Some(ref id) = self.call_id {
            meta = meta.call_id(id, CallType::Mcp);
        }

        html! {
            details class=(LINE_CARD_CLASS) {
                summary class="cursor-pointer list-none px-3 py-2" {
                    div class="flex items-center gap-2" {
                        (status_dot(StatusState::Success))
                        (line_type_label(LineType::Mcp))
                        span class="text-cyan font-medium" { (self.server_method) }
                        span class="flex-1" {}
                        (meta.build())
                    }
                    div class="pl-6 text-xs text-muted-foreground mt-1" {
                        (self.args)
                    }
                }
                div class="px-3 py-3 border-t border-border" {
                    (result_display(self.result))
                    @if !self.result_items.is_empty() {
                        div class="mt-2 pl-4" {
                            @for item in &self.result_items {
                                div class="text-xs text-muted-foreground py-0.5" {
                                    (item)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
