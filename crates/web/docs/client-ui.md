# OpenAgents Web Client UI

GPU-accelerated WGPUI frontend architecture for the web client.

## Overview

The web client is a single-page WASM application that runs entirely on the root URL (`/`). All view transitions happen client-side without navigation. The UI is rendered using WGPUI's GPU-accelerated scene graph.

## View Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ROOT URL (/)                                    │
│                                                                              │
│   ┌──────────────┐     ┌──────────────────┐     ┌─────────────────────────┐│
│   │   Landing    │────▶│   Repo Selector  │────▶│       App Shell         ││
│   │   (login)    │     │   (pick repo)    │     │   (autopilot UI)        ││
│   └──────────────┘     └──────────────────┘     └─────────────────────────┘│
│         │                      │                         │                  │
│         │ /api/auth/github     │ click repo              │ cmd-a, click    │
│         │                      │                         │ Full Auto        │
│         ▼                      ▼                         ▼                  │
│   GitHub OAuth           Set hud_context           Toggle full_auto        │
│   + redirect back        + switch view             enabled state           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Views

### 1. Landing View (`AppView::Landing`)

Shown when user is not logged in. The landing page serves as "The Bazaar" - an open market for agent work.

```
┌─────────────────────────────────────────────────────────────────┐
│ THE BAZAAR                                                       │
│ An open market for agent work                                    │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ LIVE MARKET FEED                                             │  │
│ │ [PATCH] 8f3a21... openagents/runtime#142   4,200 sats ⚡PAID │  │
│ │ [REVIEW] a1b2c3.. vercel/next.js#58921    2,800 sats VERIFY │  │
│ │ [PATCH] d4e5f6... rust-lang/rust#12847    6,100 sats ⚡PAID │  │
│ │ Jobs: 3 | Cleared: 10,300 sats | Providers: 2               │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ DVM MARKETPLACE                          [FEED] [DVMs]       │  │
│ │ • TextGen job from abc123... (2m ago)                        │  │
│ │ • Translation job from def456... (5m ago)                    │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ GLOBAL NOTES                                                 │  │
│ │ Real-time NIP-01 text notes from Nostr                       │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                  │
│   [Connect GitHub]            [Start Earning]                   │
└─────────────────────────────────────────────────────────────────┘
```

#### Landing Page Components

**Live Market Feed (Bazaar Jobs):**
- Displays real Bazaar jobs from Nostr relays (NIP-90 kinds 5930-5933)
- Falls back to demo data when no real jobs are available
- Job type badges: `[PATCH]`, `[REVIEW]`, `[RUN]`, `[INDEX]`
- Status indicators with colors:
  - WORKING (blue) - Job in progress
  - VERIFYING (yellow) - Result received, buyer checking
  - VERIFIED (green) - All checks pass
  - PAID (green with ⚡) - Lightning payment confirmed
- Stats bar shows: job count, cleared sats, unique providers

**DVM Marketplace:**
- Tabbed view: FEED (NIP-90 events) | DVMs (NIP-89 directory)
- Shows real-time NIP-90 job requests and results
- Scrollable with fixed height (8 visible rows)
- Job type badges for generic DVMs: TXT, SUM, GEN, IMG, etc.

**Global Notes Feed:**
- Real-time NIP-01 kind:1 text notes from Nostr
- Author metadata (display names, profile pictures)
- Scrollable feed with newest notes first

**Nostr Relay Connection:**
- Connects to `wss://relay.damus.io` on landing page load
- Subscribes to: NIP-90 jobs, NIP-89 DVMs, NIP-01 notes, Bazaar (5930-5933)
- Status indicator shows connection state

**CTAs:**
- "Connect GitHub" → `/api/auth/github/start`
- "Start Earning" → Links to contributor documentation

### 2. Repo Selector View (`AppView::RepoSelector`)

Shown after login, before selecting a repository.

