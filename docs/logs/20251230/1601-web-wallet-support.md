# Web wallet support

Date: 2025-12-30

## Summary
- Wired the web client to Spark wallet endpoints and added a WGPUI wallet panel in the right dock (overview/send/receive).
- Added input event dispatch and async wallet actions (refresh/send/receive) with API response parsing and UI notices.
- Updated web and Spark docs to describe wallet routes, config, and entropy-based signer/storage guidance.

## Implementation details
- Added wallet request helpers in the web client for `/api/wallet/summary`, `/api/wallet/send`, and `/api/wallet/receive` plus JSON parsing for balances, addresses, payments, and invoices.
- Implemented wallet input event handling (mouse + key) and action dispatch to keep text input focus from clobbering dock hotkeys.
- Replaced the right dock usage placeholder with the wallet UI and expanded payment list rendering to fill available space.
- Documented Spark entropy seeding, wasm storage notes, and web wallet configuration/env vars.

## Files touched
- `crates/web/client/src/lib.rs`
- `crates/web/docs/README.md`
- `crates/web/docs/client-ui.md`
- `crates/web/docs/architecture.md`
- `crates/spark/README.md`
- `crates/spark/docs/README.md`
- `crates/spark/docs/CONFIGURATION.md`
- `crates/spark/docs/API.md`

## Builds
- Not run (not requested).
