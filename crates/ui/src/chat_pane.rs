//! ChatPane component.
//!
//! Collapsible pane for streaming autopilot output with toggle between
//! raw text view and formatted component view.

use maud::{Markup, PreEscaped, html};

/// View mode for the chat pane.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatViewMode {
    /// Raw text output (like a terminal)
    Raw,
    /// Formatted component view
    #[default]
    Formatted,
}

/// Autopilot output chat pane with view toggle.
pub struct ChatPane {
    visible: bool,
    mode: ChatViewMode,
}

impl ChatPane {
    pub fn new(visible: bool) -> Self {
        Self {
            visible,
            mode: ChatViewMode::default(),
        }
    }

    pub fn mode(mut self, mode: ChatViewMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn build(self) -> Markup {
        let visibility_class = if self.visible { "" } else { "hidden" };

        // Button base styles matching our Button component
        let btn_base = "inline-flex items-center gap-1 font-mono cursor-pointer transition-colors select-none px-3 py-1.5 text-xs";
        let btn_active = "bg-secondary text-foreground border border-border";
        let btn_inactive = "bg-transparent text-muted-foreground border border-transparent hover:bg-accent hover:text-foreground";

        let raw_class = if self.mode == ChatViewMode::Raw { btn_active } else { btn_inactive };
        let formatted_class = if self.mode == ChatViewMode::Formatted { btn_active } else { btn_inactive };

        html! {
            div
                id="chat-pane"
                class={"bg-background border-t border-border font-mono " (visibility_class)}
                style="position: fixed; bottom: 0; left: 0; right: 0; height: 50vh; max-height: 600px; display: flex; flex-direction: column; z-index: 100;"
            {
                // Header bar
                div class="flex items-center justify-between px-4 py-2 border-b border-border bg-card flex-shrink-0" {
                    // Left: Title
                    span class="text-xs text-muted-foreground uppercase tracking-wider" {
                        "Autopilot"
                    }

                    // Center: View toggle + Copy button
                    div class="flex items-center gap-2" {
                        // Toggle group
                        div class="flex items-center border border-border" {
                            button
                                id="chat-view-raw"
                                onclick="switchChatView('raw')"
                                class={(btn_base) " " (raw_class)}
                            {
                                "Raw"
                            }
                            button
                                id="chat-view-formatted"
                                onclick="switchChatView('formatted')"
                                class={(btn_base) " " (formatted_class)}
                            {
                                "Formatted"
                            }
                        }

                        // Divider
                        div class="w-px h-4 bg-border" {}

                        // Copy button
                        button
                            id="chat-copy-btn"
                            onclick="copyCurrentView()"
                            class={(btn_base) " bg-transparent text-muted-foreground border border-border hover:bg-accent hover:text-foreground"}
                            title="Copy to clipboard"
                        {
                            // Clipboard icon (simple SVG)
                            (PreEscaped(r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter"><rect x="9" y="9" width="13" height="13" rx="0" ry="0"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>"#))
                            span { "Copy" }
                        }
                    }

                    // Right: Close button
                    button
                        onclick="document.getElementById('chat-pane').classList.add('hidden')"
                        class="p-1 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                        title="Close"
                    {
                        (PreEscaped(r#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>"#))
                    }
                }

                // Content area - two divs, one shown at a time
                div class="flex-1 overflow-hidden relative" {
                    // Raw view
                    div
                        id="chat-content-raw"
                        class={"absolute inset-0 overflow-y-auto p-4 text-xs leading-relaxed " (if self.mode == ChatViewMode::Raw { "" } else { "hidden" })}
                    {
                        div class="text-muted-foreground" {
                            "Waiting for autopilot output..."
                        }
                    }

                    // Formatted view
                    div
                        id="chat-content-formatted"
                        class={"absolute inset-0 overflow-y-auto p-4 " (if self.mode == ChatViewMode::Formatted { "" } else { "hidden" })}
                    {
                        div class="text-sm text-muted-foreground" {
                            "Waiting for autopilot output..."
                        }
                    }
                }
            }

            // Scripts
            (PreEscaped(r#"<script>
// View switching
function switchChatView(mode) {
    const rawContent = document.getElementById('chat-content-raw');
    const formattedContent = document.getElementById('chat-content-formatted');
    const rawBtn = document.getElementById('chat-view-raw');
    const formattedBtn = document.getElementById('chat-view-formatted');

    const activeClasses = ['bg-secondary', 'text-foreground', 'border', 'border-border'];
    const inactiveClasses = ['bg-transparent', 'text-muted-foreground', 'border', 'border-transparent', 'hover:bg-accent', 'hover:text-foreground'];

    if (mode === 'raw') {
        rawContent.classList.remove('hidden');
        formattedContent.classList.add('hidden');
        activeClasses.forEach(c => rawBtn.classList.add(c));
        inactiveClasses.forEach(c => rawBtn.classList.remove(c));
        inactiveClasses.forEach(c => formattedBtn.classList.add(c));
        activeClasses.forEach(c => formattedBtn.classList.remove(c));
    } else {
        rawContent.classList.add('hidden');
        formattedContent.classList.remove('hidden');
        activeClasses.forEach(c => formattedBtn.classList.add(c));
        inactiveClasses.forEach(c => formattedBtn.classList.remove(c));
        inactiveClasses.forEach(c => rawBtn.classList.add(c));
        activeClasses.forEach(c => rawBtn.classList.remove(c));
    }

    localStorage.setItem('chatViewMode', mode);
}

// Copy current view to clipboard
function copyCurrentView() {
    const rawContent = document.getElementById('chat-content-raw');
    const formattedContent = document.getElementById('chat-content-formatted');
    const btn = document.getElementById('chat-copy-btn');

    // Get currently visible content
    const isRaw = !rawContent.classList.contains('hidden');
    const content = isRaw ? rawContent : formattedContent;

    // Get text content
    const text = content.innerText || content.textContent;

    navigator.clipboard.writeText(text).then(() => {
        // Show success feedback
        const originalHTML = btn.innerHTML;
        btn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter"><polyline points="20 6 9 17 4 12"></polyline></svg><span>Copied!</span>';
        btn.classList.add('text-green');

        setTimeout(() => {
            btn.innerHTML = originalHTML;
            btn.classList.remove('text-green');
        }, 1500);
    }).catch(err => {
        console.error('Failed to copy:', err);
        const originalHTML = btn.innerHTML;
        btn.innerHTML = '<span>Failed</span>';
        btn.classList.add('text-red');

        setTimeout(() => {
            btn.innerHTML = originalHTML;
            btn.classList.remove('text-red');
        }, 1500);
    });
}

// Auto-scroll observer for both content areas
(function() {
    ['chat-content-raw', 'chat-content-formatted'].forEach(id => {
        const el = document.getElementById(id);
        if (el) {
            const observer = new MutationObserver(() => {
                el.scrollTop = el.scrollHeight;
            });
            observer.observe(el, { childList: true, subtree: true });
        }
    });

    // Restore preference
    const saved = localStorage.getItem('chatViewMode');
    if (saved === 'raw' || saved === 'formatted') {
        switchChatView(saved);
    }
})();
</script>"#))

            // Hidden class style
            (PreEscaped(r#"<style>
#chat-pane.hidden { display: none !important; }
#chat-content-raw.hidden, #chat-content-formatted.hidden { display: none !important; }
#chat-content-raw .log-line { color: var(--color-muted-foreground); }
#chat-content-raw .log-error { color: var(--color-red); }
#chat-content-raw .log-success { color: var(--color-green); }
</style>"#))
        }
    }
}
