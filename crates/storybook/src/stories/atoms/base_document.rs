//! Base document story - HTML shell with Tailwind CSS

use maud::{Markup, html};

fn section_title(title: &str) -> Markup {
    html! {
        h2 class="text-base font-medium text-muted-foreground mt-8 mb-4" {
            (title)
        }
    }
}

fn section(content: Markup) -> Markup {
    html! {
        div class="p-4 border border-border bg-card mb-4" {
            (content)
        }
    }
}

fn code_block(code: &str) -> Markup {
    html! {
        pre class="text-xs bg-secondary border border-border p-4 overflow-x-auto text-muted-foreground" {
            code { (code) }
        }
    }
}

pub fn base_document_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" {
            "Base Document"
        }
        p class="text-sm text-muted-foreground mb-6" {
            "The base HTML document shell with Tailwind CSS and semantic color theme."
        }

        (section_title("Features"))
        (section(html! {
            ul class="list-disc list-inside space-y-2 text-sm text-muted-foreground" {
                li { "Tailwind Play CDN for development (inline script)" }
                li { "Custom semantic color theme with CSS custom properties" }
                li { "Monospace font stack (Vera Mono / system monospace)" }
                li { "Dark mode by default with semantic tokens" }
                li { "Responsive viewport meta tag" }
                li { "Sharp corners only (no curves)" }
            }
        }))

        (section_title("Color Tokens"))
        (section(html! {
            div class="grid grid-cols-2 gap-4" {
                div class="space-y-2" {
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-background border border-border" {}
                        span class="text-xs" { "bg-background" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-foreground" {}
                        span class="text-xs" { "bg-foreground" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-primary" {}
                        span class="text-xs" { "bg-primary" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-secondary" {}
                        span class="text-xs" { "bg-secondary" }
                    }
                }
                div class="space-y-2" {
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-accent" {}
                        span class="text-xs" { "bg-accent" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-muted" {}
                        span class="text-xs" { "bg-muted" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 bg-card border border-border" {}
                        span class="text-xs" { "bg-card" }
                    }
                    div class="flex items-center gap-2" {
                        div class="w-4 h-4 border border-border" {}
                        span class="text-xs" { "border-border" }
                    }
                }
            }
        }))

        (section_title("Usage"))
        (code_block(r#"use ui::base_document;
use maud::html;

// Create a page
let page = base_document("My Page", html! {
    div class="p-8" {
        h1 class="text-2xl font-bold" { "Hello World" }
        p class="text-muted-foreground" { "Welcome to OpenAgents" }
    }
});

// Returns complete HTML document with:
// - DOCTYPE
// - Tailwind Play CDN
// - Custom semantic theme
// - body with bg-background text-foreground"#))

        (section_title("Example Output Structure"))
        (code_block(r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>My Page</title>
    <script>/* Tailwind Play CDN */</script>
    <style type="text/tailwindcss">
      @theme { /* Semantic color tokens */ }
    </style>
  </head>
  <body class="bg-background text-foreground font-mono antialiased">
    <!-- Your content -->
  </body>
</html>"#))
    }
}
