# Plan: Pylon MVP - Complete NIP-90 + NIP-28 Implementation

**Goal:** Complete the Pylon MVP demo flow - serve inference, earn credits, request inference, chat with providers.

**Status:** IMPLEMENTATION COMPLETE

---

## Completed Steps

### Step 1: Fix Chat Message Tags [DONE]
Added relay URL to "e" tags in `nostr_runtime.rs:397-404` for NIP-28 compliance.

### Step 2: Add Self-Echo for Chat [DONE]
Added optimistic UI - chat messages appear immediately in local state in `app.rs:526-553`.

### Step 3: NIP-90 Client Mode [DONE]
- Added `PendingRequest` struct to track our requests in `state.rs`
- Added `pending_requests` HashMap to `FmVizState`
- Cmd+Enter now publishes job request to network in `app.rs:639-687`
- JobResult handler checks pending_requests and displays result

### Step 4: NIP-28 Channel Creation [DONE]
- Added `CreateOrFindChannel` command and `ChannelFound` event to `nostr_runtime.rs`
- Channel created/found after authentication
- Channel ID stored in state for chat subscriptions

### Step 5: Improve Job Display [DONE]
Updated `ui/jobs_panel.rs` to show:
- Incoming jobs (we serve): `*` pending, `>` serving, `+` complete
- Outgoing jobs (we requested): `<` waiting, `.` received
- "from" vs "to" pubkey display
- Different status text: PEND/SERV/+1 vs WAIT/RECV

### Step 6: CLI Mode [DONE]
- Created `src/core.rs` with `PylonCore` struct
- Created `src/cli.rs` with headless mode
- Updated `src/main.rs` with clap arg parsing
- Added `clap` dependency to Cargo.toml

Usage:
```bash
# GUI mode (default)
pylon-desktop

# CLI provider mode
pylon-desktop --cli

# With specific relay
pylon-desktop --cli --relay wss://relay.openagents.com
```

---

## Tested Functionality

- [x] CLI mode starts without crash
- [x] FM Bridge connects (FM:ON status)
- [x] Nostr relay connects (NOSTR:ON status)
- [x] Pubkey displayed on startup
- [x] Status updates print correctly
- [x] Build compiles without errors

---

## Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added clap dependency |
| `src/main.rs` | CLI arg parsing, dual-mode entry |
| `src/core.rs` | **NEW** - Shared PylonCore struct |
| `src/cli.rs` | **NEW** - Headless CLI mode runner |
| `src/state.rs` | Added `pending_requests`, `is_outgoing`, `PendingRequest` |
| `src/nostr_runtime.rs` | Channel creation, fix chat tags, ChannelFound event |
| `src/app.rs` | Cmd+Enter handling, JobResult handling, self-echo |
| `src/ui/jobs_panel.rs` | Incoming vs outgoing job display |

---

## Remaining Work

### E2E Testing (Manual)
- [ ] Launch GUI and verify all panels render
- [ ] Test Tab to switch focus between panels
- [ ] Serve a job from another instance
- [ ] Request inference with Cmd+Enter
- [ ] Verify chat messages self-echo
- [ ] Run two CLI instances and have them serve each other

### Future Enhancements
- Subscribe to job results for our pubkey (#p filter)
- Proper channel discovery (query kind 40 events)
- Credit system persistence
- Job queue management for multiple pending jobs
