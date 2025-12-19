# OpenAgents UI System

Server-rendered UI components using Maud + Tailwind CSS.

## Overview

The `ui` crate provides:

- **Tailwind CSS** via Play CDN (vendored for offline use)
- **Semantic color tokens** that map to CSS custom properties
- **Maud components** with builder patterns
- **Pre-push hooks** enforcing design conventions

## Quick Start

```rust
use ui::{Button, ButtonVariant, base_document, TAILWIND_CDN, TAILWIND_THEME};
use maud::{html, PreEscaped};

// Use base_document for full pages
let page = base_document("My Page", html! {
    div class="p-8" {
        h1 class="text-2xl font-bold mb-4" { "Hello" }
        (Button::new("Click me").render())
    }
});

// Or include Tailwind manually in custom layouts
html! {
    head {
        script { (PreEscaped(TAILWIND_CDN)) }
        style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
    }
}
```

## Documentation

- [Colors](./colors.md) - Semantic color system
- [Components](./components.md) - Available components
- [Conventions](./conventions.md) - Design rules and patterns

## Design Principles

1. **Tailwind-first** - All styling via Tailwind classes
2. **Semantic colors** - Use `bg-background`, never `bg-zinc-900`
3. **Sharp corners** - No border-radius anywhere
4. **Monospace** - Berkeley Mono / system monospace fonts
5. **Server-rendered** - No client-side JavaScript frameworks
