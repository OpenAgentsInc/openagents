# OpenClaw: Earn Your First Bitcoin (Breez/Spark)

**Goal:** OpenClaw earns its first sats using Breez/Spark, with a path you can implement today.

**Context:** OpenAgents already has a full Spark (Breez) stack: Rust crate `crates/spark/`, CLI `openagents spark` (receive, send, wallet, Lightning address, faucet), and the website uses Breez SDK in the browser. OpenClaw does not yet have wallet tools. This doc is the minimal plan to get “OpenClaw earned its first bitcoin” using that stack.

**Related:** [bitcoin-wallets-plan.md](./bitcoin-wallets-plan.md) (full wallet roadmap), [codebase-tour.md](./codebase-tour.md) (OpenClaw entry points), [autopilot-integration.md](./autopilot-integration.md) (plugin pattern). Wallet stack comparison: `docs/wallet-considerations.md`.

---

## 1. What “earn first bitcoin” means

- **Receive** at least one payment (sats) into a wallet that is **under OpenClaw’s control** (or explicitly “OpenClaw’s” identity).
- **Stack:** Breez/Spark (nodeless). No Lightning node to run.
- **Today:** Prefer the path that needs no new backend services and reuses existing OpenAgents CLI + env/config.

---

## 2. Options (implementable today)

| Option | What you do | Time | “Earn” moment |
|--------|-------------|------|----------------|
| **A. OpenClaw tool → OpenAgents CLI** | Add an OpenClaw plugin tool that shells out to `openagents spark receive` and returns an invoice/address. Share it; someone pays. | 1–2 hours | Payment hits wallet used by that tool. |
| **B. Website wallet + payment pointer** | Create wallet on OpenAgents.com (Breez), get Lightning address, publish as “OpenClaw’s” payment pointer (e.g. Nostr `lud16`). Someone zaps. | ~30 min | Payment hits that wallet; you treat it as OpenClaw’s. |
| **C. Regtest + faucet** | Use `openagents spark faucet` on regtest; wallet gets test sats immediately. | ~15 min | Proves stack; not mainnet. |

**Recommendation:** Do **B** first (today, zero code), then **A** so OpenClaw itself can generate receive coordinates and you can say “OpenClaw earned it” from inside the product.

---

## 3. Prerequisites (Breez/Spark)

- **Breez API key**  
  Free; request from Breez. Required for mainnet (and testnet in practice). Set as `SPARK_API_KEY` or `BREEZ_API_KEY` (or `--api-key` for CLI).

- **Mnemonic**  
  12/24 words. For OpenClaw “earn” you can:
  - **Dedicated OpenClaw wallet:** Generate once (`openagents spark new`), store in OpenClaw credential store or env (`SPARK_MNEMONIC` / `OPENAGENTS_MNEMONIC`), never commit.
  - **Reuse existing:** If you already have a wallet (e.g. from OpenAgents website or CLI), use the same mnemonic for “OpenClaw’s” identity (same keys = same wallet).

- **Storage directory**  
  Spark SDK persists state. For CLI: `--storage-dir` or default (see CLI help). For OpenClaw tool: e.g. `~/.openagents/spark-openclaw` or inside OpenClaw state dir (`~/.openclaw/` or `OPENCLAW_STATE_DIR`).

- **Network**  
  Mainnet = real sats. Testnet/regtest = test only. CLI: `--network mainnet|testnet|signet|regtest`.

---

## 4. Option A: OpenClaw tool that shells out to OpenAgents Spark CLI

This is “Option A” from [bitcoin-wallets-plan.md](./bitcoin-wallets-plan.md): OpenClaw tool shells out to an OpenAgents CLI command and parses JSON.

### 4.1 Flow

1. User (or agent) asks OpenClaw to “get a receive invoice” or “give me an address to pay OpenClaw.”
2. OpenClaw plugin tool runs:
   - `openagents spark receive --method bolt11 --amount <sats> --description "OpenClaw" --json`
   - with wallet options: mnemonic (file or env), API key (env), `--storage-dir`, `--network`.
3. CLI returns JSON (e.g. `payment_request`, or Spark address).
4. Tool returns that to the user (and/or posts it to a channel).
5. Someone pays the invoice/address.
6. Wallet (same mnemonic/storage) sees the payment → **OpenClaw earned.**

### 4.2 CLI usage (reference)

Binary: **`openagents`** (from `crates/openagents-cli`). Subcommand: **`spark`**.

**Generate a Bolt11 invoice (fixed amount):**
```bash
openagents spark receive --method bolt11 --amount 1000 --description "OpenClaw" \
  --mnemonic-file ~/.openclaw/spark-mnemonic.txt \
  --api-key "$BREEZ_API_KEY" \
  --storage-dir ~/.openagents/spark-openclaw \
  --network mainnet \
  --json
```

**Generate a Spark address (any amount):**
```bash
openagents spark receive --method spark-address \
  --mnemonic-file ~/.openclaw/spark-mnemonic.txt \
  --api-key "$BREEZ_API_KEY" \
  --storage-dir ~/.openagents/spark-openclaw \
  --network mainnet \
  --json
```

Mnemonic can be env instead of file: `SPARK_MNEMONIC` or `OPENAGENTS_MNEMONIC`; API key: `SPARK_API_KEY` or `BREEZ_API_KEY`. Then you can omit `--mnemonic-file` and `--api-key`.

### 4.3 OpenClaw plugin sketch

