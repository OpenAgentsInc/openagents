# Plan: Bloomberg Terminal-Style Nostr Chat Interface

## Overview

Build a Bloomberg Terminal-inspired chat interface for OpenAgents Commander that combines:
- **NIP-01**: Event structure, signing, verification
- **NIP-06**: Key derivation from mnemonic (identity)
- **NIP-28**: Public chat channels
- **NIP-90**: DVM job requests/results (agent coordination)

Future: NIP-EE for private encrypted group chat (MLS protocol).

---

## Design Philosophy (from Inspiration Docs)

### Bloomberg Terminal Principles
1. **Information density** - More data per pixel than any mainstream UI
2. **Keyboard-first** - Every interaction has a shortcut; mouse optional
3. **Command-driven** - Global command bar always active
4. **Real-time awareness** - Colors + flashes communicate state changes
5. **Predictable layout** - Four-panel tiling, consistent hierarchy
6. **Stability** - UI rarely changes; muscle memory sacred

### Instant Bloomberg (IB) Chat Principles
- Text-first, no avatars, no rich media
- **Yellow**: outgoing messages, **White/green**: incoming, **Red**: errors
- Multi-pane monitoring (10-30 chat windows simultaneously)
- Templates & snippets for velocity
- Data streaming embedded in chat (tickers, prices)

### StarCraft/Factorio Patterns
- Control groups (Cmd+1-9) for agent clusters
- APM tracking (jobs/hour, efficiency metrics)
- Minimap for always-visible overview
- Real-time status indicators (pulse, flash)

---

## Architecture

### Leveraging Existing Codebase

The implementation builds on existing patterns in Commander:

```
crates/
├── nostr/              # Already implemented: NIP-01, 06, 28, 90
├── nostr-relay/        # NEW: WebSocket relay connections
├── nostr-chat/         # NEW: Chat state machine
├── commander/src/
│   ├── screens/
│   │   └── chat.rs     # NEW: Chat screen (similar to marketplace screen.rs)
│   └── components/
│       └── chat/       # NEW: Chat-specific components
└── marketplace/src/
    └── activity_feed.rs # REFERENCE: Dense list rendering pattern
```

### New Crate: `crates/nostr-relay`

WebSocket relay connection pool (similar to existing FM bridge patterns):

```rust
pub struct RelayPool {
    connections: HashMap<String, RelayConnection>,
    pending_updates: Arc<Mutex<Vec<NostrMessage>>>,
}

pub struct RelayConnection {
    url: String,
    state: ConnectionState,
    subscriptions: Vec<Subscription>,
}

// Key functions matching existing streaming patterns
pub async fn connect(url: &str) -> Result<RelayConnection>;
pub async fn subscribe(filter: Filter) -> Result<Subscription>;
pub async fn publish(event: Event) -> Result<EventId>;
```

### New Crate: `crates/nostr-chat`

Chat state machine combining NIP-28 channels with NIP-90 DVM:

```rust
pub struct ChatState {
    identity: Keypair,                        // NIP-06 derived
    channels: HashMap<EventId, Channel>,
    messages: Vec<ChatMessage>,               // Like ThreadItem enum
    dvm_jobs: HashMap<EventId, JobState>,
}

// Message types (similar to ThreadItem pattern)
pub enum ChatMessage {
    ChannelMessage { id, author, content, timestamp, is_reply },
    JobRequest { id, kind, status },
    JobResult { id, content },
    SystemNotice { message },
}
```

---

## UI Components (GPUI)

### Screen Layout (4-Panel Bloomberg Style)

```
┌─────────────────────────────────────────────────────────────────────────┐
│ > join #bitcoin                                    npub1abc... | 15:32  │  <- Command Bar
├──────────────────┬──────────────────────────────────┬───────────────────┤
│ CHANNELS         │ #bitcoin-dev              1,234u │ CHANNEL INFO      │
├──────────────────┤──────────────────────────────────┤───────────────────┤
│ > #bitcoin-dev 12│ 15:32 satoshi  Looking for...   │ Created: 2024-01  │
│   #nostr-dev    3│ 15:33 hal      @satoshi I'll... │ Users: 1,234      │
│   #agents        │ 15:34 [DVM:5050] Job submitted  │                   │
├──────────────────┤ 15:35 [DVM:5050] Result: ...    │ RECENT USERS      │
│ DVM JOBS         │ 15:36 adam     Can someone...   │───────────────────│
├──────────────────┤                                  │ satoshi           │
│   5050 text OK   │                                  │ hal               │
│   5100 img  ..   │                                  │ adam              │
└──────────────────┴──────────────────────────────────┴───────────────────┘
│ CONNECTED: relay.damus.io | npub1abc... | 15:36:02 UTC                  │  <- Status Bar
└─────────────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. ChatScreen (screens/chat.rs)
Main orchestrator, similar to `marketplace/screen.rs`:
- Manages 4-panel layout
- Handles keyboard shortcuts
- Routes commands to appropriate handlers

#### 2. CommandBar
Always-active input at top (like Bloomberg command bar):
```rust
pub struct CommandBar {
    input: TextInput,  // Reuse existing TextInput component
    history: Vec<String>,
}

// Commands:
// join #channel, create #channel <name>, msg @npub <text>
// job <kind> <input>, mute @npub, hide <event-id>
```

#### 3. ChannelList (left panel)
Dense list of channels + DVM jobs (like activity_feed pattern):
```rust
pub struct ChannelList {
    channels: Vec<ChannelItem>,
    jobs: Vec<JobItem>,
    selected: Option<EventId>,
}

