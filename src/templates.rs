use actix_web::web::Html;
use hypertext::Element;

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
    let mut header = Element::new("header");
    header.add_class("py-2 mb-5");

    let mut div_flex = Element::new("div");
    div_flex.add_class("flex justify-between items-center w-full");

    let mut span = Element::new("span");
    span.add_class("text-lg font-bold");
    span.set_text("OpenAgents");
    div_flex.append_child(span);

    let mut nav_div = Element::new("div");
    nav_div.add_class("mt-4");

    let mut nav = Element::new("nav");
    let mut ul = Element::new("ul");
    ul.add_class("grid grid-cols-3 gap-2 lg:grid-cols-6");
    ul.set_attribute("id", "nav-buttons");

    for (href, text) in buttons {
        let mut li = Element::new("li");
        li.add_class("flex justify-center mb-1");

        let mut a = Element::new("a");
        a.add_class("btn-nav bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white");
        a.set_attribute("href", href);
        a.set_text(text);

        li.append_child(a);
        ul.append_child(li);
    }

    nav.append_child(ul);
    nav_div.append_child(nav);
    header.append_child(div_flex);
    header.append_child(nav_div);

    Html::new(header.to_string())
}

pub fn render_agent_item_template(agent: &Agent) -> Html {
    let mut div = Element::new("div");
    div.add_class("agent-item border border-gray-300 rounded-lg p-4 mb-4");
    div.set_attribute("data-agent-id", &agent.id);

    let mut h3 = Element::new("h3");
    h3.add_class("text-lg font-bold mb-2");
    h3.set_text(&agent.name);
    div.append_child(h3);

    let mut p = Element::new("p");
    p.add_class("text-sm text-gray-700 mb-2");
    p.set_text(&agent.description);
    div.append_child(p);

    let mut status_div = Element::new("div");
    status_div.add_class("agent-status mb-2");
    status_div.set_text("Status: ");

    let mut status_span = Element::new("span");
    status_span.add_class(&format!("status-badge {}", agent.status));
    status_span.set_text(&agent.status);
    status_div.append_child(status_span);
    div.append_child(status_div);

    let mut metrics_div = Element::new("div");
    metrics_div.add_class("agent-metrics text-sm text-gray-600 mb-2");

    let mut memory_div = Element::new("div");
    memory_div.set_text(&format!("Memory: {} MB / {} MB", agent.memory_usage, agent.memory_limit));
    metrics_div.append_child(memory_div);

    let mut cpu_div = Element::new("div");
    cpu_div.set_text(&format!("CPU: {} ms / {} ms", agent.cpu_usage, agent.cpu_limit));
    metrics_div.append_child(cpu_div);
    div.append_child(metrics_div);

    let mut actions_div = Element::new("div");
    actions_div.add_class("agent-actions");

    let button_classes = "btn-nav bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white";

    let mut start_button = Element::new("button");
    start_button.add_class(button_classes);
    start_button.set_attribute("nostr-action", "start");
    start_button.set_attribute("data-agent-id", &agent.id);
    start_button.set_text("Start");
    actions_div.append_child(start_button);

    let mut stop_button = Element::new("button");
    stop_button.add_class(button_classes);
    stop_button.set_attribute("nostr-action", "stop");
    stop_button.set_attribute("data-agent-id", &agent.id);
    stop_button.set_text("Stop");
    actions_div.append_child(stop_button);

    let mut delete_button = Element::new("button");
    delete_button.add_class("btn-nav bg-red-600 hover:bg-red-700 text-white text-xs inline-flex items-center justify-center whitespace-nowrap select-none text-center align-middle no-underline outline-none w-full px-6 border border-white");
    delete_button.set_attribute("nostr-action", "delete");
    delete_button.set_attribute("data-agent-id", &agent.id);
    delete_button.set_text("Delete");
    actions_div.append_child(delete_button);

    div.append_child(actions_div);

    Html::new(div.to_string())
}