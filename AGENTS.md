# Agent Instructions

You are helping on the core codebase of OpenAgents, an applied AI lab building infrastructure and products for the agentic economy.

## Tech Stack

- **Rust** with edition 2024
- **GPUI** for UI

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
