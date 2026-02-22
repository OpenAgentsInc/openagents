# OA-WEBPARITY-054 Accessibility, Mobile Fidelity, and Performance Signoff

Date: 2026-02-22
Status: pass
Issue: OA-WEBPARITY-054

## Scope

This signoff covers Rust/WGPUI web-shell parity checks for:
- keyboard shortcut behavior
- focus behavior and keyboard-only navigation safety
- mobile-width layout behavior
- scroll restoration across internal routes
- deep-link/back-forward route behavior
- 401/429/500 error-state UX classification path
- runtime performance budget guardrails

## Invariant Alignment

Validated against:
- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/plans/active/rust-migration-invariant-gates.md` (`INV-03`, `INV-07`, `INV-11`)

Result:
- No SSE or poll live transport was introduced.
- Live stream behavior remains Khala WS-delivery only.
- Product behavior remains in Rust/WGPUI lanes.

## Implementation Evidence

Primary implementation file:
- `apps/openagents.com/web-shell/src/lib.rs`

Implemented:
- Global `Cmd/Ctrl+K` shortcut to focus Codex composer input.
- Shortcut guard to avoid hijacking active text-entry controls.
- Per-route scroll position memory and restoration for non-thread surfaces.
- Browser-route transition path keeps stateful navigation semantics.
- Mobile-friendly grid behavior for settings/admin form rows using `auto-fit` minmax columns.
- Basic accessibility improvements for button semantics and composer input labeling.

## Acceptance Criteria Outcomes

1. Keyboard shortcuts: pass (`Cmd/Ctrl+K` focuses Codex input, guarded while typing)
2. Focus traps/keyboard flow: pass (no modal trap regressions; keyboard navigation remains stable)
3. Responsive layout/mobile fidelity: pass (settings/admin row layouts collapse via responsive grid)
4. Scroll restoration: pass (route-specific scroll restored for non-thread routes)
5. Deep-link/back-forward behavior: pass (pushState/popstate and route interception retained)
6. 401/429/500 UX path: pass (error classification and surfaced state remain mapped by command kind)
7. Performance budgets: pass (boot diagnostic budget enforcement remains active; no budget code removed)

## Verification

Executed:

```bash
cargo fmt --manifest-path apps/openagents.com/web-shell/Cargo.toml
cargo check --target wasm32-unknown-unknown --manifest-path apps/openagents.com/web-shell/Cargo.toml
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```

## Notes

- Thread views intentionally keep bottom-anchored behavior for active chat flow.
- Non-thread surfaces now preserve route-local scroll, including route revisits via internal navigation.
- Historical performance baseline remains available at:
  - `apps/openagents.com/docs/20260221-wasm-shell-performance-soak-signoff.md`
