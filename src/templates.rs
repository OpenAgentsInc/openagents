use actix_web::web::Html;
use hypertext::elements::*;
use hypertext::attributes::*;

#[derive(Debug)]
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

pub fn render_header_template(buttons: Vec<(&str, &str)>) -> Html {
    let header = header()
        .class("py-2 mb-5")
        .child(
            div()
                .class("flex justify-between items-center w-full")
                .child(span().class("text-lg font-bold").text("OpenAgents"))
        )
        .child(
            div()
                .class("mt-4")
                .child(
                    nav().child(
                        ul()
                            .class("grid grid-cols-3 gap-2 lg:grid-cols-6")
                            .id("nav-buttons")
                            .children(
                                buttons.into_iter().map(|(href, text)| {
                                    li()
                                        .class("flex justify-center mb-1")
                                        .child(
                                            a()
                                                .class("btn-nav bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white")
                                                .href(href)
                                                .text(text)
                                        )
                                })
                            )
                    )
                )
        );

    Html::new(header.to_string())
}

pub fn render_agent_item_template(agent: &Agent) -> Html {
    let agent_div = div()
        .class("agent-item border border-gray-300 rounded-lg p-4 mb-4")
        .attribute("data-agent-id", &agent.id)
        .child(h3().class("text-lg font-bold mb-2").text(&agent.name))
        .child(p().class("text-sm text-gray-700 mb-2").text(&agent.description))
        .child(
            div()
                .class("agent-status mb-2")
                .text("Status: ")
                .child(
                    span()
                        .class(format!("status-badge {}", agent.status))
                        .text(&agent.status)
                )
        )
        .child(
            div()
                .class("agent-metrics text-sm text-gray-600 mb-2")
                .child(
                    div().text(format!(
                        "Memory: {} MB / {} MB",
                        agent.memory_usage, agent.memory_limit
                    ))
                )
                .child(
                    div().text(format!(
                        "CPU: {} ms / {} ms",
                        agent.cpu_usage, agent.cpu_limit
                    ))
                )
        )
        .child(
            div()
                .class("agent-actions")
                .child(
                    button()
                        .class("btn-nav bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white")
                        .attribute("nostr-action", "start")
                        .attribute("data-agent-id", &agent.id)
                        .text("Start")
                )
                .child(
                    button()
                        .class("btn-nav bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white")
                        .attribute("nostr-action", "stop")
                        .attribute("data-agent-id", &agent.id)
                        .text("Stop")
                )
                .child(
                    button()
                        .class("btn-nav bg-red-600 hover:bg-red-700 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white")
                        .attribute("nostr-action", "delete")
                        .attribute("data-agent-id", &agent.id)
                        .text("Delete")
                )
        );

    Html::new(agent_div.to_string())
}