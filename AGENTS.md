# Repository Guidelines

## Project Structure & Module Organization
- `src/` — Leptos (CSR) UI; entry points `src/main.rs`, `src/app.rs`. Add UI modules under `src/components/` or `src/pages/`.
- `index.html` — Trunk entry; Tailwind (Play CDN) and Berkeley Mono font-face. Static assets in `public/` (copied to `dist/`).
- `public/fonts/` — Berkeley Mono TTFs used globally.
- `src-tauri/` — Tauri desktop app (`src-tauri/src/lib.rs`, `src-tauri/src/main.rs`), config in `src-tauri/tauri.conf.json`.
- `Cargo.toml` (workspace) and `Trunk.toml` — build configuration. Release assets to `dist/`.

## Build, Test, and Development Commands
Prereqs: `rustup target add wasm32-unknown-unknown`; `cargo install trunk tauri-cli`.
- Dev (web): `trunk serve` → http://localhost:1420
- Dev (desktop): `cd src-tauri && cargo tauri dev` (runs Trunk via `beforeDevCommand`).
- Build (web): `trunk build --release` → `dist/`
- Build (desktop): `cd src-tauri && cargo tauri build`
- Tests (workspace): `cargo test` or per crate: `cargo test -p openagents`

## Coding Style & Naming Conventions
- Format: `cargo fmt --all`; Lint: `cargo clippy --all -- -D warnings`.
- Rust naming: modules `snake_case`, types/traits/components `PascalCase`, fns/vars `snake_case`, consts `SCREAMING_SNAKE_CASE`.
- Styling: Tailwind via Play CDN only (no standalone CSS). Berkeley Mono is the site-wide font; prefer utility classes.
- Leptos: components use `#[component]` and `PascalCase` (e.g., `pub fn Sidebar()`); prefer small, focused modules.
- Tauri: commands annotated with `#[tauri::command]` in `src-tauri/src/lib.rs` and wired via `invoke_handler`.

## Testing Guidelines
- Use `cargo test` for Rust logic (primarily `src-tauri`). Place unit tests alongside modules; integration tests in `src-tauri/tests/`.
- UI (WASM) tests are limited; test pure Rust logic extracted from components where possible.
- Ensure tests run cleanly with `--all-features` if applicable.

## Commit & Pull Request Guidelines
- Commits: short imperative title (≤72 chars), optional body for context (e.g., "Add Tauri greet command"). Conventional Commits not required.
- PRs: include clear description, linked issues, and screenshots/GIFs for UI changes. Note config/security changes (e.g., `tauri.conf.json`). Keep diffs focused and formatted.

## Security & Configuration Tips
- Tauri CSP is `null` for dev; restrict before release if loading remote content. Avoid embedding secrets; prefer environment or OS keychain.
- `frontendDist` points to `../dist`; ensure `trunk build` precedes packaging.

## Build Health (Required)
- Do not leave the repository in a broken state. All changes must compile:
  - UI: `cargo check --target wasm32-unknown-unknown`
  - Tauri: `cd src-tauri && cargo check`
  - Optional: `trunk build` and `cargo tauri build` for end-to-end validation.