- **Where:** New extension under OpenClaw repo, e.g. `extensions/spark-wallet/` (or `bitcoin-wallet`), following pattern of `extensions/llm-task/` and [docs/plugins/agent-tools.md](https://github.com/openclaw/docs/blob/main/plugins/agent-tools.md) (in OpenClaw repo).
- **What:** Register one or more tools, e.g.:
  - `spark.receive_invoice` — params: `amount_sats`, optional `description`. Shells out to `openagents spark receive --method bolt11 ... --json`, parses JSON, returns `payment_request` (and optionally Spark address fallback).
  - `spark.receive_address` — no amount. Shells out to `openagents spark receive --method spark-address ... --json`, returns address.
- **Config:** Plugin reads from OpenClaw config or env: path to `openagents` binary (or assume in PATH), mnemonic file path or “use env”, API key env name, storage dir, network. No secrets in config file; use env or credential store.
- **Safety:** Tool optional/allowlisted; no automatic pay-from-OpenClaw without a separate, explicit pay tool and budget (future).

### 4.4 Implementation checklist (today)

- [ ] Build OpenAgents CLI: `cargo build -p openagents-cli --release`; ensure `openagents` is on PATH or set path in plugin config.
- [ ] Create OpenClaw wallet identity: `openagents spark new` (or use existing mnemonic); save mnemonic to file or env; set `BREEZ_API_KEY` / `SPARK_API_KEY`.
- [ ] Create storage dir: e.g. `mkdir -p ~/.openagents/spark-openclaw`.
- [ ] Add OpenClaw extension that implements `spark.receive_invoice` (and optionally `spark.receive_address`): spawn `openagents spark receive ... --json`, parse stdout, return invoice/address.
- [ ] Wire config: mnemonic path or env, API key env, storage dir, network (mainnet for “real” earn).
- [ ] Test: call tool from OpenClaw (chat or API), get invoice, pay from another wallet (e.g. OpenAgents website or Phoenix); confirm payment in Spark (e.g. `openagents spark payments list --json` with same wallet opts).

---

## 5. Option B: Website wallet + payment pointer (no OpenClaw code)

- Create (or use) a wallet on **OpenAgents.com** (Breez SDK in browser). Generate a Lightning address or Bolt11 invoice.
- Publish that as **OpenClaw’s** payment pointer: e.g. Nostr profile `lud16` or a static page “Pay my OpenClaw: you@getalby.com” (or the Lightning address you got from the site).
- When someone zaps that address or pays that invoice, **that** wallet receives the sats. You define that wallet as “OpenClaw’s” for the purpose of “OpenClaw earned its first bitcoin.”

No plugin work; just identity + docs. Good for “first earn” today; later you can move to Option A so the receive coordinate is generated inside OpenClaw.

---

## 6. Option C: Regtest + faucet (prove stack today)

- Use **regtest** so you don’t need mainnet funds.
- Start a wallet with `openagents spark` (mnemonic + storage dir + `--network regtest`). Regtest may not require Breez API key (check CLI).
- Get a receive address: `openagents spark receive --method bitcoin --json` (or Spark address).
- Request test sats: `openagents spark faucet --amount 100000` (uses built-in faucet URL for regtest; see `crates/openagents-cli/src/spark_cli.rs` and `crates/spark/docs/REGTEST.md`).
- Faucet sends to your wallet → you see balance / payments. That’s “first sats” in the wallet; switch to mainnet + Option A or B for “first real bitcoin.”

---

## 7. Summary: “Implement today”

| Step | Action |
|------|--------|
| 1 | Get Breez API key; create or choose a mnemonic; set `SPARK_MNEMONIC` (or file) and `BREEZ_API_KEY`. |
| 2 | Build CLI: `cargo build -p openagents-cli --release`; create storage dir `~/.openagents/spark-openclaw`. |
| 3 | **Fast path (no code):** Use Option B — wallet on OpenAgents.com, publish payment pointer, get someone to zap. **Code path:** Implement Option A — OpenClaw extension, one tool `spark.receive_invoice` shelling out to `openagents spark receive --method bolt11 ... --json`. |
| 4 | Share the invoice or Lightning address (Nostr, link, etc.); receive a payment. |
| 5 | Confirm: `openagents spark wallet balance` or `openagents spark payments list` (same mnemonic/storage). |

**First bitcoin = first payment received into that wallet.** After that, you can add send, balance, and Lightning address tools (see [bitcoin-wallets-plan.md](./bitcoin-wallets-plan.md) Phase 3).

---

## 8. Code references (OpenAgents)

- Spark CLI (receive, wallet, faucet): `crates/openagents-cli/src/spark_cli.rs`
- Spark crate: `crates/spark/` (wallet, signer, Breez SDK integration)
- Wallet options (mnemonic, api_key, storage_dir, network): `WalletOptions` in `spark_cli.rs`; env vars `SPARK_MNEMONIC`, `OPENAGENTS_MNEMONIC`, `SPARK_API_KEY`, `BREEZ_API_KEY`
- Website receive flow: `apps/website/src/lib/wallet/walletService.ts` (`receivePayment`), `ReceivePaymentDialog.tsx`

---

## 9. Doc references

- **OpenClaw:** [bitcoin-wallets-plan.md](./bitcoin-wallets-plan.md), [codebase-tour.md](./codebase-tour.md), [autopilot-integration.md](./autopilot-integration.md)
- **Wallet stack:** `docs/wallet-considerations.md`
- **Spark regtest:** `crates/spark/docs/REGTEST.md`
