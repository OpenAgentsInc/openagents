# Nexus + Pylon v0.1 E2E Test Log (v2)

**Date:** 2026-01-07 14:37
**Status:** in-progress

## Nexus Endpoint Checks

### NIP-11

```bash
curl -s -H "Accept: application/nostr+json" https://nexus.openagents.com/
```

```
{"name":"nexus.openagents.com","description":"OpenAgents Nexus relay","pubkey":"","contact":"nexus@openagents.com","supported_nips":[1,11,42,89,90],"software":"https://github.com/OpenAgentsInc/openagents","version":"0.1.0","limitation":{"max_message_length":524288,"max_subscriptions":20,"max_filters":10,"max_limit":500,"max_subid_length":64,"auth_required":true,"payment_required":false}}
```

### Browser HTML

```bash
curl -s https://nexus.openagents.com/ | head -n 5
```

```
<!DOCTYPE html>
<html>
<head>
    <title>Nexus Relay</title>
    <meta charset="utf-8">
```

### Stats API

```bash
curl -s https://nexus.openagents.com/api/stats
```

```
{"events":{"total":69,"last_24h":69,"by_kind":[{"kind":7000,"count":65},{"kind":5050,"count":2},{"kind":31990,"count":2}]},"jobs":{"pending":2,"completed_24h":0,"by_kind":[{"kind":5050,"count":2}]},"handlers":{"total":2,"by_kind":[{"kind":5050,"count":2}]},"timestamp":1767818284}
```

### WebSocket AUTH Challenge

```bash
node -e "const ws = new WebSocket('wss://nexus.openagents.com'); ws.onopen=()=>console.log('open'); ws.onmessage=(m)=>{console.log('msg', m.data); ws.close();}; ws.onerror=(e)=>{console.error('error', e);};"
```

```
open
msg ["AUTH","368eed4251cc0a9612c368302a70519d7a61809a3013bc8c4f7321beeeba5cd1"]
```

## Pylon Provider (separate HOME)

Provider home: `/tmp/pylon-provider-home/.openagents/pylon`
Log: `/tmp/pylon-provider.log`

```bash
HOME=/tmp/pylon-provider-home RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- start -f -m provider
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.12s
     Running `target/debug/pylon start -f -m provider`
Starting Pylon in foreground mode...
Identity: npub1zv8zyhtva6fpl597pfet98up622udhc3aj94geapp8shx4j0vwls4v7r8k
Mode: Provider
[2m2026-01-07T20:17:55.793948Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Database opened at "/tmp/pylon-provider-home/.openagents/pylon/pylon.db"
[2m2026-01-07T20:17:55.794183Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Loaded stats: 0 jobs completed, 0 sats earned
[2m2026-01-07T20:17:55.794542Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge binary found, attempting to start...
[2m2026-01-07T20:17:55.794698Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m Starting FM Bridge from: "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge"
[2m2026-01-07T20:17:55.804624Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m FM Bridge ready at http://localhost:11435
[2m2026-01-07T20:17:55.804794Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge started at http://localhost:11435
[2m2026-01-07T20:17:55.807124Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Detected inference backends: apple_fm
[2m2026-01-07T20:17:55.810173Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Claude Code not available (no API key or CLI)
[2m2026-01-07T20:17:55.815526Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Spark wallet initialized for payments
[2m2026-01-07T20:17:55.815670Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider backends: apple_fm
[2m2026-01-07T20:17:57.623658Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Pylon provider started
[2m2026-01-07T20:17:57.623745Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider mode started
[2m2026-01-07T20:17:57.623798Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Pylon daemon running
[2m2026-01-07T20:17:57.623845Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider event: Invoice created: 1 sats
[2m2026-01-07T20:17:57.625410Z[0m [33m WARN[0m [2mpylon::cli::start[0m[2m:[0m Failed to record invoice: UNIQUE constraint failed: invoices.id
```

## Buyer Behavior Tests

### 1) Auto-pay, targeted provider (Nexus only)

```bash
cargo run -p pylon -- job submit "e2e targeted auto-pay" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com --provider <PROVIDER_PUBKEY>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.05s
     Running `target/debug/pylon job submit 'e2e targeted auto-pay 143859' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com' --provider 130e225d6cee921fd0be0a72b29f81d295c6df11ec8b5467a109e173564f63bf`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e targeted auto-pay 143859
Auto-pay: enabled

