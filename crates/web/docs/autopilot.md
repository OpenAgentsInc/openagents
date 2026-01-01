# Autopilot Chat System

The autopilot chat system provides an interactive overlay pane where an AI agent introduces itself and explores the user's GitHub repository. This document covers the client-side components, the BrowserRuntime integration, and the backend API.

## Overview

When a user selects a repository, an autopilot agent automatically starts and displays a centered chat overlay. The agent:

1. Greets the user by name
2. Fetches repository metadata from GitHub
3. Explores issues, PRs, file structure, README, commits, and contributors
4. Reports findings in real-time via the chat interface

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              RepoView                                       │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                    Centered Overlay Pane (600px max)                  │  │
│  │  ┌────────────────────────────────────────────────────────────────┐  │  │
│  │  │ Autopilot                                                       │  │  │
│  │  ├────────────────────────────────────────────────────────────────┤  │  │
│  │  │                                                                 │  │  │
│  │  │ Hello Chris, I am your first Autopilot. I'll begin by          │  │  │
│  │  │ learning about your repo.                                       │  │  │
│  │  │                                                                 │  │  │
│  │  │ 🔧 GitHub API: Fetching repository metadata...                  │  │  │
│  │  │                                                                 │  │  │
│  │  │ Found repository: OpenAgents runtime library                    │  │  │
│  │  │ Language: Rust, 42 stars                                        │  │  │
│  │  │                                                                 │  │  │
│  │  │ 🔧 GitHub API: Checking open issues...                          │  │  │
│  │  │                                                                 │  │  │
│  │  │ Found 15 open issues. Recent ones:                              │  │  │
│  │  │ #142: Add browser runtime support                               │  │  │
│  │  │ #138: Fix container startup timeout                             │  │  │
│  │  │                                                                 │  │  │
│  │  └────────────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────┘
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                               BROWSER (WASM)                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────┐    ┌──────────────────┐                              │
│  │  AutopilotChat   │    │ IntroAgentCtrl   │                              │
│  │  Pane            │◀───│                  │                              │
│  │                  │    │  Async phases:   │                              │
│  │  - ThreadView    │    │  1. Greeting     │                              │
│  │  - Messages      │    │  2. Metadata     │                              │
│  │  - Tool updates  │    │  3. Issues       │                              │
│  │                  │    │  4. PRs          │                              │
│  │                  │    │  5. Tree         │                              │
│  │                  │    │  6. README       │                              │
│  │                  │    │  7. Commits      │                              │
│  │                  │    │  8. Contributors │                              │
│  │                  │    │  9. Complete     │                              │
│  └──────────────────┘    └──────────────────┘                              │
│           │                       │                                         │
│           │                       │ fetch()                                 │
│           │                       ▼                                         │
└───────────│───────────────────────────────────────────────────────────────-─┘
            │                       │
            │                       │ /api/github/explore
            ▼                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          CLOUDFLARE WORKER                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  routes/github_explore.rs                                                   │
│  ├── Authenticate request (session cookie)                                 │
│  ├── Decrypt GitHub token from D1                                          │
│  ├── Call GitHub API (repos, issues, pulls, contents, commits, etc.)       │
│  └── Return aggregated exploration data                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

> **Note:** The full BrowserRuntime from `openagents-runtime` is not currently used because
> it relies on `std::time::SystemTime` which doesn't work in WASM. Once the runtime is updated
> to use `web_time`, we can integrate it for proper agent lifecycle management.

## Client Components

### AutopilotChatPane (`autopilot_chat.rs`)

A centered overlay component that wraps WGPUI's `ThreadView` for chat display.

**Location:** `crates/web/client/src/autopilot_chat.rs`

```rust
pub(crate) struct AutopilotChatPane {
    pub(crate) thread: ThreadView,    // Chat message display
    pub(crate) visible: bool,          // Overlay visibility
    pub(crate) bounds: Bounds,         // Pane bounds (centered)
    backdrop_bounds: Bounds,           // Full-screen backdrop
}
```

