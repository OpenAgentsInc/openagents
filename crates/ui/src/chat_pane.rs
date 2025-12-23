//! ChatPane component.
//!
//! Collapsible pane for streaming autopilot output with toggle between
//! formatted view, raw JSON view, and raw RLOG view.

use maud::{Markup, PreEscaped, html};

/// View mode for the chat pane.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatViewMode {
    /// Formatted component view (default)
    #[default]
    Formatted,
    /// Raw JSON output (full untruncated Claude events)
    Json,
    /// Raw RLOG text output (truncated human-readable)
    Raw,
}

/// Autopilot output chat pane with view toggle.
///
/// # Examples
///
/// ```
/// use ui::{ChatPane, ChatViewMode};
///
/// // Create visible pane in raw mode
/// let pane = ChatPane::new(true)
///     .mode(ChatViewMode::Raw)
///     .build();
///
/// // Create hidden pane (default formatted mode)
/// let pane = ChatPane::new(false).build();
/// ```
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

        let formatted_class = if self.mode == ChatViewMode::Formatted { btn_active } else { btn_inactive };
        let json_class = if self.mode == ChatViewMode::Json { btn_active } else { btn_inactive };
        let raw_class = if self.mode == ChatViewMode::Raw { btn_active } else { btn_inactive };

        html! {
            div
                id="chat-pane"
                class={"bg-background border border-border font-mono " (visibility_class)}
                style="position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 80vw; max-width: 1000px; height: 70vh; max-height: 700px; display: flex; flex-direction: column; z-index: 100;"
            {
                // Header bar (draggable)
                div
                    id="chat-pane-header"
                    class="flex items-center justify-between px-4 py-2 border-b border-border bg-card flex-shrink-0"
                    style="cursor: grab;"
                {
                    // Left: Title
                    span class="text-xs text-muted-foreground uppercase tracking-wider" {
                        "Autopilot"
                    }

                    // Center: View toggle + Copy button
                    div class="flex items-center gap-2" {
                        // Toggle group: Formatted | Raw JSON | Raw RLOG
                        div class="flex items-center border border-border" {
                            button
                                id="chat-view-formatted"
                                onclick="switchChatView('formatted')"
                                class={(btn_base) " " (formatted_class)}
                            {
                                "Formatted"
                            }
                            button
                                id="chat-view-json"
                                onclick="switchChatView('json')"
                                class={(btn_base) " " (json_class)}
                            {
                                "Raw JSON"
                            }
                            button
                                id="chat-view-raw"
                                onclick="switchChatView('raw')"
                                class={(btn_base) " " (raw_class)}
                            {
                                "Raw RLOG"
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

                // Content area - three divs, one shown at a time
                div class="flex-1 overflow-hidden relative" {
                    // Formatted view
                    div
                        id="chat-content-formatted"
                        class={"absolute inset-0 overflow-y-auto p-4 " (if self.mode == ChatViewMode::Formatted { "" } else { "hidden" })}
                    {
                        div class="text-sm text-muted-foreground" {
                            "Waiting for autopilot output..."
                        }
                    }

                    // Raw JSON view (full untruncated Claude events)
                    div
                        id="chat-content-json"
                        class={"absolute inset-0 overflow-y-auto p-4 text-xs leading-relaxed " (if self.mode == ChatViewMode::Json { "" } else { "hidden" })}
                    {
                        div class="text-muted-foreground" {
                            "Waiting for JSON events..."
                        }
                    }

                    // Raw RLOG view (truncated human-readable)
                    div
                        id="chat-content-raw"
                        class={"absolute inset-0 overflow-y-auto p-4 text-xs leading-relaxed " (if self.mode == ChatViewMode::Raw { "" } else { "hidden" })}
                    {
                        div class="text-muted-foreground" {
                            "Waiting for autopilot output..."
                        }
                    }
                }
            }

            // Scripts
            (PreEscaped(r#"<script>
// View switching for 3 modes: formatted, json, raw
function switchChatView(mode) {
    const formattedContent = document.getElementById('chat-content-formatted');
    const jsonContent = document.getElementById('chat-content-json');
    const rawContent = document.getElementById('chat-content-raw');
    const formattedBtn = document.getElementById('chat-view-formatted');
    const jsonBtn = document.getElementById('chat-view-json');
    const rawBtn = document.getElementById('chat-view-raw');

    const activeClasses = ['bg-secondary', 'text-foreground', 'border', 'border-border'];
    const inactiveClasses = ['bg-transparent', 'text-muted-foreground', 'border', 'border-transparent', 'hover:bg-accent', 'hover:text-foreground'];

    // Helper to set button state
    function setActive(btn) {
        activeClasses.forEach(c => btn.classList.add(c));
        inactiveClasses.forEach(c => btn.classList.remove(c));
    }
    function setInactive(btn) {
        inactiveClasses.forEach(c => btn.classList.add(c));
        activeClasses.forEach(c => btn.classList.remove(c));
    }

    // Hide all content areas first
    formattedContent.classList.add('hidden');
    jsonContent.classList.add('hidden');
    rawContent.classList.add('hidden');

    // Set all buttons inactive first
    setInactive(formattedBtn);
    setInactive(jsonBtn);
    setInactive(rawBtn);

    // Show selected view and activate button
    if (mode === 'formatted') {
        formattedContent.classList.remove('hidden');
        setActive(formattedBtn);
    } else if (mode === 'json') {
        jsonContent.classList.remove('hidden');
        setActive(jsonBtn);
    } else if (mode === 'raw') {
        rawContent.classList.remove('hidden');
        setActive(rawBtn);
    }

    localStorage.setItem('chatViewMode', mode);
}

// Copy current view to clipboard
function copyCurrentView() {
    const formattedContent = document.getElementById('chat-content-formatted');
    const jsonContent = document.getElementById('chat-content-json');
    const rawContent = document.getElementById('chat-content-raw');
    const btn = document.getElementById('chat-copy-btn');

    // Get currently visible content
    let content;
    if (!formattedContent.classList.contains('hidden')) {
        content = formattedContent;
    } else if (!jsonContent.classList.contains('hidden')) {
        content = jsonContent;
    } else {
        content = rawContent;
    }

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

// Drag functionality for chat pane
(function() {
    const pane = document.getElementById('chat-pane');
    const header = document.getElementById('chat-pane-header');
    let isDragging = false;
    let offsetX = 0;
    let offsetY = 0;

    header.addEventListener('mousedown', function(e) {
        // Don't drag if clicking on buttons
        if (e.target.tagName === 'BUTTON' || e.target.closest('button')) return;

        isDragging = true;
        header.style.cursor = 'grabbing';

        const rect = pane.getBoundingClientRect();
        offsetX = e.clientX - rect.left;
        offsetY = e.clientY - rect.top;

        // Remove transform so we can use top/left
        pane.style.transform = 'none';
        pane.style.left = rect.left + 'px';
        pane.style.top = rect.top + 'px';

        e.preventDefault();
    });

    document.addEventListener('mousemove', function(e) {
        if (!isDragging) return;

        pane.style.left = (e.clientX - offsetX) + 'px';
        pane.style.top = (e.clientY - offsetY) + 'px';
    });

    document.addEventListener('mouseup', function() {
        if (isDragging) {
            isDragging = false;
            header.style.cursor = 'grab';
        }
    });
})();

// Auto-scroll observer for all content areas
(function() {
    ['chat-content-formatted', 'chat-content-json', 'chat-content-raw'].forEach(id => {
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
    if (saved === 'formatted' || saved === 'json' || saved === 'raw') {
        switchChatView(saved);
    }
})();
</script>"#))

            // Hidden class style
            (PreEscaped(r#"<style>
#chat-pane.hidden { display: none !important; }
#chat-content-formatted.hidden, #chat-content-json.hidden, #chat-content-raw.hidden { display: none !important; }
#chat-content-raw .log-line { color: var(--color-muted-foreground); }
#chat-content-raw .log-error { color: var(--color-red); }
#chat-content-raw .log-success { color: var(--color-green); }
#chat-content-json .json-line { color: var(--color-muted-foreground); word-break: break-all; }
</style>"#))
        }
    }
}
