# CLI Workflow

This skill assumes the seller flow remains app-owned. The shell path drives the
same seller logic through `autopilotctl`.

## 1. Start or target a runtime

For real publish, first start local `nexus-control`, mint a desktop session,
and export:

- `OA_CONTROL_BASE_URL`
- `OA_CONTROL_BEARER_TOKEN`

Without that local authority session you can still preview drafts, but you do
not have the authority path required for real asset, grant, delivery, or
revocation truth.

If no normal desktop session is serving desktop-control, start the no-window
host:

```bash
cargo run -p autopilot-desktop --bin autopilot_headless_data_market -- \
  --manifest-path /tmp/openagents-data-market-desktop-control.json
```

Use the same manifest path for all later commands:

```bash
MANIFEST=/tmp/openagents-data-market-desktop-control.json
```

Leave Codex enabled if you plan to use `autopilotctl data-market seller-prompt`
against this runtime. The repo-owned smoke and E2E harnesses only set
`OPENAGENTS_DISABLE_CODEX=true` because they use the typed DS-first CLI flow
directly.

## 2. Package local material

```bash
skills/autopilot-data-seller-cli/scripts/package_data_asset.sh \
  --source ./my-data \
  --output-dir ./tmp/package \
  --title "My Data Bundle" \
  --price-sats 250
```

If the material is a bundle of Codex conversations rather than an arbitrary
directory, use the dedicated redaction packager instead:

```bash
skills/autopilot-data-seller-cli/scripts/package_codex_conversations.sh \
  --limit 5 \
  --output-dir ./tmp/codex-package \
  --title "Redacted Codex conversation bundle" \
  --price-sats 500
```

That wrapper:

1. reads rollout JSONL from `~/.codex/sessions` or explicit `--session` paths
2. exports a redacted conversation bundle into `redacted-codex-conversations/`
3. emits the normal `listing-template.json` and `grant-template.json`

## 3. Inspect seller state first

```bash
cargo run -p autopilot-desktop --bin autopilotctl -- \
  --manifest "$MANIFEST" --json data-market seller-status
```

## 4. Draft, preview, and publish the asset

```bash
skills/autopilot-data-seller-cli/scripts/publish_asset.sh \
  --manifest "$MANIFEST" \
  --file ./tmp/package/listing-template.json
```

That wrapper runs:

1. `data-market draft-asset --file ...`
2. `data-market preview-asset`
3. `data-market publish-asset --confirm`
4. `data-market snapshot`

## 5. Draft, preview, and publish a grant

```bash
skills/autopilot-data-seller-cli/scripts/publish_grant.sh \
  --manifest "$MANIFEST" \
  --file ./tmp/package/grant-template.json
```

That wrapper runs:

1. `data-market draft-grant --file ...`
2. `data-market preview-grant`
3. `data-market publish-grant --confirm`
4. `data-market snapshot`

## 6. Continue with post-sale control

Use the semantic CLI for later lifecycle steps:

- `data-market request-payment --request-id ...`
- `data-market prepare-delivery --request-id ... --file ...`
- `data-market issue-delivery --request-id ...`
- `data-market revoke-grant --request-id ... --action revoke|expire --confirm`

After every mutation, read back with at least one of:

- `data-market seller-status`
- `data-market snapshot`

## Required discipline

- Preview before every publish.
- Publish or revoke only with explicit `--confirm`.
- Read back after every mutation.
- Do not claim a listing, grant, delivery, or revocation exists until the
  returned state shows the canonical id or receipt.

## Verification posture

- Portable local gate:
  `scripts/autopilot/verify-data-market-cli-headless.sh`
- Live public-relay probe:
  `scripts/autopilot/headless-data-market-public-e2e.sh`
- Treat the local verifier as the deterministic launch gate and the public
  harness as an operator probe.
