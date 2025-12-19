//! Home page with counter demo

use maud::{Markup, html};
use ui::Button;

use super::layout;

/// Full home page with counter
pub fn home_page(count: u64) -> Markup {
    layout(
        "OpenAgents Desktop",
        html! {
            div class="text-center" {
                h1 class="mb-8 text-3xl font-semibold" {
                    "OpenAgents Desktop"
                }

                // Counter display (updated via WebSocket OOB swap)
                (counter_display(count))

                // Increment button using fetch POST
                div class="mt-6" {
                    (Button::new("Increment").render())
                }

                p class="mt-8 text-muted-foreground text-sm" {
                    "Click the button to update the counter via WebSocket"
                }

                // Script to handle button click
                script {
                    (maud::PreEscaped(r#"
                    document.querySelector('button').onclick = function() {
                        fetch('/increment', { method: 'POST' });
                    };
                    "#))
                }
            }
        },
    )
}

/// Counter display element (for initial render)
fn counter_display(count: u64) -> Markup {
    html! {
        div id="counter" class="text-6xl font-bold text-green" {
            (count)
        }
    }
}

/// Counter fragment for WebSocket OOB update
pub fn counter_fragment(count: u64) -> String {
    html! {
        div id="counter" class="text-6xl font-bold text-green" {
            (count)
        }
    }
    .into_string()
}
