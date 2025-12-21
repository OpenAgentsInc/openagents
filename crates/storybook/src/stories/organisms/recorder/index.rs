//! Recorder component index.

use maud::{Markup, html};

use super::shared::{section, section_title};

struct BlackboxLink {
    name: &'static str,
    href: &'static str,
    description: &'static str,
}

const BLACKBOX_LINKS: &[BlackboxLink] = &[
    BlackboxLink {
        name: "Atoms",
        href: "/stories/recorder/atoms",
        description: "Smallest primitives used in log rendering.",
    },
    BlackboxLink {
        name: "Molecules",
        href: "/stories/recorder/molecules",
        description: "Composed units like headers, meta blocks, and indicators.",
    },
    BlackboxLink {
        name: "Organisms",
        href: "/stories/recorder/organisms",
        description: "Full log line components (user, tool, MCP, etc.).",
    },
    BlackboxLink {
        name: "Sections",
        href: "/stories/recorder/sections",
        description: "Sidebar and header assemblies.",
    },
    BlackboxLink {
        name: "Demo",
        href: "/stories/recorder/demo",
        description: "Full session viewer demo using all components.",
    },
];

pub fn recorder_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Recorder" }
        p class="text-sm text-muted-foreground mb-6" {
            "Grouped Recorder UI components for session log rendering."
        }

        (section_title("Index"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                @for link in BLACKBOX_LINKS {
                    a href=(link.href) class="block border border-border bg-background px-4 py-3 hover:bg-secondary" {
                        div class="text-sm font-medium text-foreground" { (link.name) }
                        div class="text-xs text-muted-foreground mt-1" { (link.description) }
                    }
                }
            }
        }))
    }
}
