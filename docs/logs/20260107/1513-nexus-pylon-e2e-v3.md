# Nexus + Pylon v0.1 E2E Test Log (v3)

**Date:** 2026-01-07 15:13
**Status:** in-progress

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
msg ["AUTH","d6d104bc9abfd0b2b54d9c1cfd65fece8ccf266228f5efbe0fb99145c30e093f"]
```

## Provider Setup (isolated HOME)

```bash
HOME=/tmp/pylon-provider-v3 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- init
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.10s
     Running `target/debug/pylon init`
Generating new identity...

⚠️  IMPORTANT: Write down these words and store them securely!
This is your seed phrase - it controls your identity and funds.

  medal bounce clinic steak nest window position walnut rib diagram ridge review

Nostr Public Key (npub): npub1vlj4haqpgl0gerpxj6965a0zmvzfxqm2e0k29csv0xtazsfxfyuq2tkv3y
Nostr Public Key (hex):  67e55bf40147de8c8c26968baa75e2db0493036acbeca2e20c7997d141264938

⚠️  Saving mnemonic to "/tmp/pylon-provider-v3/.openagents/pylon/identity.mnemonic"
   This file contains your private key. Keep it secure!

✅ Identity initialized successfully!
   Config: "/tmp/pylon-provider-v3/.openagents/pylon/config.toml"
   Identity: "/tmp/pylon-provider-v3/.openagents/pylon/identity.mnemonic"

Run 'pylon start' to begin earning.
```

```bash
HOME=/tmp/pylon-provider-v3 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- wallet whoami
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.89s
     Running `target/debug/pylon wallet whoami`

Wallet Identity
===============
Nostr (hex):  67e55bf40147de8c8c26968baa75e2db0493036acbeca2e20c7997d141264938
Nostr (npub): npub1vlj4haqpgl0gerpxj6965a0zmvzfxqm2e0k29csv0xtazsfxfyuq2tkv3y
Spark:        02c1e06ad27cc3465fc49f6cfdba6544c0003f30e63eecf6b7a76674b5c60432ca
```

```bash
HOME=/tmp/pylon-provider-v3 RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- start -f -m provider
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.78s
     Running `target/debug/pylon start -f -m provider`
Starting Pylon in foreground mode...
Identity: npub1vlj4haqpgl0gerpxj6965a0zmvzfxqm2e0k29csv0xtazsfxfyuq2tkv3y
Mode: Provider
[2m2026-01-07T21:15:04.117428Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 001_initial_schema
[2m2026-01-07T21:15:04.117611Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 002_invoices
[2m2026-01-07T21:15:04.118024Z[0m [32m INFO[0m [2mpylon::db[0m[2m:[0m Applied migration: 003_neobank
[2m2026-01-07T21:15:04.118048Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Database opened at "/tmp/pylon-provider-v3/.openagents/pylon/pylon.db"
[2m2026-01-07T21:15:04.118142Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Loaded stats: 0 jobs completed, 0 sats earned
[2m2026-01-07T21:15:04.118418Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge binary found, attempting to start...
[2m2026-01-07T21:15:04.118538Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m Starting FM Bridge from: "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge"
[2m2026-01-07T21:15:04.134604Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m FM Bridge ready at http://localhost:11435
[2m2026-01-07T21:15:04.134809Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge started at http://localhost:11435
[2m2026-01-07T21:15:04.136889Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Detected inference backends: apple_fm
[2m2026-01-07T21:15:04.140130Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Claude Code not available (no API key or CLI)
[2m2026-01-07T21:15:04.147116Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Spark wallet initialized for payments
[2m2026-01-07T21:15:04.147207Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider backends: apple_fm
[2m2026-01-07T21:15:04.968714Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Pylon provider started
[2m2026-01-07T21:15:04.968745Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider mode started
[2m2026-01-07T21:15:04.968775Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Pylon daemon running
```

## Buyer Tests

### 1) Targeted Nexus-only auto-pay

```bash
cargo run -p pylon -- job submit "e2e targeted auto-pay <ts>" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com --provider <PROVIDER_PUBKEY>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.92s
     Running `target/debug/pylon job submit 'e2e targeted auto-pay 151516' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com' --provider 67e55bf40147de8c8c26968baa75e2db0493036acbeca2e20c7997d141264938`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e targeted auto-pay 151516
Auto-pay: enabled

Job Submitted
=============
ID:     4c933eff070987257916ce7af4a61062b6d17a1239df07536008cfb42d4ca84c
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.889235542s
```

### 2) Broadcast Nexus-only auto-pay

```bash
cargo run -p pylon -- job submit "e2e broadcast auto-pay <ts>" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.04s
     Running `target/debug/pylon job submit 'e2e broadcast auto-pay 151831' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com'`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e broadcast auto-pay 151831
Auto-pay: enabled

Job Submitted
=============
ID:     914d69c0ec480e290a58d5ff1dceb2161cd71518ee20f452abc45268df6b9304
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.891270542s
```

### 3) Job list + status

```bash
cargo run -p pylon -- job list --limit 5
cargo run -p pylon -- job status <JOB_ID>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.36s
     Running `target/debug/pylon job list --limit 5`

Submitted Jobs
==============
[fail] 01/07 21:18 914d69c0ec480e29 "e2e broadcast auto-pay 151831"
[fail] 01/07 21:15 4c933eff07098725 "e2e targeted auto-pay 151516"
[done] 01/07 20:47 91dbe03de3ad0527 "e2e multi-relay auto-pay 14470..."
[fail] 01/07 20:44 99a2b96ec6e9420c "e2e broadcast auto-pay 144358"
[fail] 01/07 20:42 82b39868f277966a "e2e wait no-pay 144208"

    Finished `dev` profile [optimized + debuginfo] target(s) in 0.44s
     Running `target/debug/pylon job status 4c933eff070987257916ce7af4a61062b6d17a1239df07536008cfb42d4ca84c`

Job Status
==========
ID:      4c933eff070987257916ce7af4a61062b6d17a1239df07536008cfb42d4ca84c
Kind:    5050
Status:  failed
Relay:   wss://nexus.openagents.com
Provider: 67e55bf40147de8c8c26968baa75e2db0493036acbeca2e20c7997d141264938
Bid:     1000 msats
Created: 2026-01-07 21:15:19
```

## Provider Log Excerpts

```
No job receipt lines found for `4c933e...` or `914d69...`.

Log tail:
[2m2026-01-07T21:15:04.147207Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider backends: apple_fm
[2m2026-01-07T21:15:04.968714Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Pylon provider started
[2m2026-01-07T21:15:04.968745Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider mode started
[2m2026-01-07T21:15:04.968775Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Pylon daemon running
[2m2026-01-07T21:21:01.936935Z[0m [31mERROR[0m [2mspark::events::server_stream[0m[2m:[0m Error receiving event, reconnecting: status: Internal, message: "h2 protocol error: error reading a body from connection", details: [], metadata: MetadataMap { headers: {} }
```

## Notes

- Capture provider receipt + invoice creation for targeted job.
- Record whether payment request arrives for Nexus-only jobs.