```
┌─────────────────────────────────────────────────────────────────┐
│ Welcome, username                                    [Logout]   │
│ npub: npub1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx       │
│                                                                  │
│ Select a repository:                                            │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ owner/repo-name                                      Private│ │
│ │ Description of the repository...                            │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ owner/another-repo                                          │ │
│ │ Another description...                                       │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                          ▮      │
└─────────────────────────────────────────────────────────────────┘
```

- **Header:** "Welcome, {username}"
- **Nostr identity:** `npub` shown under the GitHub username
- **Logout button:** Top right, POST to `/api/auth/logout`
- **Repo list:** Scrollable, fetched from `/api/repos`
- **Private badge:** Shown for private repos
- **Scroll indicator:** Right edge when list overflows

### 3. App Shell View (`AppView::RepoView`)

The main Autopilot interface, shown after selecting a repository.

```
┌────────────────────────────────────────────────────────────────────────────┐
│ LEFT DOCK (280px)    │      CENTER PANE         │ RIGHT DOCK (300px)       │
│                      │                          │                          │
│ Model: sonnet        │                          │ ┌──────────────────────┐ │
│ ───────────────────  │                          │ │ ○ FULL AUTO OFF      │ │
│ Sessions             │      owner/repo          │ └──────────────────────┘ │
│                      │                          │                          │
│ ┌──────────────────┐ │  (ThreadView will go     │ Wallet                   │
│ │ Today 14:32      │ │   here)                  │ ┌──────────────────────┐ │
│ │            sonnet│ │                          │ │ Overview | Send | Rx  │ │
│ └──────────────────┘ │                          │ │ Balance + addresses  │ │
│ ┌──────────────────┐ │                          │ │ Recent payments      │ │
│ │ Yesterday 09:15  │ │                          │ └──────────────────────┘ │
│ │             opus │ │                          │                          │
│ └──────────────────┘ │                          │                          │
│ ┌──────────────────┐ │                          │                          │
│ │ Dec 28 16:45     │ │                          │                          │
│ │            sonnet│ │                          │                          │
│ └──────────────────┘ │                          │                          │
│                      │                          │                          │
│ Hotkeys              │                          │                          │
│ cmd-[   left dock    │                          │                          │
│ cmd-]   right dock   │                          │                          │
│ cmd-\   both docks   │                          │                          │
│ cmd-a   full auto    │                          │                          │
├──────────────────────┴──────────────────────────┴──────────────────────────┤
│ cmd-[ / cmd-] toggle docks                                  owner/repo     │
└────────────────────────────────────────────────────────────────────────────┘
                                STATUS BAR (28px)
```

#### Layout Constants

| Element | Size | Notes |
|---------|------|-------|
| Left dock | 280px | Collapsible |
| Right dock | 300px | Collapsible |
| Status bar | 28px | Always visible |
| Padding | 12px | Inside docks |

#### Components

**Left Dock:**
- Model selector (current model name)
- Sessions list (mock data for now)
- Hotkey legend

**Center Pane:**
- Repository name (owner/repo)
- Placeholder for ThreadView (future)

**Right Dock:**
- Full Auto toggle (clickable)
- Wallet panel (Spark/Breez)
  - Balance card (Spark/Lightning/On-chain)
  - Overview/Send/Receive tabs
  - Addresses and recent payments
  - Send/receive controls + invoice display

**Status Bar:**
- Left: Keyboard shortcut hints
- Right: Current repo path

#### Codex Tunnel Overlay

After the intro agent finishes, a **Start Codex** CTA appears in the Autopilot overlay.
Launching it opens a second overlay with:

- Tunnel status header (relay + tunnel connection)
- Local connect command (`openagents pylon connect --tunnel-url ...`)
- Chat thread + prompt input
- Tool approval bar when Codex requests permissions

This flow keeps Codex running locally while the browser remains the UI.

## State Management

### AppState Structure