Job Submitted
=============
ID:     78c285e5b3d9a980ed768c93d099b79623caf2cde97667b8973c46a3ce7cbbcc
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.88750275s
```

### 2) Wait without auto-pay (no payment)

```bash
cargo run -p pylon -- job submit "e2e wait no-pay" --model apple-foundation-model --bid 1000 --wait --timeout 30 --relay wss://nexus.openagents.com
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.08s
     Running `target/debug/pylon job submit 'e2e wait no-pay 144208' --model apple-foundation-model --bid 1000 --wait --timeout 30 --relay 'wss://nexus.openagents.com'`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e wait no-pay 144208

Job Submitted
=============
ID:     82b39868f277966a8d430a0a24086ea39f1a65fc73a7cfb99c055a82d8e664f4
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for result (30s remaining)...

Failed to get result: Timeout error: Job result timeout after 29.999999791s
```

### 3) Manual pay flow

```bash
cargo run -p pylon -- job results <JOB_ID> --timeout 30
cargo run -p pylon -- wallet pay <BOLT11>
cargo run -p pylon -- job results <JOB_ID> --timeout 180
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 1.08s
     Running `target/debug/pylon job results 82b39868f277966a8d430a0a24086ea39f1a65fc73a7cfb99c055a82d8e664f4 --timeout 30`
Waiting for result (30s timeout)...
Error: Failed to get result: Timeout error: Job result timeout after 30s
```

### 4) Auto-pay, broadcast (Nexus only)

```bash
cargo run -p pylon -- job submit "e2e broadcast auto-pay" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.88s
     Running `target/debug/pylon job submit 'e2e broadcast auto-pay 144358' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com'`
Submitting job to 1 relays...
  - wss://nexus.openagents.com
Kind: 5050
Prompt: e2e broadcast auto-pay 144358
Auto-pay: enabled

Job Submitted
=============
ID:     99a2b96ec6e9420c02a3ead921a6cf7493cfbc81c4694c9745284e0aedc164e3
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...

No payment request received within 30s.
The provider may not have seen the job or may offer free service.

Waiting for result (150s remaining)...

Failed to get result: Timeout error: Job result timeout after 149.884834791s
```

### 5) Auto-pay, multi-relay

```bash
cargo run -p pylon -- job submit "e2e multi-relay auto-pay" --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.84s
     Running `target/debug/pylon job submit 'e2e multi-relay auto-pay 144707' --model apple-foundation-model --bid 1000 --auto-pay --timeout 180 --relay 'wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol'`
Submitting job to 3 relays...
  - wss://nexus.openagents.com
  - wss://relay.damus.io
  - wss://nos.lol
Kind: 5050
Prompt: e2e multi-relay auto-pay 144707
Auto-pay: enabled

Job Submitted
=============
ID:     91dbe03de3ad0527757b4624e7a401ca3796801d1763a542feaa2efae41e1473
Pubkey: 0636a64c3b560dac740d09d434d5f5d4d993c3249a15c061e0e5409c9ba0a093

Waiting for payment request...
Received feedback: PaymentRequired

Payment Required
================
Amount: 1000 msats

