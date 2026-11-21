# Capstone: Coder takes a job and gets paid

> **Status: verified on CPU / `#authority` boundary.** Every step below maps to
> live checked Coax code on `main`. The runnable check in
> [`pay/capstone-job-payout-check.coax`](../../../pay/capstone-job-payout-check.coax)
> verifies the end-to-end ATIF-v1.8 digest -> `#authority` spend-admission ->
> refusal/allow boundary; live mainnet satoshi movement remains behind
> `OpenAgents.app`'s non-bypassable interactive approval dialog.

An operator posts a bounded coding task. **Coder** runs the task in an isolated
worktree, records every model-visible step into a canonical **ATIF-v1.8** log
with a SHA-256 `steps_digest`, passes a `/goal` verification turn, and requests
settlement through the local **Breez Spark** Lightning wallet. The `#authority`
policy gates verify the digest, check the fee cap and daily spend limit, refuse
any unapproved or tampered request, and—once explicitly approved—emit a signed
receipt that attaches back to the thread.

## Why this route

- It connects **Coder**, **Nostr**, **Bitcoin**, and **OpenAgents** into one
  concrete economic loop (Episode 288–289).
- It exercises all five steps of [the argument](../README.md#the-argument):
  **bounded** (worktree + budget + daily limit), **deterministic** (canonical
  sorted-key ATIF-v1.8 digest), **checkable** (`/goal` verification + SHA-256
  log verifier), **payable** (`pay/wallet.coax` + `pay/wallet-rpc.coax` over
  Breez Spark Lightning), and **ownable** (same 12 BIP-39 words for NIP-06
  Nostr identity and Spark wallet).

## The trace

| # | Step | Owner | Live Code Today |
| --- | --- | --- | --- |
| 1 | The task is admitted with a working folder, isolated worktree, and hard budget. | [Coder 1, 4, 5](../../../coder/book/ch001-the-simplest-loop.md) | `apps/openagents-desktop/Worktree.swift`, `coder/tui/mcworker.coax` |
| 2 | Every step is logged canonically in ATIF-v1.8 and hashed into a deterministic SHA-256 `steps_digest`. | [Coder 2](../../../coder/book/ch002-model-visible-means-logged.md); [Nostr 17, 21](../../../nostr/book/ch017-json-you-can-sign.md) | `coax/lib/nostr/atif/digest.coax`, `coax/lib/nostr/atif/log.coax` (`#13830`) |
| 3 | Before closing, `/goal` forces an independent verification audit turn checking every acceptance criterion. | [Coder 21](../../../coder/book/README.md); [OpenAgents](../../../openagents/book/README.md) | `apps/openagents-desktop/page/goal.coax` (`649c31f297`) |
| 4 | A Lightning invoice or `x402` payment quote is prepared via the shared Apple Breez Spark host. | [Bitcoin 113–114](../../../bitcoin/book/ch113-find-a-route-and-pay.md); [Lightning plan](../../lightning/README.md) | `apps/shared/host/apple/SparkWallet.swift` (`#13899`) |
| 5 | `#authority` policy checks amount, fee cap, expiry, daily remaining limit, and explicit user approval. | [Bitcoin 121](../../../bitcoin/book/ch121-one-seed-for-nostr-and-bitcoin.md); `pay/` | `pay/wallet.coax`, `pay/wallet-backup.coax`, `pay/wallet-rpc.coax` (`#13901`) |
| 6 | Desktop pops a non-bypassable approval dialog (default Deny) and writes a structured receipt to `~/.openagents/wallet/receipts.jsonl`. | [OpenAgents](../../../openagents/book/PLAN.md) | `apps/openagents-desktop/Window+WalletRPC.swift`, `apps/openagents-cli/wallet.coax` |

## Broken / refusal cases (checked)

1. **Unapproved agent spend:** `wallet_rpc_spend_admit(..., approved = false, ...)` returns `WrpcVerdict.needs_approval` (HTTP 403) and writes a refusal receipt (`wallet_rpc_writes_receipt == true`).
2. **Daily limit exceeded:** Requesting `1000` sats + `10` sat fee with `1005` sats daily remaining returns `WrpcVerdict.over_daily_limit` (HTTP 403) before any prompt or signing.
3. **Tampered or expired quote:** `now >= expires` returns `WrpcVerdict.expired` (HTTP 409); `fee > fee_cap` returns `WrpcVerdict.over_fee_cap` (HTTP 403).

## Runnable check

```shell
coax/bin/coax test pay/capstone-job-payout-check.coax
```

This runs the verified `#authority` admission + refusal + receipt checks against `pay/wallet-rpc.coax` and `pay/wallet.coax`.
