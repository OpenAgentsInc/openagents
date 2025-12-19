use maud::{Markup, html};

use super::line_meta::LineMeta;
use super::super::atoms::{LineType, StatusState, line_type_label, status_dot};

/// Full line header: status dot + type label + name + metadata.
pub struct LineHeader {
    pub status: StatusState,
    pub line_type: LineType,
    pub name: Option<String>,
    pub meta: LineMeta,
}

impl LineHeader {
    pub fn new(status: StatusState, line_type: LineType) -> Self {
        Self {
            status,
            line_type,
            name: None,
            meta: LineMeta::new(),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn meta(mut self, meta: LineMeta) -> Self {
        self.meta = meta;
        self
    }

    pub fn build(self) -> Markup {
        html! {
            div class="flex items-center gap-2 py-2" {
                (status_dot(self.status))
                (line_type_label(self.line_type))
                @if let Some(name) = &self.name {
                    span class="text-foreground ml-1" { (name) }
                }
                span class="flex-1" {}
                (self.meta.build())
            }
        }
    }
}