```rust
struct AppState {
    // View routing
    view: AppView,                    // Landing | RepoSelector | RepoView
    loading: bool,                    // Initial auth check

    // User info
    user: UserInfo,                   // github_username
    repos: Vec<RepoInfo>,             // Fetched from /api/repos
    repos_loading: bool,

    // Selected repository
    selected_repo: Option<String>,    // "owner/repo"
    hud_context: Option<HudContext>,  // Parsed owner + repo

    // App shell state
    left_dock_open: bool,             // Default: true
    right_dock_open: bool,            // Default: true
    full_auto_enabled: bool,          // Default: false
    selected_model: String,           // "sonnet" | "opus" | "haiku"
    sessions: Vec<SessionInfo>,       // Mock session history
    wallet: WalletUi,                 // Spark wallet UI state

    // UI interaction
    mouse_pos: Point,
    button_bounds: Bounds,            // Login/logout button
    button_hovered: bool,
    repo_bounds: Vec<Bounds>,         // Repo list items
    hovered_repo_idx: Option<usize>,
    full_auto_bounds: Bounds,         // Full Auto toggle
    scroll_offset: f32,               // Repo list scroll
}
```

### View Transitions

| From | To | Trigger |
|------|-----|---------|
| Landing | RepoSelector | Successful login (fetch_current_user returns Some) |
| RepoSelector | RepoView | Click repo in list |
| RepoView | Landing | Click logout |
| Any | Landing | Session expired (fetch_current_user returns None) |

## Keyboard Shortcuts

All shortcuts require `cmd` (Mac) or `ctrl` (Windows/Linux) modifier.

| Shortcut | Action | View |
|----------|--------|------|
| `cmd-[` | Toggle left dock | RepoView |
| `cmd-]` | Toggle right dock | RepoView |
| `cmd-\` | Toggle both docks | RepoView |
| `cmd-a` | Toggle Full Auto | RepoView |

Dock toggles are ignored when a wallet text input is focused to avoid clobbering form input.

Implementation:

```rust
// In start_demo(), keydown listener
let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
    let meta = event.meta_key() || event.ctrl_key();
    let wallet_focused = state.wallet.has_focus();
    if meta && !wallet_focused && state.view == AppView::RepoView {
        match event.key().as_str() {
            "[" => state.left_dock_open = !state.left_dock_open,
            "]" => state.right_dock_open = !state.right_dock_open,
            "\\" => { /* toggle both */ },
            "a" => state.full_auto_enabled = !state.full_auto_enabled,
            _ => {}
        }
        event.prevent_default();
    }
});
```

## Click Handling

Click regions are tracked via `Bounds` stored in state:

| Region | Action |
|--------|--------|
| `button_bounds` | Login (Landing) / Logout (other views) |
| `repo_bounds[i]` | Select repo, switch to RepoView |
| `full_auto_bounds` | Toggle Full Auto state |
| Wallet panel | WGPUI component events (send/receive/refresh) |

## Wallet Panel

- **Data:** `/api/wallet/summary` feeds balance, addresses, and recent payments.
- **Actions:** `/api/wallet/send` and `/api/wallet/receive` handle send + receive flows.
- **Components:** BalanceCard, InvoiceDisplay, PaymentRow, TextInput, Button.
- **Events:** Mouse + key events are dispatched into `WalletUi` and spawn async requests.

## Rendering Pipeline

```
┌────────────────────────────────────────────────────────────────────────────┐
│                            Animation Loop (60fps)                          │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  1. Get logical size from platform                                         │
│                                                                            │
│  2. Create new Scene                                                       │
│                                                                            │
│  3. Match current view:                                                    │
│     ├─ Landing    → build_landing_page()                                   │
│     ├─ RepoSelector → build_repo_selector()                                │
│     └─ RepoView   → build_repo_view()                                      │
│                        ├─ draw_left_sidebar()                              │
│                        ├─ draw_center_pane()                               │
│                        ├─ draw_right_sidebar()                             │
│                        └─ draw_status_bar()                                │
│                                                                            │
│  4. Render scene to GPU                                                    │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

## Theme Colors

Using WGPUI's theme system:

