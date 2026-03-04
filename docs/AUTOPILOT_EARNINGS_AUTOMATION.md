# Autopilot Earnings Automation Guide

## Purpose

This guide covers how to run the "earn bitcoin on autopilot" flow from Autopilot Chat with goal automation, scheduler controls, BTC/USD swap support, and authoritative payout checks.

## Scope Note: Revenue Lanes

Autopilot Earn is a multi-lane provider model:

- compute provider lane (active in MVP),
- liquidity solver lane (future Hydra lane).

This runbook currently covers compute-lane automation and wallet/swap controls around that loop. Solver-lane automation is future scope and must be explicitly opt-in when introduced.

## Prerequisites

- OpenAgents Desktop is running and wallet is connected.
- At least one earnings goal exists in `autopilot_goals` state.
- Required skills are enabled (`blink`, `l402`, `moneydevkit`, `neutronpay` as applicable).
- Provider mode can be toggled online when the goal requires incoming paid work.
- Liquidity solver mode is disabled unless explicitly enabled in a dedicated future rollout.

## Basic Flow

1. Define an earnings objective (for example: `EarnBitcoin min_wallet_delta_sats=1000`).
2. Ensure stop conditions include a wallet-based target (for example `WalletDeltaSatsAtLeast`).
3. Start/queue the goal.
4. Let the goal loop run attempts until completion or stop/abort conditions trigger.
5. Verify success from authoritative wallet-confirmed evidence (not synthetic receipts).

## Scheduler Modes

Scheduler behavior is managed per goal.

- `Manual`
  - Runs only when explicitly queued (`run_now`).
- `IntervalSeconds`
  - Executes at a fixed interval (for example every `900` seconds).
- `Cron`
  - Executes on a cron expression with timezone support.

Missed-run policies:

- `single_replay`: queue one missed run after restart/recovery.
- `skip`: move cursor forward and skip missed runs.
- `catch_up`: queue backlog and track pending catch-up runs.

OS scheduler adapter:

- Optional adapter can be toggled per goal.
- Adapter reconciliation can be invoked to refresh descriptor state.

## Goal Scheduler Tool Actions

The `openagents_goal_scheduler` tool supports:

- `status`
- `recover_startup`
- `run_now`
- `set_missed_policy`
- `set_kill_switch`
- `toggle_os_adapter`
- `reconcile_os_adapters`

Use `status` with `goal_id` to inspect:

- lifecycle/schedule state,
- policy snapshot,
- reconciliation summary,
- latest run audit receipt.

## Swap Behavior (BTC <-> stablesat USD)

Two controlled tools are exposed:

- `openagents_swap_quote`
- `openagents_swap_execute`

Quote behavior:

- Supports `btc_to_usd` and `usd_to_btc`.
- Uses Blink infrastructure (`skills/blink/scripts/swap_quote.js`) for authoritative quote terms.
- Persists quote audits keyed by goal and request id.
- Emits structured quote fields (`amount_in`, `amount_out`, expiration, provider).

Execution behavior:

- Executes through Blink infrastructure (`skills/blink/scripts/swap_execute.js`) and derives status from the real response.
- Persists execution receipts linked to quote audits and goal ID.
- Stores Blink command provenance on quote/execution receipts (`script_path`, args, execution timestamp, parse version).
- Emits timeline events for settled/failed swaps.

## Safety Controls

Per-goal policy controls include:

- `max_runtime_seconds`
- `max_attempts`
- `max_total_spend_sats`
- `max_total_swap_cents`
- swap policy limits:
  - max per swap/day,
  - max fee,
  - max slippage,
  - quote-confirmation behavior
- autonomy policy:
  - `allowed_command_prefixes`
  - `allowed_file_roots`
  - `kill_switch_active` and `kill_switch_reason`

Important:

- If `kill_switch_active=true`, goal-scope tool commands are denied.
- Goal success must remain payout-gated by wallet-confirmed evidence.

## Troubleshooting

### Goal does not run

- Check goal lifecycle is `Queued` or `Running` (not paused/terminal).
- Check scheduler `enabled`, `next_run_epoch_seconds`, and missed-run policy.
- Run `recover_startup` then re-check `status`.

### Goal runs but does not complete

- Inspect condition evaluation reasons in run audit attempts.
- Verify wallet delta is increasing from receive payments.
- Confirm retries are not exhausted and kill switch is not enabled.

### Swap quote or execution fails

- Check quote/execute tool response code and `command_provenance` fields.
- Validate request amount/unit against policy limits.
- Confirm quote is not expired before execute is recorded.

### Success reported but payout appears wrong

- Read `latest_run_audit` and reconciliation payload from scheduler `status`.
- Ensure payout pointers map to real wallet payment IDs (not synthetic `pay:*` pointers).
- Confirm wallet source has no active error and balance was available.

### Restart/recovery anomalies

- Run `recover_startup` and confirm replay/catch-up results.
- Inspect `pending_catchup_runs`, `last_recovery_epoch_seconds`, and run audit receipts.

## Validation Gate

Before merge/release, run:

- `scripts/lint/autopilot-earnings-epic-test-gate.sh`

This gate covers deterministic tests for:

- condition evaluation,
- skill ordering,
- swap quote/accept paths,
- scheduler trigger/recovery,
- policy enforcement,
- payout-gated success,
- earn-until-+N-sats flow.
