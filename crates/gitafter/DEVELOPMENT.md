# GitAfter Development Guide

This guide helps contributors understand the GitAfter codebase and make changes effectively.
GitAfter now defaults to the native WGPUI renderer; the legacy web UI is
still available behind `OPENAGENTS_GITAFTER_LEGACY_WEB=1`.

## Quick Start

```bash
# Clone the repo
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# Run GitAfter (WGPUI)
cargo run -p gitafter

# Or via the unified binary
cargo run --bin openagents -- gitafter repos

# Legacy web UI (optional)
OPENAGENTS_GITAFTER_LEGACY_WEB=1 cargo run -p gitafter
```

## Native WGPUI Overview

The WGPUI renderer lives in `src/gui/` and uses a winit + wgpu event loop.
Start with:

- `src/gui/app.rs` for the app runner and event loop integration
- `src/gui/view.rs` for view composition and layout
- `src/gui/backend.rs` for Nostr/event cache data loading

## Legacy Web UI Architecture Overview

GitAfter is a desktop application with three main components:

1. **Desktop Shell** (wry + tao) - Native window container
2. **Web Server** (Actix) - Local HTTP server serving UI
3. **Nostr Client** - Connects to relays, manages subscriptions

### Request Flow

```
User clicks "View Issues"
    ↓
Browser → GET /repo/{id}/issues
    ↓
Actix handler: repository_issues()
    ↓
NostrClient.get_issues_by_repo()
    ↓
Query local cache (SQLite)
    ↓
Render with Maud template
    ↓
Return HTML to browser
```

### Real-time Updates

```
Nostr relay → New event received
    ↓
NostrClient processes event
    ↓
Store in cache
    ↓
Broadcast via WebSocket
    ↓
HTMX updates page without reload
```

## Module Organization

### `src/main.rs`

Entry point that:
1. Initializes tracing
2. Creates WebSocket broadcaster
3. Spawns tokio runtime for async server
4. Connects to Nostr relays
5. Starts Actix server
6. Opens wry/tao window

### `src/server.rs` + `src/server/`

Actix-web server entrypoint and handler modules:

- `GET /` - Home page (repository list)
- `GET /repo/{id}` - Repository detail
- `GET /repo/{id}/issues` - Issues list
- `GET /repo/{id}/issues/new` - Create issue form
- `POST /repo/{id}/issues` - Submit new issue
- `GET /repo/{id}/pulls` - Pull requests list
- `GET /repo/{id}/pulls/new` - Create PR form
- `POST /repo/{id}/pulls` - Submit new PR
- `GET /search` - Search page
- `GET /ws` - WebSocket upgrade

Each route follows the pattern:
1. Extract path parameters
2. Fetch data from NostrClient
3. Render Maud template
4. Return HTML response

### `src/views.rs` + `src/views/`

Maud templates entrypoint and view modules. Each function returns `Markup`:

```rust
pub fn repository_detail_page(
    repository: &Event,
    is_cloned: bool,
    local_path: Option<String>,
) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head { /* ... */ }
            body {
                header { /* ... */ }
                main { /* content */ }
                footer { /* ... */ }
            }
        }
    }
}
```

UI conventions:
- Dark theme by default
- Sharp corners (no border-radius)
- Inline CSS with custom properties
- Server-rendered (no JavaScript framework)
- HTMX for interactivity

### `src/nostr/client.rs`

NostrClient manages Nostr connections:

```rust
pub struct NostrClient {
    pool: Arc<RelayPool>,           // Connection pool
    broadcaster: Arc<WsBroadcaster>, // WebSocket updates
    cache: Arc<Mutex<EventCache>>,  // SQLite cache
}
```

Key methods:
- `connect(relay_urls)` - Connect to relays
- `subscribe_to_git_events()` - Subscribe to NIP-34 events
- `get_repository_by_identifier(id)` - Fetch repository
- `get_issues_by_repo(address, limit)` - Fetch issues
- `get_pull_requests_by_repo(address, limit)` - Fetch PRs

The client:
- Maintains connection pool to multiple relays
- Subscribes to events matching filters
- Caches all events locally (SQLite)
- Broadcasts updates via WebSocket

### `src/nostr/events.rs`

Event builders for creating NIP-34 events:

```rust
// Create a pull request event
let template = PullRequestBuilder::new(
    "30617:pubkey:repo-id",  // repo address
    "Fix authentication bug", // subject
    "This PR fixes...",       // description
)
.commit("abc123def456")
.clone_url("https://github.com/user/repo.git")
.trajectory("session_xyz")
.depends_on("pr_layer_1_event_id")  // for stacked diffs
.build();

// Sign and publish (requires identity - not yet implemented)
// let event = identity.sign_event(template)?;
// nostr_client.publish_event(event).await?;
```

Available builders:
- `PullRequestBuilder` - kind:1618
- `PatchBuilder` - kind:1617
- `IssueClaimBuilder` - kind:1634
- `BountyOfferBuilder` - kind:1636
- `WorkAssignmentBuilder` - kind:1635
- `BountyClaimBuilder` - kind:1637
- `StatusEventBuilder` - kinds:1630-1633

### `src/git/`

