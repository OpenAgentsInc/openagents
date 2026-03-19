# Packaging Contract

Use the deterministic packaging helper before drafting an asset when local
files still need digest or provenance truth.

Primary entrypoint:

```bash
skills/autopilot-data-seller-cli/scripts/package_data_asset.sh \
  --source ./my-data \
  --output-dir ./tmp/package \
  --title "My Data Bundle" \
  --price-sats 250
```

That wrapper calls:

```bash
scripts/autopilot/data_market_package.py
```

For Codex rollout conversations, use:

```bash
skills/autopilot-data-seller-cli/scripts/package_codex_conversations.sh
```

That wrapper calls:

```bash
scripts/autopilot/package_codex_conversations.py
```

## Emitted files

- `listing-template.json`
- `grant-template.json` unless `--skip-grant-template`
- `packaging-manifest.json`
- `packaging-summary.json`
- `redacted-codex-conversations/` and `conversation-index.json` when packaging
  Codex sessions

## Important constraints

- Packaging metadata must stay flat and string-valued so it remains compatible
  with the seller draft contract.
- Do not invent `content_digest` or `provenance_ref` in prose when packaging
  can derive them.
- Re-run packaging if the underlying source material changes.
- Treat emitted JSON as draft input, not published market truth.
- For Codex session sales, do not publish raw rollout files without running the
  redaction packager first unless the user explicitly asks for unredacted
  material.

## Listing template mapping

The emitted `listing-template.json` is designed to feed:

```bash
autopilotctl data-market draft-asset --file ...
```

Typical fields include:

- `asset_kind`
- `title`
- `description`
- `content_digest`
- `provenance_ref`
- `default_policy`
- `price_hint_sats`
- `delivery_modes`
- `visibility_posture`
- `sensitivity_posture`
- `metadata`

The emitted `grant-template.json` is designed to feed:

```bash
autopilotctl data-market draft-grant --file ...
```

Typical fields include:

- `consumer_id`
- `policy_template`
- `price_sats`
- `expires_in_hours`
- `warranty_window_hours`
- `delivery_modes`
- `metadata`
