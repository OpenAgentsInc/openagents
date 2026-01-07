# Nexus + Pylon v0.1 E2E Test Log (v4)

**Date:** 2026-01-07 15:25
**Status:** in-progress

## Change Under Test

- Pylon provider now calls `relay_service.connect()` before `dvm.start()`.

## Nexus Endpoint Checks

### NIP-11

```bash
curl -s -H "Accept: application/nostr+json" https://nexus.openagents.com/
```

```
{"name":"nexus.openagents.com","description":"OpenAgents Nexus relay","pubkey":"","contact":"nexus@openagents.com","supported_nips":[1,11,42,89,90],"software":"https://github.com/OpenAgentsInc/openagents","version":"0.1.0","limitation":{"max_message_length":524288,"max_subscriptions":20,"max_filters":10,"max_limit":500,"max_subid_length":64,"auth_required":true,"payment_required":false}}
```

### WebSocket AUTH Challenge

```bash
node -e "const ws = new WebSocket('wss://nexus.openagents.com'); ws.onopen=()=>console.log('open'); ws.onmessage=(m)=>{console.log('msg', m.data); ws.close();}; ws.onerror=(e)=>{console.error('error', e);};"
```

```
open
msg ["AUTH","f5f1e3eba4f43aeac4464a860df3ce6368e06c2dabe672ca9af1508675799416"]
```

## Provider Setup (isolated HOME)

```bash
HOME=/tmp/pylon-provider-v4 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- init
```

```
   Compiling pylon v0.1.0 (/Users/christopherdavid/code/openagents/crates/pylon)
    Finished `dev` profile [optimized + debuginfo] target(s) in 26.13s
     Running `target/debug/pylon init`
Generating new identity...

⚠️  IMPORTANT: Write down these words and store them securely!
This is your seed phrase - it controls your identity and funds.

  camera lion sail apology custom carry hard order absorb minor radio guide

Nostr Public Key (npub): npub1f95dpcvslkul2ww0pag9d6vas3pc4ksjsx98u9xf9x5m52s5e0usd323wt
Nostr Public Key (hex):  4968d0e190fdb9f539cf0f5056e99d84438ada12818a7e14c929a9ba2a14cbf9

⚠️  Saving mnemonic to "/tmp/pylon-provider-v4/.openagents/pylon/identity.mnemonic"
   This file contains your private key. Keep it secure!

✅ Identity initialized successfully!
   Config: "/tmp/pylon-provider-v4/.openagents/pylon/config.toml"
   Identity: "/tmp/pylon-provider-v4/.openagents/pylon/identity.mnemonic"

Run 'pylon start' to begin earning.
```

```bash
HOME=/tmp/pylon-provider-v4 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- wallet whoami
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.99s
     Running `target/debug/pylon wallet whoami`

Wallet Identity
===============
Nostr (hex):  4968d0e190fdb9f539cf0f5056e99d84438ada12818a7e14c929a9ba2a14cbf9
Nostr (npub): npub1f95dpcvslkul2ww0pag9d6vas3pc4ksjsx98u9xf9x5m52s5e0usd323wt
Spark:        0231633e0754153484b2c76442ad1bc2fbd8aac45b7d1168f3e6532c1b99a5e13c
```

```bash
HOME=/tmp/pylon-provider-v4 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- start -f -m provider
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.88s
     Running `target/debug/pylon start -f -m provider`
Starting Pylon in foreground mode...
Identity: npub1f95dpcvslkul2ww0pag9d6vas3pc4ksjsx98u9xf9x5m52s5e0usd323wt
Mode: Provider
[2m2026-01-07T21:27:42.461680Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 001_initial_schema
[2m2026-01-07T21:27:42.461873Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 002_invoices
[2m2026-01-07T21:27:42.462280Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 003_neobank
[2m2026-01-07T21:27:42.462303Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Database opened at "/tmp/pylon-provider-v4/.openagents/pylon/pylon.db"
[2m2026-01-07T21:27:42.462380Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Loaded stats: 0 jobs completed, 0 sats earned
[2m2026-01-07T21:27:42.462665Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge binary found, attempting to start...
[2m2026-01-07T21:27:42.462795Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m Starting FM Bridge from: "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge"
[2m2026-01-07T21:27:42.470839Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m FM Bridge ready at http://localhost:11435
[2m2026-01-07T21:27:42.470983Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge started at http://localhost:11435
[2m2026-01-07T21:27:42.473124Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Detected inference backends: apple_fm
[2m2026-01-07T21:27:42.475416Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Claude Code not available (no API key or CLI)
[2m2026-01-07T21:27:42.481185Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Spark wallet initialized for payments
[2m2026-01-07T21:27:42.481219Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider backends: apple_fm
[2m2026-01-07T21:27:43.568157Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Pylon provider started
[2m2026-01-07T21:27:43.568192Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider mode started
[2m2026-01-07T21:27:43.568215Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Pylon daemon running
```

