# Executor W4.3 Verification Record: Live 2-Device Bitcoin Mainnet Spark Settlement & Reconciled Settled Feed (Ch 238)

**Date:** 2026-11-21  
**Issue:** [#13922](https://github.com/OpenAgentsInc/openagents/issues/13922) (Parent Epic [#13912](https://github.com/OpenAgentsInc/openagents/issues/13912))  
**Master Spec:** [psionic/docs/executor-network-master-spec.md](../../psionic/docs/executor-network-master-spec.md)  
**Chapter:** [Ch 238 — Pay for verified work](../../psionic/book/ch238-pay-for-verified-work.md) (`implemented`)  
**Network:** Live **Bitcoin mainnet (`--network bitcoin`)** via Breez Spark SDK `0.27.2` (rescoped from regtest per owner instruction 2026-11-21; default mainnet arming remains OFF unless both `--arm-mainnet-owner-approved` and `OA_EXECUTOR_MAINNET_ARMED=1` are set).

---

## 1. Two-Device Live Architecture

| Role | Physical Machine | Nostr Pubkey (hex) | Spark Mainnet Address (`bitcoin`) |
| --- | --- | --- | --- |
| **Worker** (`cj-executor`) | Apple M5 Max (`apple-m5-max-darwin-arm64`, macOS ARM64) | `5eea94abb4647e00a9afd371b7d239b1b392601720590de97265f5e89d9f5116` | `spark1pgssy689z40w8qrgawmakt5c054rfjvffjwpj9anaa390d9xmtksl36wetzhxk` |
| **Validator** (`cj-validate`) | `coderos-4080` (`coderos-4080-rtx4080-linux-x86_64`, NixOS x86_64) | `2d7bceead5633d70a76e2107fc8833488e178139430d8fb14601755ace32c230` | `spark1pgss9777x2pt5z32p0waq73jhlfl9s47yc37z0l7yjdta2h9dywasn4y0mzl5z` |
| **Buyer / Aggregator** | Apple M5 Max (`openagents pylon settle` / `pool-feed`) | `dc8770b074b6e0b0a4c9c69a479749d4715d40cf8e69ea304c66d40a4c26dc4e` | `spark1pgss9lfygl3sthregussfpl56f5tsjtyy5y50su5cwnamw86ezlnuu9np8ny8s` |
| **Nostr Relay** | Coax Nostr Relay (`nostr/relay/bin/relay`) over Tailscale `ws://100.127.107.31:18743` | — | — |

---

## 2. Live Bug Caught & Fixed During Mainnet Qualification

During the first live funded Bitcoin mainnet send via `spark-settle-helper.sh`, `BreezSdkSpark` (`0.27.2`) rejected W4.3's 71-character `<64hex-receipt_digest>-worker` idempotency key with `SdkError.Generic("Invalid TransferId format")`, because Spark's `TransferId` requires a canonical RFC-4122 UUID.

- **Fix (`apps/shared/host/apple/SparkWallet.swift` + `apps/openagents-desktop/spark-settle-helper.swift`):**
  - Added deterministic `OASparkWallet.sparkIdempotencyUUID(_ raw: String)` normalizing arbitrary deterministic idempotency keys via `SHA-256("openagents.spark-idempotency.v1:" + raw)` into canonical lowercase RFC-4122 UUIDv5 format (passing valid UUIDs through unchanged), preserving end-to-end Spark-protocol idempotency across retries and restarts.
  - Added `OASparkWallet.lastSendError` so `spark-settle-helper` reports exact SDK diagnostics on `.waiting` instead of an empty string.

---

## 3. Verbatim Live 2-Device Bitcoin Mainnet Run

### 3.1 Device 1 (Mac M5 Max) Executes W1.2 Compiled Transformer & Publishes `30200` + `30210`

```json
{"selfcheck":"admitted","lane":"cj-executor","beacon_id":"ca1d2ad6df6dbc7ffd221eeba95dab8fed18254a22b604d7e2e47a7037022bc3","receipt_id":"1f8401fee908409e670801c0d71b07240d83368920fb30168196448f330357cf","device_ref":"apple-m5-max-darwin-arm64","artifact_sha256":"b1b6bbceedeb3c446791a4dd675acd092726a775ac30697ff2e6f8c59d32bc71","trace_digest":"2bf78f1706757b5ae2bfd1490506efdc7b3d76e7ba740ee19fdce2616661ed29","window":1,"steps":20966,"result":86,"weight_calls":6575,"ok":true}
```

### 3.2 Device 2 (`coderos-4080` Linux x86_64) Replays via Ch 234 Over Tailscale & Publishes `30211` + `1985`

```json
{"receipts_seen":1,"validated":1,"rejected":0,"quarantined":0,"admitted":1,"device_ref":"coderos-4080-rtx4080-linux-x86_64","verdict_id":"57eb7f7d19a3e631d68f410acd52487d7642a65c694d398dce4f7237c2227959","label_id":"513afbc241280b5be4f54f913d5eadb7624629fc9bf333ac4b85f2b7259b46cf","ok":true}
```

### 3.3 Adversarial Check — Unarmed Mainnet Refusal (`--network bitcoin --armed` without `OA_EXECUTOR_MAINNET_ARMED=1`)

```json
{"network":"bitcoin","period_id":"2026-11-21","bridge":"spark-helper","owner_armed":true,"mainnet_armed":false,"ledger_dir":"/tmp/oa-w43-mainnet-ledger","receipts_seen":1,"verdicts_seen":1,"rejected_pairing":0,"released_legs":0,"held_legs":2,"withheld_legs":0,"duplicate_legs":0,"settled_real_legs":0,"unfunded_or_dry_legs":0,"feed_settled_count":0,"feed_excluded_simulated":0,"paid_msat_bitcoin":0,"paid_msat_regtest":0,"paid_msat_signet":0,"paid_msat_testnet":0,"receipts_digest":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","pool_30201_published":false,"ok":true}
```

### 3.4 Live Armed Bitcoin Mainnet Two-Leg Settlement (`200 sats` Worker + `50 sats` Validator, Zero Operator POSTs)

```json
{"network":"bitcoin","period_id":"2026-11-21","bridge":"spark-helper","owner_armed":true,"mainnet_armed":true,"ledger_dir":"/tmp/oa-w43-mainnet-ledger","receipts_seen":1,"verdicts_seen":1,"rejected_pairing":0,"released_legs":2,"held_legs":0,"withheld_legs":0,"duplicate_legs":0,"settled_real_legs":2,"unfunded_or_dry_legs":0,"last_bridge_status":"settled","feed_settled_count":2,"feed_excluded_simulated":0,"paid_msat_bitcoin":250000,"paid_msat_regtest":0,"paid_msat_signet":0,"paid_msat_testnet":0,"receipts_digest":"88866eb2b05f6f76c8151abfab0ddc230d1f3d8d04f127dbcbfefadb2a30cb98","pool_30201_published":true,"pool_30201_event_id":"a67bc551fad260fce438c3ba650ddac624c61952ebca022255c4a60084d616e1","ok":true}
```

### 3.5 Restart Idempotency Check (Exact Same Armed Mainnet Command Against Persisted Ledger)

```json
{"network":"bitcoin","period_id":"2026-11-21","bridge":"spark-helper","owner_armed":true,"mainnet_armed":true,"ledger_dir":"/tmp/oa-w43-mainnet-ledger","receipts_seen":1,"verdicts_seen":1,"rejected_pairing":0,"released_legs":0,"held_legs":0,"withheld_legs":0,"duplicate_legs":2,"settled_real_legs":0,"unfunded_or_dry_legs":0,"feed_settled_count":2,"feed_excluded_simulated":0,"paid_msat_bitcoin":250000,"paid_msat_regtest":0,"paid_msat_signet":0,"paid_msat_testnet":0,"receipts_digest":"88866eb2b05f6f76c8151abfab0ddc230d1f3d8d04f127dbcbfefadb2a30cb98","pool_30201_published":false,"ok":true}
```

### 3.6 Independent Live Bitcoin Mainnet Spark Wallet Balance Verification

| Wallet | Balance Before | Balance After | Delta (Mainnet Spark, `0` sat fee) |
| --- | ---: | ---: | ---: |
| Buyer (`spark1pgss9lfy...ny8s`) | `5,000 sats` | **`4,750 sats`** | `-250 sats` |
| Worker — Mac M5 Max (`spark1pgssy689...zhxk`) | `0 sats` | **`200 sats`** | **`+200 sats`** |
| Validator — `coderos-4080` (`spark1pgss9777...zl5z`) | `0 sats` | **`50 sats`** | **`+50 sats`** |

Persisted `settlement-receipts.jsonl` commitments (domain-separated SHA-256, zero invoices/preimages/seeds):
- **WORKER (`200,000 msat`, `real_bitcoin_moved: true`):** `28659fa5b620f0a42fcb087e8ebbab3b09aab92609ad067267349b4d8faf23f1` (`settled_at: 1795296061`)
- **VALIDATOR (`50,000 msat`, `real_bitcoin_moved: true`):** `a630955073ead4fea9862fa1da72e55bb4dea6ce827a7e683d4d2e0a1e9bf552` (`settled_at: 1795296066`)

Signed NIP-PYLON kind `30201` event queried back from live Coax relay (`a67bc551fad260fce438c3ba650ddac624c61952ebca022255c4a60084d616e1`):
- `totals.paid_msat`: `{"bitcoin":250000,"regtest":0,"signet":0,"testnet":0}`
- `inputs.receipts`: `{"count":2,"digest":"88866eb2b05f6f76c8151abfab0ddc230d1f3d8d04f127dbcbfefadb2a30cb98"}`

### 3.7 Cold-Start `pool-feed` Mixed-Network Separation & Simulated Exclusion

Appending a simulated `bitcoin` row (`real_bitcoin_moved: false`, `999,000 msat`) and a `regtest` row (`real_bitcoin_moved: true`, `100,000 msat`) to a copy of the live mainnet ledger and running `openagents --json pylon pool-feed --publish`:

```json
{"pool":"executor-mainnet-w43","ledger_dir":"/tmp/oa-w43-mainnet-mixed","total_input_receipts":4,"settled_count":3,"worker_settled_count":2,"validator_settled_count":1,"excluded_simulated_count":1,"excluded_simulated_msat":999000,"duplicate_count":0,"paid_msat_bitcoin":250000,"paid_msat_regtest":100000,"paid_msat_signet":0,"paid_msat_testnet":0,"receipts_digest":"7d5375d5a81c373489a1a7f9e6ce10fc31d71e16d451dc609ef2c1fed5b9cb4b","pool_30201_published":true,"pool_30201_event_id":"c3eb47119053327f40015cb9a0df3b01eb1a3a107fffa2b70156e43218dba9de","ok":true}
```

---

## 4. Authority Unit Tests, Adversarial Harness, Mutation & Wallet Smoke Checks

| Check | Result |
| --- | --- |
| `coax test pay/executor-settlement.coax` | `1/1` passed |
| `coax mutate pay/executor-settlement.coax --root-only` | `63/63` killed, **`0` survived** |
| `coax test psionic/lib/peer/pay.coax` | `163/163` passed |
| `coax mutate psionic/lib/peer/pay.coax --root-only` | `438/438` killed, **`0` survived** |
| `coax test psionic/lib/peer/settled-feed.coax` | `126/126` passed |
| `coax mutate psionic/lib/peer/settled-feed.coax --root-only` | `581/581` killed, **`0` survived** |
| `coax/web/tool.sh psionic/tools/executor-settle-e2e.coax` | **PASS** (5/5 adversarial checks: tamper withhold, duplicate zero double-spend, cap hold, UTC day rollover, restart recount, 30201 verification) |
| `apps/openagents-desktop/wallet-smoke-test.sh` | **PASS** (live Breez Spark mainnet throwaway seed, `SparkWallet.swift` + `pay/wallet.coax` C ABI) |
