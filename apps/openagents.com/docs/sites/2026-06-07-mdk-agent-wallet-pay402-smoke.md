# MDK Agent Wallet And Pay402 Smoke Runbook

Date: 2026-06-07
Issue: #450 / OPENAGENTS-H-013

## Summary

This runbook describes how an operator or explicitly authorized agent can test
MoneyDevKit agent-wallet and pay402-compatible L402 flows without committing
wallet material or accidentally spending unbounded bitcoin.

The default smoke is documentation-only and no-spend. A signet or sandbox spend
requires separate operator approval, a named wallet, a concrete amount, and a
spend cap. Funding a wallet is always an operator-controlled step. Source code
must never pretend to set a wallet balance.

## Current Implementation State

| State          | Meaning                                                                                             |
| -------------- | --------------------------------------------------------------------------------------------------- |
| `fake_sandbox` | OpenAgents product surface fake-provider tests and fixture plans. No live wallet spend.                                  |
| `signet`       | Allowed only after explicit operator approval, named wallet, amount, and spend cap.                 |
| `live_blocked` | Mainnet-like live bitcoin movement is blocked unless a separate instruction grants exact authority. |
| `planned`      | Routes or checkout products that do not yet advertise live L402/MDK recovery remain docs-only.      |

The fixture backing this runbook is:

```bash
bun run --cwd workers/api test -- src/mdk-agent-wallet-smoke-fixture.test.ts
```

That fixture builds a command plan with placeholders. It does not call the
wallet CLI, create invoices, pay invoices, cache credentials, or move bitcoin.

## Wallet Setup

Use the MoneyDevKit agent wallet CLI. Every command emits JSON to stdout and
uses exit code `0` for success and `1` for error.

Check whether a daemon or wallet already exists:

```bash
npx @moneydevkit/agent-wallet@latest status
```

Show redacted config without printing the mnemonic:

```bash
npx @moneydevkit/agent-wallet@latest init --show
```

Initialize a new signet test wallet only when the operator has asked for a new
wallet:

```bash
npx @moneydevkit/agent-wallet@latest init --network signet
```

Check balance:

```bash
npx @moneydevkit/agent-wallet@latest balance
```

The balance output shape is:

```json
{ "balance_sats": "<integer_redacted_for_docs>" }
```

The JSON field uses `sats` because that is the wallet CLI denomination field.
In OpenAgents product surface docs and UI, prefer saying bitcoin and add the satoshi denomination
only when the exact CLI amount matters.

## Receive For Funding

Funding is operator-controlled. To request a bounded test amount, generate an
invoice and hand it to the operator through a private channel. Do not commit it,
paste it into issue comments, or log it into public docs.

```bash
npx @moneydevkit/agent-wallet@latest receive 1000 --description "OpenAgents signet smoke"
```

Expected JSON shape:

```json
{
  "invoice": "<redacted_bolt11_invoice>",
  "payment_hash": "<redacted_payment_hash>",
  "expires_at": "2026-06-07T00:00:00.000Z"
}
```

After the operator pays, check balance again:

```bash
npx @moneydevkit/agent-wallet@latest balance
```

Do not print the raw invoice, payment hash, wallet home path, or exact balance
in public projections. Use a public-safe readiness ref such as
`wallet_readiness.signet.funded_under_cap`.

## L402 / Pay402-Compatible Smoke

Use this only against a route that explicitly advertises L402/MDK recovery in
OpenAgents discovery or in a fake/sandbox test. Do not infer payment authority
from a generic `402` response.

1. Request the paid endpoint without a payment credential:

```bash
curl --fail-with-body "$OPENAGENTS_PAID_ENDPOINT"
```

2. Parse the payment-required response. Keep the token and invoice in local
   ignored state only:

```bash
export OPENAGENTS_L402_TOKEN="<token_from_payment_required_response>"
export OPENAGENTS_L402_INVOICE="<bolt11_invoice_from_payment_required_response>"
```