| Usage | Color |
|-------|-------|
| `theme::bg::APP` | Main background |
| `theme::bg::SURFACE` | Sidebar/panel background |
| `theme::bg::ELEVATED` | Elevated elements |
| `theme::bg::HOVER` | Hover state |
| `theme::text::PRIMARY` | Main text |
| `theme::text::MUTED` | Secondary text |
| `theme::border::DEFAULT` | Borders |
| `theme::accent::PRIMARY` | Accent color |
| `theme::status::SUCCESS` | Full Auto ON state |
| `theme::status::WARNING` | Private repo badge |
| `theme::status::ERROR` | Logout button |

## Autopilot Chat Overlay

When a repository is selected, an autopilot agent automatically starts and displays a centered chat overlay. The overlay shows the agent introducing itself and exploring the repository.

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              RepoView                                       │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                 Centered Overlay (600px max width)                    │  │
│  │  ┌────────────────────────────────────────────────────────────────┐  │  │
│  │  │ Autopilot                                            (header)   │  │  │
│  │  ├────────────────────────────────────────────────────────────────┤  │  │
│  │  │                                                                 │  │  │
│  │  │ Hello Chris, I am your first Autopilot. I'll begin by          │  │  │
│  │  │ learning about your repo.                                       │  │  │
│  │  │                                                                 │  │  │
│  │  │ 🔧 GitHub API: Fetching repository metadata...                  │  │  │
│  │  │                                                                 │  │  │
│  │  │ Found repository: OpenAgents                                    │  │  │
│  │  │ Language: Rust, 42 stars                                        │  │  │
│  │  │                                                                 │  │  │
│  │  │ 🔧 GitHub API: Checking open issues...                          │  │  │
│  │  │                                                                 │  │  │
│  │  └────────────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│          (backdrop with 85% opacity covers entire viewport)                 │
└────────────────────────────────────────────────────────────────────────────┘
```

### AutopilotChatPane Component

**Location:** `src/autopilot_chat.rs`

The chat overlay wraps WGPUI's `ThreadView` component for message display.

| Property | Value |
|----------|-------|
| Max width | 600px |
| Top margin | 60px |
| Bottom margin | 40px |
| Backdrop | `theme::bg::APP` at 85% alpha |
| Pane background | `theme::bg::SURFACE` |
| Header height | 40px |
| Corner radius | 4px |

### Message Types

| Type | Description | Example |
|------|-------------|---------|
| Assistant | Agent responses | "Found 15 open issues..." |
| Tool | Tool call updates | "🔧 GitHub API: Fetching..." |
| System | System messages | "Connection established" |
| Error | Error messages | "Failed to fetch repo" |

### Event Handling

The overlay intercepts events when visible:
- **Scroll** - Forwarded to ThreadView for scrolling messages
- **Mouse** - Click/move events handled within pane
- **Click outside** - Currently does not dismiss (future enhancement)

### Exploration Phases

The IntroAgent progresses through these phases:

1. **Greeting** - "Hello {username}, I am your first Autopilot..."
2. **Metadata** - Repository description, language, stars
3. **Issues** - Recent open issues
4. **PRs** - Recent pull requests
5. **Tree** - Key directories in the file tree
6. **README** - First 500 characters of README
7. **Commits** - Last 5 commit messages
8. **Contributors** - Top 5 contributors
9. **Complete** - "I've finished learning about your repository..."

See [autopilot.md](./autopilot.md) for detailed implementation documentation.

## Future Enhancements

1. **ThreadView Integration**
   - Connect center pane to actual autopilot thread
   - Stream messages from backend
   - Render tool cards, code blocks

2. **Real Session Data**
   - Fetch sessions from API
   - Resume session on click
   - Delete/archive sessions

3. **Model Selector Dropdown**
   - Click to expand model list
   - Select sonnet/opus/haiku
   - Persist preference

4. **Real Usage Stats**
   - Connect to usage API
   - Live token counts
   - Cost calculation

5. **Full Auto Mode**
   - Connect to autopilot backend
   - Show running state
   - Progress indicators

6. **Dismissable Overlay**
   - Click outside to close chat
   - Minimize/maximize controls
   - Dock to side panel