pub struct ChannelItem {
    id: EventId,
    name: String,
    unread_count: u32,
}
```

#### 4. MessageView (center panel)
Bloomberg IB-style message display:
```rust
pub struct MessageView {
    channel_id: EventId,
    messages: Vec<ChatMessage>,
    scroll_position: f32,
}

// Color coding (Bloomberg style):
// Yellow: outgoing, White: incoming, Green: DVM results
// Orange: DVM in-progress, Red: errors
```

#### 5. InfoPanel (right panel)
Context-sensitive info (channel metadata, user info, job details)

#### 6. StatusBar (bottom)
Connection status, identity, timestamps

---

## Keyboard Shortcuts (StarCraft-inspired)

### Navigation
| Key | Action |
|-----|--------|
| `Cmd+1-9` | Switch to channel 1-9 |
| `Cmd+J` | Jump to channel (fuzzy search) |
| `Cmd+K` | Command palette |
| `Tab` | Cycle panels |
| `Esc` | Focus command bar |

### Actions
| Key | Action |
|-----|--------|
| `Enter` | Send message / Execute command |
| `Cmd+Enter` | Submit DVM job |
| `Cmd+R` | Reply to selected message |
| `Cmd+M` | Mute selected user |

### Panel Management
| Key | Action |
|-----|--------|
| `F1` | Toggle channel list |
| `F2` | Toggle info panel |
| `F3` | Focus message input |

---

## Implementation Phases

### Phase 1: Relay Infrastructure
**Files to create:**
- `crates/nostr-relay/Cargo.toml`
- `crates/nostr-relay/src/lib.rs`
- `crates/nostr-relay/src/connection.rs` - WebSocket connection
- `crates/nostr-relay/src/pool.rs` - Connection pooling
- `crates/nostr-relay/src/subscription.rs` - Filter & subscription management
- `crates/nostr-relay/src/message.rs` - Nostr relay message types (EVENT, REQ, CLOSE, OK, etc.)

**Pattern to follow:** Existing FM bridge streaming pattern in `commander/src/main.rs:120-275`

### Phase 2: Chat State Machine
**Files to create:**
- `crates/nostr-chat/Cargo.toml`
- `crates/nostr-chat/src/lib.rs`
- `crates/nostr-chat/src/state.rs` - ChatState management
- `crates/nostr-chat/src/channel.rs` - Channel operations (join, leave, message)
- `crates/nostr-chat/src/dvm.rs` - DVM job tracking

**Pattern to follow:** ThreadItem enum pattern from `commander/src/components/thread_item.rs`

### Phase 3: Chat UI Components
**Files to create:**
- `crates/commander/src/screens/chat.rs` - Main chat screen
- `crates/commander/src/components/chat/mod.rs`
- `crates/commander/src/components/chat/command_bar.rs`
- `crates/commander/src/components/chat/channel_list.rs`
- `crates/commander/src/components/chat/message_view.rs`
- `crates/commander/src/components/chat/info_panel.rs`
- `crates/commander/src/components/chat/status_bar.rs`

**Files to modify:**
- `crates/commander/src/main.rs` - Add Chat to Screen enum
- `Cargo.toml` - Add new crate members

**Pattern to follow:** `marketplace/src/screen.rs` for multi-panel layout, `marketplace/src/activity_feed.rs` for dense list rendering

### Phase 4: Integration
1. Add chat screen to Commander navigation
2. Implement identity flow (NIP-06 mnemonic -> keypair on first run)
3. Connect to default relays on startup
4. Persist channel subscriptions to SQLite
5. Add DVM job submission from command bar

### Phase 5: Polish
1. Message flashing for new messages (Bloomberg-style)
2. Unread counts and indicators
3. Sound notifications (configurable)
4. Multi-pane tiling (split view for multiple channels)

---

## Data Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Relay Pool     │────▶│  ChatState      │────▶│  GPUI Views     │
│ (WebSocket)     │     │ (Arc<Mutex>)    │     │ (render)        │
│                 │     │                 │     │                 │
│ - connect()     │     │ - channels      │     │ - CommandBar    │
│ - subscribe()   │     │ - messages      │     │ - ChannelList   │
│ - publish()     │     │ - dvm_jobs      │     │ - MessageView   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
        └───────────────────────┴───────────────────────┘
                    50ms polling loop
              (same pattern as FM bridge)
```

---

## Critical Files Reference

### Existing patterns to follow:
- `crates/commander/src/main.rs:120-275` - Message subscription/streaming pattern
- `crates/marketplace/src/activity_feed.rs` - Dense list rendering
- `crates/marketplace/src/screen.rs` - Multi-panel screen composition
- `crates/commander/src/text_input.rs` - TextInput component
- `crates/commander/src/components/thread_item.rs` - Message type enum

### NIPs we built:
- `crates/nostr/src/nip01.rs` - Event structure, signing
- `crates/nostr/src/nip06.rs` - Key derivation from mnemonic
- `crates/nostr/src/nip28.rs` - Public chat channels
- `crates/nostr/src/nip90.rs` - DVM job requests/results

---

## Success Criteria

1. Can derive identity from mnemonic (NIP-06)
2. Can connect to relays and receive events (NIP-01)
3. Can join/create public channels (NIP-28)
4. Can send/receive channel messages (NIP-28)
5. Can submit DVM jobs and see results (NIP-90)
6. Keyboard-driven navigation works
7. Multi-panel Bloomberg layout works
8. Performance: <16ms render, <100ms message latency

---

## Future: NIP-EE Private Encrypted Group Chat

After public chat is working, add NIP-EE support:
- MLS protocol for forward secrecy
- KeyPackage events (kind 443)
- Welcome messages (kind 444)
- Encrypted group events (kind 445)
- Group admin/member management