3. Confirm send readiness before sending. The shared OpenAgents MDK
   send-readiness helper requires an explicitly original funded wallet home.
   Mnemonic-only restore is blocked as send-ready evidence until MDK documents
   a repair/sync path that proves outbound capacity. Positive balance and
   receive readiness are not enough.

4. Confirm the payment is under the operator-approved cap before sending:

```bash
test "$OPENAGENTS_PAYMENT_AMOUNT_SATOSHIS" -le "$OPENAGENTS_SPEND_CAP_SATOSHIS"
```

The shell variable uses satoshis because the wallet CLI `send` amount semantics
and many invoice amounts are satoshi-denominated. The operator approval should
still describe the spend as a small bounded bitcoin test.

5. Pay with the agent wallet only after explicit approval and send-readiness
   preflight:

```bash
npx @moneydevkit/agent-wallet@latest send "$OPENAGENTS_L402_INVOICE"
```

Expected JSON shape:

```json
{ "payment_hash": "<redacted_payment_hash>" }
```

If the wallet output or a compatible pay402 helper returns a preimage, keep it
in the local ignored token cache. Do not print it.

6. Retry the paid endpoint with the L402 token and preimage:

```bash
curl --fail-with-body \
  -H "Authorization: L402 ${OPENAGENTS_L402_TOKEN}:${OPENAGENTS_PAYMENT_PREIMAGE}" \
  "$OPENAGENTS_PAID_ENDPOINT"
```

7. Verify the OpenAgents result using public-safe receipt/proof routes only.
   The public result should show stable refs, state, and entitlement status, not
   raw invoices, preimages, payment hashes, wallet paths, or customer/operator
   identifiers.

## Token Cache Behavior

The local token cache must be outside tracked source. Acceptable examples:

- an ignored local temp file under `.secrets/`;
- a process-local environment variable;
- an agent runtime secret store;
- a private wallet-daemon state file.

The token cache may hold:

- L402 token;
- payment preimage;
- route/method binding;
- expiry;
- public-safe receipt ref after retry.

The token cache must not be committed, printed to GitHub, sent to public Forum
posts, or embedded in docs.

## Redaction Rules

Never print, commit, or include in public projections:

- wallet mnemonic or recovery phrase;
- `~/.mdk-wallet/config.json` contents;
- `MDK_WALLET_MNEMONIC`;
- `MDK_WALLET_PORT` when it reveals a private local setup;
- `MDK_ACCESS_TOKEN`;
- `MDK_MNEMONIC`;
- MDK webhook secrets;
- raw BOLT11/BOLT12/LNURL payloads;
- raw invoice strings;
- raw payment hashes;
- preimages;
- exact wallet home paths;
- exact private balances;
- customer emails, operator emails, or private user IDs.

Use refs instead:

- `wallet_home.local.redacted`;
- `payment_hash.redacted.<scope>`;
- `receipt.openagents.<scope>`;
- `entitlement.openagents.<scope>`;
- `token_cache.local.redacted`;
- `spend_cap.bitcoin_satoshis.<bounded_amount>`.

## Failure Handling

If the daemon hangs or commands return no output:

```bash
npx @moneydevkit/agent-wallet@latest restart
```

If payment fails, do not retry unboundedly. Re-check:

- the route still advertises L402/MDK recovery;
- the invoice has not expired;
- the payment remains under the spend cap;
- the token cache has not mixed credentials from another route;
- the wallet is still the named operator-approved wallet.

If any value is uncertain, stop and request operator review.

## OpenAgents Route Guidance

Use only these OpenAgents categories for this smoke:

- fake/sandbox Site MDK smoke fixtures and tests;
- routes whose discovery explicitly says L402/MDK recovery is available;
- operator-approved signet tests with a named wallet and amount;
- public-safe receipt/proof lookup routes after retry.

Do not use this runbook to:

- initialize or fund a production wallet in source code;
- create a live checkout product;
- spend live bitcoin;
- bridge buyer payment to Pylon payout;
- settle provider payouts;
- test private customer routes.

Those require separate issue-specific authority and evidence.
