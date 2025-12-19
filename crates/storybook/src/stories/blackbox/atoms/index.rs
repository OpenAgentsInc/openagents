//! Atoms index story.

use maud::{Markup, html};

use super::shared::{section, section_title};

struct AtomLink {
    name: &'static str,
    href: &'static str,
    description: &'static str,
}

const ATOM_LINKS: &[AtomLink] = &[
    AtomLink {
        name: "Status Dot",
        href: "/stories/blackbox/atoms/status-dot",
        description: "Colored dot indicator for status states.",
    },
    AtomLink {
        name: "Line Type Label",
        href: "/stories/blackbox/atoms/line-type-label",
        description: "Uppercase label for log line types.",
    },
    AtomLink {
        name: "Step Badge",
        href: "/stories/blackbox/atoms/step-badge",
        description: "Compact step indicator for log ordering.",
    },
    AtomLink {
        name: "Timestamp Badge",
        href: "/stories/blackbox/atoms/timestamp-badge",
        description: "Elapsed and wall clock time badges.",
    },
    AtomLink {
        name: "Call ID Badge",
        href: "/stories/blackbox/atoms/call-id-badge",
        description: "Identifier for tool, MCP, and subagent calls.",
    },
    AtomLink {
        name: "Cost Badge",
        href: "/stories/blackbox/atoms/cost-badge",
        description: "Cost indicator with threshold-based coloring.",
    },
    AtomLink {
        name: "Token Badge",
        href: "/stories/blackbox/atoms/token-badge",
        description: "Prompt/completion token counts with cached totals.",
    },
    AtomLink {
        name: "Latency Badge",
        href: "/stories/blackbox/atoms/latency-badge",
        description: "Latency indicator with threshold-based coloring.",
    },
    AtomLink {
        name: "Attempt Badge",
        href: "/stories/blackbox/atoms/attempt-badge",
        description: "Retry count for attempts.",
    },
    AtomLink {
        name: "TID Badge",
        href: "/stories/blackbox/atoms/tid-badge",
        description: "Thread ID badge with color mapping.",
    },
    AtomLink {
        name: "Blob Ref",
        href: "/stories/blackbox/atoms/blob-ref",
        description: "Blob reference with size and optional MIME type.",
    },
    AtomLink {
        name: "Redacted Value",
        href: "/stories/blackbox/atoms/redacted-value",
        description: "Redaction marker for sensitive values.",
    },
    AtomLink {
        name: "Result Arrow",
        href: "/stories/blackbox/atoms/result-arrow",
        description: "Separator arrow for results.",
    },
];

pub fn atoms_index_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Atoms" }
        p class="text-sm text-muted-foreground mb-6" {
            "Atomic UI primitives for rendering BlackBox session logs."
        }

        (section_title("Index"))
        (section(html! {
            div class="grid gap-4 md:grid-cols-2" {
                @for atom in ATOM_LINKS {
                    a href=(atom.href) class="block border border-border bg-background px-4 py-3 hover:bg-secondary" {
                        div class="text-sm font-medium text-foreground" { (atom.name) }
                        div class="text-xs text-muted-foreground mt-1" { (atom.description) }
                    }
                }
            }
        }))
    }
}