Git operations using libgit2:

- `clone.rs` - Clone repositories locally
- Future: diff generation, patch application, rebase

### `src/ws.rs`

WebSocket broadcaster for real-time updates:

```rust
pub struct WsBroadcaster {
    clients: Arc<RwLock<Vec<WsClient>>>,
}

impl WsBroadcaster {
    pub async fn broadcast(&self, message: &str) {
        // Send to all connected WebSocket clients
    }
}
```

## Adding a New Page

1. **Add route in `src/server.rs`** (entrypoint; handlers live under `src/server/`):

```rust
.route("/repo/{identifier}/branches", web::get().to(repository_branches))
```

2. **Add handler function**:

```rust
async fn repository_branches(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let identifier = path.into_inner();

    // Fetch data from NostrClient
    let repo = state.nostr_client
        .get_repository_by_identifier(&identifier)
        .await?;

    // Render view
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(branches_page(&repo, &identifier).into_string())
}
```

3. **Add view in `src/views/`** (entrypoint is `src/views.rs`):

```rust
pub fn branches_page(repository: &Event, identifier: &str) -> Markup {
    html! {
        (DOCTYPE)
        html {
            // Use base_layout pattern from other pages
        }
    }
}
```

4. **Update imports** in `src/server.rs`:

```rust
use crate::views::{..., branches_page};
```

## Adding a New Event Type

1. **Create builder in `src/nostr/events.rs`**:

```rust
pub struct MyNewEventBuilder {
    field1: String,
    field2: Option<String>,
}

impl MyNewEventBuilder {
    pub fn new(field1: impl Into<String>) -> Self {
        Self {
            field1: field1.into(),
            field2: None,
        }
    }

    pub fn field2(mut self, val: impl Into<String>) -> Self {
        self.field2 = Some(val.into());
        self
    }

    pub fn build(self) -> EventTemplate {
        let tags = vec![
            vec!["tag1".to_string(), self.field1],
        ];

        EventTemplate {
            created_at: unix_timestamp(),
            kind: 12345,  // your kind number
            tags,
            content: self.field2.unwrap_or_default(),
        }
    }
}
```

2. **Add test**:

```rust
#[test]
fn test_my_new_event_builder() {
    let template = MyNewEventBuilder::new("value1")
        .field2("value2")
        .build();

    assert_eq!(template.kind, 12345);
    assert!(template.tags.iter().any(|t| t[0] == "tag1"));
}
```

3. **Use in handler**:

```rust
let template = MyNewEventBuilder::new("value").build();
// Sign and publish when identity is integrated
```

## NostrClient Event Caching

Events are cached locally in SQLite at `~/.openagents/gitafter/events.db`:

```sql
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    pubkey TEXT,
    created_at INTEGER,
    kind INTEGER,
    tags TEXT,  -- JSON
    content TEXT,
    sig TEXT
);
```

Cache queries:
- `get_cached_event(id)` - Single event by ID
- `get_cached_repositories(limit)` - All repos (kind:30617)
- `get_issues_by_repo(address, limit)` - Issues for repo
- `get_pull_requests_by_repo(address, limit)` - PRs for repo

## Debugging

### Enable debug logging:

```bash
RUST_LOG=gitafter=debug cargo run -p gitafter
```

### Check what's being received from relays:

```bash
RUST_LOG=gitafter::nostr::client=trace cargo run -p gitafter
```

### Inspect local cache:

```bash
sqlite3 ~/.openagents/gitafter/events.db "SELECT kind, COUNT(*) FROM events GROUP BY kind;"
```

### WebSocket messages:

Open browser DevTools → Network → WS → inspect frames

## Testing

Run tests:

```bash
# All tests
cargo test -p gitafter

# Specific module
cargo test -p gitafter events

# With output
cargo test -p gitafter -- --nocapture
```

## Common Tasks

### Update relay list:

Edit `src/main.rs`:

```rust
let relay_urls = vec![
    "wss://relay.damus.io".to_string(),
    "wss://your-relay.com".to_string(),  // Add here
];
```

### Change window size:

Edit `src/main.rs`:

```rust
.with_inner_size(tao::dpi::LogicalSize::new(1600.0, 1000.0))
```

### Add CSS styling:

Edit `src/styles.css` (included in views via `include_str!`)

### Clear cache and restart:

```bash
rm -rf ~/.openagents/gitafter/events.db
cargo run -p gitafter
```

## Known Limitations

1. **No event signing/publishing** - Requires identity integration (#342)
2. **No offline support** - Relies on active relay connections
3. **Read-only** - Can view but not yet publish events
4. **Limited search** - Depends on relay NIP-50 support
5. **No authentication** - All data is public

## Next Steps

Priority tasks:
1. Integrate wallet identity for event signing
2. Implement event publishing to relays
3. Add repository creation
4. Build code review interface
5. Integrate Lightning payments for bounties

## Resources

- [NIP-34 Specification](https://github.com/nostr-protocol/nips/blob/master/34.md)
- [Maud Documentation](https://maud.lambda.xyz/)
- [Actix-web Guide](https://actix.rs/)
- [wry Documentation](https://docs.rs/wry/)
