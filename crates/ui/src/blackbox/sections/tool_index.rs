use maud::{Markup, html};

pub struct ToolIndex {
    pub tools: Vec<(String, u32)>,
}

impl ToolIndex {
    pub fn new() -> Self {
        Self { tools: vec![] }
    }

    pub fn add(mut self, name: &str, count: u32) -> Self {
        self.tools.push((name.to_string(), count));
        self
    }

    pub fn build(self) -> Markup {
        html! {
            details class="bg-card border border-border" open {
                summary class="px-3 py-2 cursor-pointer list-none flex items-center gap-2" {
                    span class="text-xs text-muted-foreground tracking-widest" { "TOOLS USED" }
                    span class="flex-1" {}
                    span class="text-xs text-muted-foreground" { "[\u{25BE}]" }
                }
                div class="px-3 py-3 border-t border-border" {
                    @for (name, count) in &self.tools {
                        div class="flex items-center gap-2 py-1 text-xs cursor-pointer" {
                            span class="text-yellow min-w-16" { (name) }
                            span class="text-muted-foreground" { (count) " calls" }
                        }
                    }
                }
            }
        }
    }
}

impl Default for ToolIndex {
    fn default() -> Self {
        Self::new()
    }
}