#### Methods

| Method | Description |
|--------|-------------|
| `new()` | Create a new chat pane (hidden by default) |
| `show(github_username)` | Show pane with greeting message |
| `hide()` | Hide the overlay |
| `push_assistant_message(text)` | Add an assistant (agent) message |
| `push_tool_message(tool, status)` | Add a tool call update |
| `push_system_message(text)` | Add a system message |
| `push_error_message(text)` | Add an error message |
| `calculate_bounds(width, height)` | Compute centered layout |
| `paint(cx)` | Render the overlay |
| `handle_event(event)` | Process input events |
| `contains(point)` | Check if point is within overlay |

#### Layout

- **Backdrop:** Full viewport, semi-transparent (`bg::APP` at 85% alpha)
- **Pane:** Centered, max 600px wide, 60px top margin
- **Header:** 40px tall, "Autopilot" title
- **Content:** ThreadView with auto-scroll enabled

#### Message Types

Uses WGPUI's `ThreadEntryType` for different message styles:

| Type | Usage | Style |
|------|-------|-------|
| `Assistant` | Agent responses | Primary text |
| `Tool` | Tool call updates | Tool badge + status |
| `System` | System messages | Muted text |
| `Error` | Error messages | Error color |

### IntroAgentController (`intro_agent.rs`)

A controller that orchestrates the GitHub exploration via async/await and browser fetch API.

**Location:** `crates/web/client/src/intro_agent.rs`

#### Controller Structure

```rust
pub(crate) struct IntroAgentController;

impl IntroAgentController {
    /// Start the intro agent exploration.
    pub(crate) fn start(
        state: Rc<RefCell<AppState>>,
        github_username: String,
        repo: String,
    ) {
        // Show the chat pane with greeting
        state.borrow_mut().autopilot_chat.show(&github_username);

        // Start the exploration async
        wasm_bindgen_futures::spawn_local(async move {
            run_exploration(state, github_username, repo).await;
        });
    }
}

/// Public entry point
pub(crate) fn start_intro_agent(
    state: Rc<RefCell<AppState>>,
    github_username: String,
    repo: String,
) {
    IntroAgentController::start(state, github_username, repo);
}
```

#### Exploration Phases

```rust
pub(crate) enum IntroPhase {
    Greeting,            // Initial greeting message
    FetchingMetadata,    // Repo description, language, stars
    FetchingIssues,      // Open issues list
    FetchingPRs,         // Open pull requests
    FetchingTree,        // File/directory structure
    FetchingReadme,      // README excerpt
    FetchingCommits,     // Recent commits
    FetchingContributors,// Top contributors
    Complete,            // All exploration done
    Failed,              // Error occurred
}
```

#### State Structure

```rust
pub(crate) struct IntroAgentState {
    pub(crate) phase: IntroPhase,
    pub(crate) github_username: String,
    pub(crate) repo: String,
    pub(crate) repo_description: Option<String>,
    pub(crate) repo_language: Option<String>,
    pub(crate) repo_stars: Option<u64>,
    pub(crate) open_issues_count: Option<u64>,
    pub(crate) open_prs_count: Option<u64>,
    pub(crate) recent_issues: Vec<String>,
    pub(crate) recent_prs: Vec<String>,
    pub(crate) file_tree: Vec<String>,
    pub(crate) readme_excerpt: Option<String>,
    pub(crate) recent_commits: Vec<String>,
    pub(crate) contributors: Vec<String>,
    pub(crate) error: Option<String>,
}
```

#### Exploration Flow

The `run_exploration()` async function orchestrates the exploration:

