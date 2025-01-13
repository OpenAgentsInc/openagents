
use hypertext::{html_elements, maud, GlobalAttributes, Renderable};

pub mod templates {
    use super::*;

    pub fn render_header_template(buttons: Vec<(&str, &str)>) -> String {
        maud! {
            header.py-2.mb-5 {
                div.flex.justify-between.items-center.w-full {
                    span.text-lg.font-bold { "OpenAgents" }
                }
                div.mt-4 {
                    nav {
                        ul #nav-buttons.grid.grid-cols-3.gap-2.lg:grid-cols-6 {
                            @for (href, text) in buttons {
                                li.flex.justify-center.mb-1 {
                                    a.btn-nav.bg-black.hover:bg-zinc-900.text-white.text-xs.inline-flex.items-center.justify-center.whitespace-nowrap.select-none.text-center.align-middle.no-underline.outline-none.w-full.px-6.border.border-white href=(href) {
                                        (text)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }.render()
    }

    pub fn render_agent_item_template(agent: &Agent) -> String {
        maud! {
            div.agent-item.border.border-gray-300.rounded-lg.p-4.mb-4 data-agent-id=(agent.id) {
                h3.text-lg.font-bold.mb-2 { (agent.name) }
                p.text-sm.text-gray-700.mb-2 { (agent.description) }
                div.agent-status.mb-2 {
                    "Status: "
                    span class=(format!("status-badge {}", agent.status)) { (agent.status) }
                }
                div.agent-metrics.text-sm.text-gray-600.mb-2 {
                    div {
                        "Memory: " (agent.memory_usage) "MB / " (agent.memory_limit) "MB"
                    }
                    div {
                        "CPU: " (agent.cpu_usage) "ms / " (agent.cpu_limit) "ms"
                    }
                }
                div.agent-actions {
                    button.btn-nav.bg-black.hover:bg-zinc-900.text-white.text-xs.inline-flex.items-center.justify-center.whitespace-nowrap.select-none.text-center.align-middle.no-underline.outline-none.w-full.px-6.border.border-white nostr-action="start" data-agent-id=(agent.id) {
                        "Start"
                    }
                    button.btn-nav.bg-black.hover:bg-zinc-900.text-white.text-xs.inline-flex.items-center.justify-center.whitespace-nowrap.select-none.text-center.align-middle.no-underline.outline-none.w-full.px-6.border.border-white nostr-action="stop" data-agent-id=(agent.id) {
                        "Stop"
                    }
                    button.btn-nav.bg-red-600.hover:bg-red-700.text-white.text-xs.inline-flex.items-center.justify-center.whitespace-nowrap.select-none.text-center.align-middle.no-underline.outline-none.w-full.px-6.border.border-white nostr-action="delete" data-agent-id=(agent.id) {
                        "Delete"
                    }
                }
            }
        }.render()
    }

    pub struct Agent {
        pub id: String,
        pub name: String,
        pub description: String,
        pub status: String,
        pub memory_usage: u32,
        pub memory_limit: u32,
        pub cpu_usage: u32,
        pub cpu_limit: u32,
    }
}
