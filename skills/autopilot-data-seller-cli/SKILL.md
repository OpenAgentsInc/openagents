---
name: autopilot-data-seller-cli
description: Shell-first OpenAgents DS-first Data Market packaging and publication workflow using the deterministic packaging helper, autopilotctl, and the no-window headless runtime.
metadata:
  oa:
    project: openagents
    identifier: autopilot-data-seller-cli
    version: "0.3.0"
    expires_at_unix: 1798761600
    capabilities:
      - codex:shell
      - data-market:packaging
      - data-market:cli-publication
      - data-market:headless-control
      - data-market:conversation-redaction
---

# Autopilot Data Seller CLI

Use this skill when the task is to package local material for sale and publish
or manage it through CLI, not through the visible `Data Seller` pane.

## Quick start

- For real publish, point the headless runtime at the relays you actually want
  to use; you do not need `nexus-control` or `OA_CONTROL_*` for the DS-first
  seller/buyer flow.
- Package local files or folders with
  [`scripts/package_data_asset.sh`](scripts/package_data_asset.sh).
- Package redacted Codex conversations with
  [`scripts/package_codex_conversations.sh`](scripts/package_codex_conversations.sh).
- Start the no-window runtime when needed:
  `cargo run -p autopilot-desktop --bin autopilot_headless_data_market -- --manifest-path ...`
- Inspect truth first with:
  `cargo run -p autopilot-desktop --bin autopilotctl -- --manifest ... --json data-market seller-status`
- Follow the semantic CLI order:
  draft asset -> preview asset -> publish asset -> snapshot -> draft grant ->
  preview grant -> publish grant -> payment -> delivery -> revoke
- Use `seller-prompt` only when you intentionally want the conversational
  seller lane in a terminal session. Prefer the packaged CLI path for
  deterministic DS-first publication.

## Required operating rules

1. Use semantic CLI commands only. Do not simulate pane clicks.
2. Package before drafting when local files still need digest/provenance truth.
3. Preview before every publish.
4. Pass `--confirm` for publish or revoke only after preview or intent has been
   explicitly checked.
5. Read back state after every mutation with `seller-status` or `snapshot`.
6. Do not invent `content_digest`, `provenance_ref`, policy, price, or delivery
   posture.
7. Keep packaging metadata flat and string-valued so it remains compatible with
   the seller tool contract.
8. For Codex session bundles, default to the redacted conversation packager
   rather than hand-editing rollout JSONL or packaging raw `.codex` files.
9. Developer/system prompt material should stay excluded unless the user
   explicitly asks to include it after redaction.
10. Before publish, inspect the exported bundle for any project-specific names
    or literals that still need scrubbing and rerun packaging with `--scrub`
    when needed.
11. Treat DS listing and DS offer publication as the public market truth, and
    treat DS-DVM request/result traffic as the targeted fulfillment layer.
12. Use `scripts/autopilot/verify-data-market-cli-headless.sh` as the portable
    local launch gate. Treat the public-relay harness as an operator probe,
    not the deterministic gate.
13. Do not set `OPENAGENTS_DISABLE_CODEX=true` if the plan depends on
    `seller-prompt`; that flag is for the typed repo-owned verification flows.

## When to read references

- Read [references/packaging-contract.md](references/packaging-contract.md)
  before packaging or editing emitted JSON.
- Read
  [references/codex-conversation-redaction.md](references/codex-conversation-redaction.md)
  before packaging Codex sessions or editing redacted conversation bundles.
- Read [references/cli-workflow.md](references/cli-workflow.md) for the
  end-to-end flow from package to published asset/grant.
- Read
  [references/policy-template-cheatsheet.md](references/policy-template-cheatsheet.md)
  when selecting `default_policy` or `policy_template`.

## Scripts

- `scripts/package_data_asset.sh`: thin wrapper around the deterministic local
  packaging helper.
- `scripts/package_codex_conversations.sh`: redact recent or explicit Codex
  rollout sessions and turn them into normal Data Market draft artifacts.
- `scripts/publish_asset.sh`: semantic asset draft/preview/publish/snapshot
  flow.
- `scripts/publish_grant.sh`: semantic grant draft/preview/publish/snapshot
  flow.

## Boundary

- This skill is shell-first, but it still targets the app-owned Data Seller
  logic through `autopilotctl`.
- Use the dedicated pane skill for conversational in-app seller work.
- Do not create a parallel publication path that bypasses preview, confirm, or
  relay-backed status/snapshot read-back.

## Verification truth

- Portable local verifier:
  `scripts/autopilot/verify-data-market-cli-headless.sh`
- Fresh paid local DS-first audit:
  `docs/audits/2026-03-21-ds-first-headless-data-market-paid-e2e-audit.md`
- Live public-relay probe:
  `scripts/autopilot/headless-data-market-public-e2e.sh`
