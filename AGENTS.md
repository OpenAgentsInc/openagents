# Agent Instructions

You are helping on the core codebase of OpenAgents, an applied AI lab building infrastructure and products for the agentic economy.

## Tech Stack

- **Rust** with edition 2024
- **Dioxus 0.7** for web UI (see `crates/dioxus/`)
- **wgpui** for GPU-accelerated canvas rendering

## Dioxus 0.7

The web UI uses [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) with fullstack and router features. Key points: `cx`, `Scope`, and `use_state` are gone in 0.7. Use `use_signal` for local state, `use_memo` for derived values, and `use_resource` for async data. Components are functions with `#[component]` that return `Element`. Server functions use `#[post("/path")]` or `#[get("/path")]` macros. Run with `dx serve` from `crates/dioxus/`.

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
