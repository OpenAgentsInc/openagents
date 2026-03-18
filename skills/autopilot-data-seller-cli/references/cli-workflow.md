# CLI Workflow

This skill assumes the seller flow remains app-owned. The shell path drives the
same seller logic through `autopilotctl`.

## 1. Start or target a runtime

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

## 2. Package local material

```bash
skills/autopilot-data-seller-cli/scripts/package_data_asset.sh \
  --source ./my-data \
  --output-dir ./tmp/package \
  --title "My Data Bundle" \
  --price-sats 250
```

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
