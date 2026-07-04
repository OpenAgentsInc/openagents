# Khala Code Outside-User Run Receipt Template

Status: public-safe template for RL-3 / issue #8247.

Khala Code can post a public run receipt only when the user chooses the
Settings -> Run evidence action. There is no background phone-home path.

## Public Receipt Fields

The receipt may include:

- app version
- platform: `darwin`, `linux`, `win32`, or `other`
- architecture: `arm64`, `x64`, or `other`
- distribution channel: `desktop_dmg`, `npm_cli`, `source_build`, or `unknown`
- Codex CLI readiness: `ready`, `missing`, or `unknown`
- Codex auth readiness: `ready`, `credentials_missing`, `invalid`, `error`, or
  `unknown`
- Pylon readiness: `ready`, `unavailable`, `not_configured`, or `unknown`

The receipt must not include paths, prompts, tokens, logs, account identifiers,
machine identifiers, repo names, branch names, raw config, or screenshots.

## Manual Shape

The desktop app generates this body from safe local facts. If an outside user
needs to submit manually, keep the same shape:

```json
{
  "schemaVersion": "openagents.khala_code.outside_user_run_intake.v1",
  "consent": {
    "publicReceipt": true,
    "noPrivateDataIncluded": true
  },
  "appVersion": "0.0.1",
  "platform": "darwin",
  "arch": "arm64",
  "distributionChannel": "source_build",
  "harnessReadiness": {
    "codexCli": "ready",
    "codexAuth": "ready",
    "pylon": "unknown"
  },
  "idempotencyKey": "outside-user-run-REPLACE-ME"
}
```

```sh
curl -sS https://openagents.com/api/public/khala-code/outside-user-runs \
  -H 'content-type: application/json' \
  --data @khala-code-run-receipt.json
```

The response returns `receipt.receiptRef` and `receipt.receiptUrl`. Cite the
receipt URL as evidence only. It does not prove a signed DMG exists, does not
turn on free-plan trace capture, and does not flip
`khala_code.desktop_codex_wrapper.v1` green by itself.