Connecting to Spark wallet...
Preparing payment...
Sending payment...
[2m2026-01-07T20:47:11.337908Z[0m [31mERROR[0m [2mspark_wallet::wallet[0m[2m:[0m Failed to select leaves: TreeServiceError(InsufficientFunds)
Payment sent! ID: 019b9a36-acf9-7a61-84f8-171050a1d7cf

Waiting for result (176s remaining)...

Result Received
===============
No worky
```

### 6) Job list + status

```bash
cargo run -p pylon -- job list --limit 5
cargo run -p pylon -- job status <JOB_ID>
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.31s
     Running `target/debug/pylon job list --limit 5`

Submitted Jobs
==============
[done] 01/07 20:47 91dbe03de3ad0527 "e2e multi-relay auto-pay 14470..."
[fail] 01/07 20:44 99a2b96ec6e9420c "e2e broadcast auto-pay 144358"
[fail] 01/07 20:42 82b39868f277966a "e2e wait no-pay 144208"
[fail] 01/07 20:39 78c285e5b3d9a980 "e2e targeted auto-pay 143859"
[fail] 01/07 20:03 22495e8fcf918384 "e2e test 140320"

    Finished `dev` profile [optimized + debuginfo] target(s) in 0.31s
     Running `target/debug/pylon job status 91dbe03de3ad0527757b4624e7a401ca3796801d1763a542feaa2efae41e1473`

Job Status
==========
ID:      91dbe03de3ad0527757b4624e7a401ca3796801d1763a542feaa2efae41e1473
Kind:    5050
Status:  completed
Relay:   wss://nexus.openagents.com
Bid:     1000 msats
Created: 2026-01-07 20:47:11

Result:
No worky
```

### 7) Wallet history

```bash
cargo run -p pylon -- wallet history
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.34s
     Running `target/debug/pylon wallet history`
Connecting to Spark network (regtest)...

Payment History
===============
→          1 sats  [done]  01/07 20:47  019b9a36-acf9-7a
→          1 sats  [done]  01/07 20:03  019b9a0e-939d-73
→          1 sats  [done]  01/07 20:00  019b9a0b-7c6a-77
→        500 sats  [done]  01/07 06:21  019b971d-cfaa-74
←         32 sats  [pend]  01/07 06:20  85788935-f97e-4a
→      1,000 sats  [done]  01/07 06:20  019b971d-1b4e-75
←        512 sats  [pend]  01/07 06:20  6e4aba6c-89b7-4f
←          4 sats  [pend]  01/07 06:20  9dde91ce-0b24-49
←     49,901 sats  [done]  01/07 06:18  019b971b-ba85-7d
←      9,901 sats  [done]  01/07 06:14  019b9717-4b5b-7c

3 pending payments. Use --completed to hide.
```

### 8) Provider earnings/status (provider HOME)

```bash
HOME=/tmp/pylon-provider-home RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- earnings
HOME=/tmp/pylon-provider-home RUSTUP_HOME=/Users/christopherdavid/.rustup CARGO_HOME=/Users/christopherdavid/.cargo cargo run -p pylon -- status
```

```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.34s
     Running `target/debug/pylon earnings`
Pylon Earnings
==============

Summary:
  Total earned: 0 sats (0 msats)
  Jobs completed: 0

No recent earnings.

    Finished `dev` profile [optimized + debuginfo] target(s) in 0.31s
     Running `target/debug/pylon status`
[2m2026-01-07T20:48:40.102777Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge binary found, attempting to start...
[2m2026-01-07T20:48:40.103148Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m Starting FM Bridge from: "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge"
[2m2026-01-07T20:48:40.111810Z[0m [32m INFO[0m [2mpylon::bridge_manager[0m[2m:[0m FM Bridge ready at http://localhost:11435
[2m2026-01-07T20:48:40.112184Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m FM Bridge started at http://localhost:11435
[2m2026-01-07T20:48:40.114148Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Detected inference backends: apple_fm
[2m2026-01-07T20:48:40.116239Z[0m [32m INFO[0m [2mpylon::provider[0m[2m:[0m Claude Code not available (no API key or CLI)
Pylon Status
============

Daemon: Stopped

  Run 'pylon start' to start the daemon.

Identity:
  Configured

Backends:
  Available: apple_fm (default)

Relays:
  wss://nexus.openagents.com
  wss://relay.damus.io
  wss://nos.lol
```

## Notes

- Provider pubkey (hex): `130e225d6cee921fd0be0a72b29f81d295c6df11ec8b5467a109e173564f63bf`.
- Targeted/broadcast Nexus-only jobs (`78c285...`, `82b398...`, `99a2b9...`) timed out with no payment request; provider log shows no matching job IDs.
- Multi-relay job `91dbe0...` returned result `"No worky"`; provider log shows job receipt + invoice creation but no payment/result events, likely another provider handled the response.
- Provider log excerpt around multi-relay job:
```
[2m2026-01-07T20:47:11.149787Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider event: Job received: job_91db (kind 5050)
[2m2026-01-07T20:47:11.255601Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider event: Invoice created: 1 sats
[2m2026-01-07T20:47:11.358718Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider event: Job received: job_91db (kind 5050)
[2m2026-01-07T20:47:11.359147Z[0m [33m WARN[0m [2mpylon::cli::start[0m[2m:[0m Failed to record job: UNIQUE constraint failed: jobs.id
[2m2026-01-07T20:47:11.461869Z[0m [32m INFO[0m [2mpylon::cli::start[0m[2m:[0m Provider event: Invoice created: 1 sats
[2m2026-01-07T20:47:11.462250Z[0m [33m WARN[0m [2mpylon::cli::start[0m[2m:[0m Failed to record invoice: UNIQUE constraint failed: invoices.id
```
- Spark auto-pay emitted `TreeServiceError(InsufficientFunds)` but payments still recorded as `[done]` in history.
- Stopped provider process after tests: `kill 64233`.