```rust
async fn run_exploration(state: Rc<RefCell<AppState>>, username: String, repo: String) {
    // 1. Create BrowserRuntime
    let config = BrowserRuntimeConfig::new("");
    let runtime = BrowserRuntime::new(config);

    // 2. Register IntroAgent
    let agent_id = AgentId::new(format!("intro-{}", uuid_v4()));
    let agent = IntroAgent::new(username.clone(), repo.clone());
    runtime.register_agent(agent_id.clone(), agent)?;

    // 3. Fetch exploration data from backend
    push_tool_message(&state, "GitHub API", "Fetching repository metadata...");
    let response = fetch_explore(&repo).await?;

    // 4. Process and display results for each phase
    // ... (metadata, issues, PRs, tree, README, commits, contributors)

    // 5. Tick the agent to advance phases
    runtime.tick_manual(&agent_id).await?;
}
```

### Integration Points

#### AppState Fields

```rust
// In state.rs
pub(crate) struct AppState {
    // ... other fields
    pub(crate) autopilot_chat: AutopilotChatPane,
    pub(crate) intro_agent_state: IntroAgentState,
}
```

#### Starting the Agent

The agent starts automatically when a repository is selected:

```rust
// In app.rs - when repo is clicked
if state.repo_bounds[i].contains(point) {
    let repo_full = format!("{}/{}", repo.owner, repo.name);
    state.selected_repo = Some(repo_full.clone());
    state.view = AppView::RepoView;

    // Start intro agent
    let github_username = state.user.github_username.clone().unwrap_or_default();
    crate::intro_agent::start_intro_agent(
        state_clone.clone(),
        github_username,
        repo_full.clone(),
    );
}
```

#### Overlay Rendering

The overlay is painted on top of the RepoView:

```rust
// In views/mod.rs - build_repo_view()
pub(crate) fn build_repo_view(...) {
    // ... draw main HUD content

    // Draw autopilot chat overlay (on top of everything)
    if state.autopilot_chat.visible {
        state.autopilot_chat.calculate_bounds(width, height);
        let mut cx = PaintContext::new(scene, text_system, scale_factor);
        state.autopilot_chat.paint(&mut cx);
    }
}
```

#### Event Handling

Scroll events are intercepted by the overlay when visible:

```rust
// In app.rs - scroll handler
if state.autopilot_chat.visible && state.autopilot_chat.contains(point) {
    let _ = state.autopilot_chat.handle_event(scroll);
    return;
}
```

## Backend API

### GitHub Explore Endpoint

**Route:** `GET /api/github/explore?repo=owner/name`

**Location:** `crates/web/worker/src/routes/github_explore.rs`

**Authentication:** Required (session cookie)

**Request:**
```
GET /api/github/explore?repo=openagents/openagents
Cookie: oa_session=...
```

**Response:**
```json
{
  "repo": {
    "description": "Autonomous AI agents on Bitcoin",
    "language": "Rust",
    "stargazers_count": 142,
    "open_issues_count": 23
  },
  "issues": [
    { "number": 142, "title": "Add browser runtime support" },
    { "number": 138, "title": "Fix container startup timeout" }
  ],
  "pull_requests": [
    { "number": 140, "title": "Implement autopilot chat overlay" }
  ],
  "tree": [
    { "path": "crates", "type": "tree" },
    { "path": "docs", "type": "tree" },
    { "path": "README.md", "type": "blob" }
  ],
  "readme_excerpt": "# OpenAgents\n\nAutonomous AI agents...",
  "commits": [
    { "sha": "abc123def", "message": "Add autopilot chat pane" }
  ],
  "contributors": [
    { "login": "christopherdavid", "contributions": 500 }
  ]
}
```

**Implementation:**

```rust
pub async fn explore(req: Request, env: Env) -> Result<Response> {
    // 1. Authenticate user
    let user = middleware::auth::authenticate(&req, &env).await?;

    // 2. Get repo from query params
    let repo = url.query_pairs()
        .find(|(k, _)| k == "repo")
        .map(|(_, v)| v.to_string())
        .ok_or("Missing repo parameter")?;

    // 3. Get GitHub token from D1
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let token = db::users::get_github_access_token(&db, &user.user_id, &session_secret).await?;

    // 4. Call GitHub API
    let client = services::github::GitHubClient::new(&token);

    let repo_info = client.get_repo(&repo).await?;
    let issues = client.get_issues(&repo, 10).await?;
    let prs = client.get_pulls(&repo, 10).await?;
    let tree = client.get_tree(&repo).await?;
    let readme = client.get_readme(&repo).await?;
    let commits = client.get_commits(&repo, 5).await?;
    let contributors = client.get_contributors(&repo, 5).await?;

    // 5. Return aggregated response
    Response::from_json(&ExploreResponse {
        repo: Some(repo_info),
        issues,
        pull_requests: prs,
        tree,
        readme_excerpt: readme.map(|r| truncate(&r, 500)),
        commits,
        contributors,
        error: None,
    })
}
```

