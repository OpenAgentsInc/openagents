//! LogPane component.
//!
//! Collapsible log pane for streaming autopilot output.
//! Fixed at bottom of screen, receives WebSocket updates.

use maud::{Markup, PreEscaped, html};

/// Autopilot output log pane.
pub struct LogPane {
    visible: bool,
}

impl LogPane {
    pub fn new(visible: bool) -> Self {
        Self { visible }
    }

    pub fn build(self) -> Markup {
        let visibility_class = if self.visible { "" } else { "hidden" };

        html! {
            div
                id="autopilot-log"
                class=(visibility_class)
                style="
                    position: fixed;
                    bottom: 0;
                    left: 0;
                    right: 0;
                    height: 40vh;
                    max-height: 400px;
                    background: #0a0a0a;
                    border-top: 1px solid #333;
                    display: flex;
                    flex-direction: column;
                    font-family: 'Vera Mono', ui-monospace, monospace;
                    z-index: 100;
                "
            {
                // Header bar
                div style="
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 0.5rem 1rem;
                    border-bottom: 1px solid #333;
                    background: #111;
                    flex-shrink: 0;
                " {
                    span style="font-size: 0.65rem; color: #888; text-transform: uppercase; letter-spacing: 0.05em;" {
                        "AUTOPILOT OUTPUT"
                    }
                    button
                        onclick="document.getElementById('autopilot-log').classList.add('hidden')"
                        style="
                            background: none;
                            border: none;
                            color: #666;
                            cursor: pointer;
                            font-size: 1rem;
                            padding: 0.25rem;
                            line-height: 1;
                        "
                    {
                        "×"
                    }
                }

                // Scrollable log content
                div
                    id="autopilot-log-content"
                    style="
                        flex: 1;
                        overflow-y: auto;
                        padding: 0.75rem 1rem;
                        font-size: 0.7rem;
                        line-height: 1.5;
                    "
                {
                    div style="color: #666;" {
                        "Waiting for autopilot output..."
                    }
                }
            }

            // Auto-scroll script
            (PreEscaped(r#"<script>
(function() {
    const logContent = document.getElementById('autopilot-log-content');
    if (logContent) {
        const observer = new MutationObserver(() => {
            logContent.scrollTop = logContent.scrollHeight;
        });
        observer.observe(logContent, { childList: true, subtree: true });
    }
})();
</script>"#))

            // Hidden class style
            (PreEscaped(r#"<style>
#autopilot-log.hidden { display: none !important; }
#autopilot-log-content .log-line { color: #888; }
#autopilot-log-content .log-error { color: #ef4444; }
#autopilot-log-content .log-success { color: #22c55e; }
</style>"#))
        }
    }
}
