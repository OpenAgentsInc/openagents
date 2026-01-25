# Why "Requires codex-acp to send these extensions"

## The Architecture

In ACP (Agent Client Protocol), there are two roles:

1. **Agent** (`codex-acp`) - Sends notifications, responds to requests
2. **Client** (us, `autopilot`) - Receives notifications, sends requests

```
┌─────────────────┐         ┌──────────────┐         ┌─────────────┐
│ codex app-server│────────▶│  codex-acp   │────────▶│  autopilot  │
│  (Codex CLI)    │ events  │  (adapter)   │  ACP    │  (client)   │
│                 │         │               │ notifs  │             │
└─────────────────┘         └──────────────┘         └─────────────┘
     sends:                      translates:            receives:
  • rateLimits/updated    →    codex/tokenUsage   →    (we get it)
  • tokenUsage/updated     →    codex/tokenUsage   →    (we get it)
```

## The Problem

**We (autopilot) are the CLIENT** - we can only:
- ✅ Send requests TO `codex-acp` (like `session/prompt`)
- ✅ Receive notifications FROM `codex-acp` (like `session/update`)

**We CANNOT**:
- ❌ Make `codex-acp` send notifications it doesn't already send
- ❌ Add custom notifications from our side

## What "Requires codex-acp to send" Means

For custom ACP extensions like `codex/tokenUsage` to work:

1. **`codex-acp` (the adapter) must be modified** to:
   - Listen to Codex app-server events (like `account/rateLimits/updated`)
   - Translate them into ACP custom notifications (like `codex/tokenUsage`)
   - Send those notifications to ACP clients (us)

2. **We (autopilot) can only receive** what `codex-acp` sends

## Current Situation

Right now, `codex-acp` only sends:
- `session/update` notifications
- Standard ACP `SessionNotification` types (if any)

It does NOT send:
- `codex/tokenUsage` custom notifications
- `codex/rateLimits` custom notifications
- Other Codex-specific events as ACP extensions

## Solutions

### Option 1: Modify codex-acp (Requires Zed Team)
- Fork or contribute to `zed-industries/codex-acp`
- Add code to translate Codex events → ACP custom notifications
- Submit PR or maintain our own fork

### Option 2: Dual Protocol (What We're Doing Now)
- Keep `codex app-server` running for Codex-specific events
- Keep `codex-acp` running for ACP standardized events
- Merge events from both sources in our UI

### Option 3: Wait for codex-acp Updates
- Hope Zed team adds these extensions
- Use dual protocol in the meantime

## Why We Can't Add Extensions From Our Side

In ACP, the **agent sends notifications, client receives them**. The direction is:

```
codex-acp (agent) ──sends notification──> autopilot (client)
```

We can't reverse this flow. We can only:
- Send requests (which get responses)
- Receive notifications (which we can't request)

So if `codex-acp` doesn't send `codex/tokenUsage` notifications, we can't make it send them from our side.
