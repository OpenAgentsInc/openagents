//! Base layout with Tailwind CSS

use maud::{DOCTYPE, Markup, PreEscaped, html};
use ui::{TAILWIND_CDN, TAILWIND_THEME};

/// Base layout with Tailwind CSS
pub fn layout(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) }

                // Inline Tailwind CSS (Play CDN)
                script { (PreEscaped(TAILWIND_CDN)) }
                // Custom theme
                style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
            }
            body class="bg-background text-foreground font-mono antialiased min-h-screen flex items-center justify-center" {
                (content)

                // WebSocket connection for real-time updates
                (PreEscaped(r#"<script>
                (function() {
                    var ws = new WebSocket('ws://' + location.host + '/ws');
                    ws.onmessage = function(e) {
                        var tmp = document.createElement('div');
                        tmp.innerHTML = e.data;
                        var el = tmp.firstElementChild;
                        if (el && el.id) {
                            var target = document.getElementById(el.id);
                            if (target) target.outerHTML = el.outerHTML;
                        }
                    };
                    ws.onclose = function() {
                        setTimeout(function() { location.reload(); }, 1000);
                    };
                })();
                </script>"#))
            }
        }
    }
}
