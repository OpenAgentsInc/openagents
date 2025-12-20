//! Autopilot live viewer page

use maud::{DOCTYPE, Markup, PreEscaped, html};
use ui::{TAILWIND_CDN, TAILWIND_THEME};
use ui::recorder::molecules::{ResultType, SessionMode};
use ui::recorder::organisms::{AgentLine, UserLine, ToolLine};
use ui::recorder::sections::{SessionHeader, SessionStats, ToolIndex, session_sidebar};

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
                        // Render sidebar with mock data
                        (render_mock_sidebar())
                    }

                    // Main timeline area (flex-grow)
                    div class="flex-1 px-4 pb-4" {
                        // Session header (collapsible metadata)
                        div id="session-header" class="mb-4" {
                            // Header will be updated via WebSocket for live sessions
                        }

                        // Timeline container - messages appended here
                        div id="timeline" class="space-y-2" {
                            // Render mock timeline events
                            (render_mock_timeline())
                        }
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

                    // Restore auto-scroll preference from localStorage
                    var savedPref = localStorage.getItem('autopilot-auto-scroll');
                    if (savedPref !== null) {
                        autoScroll.checked = savedPref === 'true';
                    }

                    // Save preference when toggled
                    autoScroll.addEventListener('change', function() {
                        localStorage.setItem('autopilot-auto-scroll', autoScroll.checked);
                    });

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

/// Render mock sidebar for demo
fn render_mock_sidebar() -> Markup {
    let header = SessionHeader::new("sess_demo123", "claude-sonnet-4", "OpenAgentsInc/openagents")
        .mode(SessionMode::Auto)
        .sha("8321ff9")
        .branch("main")
        .budget(5.0, "2h")
        .mcp(vec!["issues", "filesystem"]);

    let stats = SessionStats {
        lines: 47,
        duration: "00:12:34".to_string(),
        cost: 0.42,
        user_msgs: 3,
        agent_msgs: 12,
        tool_calls: 28,
        mcp_calls: 4,
        subagents: 2,
        questions: 1,
        phases: 0,
        blobs: 0,
        redacted: 3,
    };

    let tool_index = ToolIndex::new()
        .add("Read", 12)
        .add("Edit", 8)
        .add("Bash", 5)
        .add("Grep", 3);

    session_sidebar(
        header,
        SessionMode::Auto,
        None, // No plan phase
        0.42, // budget spent
        5.0,  // budget total
        0.42, // cost total
        Some(0.03), // cost delta
        stats,
        tool_index,
    )
}

/// Render mock timeline events for demo
fn render_mock_timeline() -> Markup {
    html! {
        // User message
        (UserLine::new("Wire recorder components into desktop /autopilot route")
            .step(1)
            .elapsed(0, 0, 0)
            .build())

        // Agent response
        (AgentLine::new("I'll help you wire up the recorder components. Let me first explore the existing code structure.")
            .step(2)
            .elapsed(0, 0, 3)
            .metrics(1200, 450, Some(800), 0.02)
            .build())

        // Tool call - Read
        (ToolLine::new("Read", "file_path=/crates/desktop/src/views/autopilot.rs", ResultType::Ok)
            .step(3)
            .elapsed(0, 0, 5)
            .call_id("toolu_abc123")
            .latency(45)
            .build())

        // Tool call - Grep
        (ToolLine::new("Grep", "pattern=recorder path=crates/ui/src", ResultType::Count { count: 15, unit: "matches".to_string() })
            .step(4)
            .elapsed(0, 0, 6)
            .call_id("toolu_def456")
            .latency(120)
            .build())

        // Agent continues
        (AgentLine::new("I found the recorder components in crates/ui/src/recorder/. Now let me update the autopilot view to import and use them.")
            .step(5)
            .elapsed(0, 0, 8)
            .metrics(2400, 380, Some(1800), 0.03)
            .build())

        // Tool call - Edit
        (ToolLine::new("Edit", "file_path=/crates/desktop/src/views/autopilot.rs", ResultType::Ok)
            .step(6)
            .elapsed(0, 0, 12)
            .call_id("toolu_ghi789")
            .latency(15)
            .build())
    }
}
