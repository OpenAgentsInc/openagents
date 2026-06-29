# Pylon: CLI-only, fully agent-steerable — audit + plan (2026-06-15)

Owner directive (2026-06-15): **strip the TUI out of Pylon (delete it), make Pylon
work via CLI only**, with commands like `pylon balance` for every interaction, so
**Pylon can be fully programmatically steered by an agent** — with parity for
everything that will have a GUI in the Autopilot desktop app. Agents should be
able to (1) help the owner download the Autopilot app, (2) build it from source
via GitHub, or (3) **install Pylon directly and steer it headlessly**.

## 1. Current state (audited)

Pylon (`apps/pylon`, `bin: pylon → src/index.ts`) has **three** surfaces today:

### A. CLI subcommands — already broad and agent-friendly
`src/index.ts` dispatches 24 top-level commands, most with `--json`:
`accounts`, `ask-artanis`, `assignment`, `attach`, `balance`, `bootstrap`,
`claim-tip-readiness`, `context`, `dev`, `forum`, `inventory`, `memories`,
`node`, `operator`, `presence`, `provider`, `psionic`, `runtime`, `status`,
`sweep-status`, `tip`, `tip-prefs`, `wallet`, `work`. Money commands route
through one node-side wallet-action path (`balance`/`wallet` project a
projection-safe subset — balance + readiness, never seed/offers). This is
already substantially headless/agent-steerable.

### B. Control server / client — the loopback steering API
`src/node/control-server.ts` + `control-client.ts`: a running node
(`pylon node`) binds loopback (`127.0.0.1:4716`) with a `control-token` and
serves typed commands (`session.spawn` / `session.list` / `session.cancel`,
approvals, etc.). This is exactly what **Autopilot Desktop drives** over the
shared `autopilot-control-protocol`. Already programmatic.

### C. The TUI — to be deleted
`src/tui/` (~2,790 LOC: `app.tsx`, `harness.tsx`, `dialogs.tsx`,
`network-scene.ts`, `store.ts`, `bridge.ts`, `commands.ts`, `theme.ts`), built on
**Solid + `@opentui/{core,keymap,solid,three}`**, is the interactive `dashboard`
command. `index.ts` reaches it only via a single **dynamic** `import("./tui/app")`
plus a Solid-transform **preload re-exec** dance (it re-spawns itself with
`--preload @opentui/solid/preload` before the dashboard boots).

**Separability:** nothing outside `src/tui/` imports the TUI — `grep` for
`from "./tui` / `"../tui"` across `src/**` (excluding `src/tui/`) returns
nothing. The TUI is a clean leaf: deleting it does not touch the node, control
server, CLI, wallet, or runtime.

## 2. The directive → gap analysis (Autopilot GUI surface → CLI parity)

Everything the Autopilot desktop GUI shows must be reachable headlessly. Mapping
the desktop panes (`apps/autopilot-desktop`) to Pylon access:

| Autopilot GUI surface | Pylon access today | Gap for CLI-only agent steering |
| --- | --- | --- |
| Node status / online | `pylon status --json`, control `GET /health` | OK |
| Wallet balance | `pylon balance`, `pylon wallet` | OK (ensure `--json`) |
| Sessions (list/spawn/cancel) | control API `session.*`; `pylon attach` | **Add first-class `pylon sessions list/spawn/cancel --json`** (today it's control-API/attach only) |
| Decisions / approvals | control API approval queue | **Add `pylon approvals list/approve/deny --json`** |
| Assignments | `pylon assignment …` | Verify full list/claim/closeout coverage + `--json` |
| Accepted work / closeouts | `pylon work …` | Verify coverage |
| Deploy | control API deploy-cloud actions | **Add `pylon deploy …` CLI** (or document control-API path) |
| Training lane (plan/activate/claim/admit/reconcile/closeout) | control intents (Autopilot drives these) | **Add `pylon training …` CLI** mirroring the desktop training cockpit verbs |
| Tips / earnings | `pylon tip`, `pylon sweep-status`, `pylon tip-prefs` | OK |
| Notifications / intents | `pylon ask-artanis`, intent intake | Verify `pylon intents`/notifications read |
| Accounts | `pylon accounts list/usage --json` | OK |

Cross-cutting gaps:
- **No machine-readable command catalog.** Add `pylon help --json` (and
  `pylon <cmd> --help`) so an agent can *discover* every command, its args, and
  whether it mutates/spends — the headless equivalent of "seeing the GUI."
- **Consistent `--json` + exit codes** on every command (some are
  human-text only).
- **One documented steering contract** for agents: CLI for one-shot ops +
  the control API for a long-lived node.

## 3. Plan

1. **Delete the TUI** (its own issue): remove `src/tui/`, drop
   `@opentui/{core,keymap,solid,three}` from `package.json`, delete the
   `dashboard` command + the dynamic `import("./tui/app")` + the
   Solid-preload re-exec machinery in `index.ts`. Keep node/control/CLI/wallet
   intact. Update the per-file test runner / release gate. Net: a smaller,
   headless, dependency-light Pylon.
2. **CLI parity for agent steering** (its own issue): add the missing
   first-class commands (`sessions`, `approvals`, `deploy`, `training`),
   normalize `--json` + exit codes, and add `pylon help --json` command
   catalog. Every Autopilot-GUI capability gets a CLI verb.
3. **Live AGENTS.md — three paths** (its own issue / done with this audit):
   teach arriving agents they can download the Autopilot app, build from source
   via GitHub, or install Pylon directly and steer it headlessly via the CLI +
   control API.

## 4. Risks / boundaries

- Do **not** regress the node, control server, wallet, or headless CLI when
  removing the TUI — they are independent of `src/tui/`, but `index.ts` surgery
  (removing the dynamic import + preload re-exec + `dashboard` branch) must keep
  the remaining command dispatch and `pylon node` intact.
- Keep money-command discipline: `balance`/`wallet` stay projection-safe (no
  seed/mnemonic/offers); spend stays approval-gated.
- Honesty: agent-steerability is a capability claim, not an earning claim —
  installing/steering Pylon grants no automatic earning (separate gated
  promises).

## 5. Product promise

Add `pylon.agent_steerable_cli.v1` (state `planned` at first) to the registry:
Pylon is a headless, CLI-only node an agent can fully steer — every Autopilot-GUI
capability reachable via a documented `pylon <command> --json` (plus the loopback
control API), with a machine-readable command catalog and no interactive TUI.
The existing green `pylon.cli_tui_probe_background.v1` is superseded on the TUI
dimension (the TUI is removed); reconcile its copy when the deletion lands.

## 6. Issues

Tracked as a Pylon CLI epic + children (delete TUI; CLI parity + catalog;
AGENTS.md three paths) for the 2026-06-15 launch — see the GitHub issues filed
with this audit and the docs/launch/JUNE15_LAUNCH_PLAN.md roadmap entry.
