# MDK Agent-Wallet Payout Adapter Runbook

Status: local agent-wallet adapter boundary implemented; #431 proved a real
two-wallet bitcoin movement through OpenAgents product surface authority; #503 adds a separate
Worker-safe hosted MDK payout adapter for accepted Pylon work and a production
`mdk_agent_wallet` settlement bridge receipt for accepted Pylon work. The
hosted-MDK direct payout lane remains blocked by the MoneyDevKit app's
programmatic payout toggle.

OpenAgents product surface has an `mdk_agent_wallet` payout adapter boundary at
`workers/api/src/treasury-payment-mdk-agent-wallet-adapter.ts`.
The isolated-wallet prerequisite checklist for the real movement smoke lives
at `docs/nexus/2026-06-07-mdk-two-wallet-smoke-prerequisites.md`.

The adapter wraps the MoneyDevKit agent-wallet command surface through an
injected `MdkAgentWalletCommandExecutor`. No route handler shells out to MDK.
The executor is the only place that may run a local command, and it must be
operator-controlled or Pylon-local.

## Supported Command Boundary

The strict command boundary covers:

- shared send-readiness preflight, for original-wallet-home and balance checks;
- `balance`, as necessary but not sufficient send-readiness evidence;
- `receive`, for invoice creation where needed;
- `send`, for payout dispatch;
- `payments`, for history and reconciliation.

Every command returns JSON stdout. The adapter parses that JSON with Effect
schemas and rejects invalid output before it can affect ledger records.

The adapter stores only bounded references:

- wallet ref;
- executor ref;
- bucketed balance-readiness ref;
- payout intent ref;
- payout attempt ref;
- redacted invoice ref;
- redacted payment ref;
- amount;
- status;
- event refs; and
- public-safe projection JSON.

If an operator or agent supplies a payout destination string, classify it first
with OpenAgents product surface's payment destination parser boundary
`workers/api/src/payment-destination-input.ts`. The classifier is documented in
`docs/mdk/2026-06-07-bitcoin-payment-instructions-source-audit.md` and can
identify BOLT11, BOLT12, LNURL, Lightning Address, BIP353-style names,
`bitcoin:` URI payloads, unsupported, malformed, and ambiguous states. The
adapter may consume only redacted destination refs after payout-target approval.
It must never treat parser output as payout dispatch authority by itself.

It must not store or print:

- wallet mnemonic;
- wallet config;
- raw invoice;
- payment preimage;
- raw payment hash;
- exact wallet balance;
- raw payout target;
- daemon stdout containing private material;
- local wallet home path when that path reveals a user identity.

## Failure Classification

The adapter classifies command failures into bounded reasons:

- `command_timeout`;
- `daemon_unavailable`;
- `invalid_json`;
- `insufficient_balance`;
- `insufficient_outbound_capacity`;
- `mnemonic_restore_not_send_ready`;
- `payment_failed`;
- `reconciliation_mismatch`;
- `send_readiness_unknown`.

Route and operator surfaces should show those bounded reasons, not raw stderr,
raw stdout, or wallet diagnostics.

## Local Test Wallet Setup

Use isolated wallet homes for local testing. Do not reuse a personal wallet or
a production customer wallet.

Example shape for an operator-owned private shell. Redirect invoice and payment
outputs into ignored local files rather than printing them into issue comments
or public logs:

```bash
export MDK_WALLET_PORT=3457
export MDK_WALLET_MNEMONIC="<set only in an ignored local secret file>"
npx @moneydevkit/agent-wallet@latest init
npx @moneydevkit/agent-wallet@latest balance
npx @moneydevkit/agent-wallet@latest receive 1000 > .secrets/mdk-receive.json
npx @moneydevkit/agent-wallet@latest send <destination-resolved-locally> 1 > .secrets/mdk-send.json
npx @moneydevkit/agent-wallet@latest payments
```

Before `send`, the adapter requires the shared send-readiness helper. The
helper blocks mnemonic-only restore mode before `balance` and before any send
attempt. Positive balance and receive readiness are public-safe hints only; live
payout dispatch requires an explicitly original funded wallet home until MDK
documents a restore/sync repair path that proves outbound capacity.

