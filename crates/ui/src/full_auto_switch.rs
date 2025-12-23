//! FullAutoSwitch component.
//!
//! Terminal-style toggle for Chat/Full Auto mode.
//! Visual: [_][■] FULL AUTO: OFF / [■][_] FULL AUTO: ON

use maud::{Markup, PreEscaped, html};

/// Full Auto mode toggle switch.
pub struct FullAutoSwitch {
    is_auto: bool,
}

impl FullAutoSwitch {
    pub fn new(is_auto: bool) -> Self {
        Self { is_auto }
    }

    pub fn build(self) -> Markup {
        let state = if self.is_auto { "ON" } else { "OFF" };
        let state_color = if self.is_auto { "#00A645" } else { "#FF0000" };

        html! {
            div id="full-auto-switch" {
                button
                    class="full-auto-switch"
                    type="button"
                    hx-post="/api/autopilot/toggle"
                    hx-target="#full-auto-switch"
                    hx-swap="outerHTML"
                    style="display: inline-flex; align-items: center; gap: 0.5rem; cursor: pointer; font-family: 'Vera Mono', ui-monospace, monospace; font-size: 0.75rem; background: none; border: none; padding: 0; color: inherit;"
                {
                    // Toggle boxes - two fixed-size squares, no gap
                    span style="display: inline-flex;" {
                        span
                            class="fa-box-left"
                            style={
                                "width: 10px; height: 10px; border: 1px solid #fafafa;"
                                @if self.is_auto { " background: #fafafa;" }
                            }
                        {}
                        span
                            class="fa-box-right"
                            style={
                                "width: 10px; height: 10px; border: 1px solid #fafafa; margin-left: -1px;"
                                @if !self.is_auto { " background: #fafafa;" }
                            }
                        {}
                    }

                    // Label - very muted
                    span style="color: #52525b;" { "FULL AUTO:" }

                    // State indicator - red (OFF) or green (ON), bold
                    span
                        class="fa-state"
                        style={"color: " (state_color) "; font-weight: 600;"}
                    {
                        (state)
                    }
                }

                (PreEscaped(r#"<style>
.full-auto-switch:hover { opacity: 0.7; }
</style>"#))
            }
        }
    }
}
