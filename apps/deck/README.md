# `apps/deck`

Browser deck app for OpenAgents.

## Scope

- own the browser boot path for presentation viewing
- keep typed deck/domain logic out of `crates/wgpui`
- render the first six-slide `Autopilot + Five Markets` deck from a markdown-plus-metadata source file

## Build

1. `rustup target add wasm32-unknown-unknown`
2. `cargo build -p deck --target wasm32-unknown-unknown`
3. `wasm-bindgen --target web --out-dir apps/deck/static/pkg target/wasm32-unknown-unknown/debug/deck.wasm`
4. Serve `apps/deck/static/` over HTTP and open `index.html`

The HTML shell imports `./pkg/deck.js`, so the generated bindgen output should live in `apps/deck/static/pkg/`.

The current embedded deck source lives at `apps/deck/content/five-markets.deck.md`.

## Use A Local Deck

The deck app can embed a different source file at build time.

Set `OPENAGENTS_DECK_SOURCE` before running the build:

```bash
OPENAGENTS_DECK_SOURCE=docs/local/openagents-seed.deck.md cargo test -p deck
cd apps/deck
OPENAGENTS_DECK_SOURCE=../../docs/local/openagents-seed.deck.md bun run build:assets
python3 -m http.server -d dist 8000
```

Rules:

- absolute paths work
- relative paths are resolved from either `apps/deck/` or the workspace root
- if `OPENAGENTS_DECK_SOURCE` is unset, the app falls back to `content/five-markets.deck.md`

## Cloudflare

This app now follows the historical `crates/web` deployment shape from late December 2025:

- build the wasm client bundle
- copy static assets into `dist/`
- compile a Rust Cloudflare Worker in `worker/`
- let Wrangler deploy the worker with `dist/` as the assets binding

Prerequisites:

- Node.js `>= 20`
- `bun`
- `wasm-pack`
- `rustup target add wasm32-unknown-unknown`

Commands:

1. `bun install`
2. `bun run build`
3. `fnm exec --using=22.16.0 bunx wrangler deploy`

The worker serves the deck assets from the `ASSETS` binding and adds the same COOP/COEP-style headers the old web lane used for browser GPU safety.

## Source Format

Deck files are markdown with TOML metadata blocks.

- Deck-level metadata goes first and is wrapped in `+++` fences.
- Slides are separated by a standalone `---` line outside fenced code blocks.
- Each slide may begin with its own `+++` TOML metadata block.

Example:

```md
+++
title = "Bootstrap Deck"
theme = "hud"
+++

---
+++
title = "Intro"
layout = "title"
notes = """
Speaker notes live here.
"""
+++
# Hello

- markdown body
- typed slide metadata
```