Do not paste the mnemonic, raw invoice, preimage, raw payment hash, wallet
state, or raw destination into issues, docs, logs, Forum posts, public
receipts, or D1. Do not paste exact wallet balances into public or durable
readiness records; use `balance.mdk_agent_wallet.minimum_satisfied` or
`balance.mdk_agent_wallet.minimum_not_satisfied`.

For a real two-wallet smoke, use:

1. an isolated OpenAgents treasury test wallet;
2. an isolated Pylon edge test wallet;
3. an explicit spend cap;
4. a payout target approval whose private destination is resolved locally by
   the executor, not stored in OpenAgents product surface;
5. a payout intent in the OpenAgents product surface ledger;
6. adapter dispatch through `TreasuryPaymentAuthority`;
7. reconciliation through `payments`; and
8. public-safe receipt projection.

The real two-wallet movement test was completed in issue #431. Do not rerun
live movement unless the user explicitly approves the smoke and the spend cap
is present.

Issue #436 prepared the non-secret prerequisite checklist for #431. Treat that
checklist as the source of truth before attempting live movement.

## Hosted MDK Worker Payouts

#503 adds `workers/api/src/treasury-payment-hosted-mdk-payout-adapter.ts` and
the operator route
`POST /api/operator/nexus-pylon/assignments/{assignmentRef}/accepted-work-payouts`.
That adapter uses MDK's hosted programmatic payout RPC with
`MDK_ACCESS_TOKEN`. It does not import MDK's native Lightning runtime into the
Cloudflare Worker.

The hosted route still goes through `TreasuryPaymentAuthority`. It requires
accepted-work closeout, retained artifact/proof refs, fresh wallet-readiness
evidence, payout-target approval, spend-cap policy refs, and an operator
admin boundary before dispatch. The raw Lightning destination is accepted only
in the authenticated request body and is passed to MDK; OpenAgents product surface stores and
returns only redacted refs.

The production route has been deployed and exercised against accepted
assignment `assignment.public.issue502.20260608024927`. The route reached MDK,
but MDK returned the bounded non-retryable app-setting failure
`PROGRAMMATIC_PAYOUTS_DISABLED`, surfaced by OpenAgents product surface as
`hosted_mdk_programmatic_payouts_disabled`.

Do not retry the same hosted smoke as a Cloudflare, D1, invoice, or Tailnet
debug task. The next required action is to enable programmatic payouts for the
MDK app whose key is deployed as `MDK_ACCESS_TOKEN`, or deploy an app-scoped
key whose dashboard toggle is enabled and whose wallet has enough bitcoin
liquidity. Retained public-safe evidence lives in
`docs/nexus/2026-06-07-pylon-accepted-work-payout-hosted-mdk-smoke.md`.

The accepted-work payout proof for #503 was completed through the approved
local `mdk_agent_wallet` bridge. A fresh local MDK agent-wallet payment moved
real bitcoin for `assignment.public.issue502.20260608024927`; the Pylon posted
public-safe `payment_receipt` and `settlement_status` refs; and the operator
settlement bridge produced:

```text
receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

Verify through:

```text
https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
https://openagents.com/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927
```

As of issue #556, public and agent-facing surfaces must expose the explicit
MDK payout-mode gate before making payout claims. Hosted direct payout remains
blocked by `blocker.mdk.hosted_programmatic_payouts_disabled`; the live
accepted-work settlement claim is limited to
`local_mdk_agent_wallet_bridge`. See
`docs/nexus/2026-06-08-mdk-payout-mode-gate.md`.

## Verification

Current mocked verification:

```bash
bun run --cwd workers/api test -- src/treasury-payment-mdk-agent-wallet-adapter.test.ts src/treasury-payment-simulation-adapter.test.ts src/treasury-payment-authority.test.ts src/nexus-treasury-payout-ledger.test.ts
bun run --cwd workers/api typecheck
```

The MDK adapter passes the shared Treasury payment adapter conformance suite
with mocked command output. That proves it respects the same authority,
idempotency, dispatch, and reconciliation contract as the simulation adapter,
without moving bitcoin.
