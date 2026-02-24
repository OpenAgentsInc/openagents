# Allow-Attribute Inventory (Wave 2)

Date: 2026-02-24
Issue: #2171

## Scope

Workspace Rust sources under `apps/**` and `crates/**` (`*.rs`).

## Inventory Snapshot

Command:

```bash
rg -n "#\[allow\(" apps crates --glob '*.rs' | wc -l
```

Current total: **194** `#[allow(...)]` occurrences.

Issue baseline was **236**, so this tranche reduces net allow count by **42**.

### Top Classes (current)

Command:

```bash
rg -o "#\[allow\([^\]]+\)\]" apps crates --glob '*.rs' \
  | sed -E 's/^.*#\[allow\(([^\]]+)\)\].*$/\1/' \
  | sort | uniq -c | sort -nr
```

Dominant classes:
- `dead_code`
- `clippy::too_many_arguments`
- `unused_imports`

### Targeted Core Crates (current)

Commands:

```bash
rg -n "#\[allow\(" crates/autopilot --glob '*.rs' | wc -l
rg -n "#\[allow\(" crates/nostr/core --glob '*.rs' | wc -l
rg -n "#\[allow\(" crates/wgpui --glob '*.rs' | wc -l
```

Results:
- `crates/autopilot`: **74**
- `crates/nostr/core`: **13**
- `crates/wgpui`: **36**

## Reduction Tranche Applied

### `crates/nostr/core`

Removed stale/redundant `#[allow(dead_code)]` attributes from public NIP helpers:
- `crates/nostr/core/src/nipc7.rs`
- `crates/nostr/core/src/nip65.rs`
- `crates/nostr/core/src/nip68.rs`
- `crates/nostr/core/src/nip69.rs`
- `crates/nostr/core/src/nip95.rs`

### `crates/autopilot`

Replaced broad dead-code allows with scoped `#[expect(dead_code, reason = ...)]` for retained-but-not-yet-wired fields/methods:
- `crates/autopilot/src/app/state.rs`
- `crates/autopilot/src/app/session/types.rs`
- `crates/autopilot/src/app_entry/state_actions.rs`

### `crates/wgpui`

Replaced broad dead-code allows with scoped expectations and removed stale allows on public methods:
- `crates/wgpui/src/platform.rs`
- `crates/wgpui/src/renderer.rs`

## Remaining Hotspots

Current highest concentration files include:
- `crates/autopilot/src/app/workspaces.rs` (17)
- `crates/wgpui/src/platform.rs` (10, primarily `clippy::too_many_arguments`)
- `crates/nostr/core/src/nip77.rs` (9)

These should be next tranche candidates (prefer API decomposition over blanket clippy allows).

## Verification

```bash
rg -n "#\[allow\(" apps crates --glob '*.rs' | wc -l
cargo check -p nostr --all-targets
cargo check -p autopilot --all-targets
cargo check -p wgpui --all-targets
```
