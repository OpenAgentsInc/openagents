# Pylon Self-Serve Registration Smoke

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`
Related issue: #500
Companion source commit: `OpenAgentsInc/openagents@b04ebe4be`

## Summary

#500 adds the source path for a fresh Pylon operator to install a public Pylon
release asset, derive or reuse a stable local Pylon identity, register with
OpenAgents product surface, send a heartbeat, and appear in the public-safe Pylon API.

This does not publish a new Pylon release. The Pylon release freeze from #499
remains active. The registration feature is in the `openagents` source on
`main`; packaging it for broad `npx @openagentsinc/pylon@latest` use belongs
to #505 after the full network-readiness sequence passes. The second-host
smoke also found that Linux currently resolves to a `pylon-v0.2.2` release
asset, not `pylon-v0.2.4`; that is release-asset alignment evidence for #505,
not a reason to move `latest` now.

## Source Change

`OpenAgentsInc/openagents@b04ebe4be` updates
`packages/pylon-bootstrap` with:

- `--register-openagents`;
- `--openagents-api`;
- `--openagents-agent-token` with `OPENAGENTS_AGENT_TOKEN` preferred;
- `--pylon-ref`;
- `--pylon-display-name`;
- `--resource-mode`;
- repeatable `--capability-ref`.

When enabled, the launcher runs the normal first-run smoke:

1. install or reuse the selected Pylon release asset;
2. run `pylon --help`;
3. run `pylon init`;
4. run `pylon status --json`;
5. run `pylon inventory --json`;
6. derive or reuse a public-safe Pylon ref;
7. `POST /api/pylons/register`;
8. `POST /api/pylons/{pylonRef}/heartbeat`.

The registration payload uses public-safe refs only. It excludes local paths,
hostnames, SSH details, wallet files, raw wallet material, private model
inventory, provider credentials, raw invoices, payment hashes, preimages, and
exact balances.

## Command

Current source-controlled command, before #505 publishes a new package:

```bash
set -a
source /Users/christopherdavid/work/.secrets/openagents-artanis-agent.env
set +a

TMP_ROOT="$(mktemp -d /tmp/pylon-issue500.XXXXXX)"
mkdir -p "$TMP_ROOT/home" "$TMP_ROOT/pylon-home" "$TMP_ROOT/install"

HOME="$TMP_ROOT/home" \
OPENAGENTS_PYLON_HOME="$TMP_ROOT/pylon-home" \
OPENAGENTS_PYLON_CONFIG_PATH="$TMP_ROOT/pylon-config.json" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
OPENAGENTS_AGENT_TOKEN="$OPENAGENTS_AGENT_TOKEN" \
packages/pylon-bootstrap/bin/pylon \
  --install-root "$TMP_ROOT/install" \
  --register-openagents \
  --openagents-api https://openagents.com \
  --pylon-ref "pylon.issue500.local.$(date -u +%Y%m%d%H%M%S)" \
  --pylon-display-name "Issue 500 Local Smoke Pylon" \
  --resource-mode background_20 \
  --no-launch \
  --json
```

The future post-#505 public package command is expected to be:

```bash
export OPENAGENTS_AGENT_TOKEN="oa_agent_..."

npx @openagentsinc/pylon@latest \
  --register-openagents \
  --openagents-api https://openagents.com \
  --resource-mode background_20 \
  --no-launch \
  --json
```

Do not publish a new package or move npm `latest` just to expose that command.

## Local Production Smoke

Clean local smoke on this Mac used a fresh temporary HOME, Pylon home, config
path, and install root. It used the source-controlled launcher from
`OpenAgentsInc/openagents@b04ebe4be`, but installed the public
`pylon-v0.2.4` release asset.

Public-safe result:

```json
{
  "pylonRef": "pylon.issue500.local.20260608021727",
  "status": "online",
  "registration": {
    "idempotent": false,
    "publicUrl": "https://openagents.com/api/pylons/pylon.issue500.local.20260608021727"
  },
  "heartbeat": {
    "idempotent": false
  },
  "version": "0.2.4",
  "tagName": "pylon-v0.2.4",
  "installMethod": "release_asset"
}
```

Production readback from
`GET https://openagents.com/api/pylons/pylon.issue500.local.20260608021727`:

```json
{
  "pylon": {
    "pylonRef": "pylon.issue500.local.20260608021727",
    "displayName": "Issue 500 Local Smoke Pylon",
    "status": "active",
    "latestHeartbeatDisplay": "Just now",
    "walletReady": false
  },
  "events": [
    {
      "eventKind": "heartbeat",
      "status": "online",
      "createdAtDisplay": "Just now"
    },
    {
      "eventKind": "registration",
      "status": "active",
      "createdAtDisplay": "Just now"
    }
  ]
}
```

## Second-Host Smoke

After Tailscale was re-enabled, `archlinux` was reachable over Tailnet SSH.
The same source-controlled launcher path was run from a fresh temporary HOME,
Pylon home, config path, install root, and clean clone of `OpenAgentsInc/openagents`.

Public-safe result:

```json
{
  "pylonRef": "pylon.issue500.archlinux.20260608022040",
  "status": "online",
  "version": "0.2.2",
  "tagName": "pylon-v0.2.2",
  "installMethod": "release_asset"
}
```

Production readback from
`GET https://openagents.com/api/pylons/pylon.issue500.archlinux.20260608022040`:

```json
{
  "pylon": {
    "pylonRef": "pylon.issue500.archlinux.20260608022040",
    "displayName": "Issue 500 Arch Smoke Pylon",
    "status": "active",
    "latestHeartbeatDisplay": "Just now",
    "walletReady": false
  },
  "events": [
    {
      "eventKind": "heartbeat",
      "status": "online",
      "createdAtDisplay": "Just now"
    },
    {
      "eventKind": "registration",
      "status": "active",
      "createdAtDisplay": "Just now"
    }
  ]
}
```

This proves second-host self-serve registration and heartbeat. It does not
prove full release readiness because the Linux path resolved to `pylon-v0.2.2`
instead of `pylon-v0.2.4`, and no wallet, assignment, payout, receipt, repeated
run, or failure drill was part of #500.

## Verification

Focused launcher tests:

```bash
bun test packages/pylon-bootstrap/test/cli.test.js \
  packages/pylon-bootstrap/test/bootstrap.test.js
```

Result:

```text
45 pass
0 fail
```

## Remaining Boundaries

- #500 proves source-level self-serve registration and one local production
  smoke plus one reachable Arch Linux second-host registration/heartbeat smoke.
- #505 must align downloadable release assets for the intended platforms before
  broad install or earning claims.
- #501 now proves source-level MDK agent-wallet setup, redacted wallet
  readiness, and payout-target admission for one registered Pylon. The retained
  evidence lives in
  `docs/nexus/2026-06-07-pylon-mdk-wallet-readiness-smoke.md`.
- #502 still owns live work assignment, execution, proof upload, and
  accepted-work closeout.
- #503 still owns real bitcoin payout and receipt projection for accepted
  work.
- #504 still owns repeated multi-Pylon, multi-host smokes and failure drills.
- #505 still owns the next downloadable release and npm `latest` movement after
  all network readiness gates pass.
