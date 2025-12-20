//! Autopilot live viewer page

use maud::{DOCTYPE, Markup, PreEscaped, html};
use ui::{TAILWIND_CDN, TAILWIND_THEME};

/// Autopilot page with live timeline
pub fn autopilot_page() -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "Autopilot - OpenAgents" }

                // Inline Tailwind CSS (Play CDN)
                script { (PreEscaped(TAILWIND_CDN)) }
                // Custom theme
                style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
            }
            body class="bg-background text-foreground font-mono antialiased min-h-screen" {
                // Header
                header class="fixed top-0 left-0 right-0 bg-card border-b border-border z-10" {
                    div class="px-4 py-3 flex items-center gap-4" {
                        span class="text-xs text-muted-foreground tracking-widest" { "AUTOPILOT" }
                        span id="session-status" class="text-xs text-yellow" { "connecting..." }
                        span class="flex-1" {}
                        // Auto-scroll toggle
                        label class="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer" {
                            input type="checkbox" id="auto-scroll" checked class="accent-green";
                            "Auto-scroll"
                        }
                    }
                }

                // Main content area with sidebar layout
                main class="pt-14 flex" {
                    // Left sidebar (fixed width)
                    aside id="session-sidebar" class="w-72 h-screen sticky top-14 overflow-y-auto border-r border-border flex-shrink-0" {
                        // Sidebar content will be updated via WebSocket
                        div class="p-4 text-xs text-muted-foreground" {
                            "Waiting for session..."
                        }
                    }

                    // Main timeline area (flex-grow)
                    div class="flex-1 px-4 pb-4" {
                        // Session header (collapsible metadata)
                        div id="session-header" class="mb-4" {
                            // Header content will be updated via WebSocket
                        }

                        // Timeline container - messages appended here
                        div id="timeline" class="space-y-2" {}
                    }
                }

                // WebSocket connection for autopilot updates
                (PreEscaped(r#"<script>
                (function() {
                    var ws = new WebSocket('ws://' + location.host + '/ws');
                    var autoScroll = document.getElementById('auto-scroll');
                    var timeline = document.getElementById('timeline');
                    var status = document.getElementById('session-status');
                    var sidebar = document.getElementById('session-sidebar');
                    var header = document.getElementById('session-header');

                    ws.onopen = function() {
                        status.textContent = 'connected';
                        status.className = 'text-xs text-green';
                    };

                    ws.onmessage = function(e) {
                        var fragment = e.data;

                        // Check if fragment has an id attribute (for OOB swap pattern)
                        var tempDiv = document.createElement('div');
                        tempDiv.innerHTML = fragment;
                        var firstChild = tempDiv.firstElementChild;

                        if (firstChild && firstChild.id) {
                            // Replace existing element with same id
                            var existingElement = document.getElementById(firstChild.id);
                            if (existingElement) {
                                existingElement.outerHTML = fragment;
                                return;
                            }

                            // Special handling for sidebar and header updates
                            if (firstChild.id === 'session-sidebar') {
                                sidebar.innerHTML = firstChild.innerHTML;
                                return;
                            }
                            if (firstChild.id === 'session-header') {
                                header.innerHTML = firstChild.innerHTML;
                                return;
                            }
                        }

                        // Default: append to timeline
                        timeline.insertAdjacentHTML('beforeend', fragment);

                        // Auto-scroll if enabled
                        if (autoScroll.checked) {
                            window.scrollTo(0, document.body.scrollHeight);
                        }
                    };

                    ws.onclose = function() {
                        status.textContent = 'disconnected';
                        status.className = 'text-xs text-red';
                    };

                    ws.onerror = function() {
                        status.textContent = 'error';
                        status.className = 'text-xs text-red';
                    };
                })();
                </script>"#))
            }
        }
    }
}
