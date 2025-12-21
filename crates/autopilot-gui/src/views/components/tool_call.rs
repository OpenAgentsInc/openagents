//! Tool execution display component

use maud::{html, Markup};

/// Tool call panel component
pub fn tool_call_panel(tool_name: &str, input: &str, status: &str) -> Markup {
    let (bg_color, border_color, status_text, status_color) = match status {
        "running" => ("#1a2a1a", "#3a5a3a", "Running...", "#7dff7d"),
        "success" => ("#1a1a2a", "#3a3a5a", "Complete", "#7d7dff"),
        "error" => ("#2a1a1a", "#5a3a3a", "Failed", "#ff7d7d"),
        _ => ("#1a1a1a", "#3a3a3a", "Unknown", "#d0d0d0"),
    };

    html! {
        div style=(format!(
            "background: {}; border: 1px solid {}; padding: 1rem; margin-bottom: 1rem;",
            bg_color, border_color
        )) {
            // Header
            div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem;" {
                div style="display: flex; align-items: center; gap: 0.5rem;" {
                    span style="font-weight: bold; color: #4a9eff;" { "🔧 " (tool_name) }
                    span style=(format!("font-size: 0.75rem; color: {};", status_color)) {
                        (status_text)
                    }
                }
            }

            // Input parameters
            details open {
                summary style="cursor: pointer; color: #b0b0b0; margin-bottom: 0.5rem;" {
                    "Input"
                }
                pre style="background: #0a0a0a; padding: 0.75rem; overflow-x: auto; font-size: 0.875rem; color: #d0d0d0; border: 1px solid #2a2a2a;" {
                    code { (input) }
                }
            }
        }
    }
}

/// Tool result panel component
pub fn tool_result_panel(tool_name: &str, output: &str, elapsed_ms: Option<u64>) -> Markup {
    html! {
        div style="background: #1a1a2a; border: 1px solid #3a3a5a; padding: 1rem; margin-bottom: 1rem;" {
            // Header
            div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem;" {
                span style="font-weight: bold; color: #7d7dff;" { "✓ " (tool_name) " result" }
                @if let Some(ms) = elapsed_ms {
                    span style="font-size: 0.75rem; color: #b0b0b0;" {
                        (format!("{}ms", ms))
                    }
                }
            }

            // Output
            details open {
                summary style="cursor: pointer; color: #b0b0b0; margin-bottom: 0.5rem;" {
                    "Output"
                }
                pre style="background: #0a0a0a; padding: 0.75rem; overflow-x: auto; font-size: 0.875rem; color: #d0d0d0; border: 1px solid #2a2a2a; white-space: pre-wrap; word-break: break-word;" {
                    code { (output) }
                }
            }
        }
    }
}
