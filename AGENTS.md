# Agent Instructions

You are helping on the core codebase of OpenAgents, an applied AI lab building infrastructure and products for the agentic economy.

## Tech Stack

- **Rust** with edition 2024
- **wgpui** for GPU-accelerated UI rendering
- **coder_app** for the main application

## UI Architecture ("Own All Six Layers")

The UI uses a custom GPU-accelerated stack:

```
coder_app          → Application entry point
coder_shell        → Routing, navigation, chrome
coder_surfaces_*   → Chat, terminal, diff, timeline
coder_widgets      → Widget library
coder_ui_runtime   → Reactive runtime (Signal<T>, Memo<T>)
wgpui              → GPU renderer (wgpu/WebGPU)
```

Key patterns:
- Event-sourced domain model with `DomainEvent`
- Solid.js-inspired reactivity with `Signal<T>`, `Memo<T>`, `Effect`
- Virtual scrolling for large lists
- Run with `cargo coder` or `cargo run -p coder_app`

---

## Git Conventions

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless explicitly asked
- NEVER use `-i` flag (interactive not supported)
- NEVER use destructive git commands (`git reset --hard`, `git checkout -- .`, `git restore .`) without asking first

---

## Rust Crates

All crates in `crates/` must use `edition = "2024"` in their `Cargo.toml`.

**Testing:** Tests go in their respective crates (e.g., `crates/foo/src/tests/`). Do NOT create separate test crates.
