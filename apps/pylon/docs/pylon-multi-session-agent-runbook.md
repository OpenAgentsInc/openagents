# Pylon agent steering runbook (CLI-only, multi-session)

Pylon is a headless, CLI-only node. Every capability the Autopilot desktop GUI
shows is reachable from an agent with a documented `pylon <command> --json`
(plus the loopback control API for a long-lived node). This runbook is the
single steering contract for arriving agents.

There is no interactive TUI (removed in #5034). Do not look for a `dashboard`
or `attach` viewer.

## 1. Discover the surface first

```
pylon help --json          # machine-readable command catalog
pylon <command> --help     # one command's args + flags as JSON
```

`pylon help --json` emits `openagents.pylon.command_catalog.v1`: every command,
its args, and two authority flags per command:

- `mutates` — changes node/server/remote state.
- `spends` — can move sats. Only `wallet`, `work`, and `tip` are `spends:true`.
- `needsNode` — requires a running `pylon node` (loopback control API).
- `needsNetwork` — needs `--base-url` / `PYLON_OPENAGENTS_BASE_URL`.

All commands speak `--json` and use honest exit codes (nonzero on failure).

## 2. Two steering modes

1. **One-shot CLI ops** — `pylon status`, `pylon balance`, `pylon wallet`,
   `pylon accounts`, `pylon assignment`, `pylon work`, `pylon tip`,
   `pylon training …`, etc. These run without a long-lived node.
2. **Long-lived node + loopback control API** — `pylon node` binds
   `127.0.0.1:4716` with a per-home `control-token` and serves the session,
   approval, and deploy verbs. The CLI verbs `pylon sessions`,
   `pylon approvals`, and `pylon deploy` are first-class wrappers over that
   control API — the same surface the Autopilot desktop drives.

### Resolving the control endpoint

CLI control verbs resolve the endpoint exactly like the node:

- Home: `PYLON_HOME` or `~/.pylon`.
- Token: `PYLON_CONTROL_TOKEN`, else `<home>/control-token` (written by the
  node on first boot — read-only from the CLI side; the CLI never mints it).
- URL: `PYLON_CONTROL_URL`, else `http://<PYLON_CONTROL_HOST|127.0.0.1>:<PYLON_CONTROL_PORT|4716>`.

If no token is found you get `{ "ok": false, "code": "no_token" }`; if nothing
is listening you get `{ "ok": false, "code": "no_node" }`. Both exit nonzero.

## 3. Session steering (`pylon sessions`)

```
pylon sessions list
pylon sessions spawn --adapter codex|claude_agent --objective "<text>" \
  [--verify "bun test"] [--worktree <path>]
pylon sessions cancel --session-ref <ref>
```

Wraps the control-server `session.list` / `session.spawn` / `session.cancel`
verbs. The node owns execution; the CLI only forwards the command.

## 4. Approvals (`pylon approvals`)

```
pylon approvals list
pylon approvals approve --approval-ref <ref>
pylon approvals deny --approval-ref <ref>
```

Wraps the node's exactly-once operator approval queue (`approvals.list` /
`approvals.resolve`). Approving a labor first-run grant is the one side effect.

## 5. Deploy (`pylon deploy`)

```
pylon deploy status
pylon deploy cloud --target <target> --ref <ref> [--env <env>]
```

Wraps `deploy.status` / `deploy.cloud`. Execution stays gated on the node
behind `OA_DEPLOY_ENABLE=1` (fail-safe): with the gate off the node returns
`accepted:false reason:deploy_disabled` and nothing runs. The CLI verb is a
control surface only — it never deploys directly.

## 6. Training cockpit (`pylon training`)

Mirrors the desktop training cockpit verbs against the openagents.com training
HTTP API. Admin verbs need an admin token (`--admin-token` or
`OA_TRAINING_ADMIN_TOKEN`); without one they fail cleanly with a nonzero exit.

```
pylon training plan      --base-url <url> --admin-token <tok>            # POST runs + windows/plan
pylon training activate  --base-url <url> --admin-token <tok> --window-ref <ref>
pylon training reconcile --base-url <url> --admin-token <tok> --window-ref <ref>
pylon training closeout  --base-url <url> --admin-token <tok> --window-ref <ref>
pylon training claim     --base-url <url> [--pylon-ref <ref>] [--lease-seconds N]  # public lease
pylon training admit     --base-url <url> --admin-token <tok> --run-ref <ref> --packet <evidence.json>
pylon training status    --base-url <url>                                # public runs projection
```

`claim` defaults `--pylon-ref` to this node's identity when omitted.

## 7. Money discipline

- `pylon balance` / `pylon wallet status` are projection-safe: balance +
  readiness only, never seed/mnemonic/offers.
- Spend stays approval-gated. A CLI verb adds no new spend/settlement authority;
  it routes through the existing node-side wallet-action path.
- Agent-steerability is a capability claim, not an earning claim. Installing or
  steering Pylon grants no automatic earning.

## 8. GUI surface → CLI parity map

| Autopilot GUI surface | Headless CLI |
| --- | --- |
| Node status / online | `pylon status --json` |
| Wallet balance | `pylon balance`, `pylon wallet status` |
| Sessions (list/spawn/cancel) | `pylon sessions list|spawn|cancel` |
| Decisions / approvals | `pylon approvals list|approve|deny` |
| Assignments | `pylon assignment poll|accept|progress|closeout` |
| Accepted work / closeouts | `pylon work submit|status|review|request|offers|accept` |
| Deploy | `pylon deploy cloud|status` |
| Training lane | `pylon training plan|activate|claim|admit|reconcile|closeout|status` |
| Tips / earnings | `pylon tip`, `pylon sweep-status`, `pylon tip-prefs` |
| Accounts | `pylon accounts list|usage` |
| Discover everything | `pylon help --json`, `pylon <cmd> --help` |
