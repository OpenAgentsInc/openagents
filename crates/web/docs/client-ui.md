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

Shown when user is not logged in.

```
┌─────────────────────────────────────────────────────────────────┐
│ LIVE  Autopilot is working on issue #847                         │
│ @openagents/openagents                                           │
│                                                                  │
│   [live HUD panes streaming in the background]                   │
│                                                                  │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ Autopilot for code                                           │  │
│ │ Watch it work. Connect GitHub to get your own HUD in <30 sec. │  │
│ │ [Connect GitHub → Get Your Own Autopilot]                     │  │
│ └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

- **Live fishbowl:** Renders the real HUD in the background when `/api/hud/live` is enabled.
- **Issue banner:** Clickable when `LIVE_HUD_ISSUE_URL` is set.
- **CTA button:** Navigates to `/api/auth/github/start`
- **Empty state:** If no live config is present, shows a "No live session" message.

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