## Buyer Tests

### 1) Targeted Nexus-only auto-pay

```bash
cargo run -p pylon -- job submit "e2e targeted auto-pay <ts>" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com --provider <PROVIDER_PUBKEY>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.98s
     Running `target/debug/pylon job submit 'e2e targeted auto-pay 152819' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com' --provider 4968d0e190fdb9f539cf0f5056e99d84438ada12818a7e14c929a9ba2a14cbf9`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e targeted auto-pay 152819
Auto-pay: enabled

Job Submitted
=============
ID:     d6a0570237be8de85e18f24608f00085d8d8b43f37f2290fc30acfd2f6039d36
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.892197917s
```

### 2) Broadcast Nexus-only auto-pay

```bash
cargo run -p pylon -- job submit "e2e broadcast auto-pay <ts>" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.13s
     Running `target/debug/pylon job submit 'e2e broadcast auto-pay 153142' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com'`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e broadcast auto-pay 153142
Auto-pay: enabled

Job Submitted
=============
ID:     649deb7003b6c33065d4630b3f4786832a1e018858d534a568034cc4cd137bed
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.890622834s
```

### 3) Job list + status

```bash
cargo run -p pylon -- job list --limit 5
cargo run -p pylon -- job status <JOB_ID>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.12s
     Running `target/debug/pylon job list --limit 5`

Submitted Jobs
==============
[fail] 01/07 21:31 649deb7003b6c330 "e2e broadcast auto-pay 153142"
[fail] 01/07 21:28 d6a0570237be8de8 "e2e targeted auto-pay 152819"
[fail] 01/07 21:18 914d69c0ec480e29 "e2e broadcast auto-pay 151831"
[fail] 01/07 21:15 4c933eff07098725 "e2e targeted auto-pay 151516"
[done] 01/07 20:47 91dbe03de3ad0527 "e2e multi-relay auto-pay 14470..."

    Finished `dev` profile [optimized + debuginfo] target(s) in 0.38s
     Running `target/debug/pylon job status d6a0570237be8de85e18f24608f00085d8d8b43f37f2290fc30acfd2f6039d36`

Job Status
==========
ID:      d6a0570237be8de85e18f24608f00085d8d8b43f37f2290fc30acfd2f6039d36
Kind:    5050
Status:  failed
Relay:   wss://nexus.openagents.com
Provider: 4968d0e190fdb9f539cf0f5056e99d84438ada12818a7e14c929a9ba2a14cbf9
Bid:     1000 msats
Created: 2026-01-07 21:28:22
```

## Provider Log Excerpts

```
No job receipt lines found for `d6a057...` or `649deb...`.

Log tail:
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.88s
     Running `target/debug/pylon start -f -m provider`
Starting Pylon in foreground mode...
Identity: npub1f95dpcvslkul2ww0pag9d6vas3pc4ksjsx98u9xf9x5m52s5e0usd323wt
Mode: Provider
[2m2026-01-07T21:27:42.481219Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider backends: apple_fm
[2m2026-01-07T21:27:43.568157Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Pylon provider started
[2m2026-01-07T21:27:43.568192Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider mode started
[2m2026-01-07T21:27:43.568215Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Pylon daemon running
```

## Notes

- Capture provider receipt + invoice creation for Nexus-only jobs.
- Record whether payment request arrives and result is published.
- Stopped provider process after tests: `kill 55078`.
