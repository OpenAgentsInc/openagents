use dioxus::prelude::*;
use vibe::VibeScreen;

mod views;

use views::MechaCoder;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut active = use_signal(|| "vibe".to_string());

    rsx! {
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        div { style: "display: flex; flex-direction: column; min-height: 100vh; background: #030303;",
            // Simple toggle between Vibe and MechaCoder
            div {
                style: "display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #1c1c1c; background: #0a0a0a; color: #e6e6e6; font-family: 'Berkeley Mono', monospace; font-size: 13px;",
                span { style: "color: #ffb400; font-weight: 600;", "OpenAgents" }
                button {
                    style: format!(
                        "background: none; border: 1px solid #1c1c1c; color: {}; padding: 6px 10px; cursor: pointer;",
                        if active() == "vibe" { "#ffb400" } else { "#9a9a9a" }
                    ),
                    onclick: move |_| active.set("vibe".to_string()),
                    "Vibe"
                }
                button {
                    style: format!(
                        "background: none; border: 1px solid #1c1c1c; color: {}; padding: 6px 10px; cursor: pointer;",
                        if active() == "mecha" { "#ffb400" } else { "#9a9a9a" }
                    ),
                    onclick: move |_| active.set("mecha".to_string()),
                    "MechaCoder"
                }
            }

            match active().as_str() {
                "vibe" => rsx! { VibeScreen {} },
                _ => rsx! { MechaCoder {} },
            }
        }
    }
}