## BrowserRuntime Integration (Future)

The full `openagents-runtime` BrowserRuntime is **not currently used** due to WASM compatibility issues.

### Current Limitation

The runtime's `Timestamp::now()` uses `std::time::SystemTime::now()` which does not work in WASM:

```rust
// crates/runtime/src/types.rs - BROKEN IN WASM
impl Timestamp {
    pub fn now() -> Self {
        let duration = SystemTime::now()  // <- Panics in browser!
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(duration.as_millis() as u64)
    }
}
```

### Future Integration

Once the runtime is updated to use `web_time` or `js_sys::Date`, we can enable:

```toml
# In crates/web/client/Cargo.toml
[dependencies]
openagents-runtime = { path = "../../runtime", default-features = false, features = ["browser"] }
```

The BrowserRuntime will provide:

- **TickEngine** - Manages agent tick execution
- **IndexedDbStorage** - Persistent state storage in browser
- **InMemorySigner** - Cryptographic identity for agents
- **ComputeRouter** - Routes to OpenAgents compute providers
- **ContainerRouter** - Routes to container providers

### Pre-commit Hook

A pre-commit hook in `.git/hooks/pre-commit` blocks commits with `std::time::Instant` or
`std::time::SystemTime` in `crates/web/client/` or `crates/wgpui/`.

## Error Handling

### Client-Side Errors

```rust
// In run_exploration()
match fetch_explore(&repo).await {
    Ok(response) => {
        // Process response...
    }
    Err(error) => {
        push_error_message(&state, &format!("Failed to explore repository: {}", error));
    }
}
```

### Backend Errors

```rust
// In github_explore.rs
if !response.ok() {
    return Response::from_json(&ExploreResponse {
        error: Some(format!("GitHub API error: {}", response.status())),
        ..Default::default()
    });
}
```

## WASM Compatibility Notes

The following are **FORBIDDEN** in `crates/web/client/` and `crates/wgpui/`:

| Forbidden | Alternative |
|-----------|-------------|
| `std::time::Instant` | `web_time::Instant` |
| `std::time::SystemTime` | `js_sys::Date::now()` or `web_time` |

These are enforced by the pre-commit hook (see `.git/hooks/pre-commit`).

**Best practices:**

- **Async operations** - Use `wasm_bindgen_futures::spawn_local()`
- **HTTP requests** - Use `web_sys::Request` and `web_sys::Response`
- **Timers** - Use `js_sys::Promise` with `window.set_timeout()`
- **Storage** - Use `web_sys::Storage` (localStorage) or IndexedDB

## Claude Tunnel Overlay

After the intro agent finishes, the Autopilot overlay exposes a **Start Claude** CTA.
This opens a separate Claude chat overlay that:

- Registers a tunnel session via `/api/tunnel/register`
- Shows the `openagents pylon connect --tunnel-url ...` command
- Streams Claude output from the local Pylon tunnel client
- Prompts for tool approvals when required

The browser remains the UI while Claude runs locally on the user's machine.

## Future Enhancements

1. **BrowserRuntime Integration** - Use full runtime once WASM-compatible
2. **User Input** - Allow user to ask questions in the chat
3. **Dismissable Overlay** - Click outside to close
4. **Persistent Sessions** - Resume previous exploration (IndexedDB)
5. **More Tools** - Code search, dependency analysis, CI status
6. **Streaming Responses** - Real-time tool output via SSE/WebSocket
