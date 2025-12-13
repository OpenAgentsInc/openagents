# Plan: WGPUI Framework & MechaCoder Web Support

Build a cross-platform wgpu-based UI framework (`wgpui`) and port MechaCoder to run on both web and native.

## Key Decisions
- **Auth**: Build fresh, but adapt relevant patterns from Zed's collab crate
- **Platform Priority**: Web-first (native winit support later)
- **AI Backend**: Claude API only initially

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        MechaCoder App                           │
│  (crates/mechacoder - conversation UI, panels, AI backends)     │
└───────────────────────────────┬─────────────────────────────────┘
                                │ uses
┌───────────────────────────────▼─────────────────────────────────┐
│                         WGPUI Framework                         │
│  (crates/wgpui - elements, layout, styling, text, events)       │
├─────────────────────────────────────────────────────────────────┤
│  Platform Abstraction Layer                                     │
│  ┌─────────────────┐  ┌─────────────────┐                      │
│  │ Native Platform │  │  Web Platform   │                      │
│  │ (wgpu + winit)  │  │ (wgpu + canvas) │                      │
│  └─────────────────┘  └─────────────────┘                      │
└─────────────────────────────────────────────────────────────────┘
```

**Deployment:**
```
Browser (WASM)                    AWS Backend
┌──────────────────┐             ┌─────────────────────────┐
│ MechaCoder WASM  │   HTTPS/    │ API Server (Rust/Axum)  │
│ - wgpui UI       │◄───────────►│ - GitHub OAuth callback │
│ - WebSocket RPC  │   WebSocket │ - Token issuance        │
│                  │             │ - AI backend proxy      │
└──────────────────┘             │ - Session management    │
                                 └─────────────────────────┘
```

---

## Phase 1: WGPUI Core Framework

### Step 1.1: Create Crate Structure (Web-First)
**New crate:** `crates/wgpui/`

```
crates/wgpui/
├── Cargo.toml
├── Trunk.toml              # For building WASM demo
├── index.html              # Demo entry point
├── src/
│   ├── lib.rs              # Public API exports
│   ├── app.rs              # Application context, event loop
│   ├── element.rs          # Element trait, lifecycle
│   ├── component.rs        # RenderOnce trait, Component wrapper
│   ├── styled.rs           # Tailwind-like styling API
│   ├── layout.rs           # Taffy integration
│   ├── scene.rs            # Render primitives (Quad, Text, Path)
│   ├── text.rs             # Text system (cosmic-text)
│   ├── events.rs           # Input events, dispatch
│   ├── color.rs            # HSLA color type
│   ├── theme.rs            # Port from openagents-web/theme.rs
│   ├── elements/
│   │   ├── mod.rs
│   │   ├── div.rs          # Container element
│   │   ├── text.rs         # Text element
│   │   └── canvas.rs       # Custom paint element
│   └── platform/
│       ├── mod.rs          # Platform trait
│       └── web.rs          # web-sys + wgpu (browser) - START HERE
│       # native.rs         # Future: winit + wgpu (desktop)
```

### Step 1.2: Core Traits (from GPUI patterns)

**Element Lifecycle:**
```rust
// src/element.rs
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn request_layout(&mut self, cx: &mut LayoutContext)
        -> (LayoutId, Self::RequestLayoutState);

    fn prepaint(&mut self, bounds: Bounds, state: &mut Self::RequestLayoutState, cx: &mut PaintContext)
        -> Self::PrepaintState;

    fn paint(&mut self, bounds: Bounds, state: &mut Self::RequestLayoutState,
             prepaint: &mut Self::PrepaintState, cx: &mut PaintContext);
}
```

**Component Pattern:**
```rust
// src/component.rs
pub trait RenderOnce: 'static + Sized {
    fn render(self, cx: &mut RenderContext) -> impl IntoElement;
}
```

### Step 1.3: Styling System

Port GPUI's `Styled` trait for Tailwind-like API:
```rust
// src/styled.rs
pub trait Styled: Sized {
    fn style(&mut self) -> &mut Style;

    // Layout
    fn flex(mut self) -> Self { self.style().display = Display::Flex; self }
    fn flex_col(mut self) -> Self { self.style().flex_direction = FlexDirection::Column; self }
    fn gap(mut self, gap: impl Into<Length>) -> Self { ... }

    // Sizing
    fn w(mut self, width: impl Into<Length>) -> Self { ... }
    fn h(mut self, height: impl Into<Length>) -> Self { ... }
    fn size(mut self, size: impl Into<Length>) -> Self { ... }

    // Colors
    fn bg(mut self, color: impl Into<Hsla>) -> Self { ... }
    fn text_color(mut self, color: impl Into<Hsla>) -> Self { ... }
    fn border_color(mut self, color: impl Into<Hsla>) -> Self { ... }

    // Borders & Corners
    fn rounded(mut self, radius: impl Into<Length>) -> Self { ... }
    fn border(mut self, width: impl Into<Length>) -> Self { ... }
}
```

### Step 1.4: Layout Engine

Integrate Taffy for CSS Flexbox/Grid:
```rust
// src/layout.rs
pub struct LayoutEngine {
    taffy: taffy::TaffyTree<()>,
    nodes: HashMap<LayoutId, taffy::NodeId>,
}

impl LayoutEngine {
    pub fn request_layout(&mut self, style: &Style, children: &[LayoutId]) -> LayoutId;
    pub fn compute_layout(&mut self, root: LayoutId, available_space: Size);
    pub fn layout(&self, id: LayoutId) -> Bounds;
}
```

### Step 1.5: Platform Abstraction (Web-First)

Start with web platform only, add native later:

```rust
// src/platform/mod.rs
pub trait Platform: 'static {
    fn run(&self, app: Box<dyn FnOnce(&mut App)>);
    fn text_system(&self) -> &dyn TextSystem;
    fn renderer(&self) -> &dyn Renderer;
    fn request_frame(&self);
}

pub trait Renderer {
    fn begin_frame(&mut self, size: Size);
    fn draw_quad(&mut self, quad: &Quad);
    fn draw_text(&mut self, text: &TextRun);
    fn end_frame(&mut self);
}

// Web-first: start with WebPlatform only
pub use web::WebPlatform as DefaultPlatform;

// Future: Add conditional compilation for native
// #[cfg(not(target_arch = "wasm32"))]
// pub use native::NativePlatform as DefaultPlatform;
```

### Step 1.6: Dependencies (Web-First)

```toml
# crates/wgpui/Cargo.toml
[package]
name = "wgpui"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = ["web"]  # Web-first!
web = ["wasm-bindgen", "wasm-bindgen-futures", "web-sys", "js-sys",
       "console_error_panic_hook", "console_log"]
native = ["winit", "pollster"]  # Future: add native support

[dependencies]
wgpu = { version = "24.0", features = ["webgpu", "webgl"] }
taffy = "0.9"
cosmic-text = "0.12"
bytemuck = { version = "1", features = ["derive"] }
smallvec = "1"
slotmap = "1"
futures = "0.3"

# Web (default)
wasm-bindgen = { version = "0.2", optional = true }
wasm-bindgen-futures = { version = "0.4", optional = true }
web-sys = { version = "0.3", optional = true, features = [
    "Window", "Document", "Element", "HtmlCanvasElement",
    "KeyboardEvent", "MouseEvent", "WheelEvent", "PointerEvent",
    "ResizeObserver", "ResizeObserverEntry", "Navigator", "Clipboard",
] }
js-sys = { version = "0.3", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }
console_log = { version = "1.0", optional = true }
getrandom = { version = "0.3", features = ["wasm_js"] }

# Native (future)
winit = { version = "0.30", optional = true }
pollster = { version = "0.4", optional = true }
```

---

## Phase 2: Port UI Components from openagents-web

### Step 2.1: Move Existing Code

Move and refactor from `openagents-web`:
- `theme.rs` → `wgpui/src/theme.rs`
- `text.rs` → `wgpui/src/text.rs` (TextSystem implementation)
- `components/` → Rewrite as proper Elements

### Step 2.2: Core Elements

**Div Element:**
```rust
// src/elements/div.rs
pub struct Div {
    style: Style,
    children: SmallVec<[AnyElement; 4]>,
}

pub fn div() -> Div {
    Div { style: Style::default(), children: SmallVec::new() }
}

impl ParentElement for Div {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}
```

**Text Element:**
```rust
// src/elements/text.rs
pub struct Text {
    content: SharedString,
    style: TextStyle,
}

pub fn text(content: impl Into<SharedString>) -> Text { ... }
```

### Step 2.3: Higher-Level Components

Port from `openagents-web/src/components/`:
- `Button` - clickable with styles (Default, Secondary, Ghost, Outline)
- `Input` - text input with focus, cursor, selection
- `Card` - container with background/border

---

## Phase 3: Authentication for Web

### Step 3.1: OAuth Flow for Browser

**Backend Endpoints (AWS - Actix Web):**
```rust
// New: crates/api-server/src/routes/auth.rs

// 1. Initiate OAuth
GET /auth/github
  → Redirect to GitHub OAuth authorize URL
  → State param includes: redirect_uri, client nonce

// 2. OAuth Callback
GET /auth/github/callback?code=...&state=...
  → Exchange code for GitHub access token
  → Fetch GitHub user info
  → Create/update user in DB
  → Generate access token (JWT or custom)
  → Redirect to frontend with token in fragment: /#token=...

// 3. Validate Token
POST /auth/validate
  → Verify token, return user info
```

**Frontend Flow (WASM):**
```rust
// In wgpui app
pub async fn login_with_github() {
    // 1. Open popup or redirect to /auth/github
    let window = web_sys::window().unwrap();
    window.location().set_href("/auth/github").unwrap();
}

pub fn handle_auth_callback() {
    // 2. Parse token from URL fragment
    let hash = window.location().hash().unwrap();
    if hash.starts_with("#token=") {
        let token = &hash[7..];
        // 3. Store in memory (NOT localStorage for security)
        // 4. Use for subsequent API calls
    }
}
```

### Step 3.2: Token Storage & Usage

**For Web:**
- Store access token in JavaScript memory (not localStorage)
- Pass to WASM via `wasm-bindgen`
- Include in WebSocket connection headers
- Refresh via OAuth popup if expired

**For Native:**
- Use system keychain (existing `credentials_provider`)
- Same token format works for both

### Step 3.3: WebSocket Authentication

```rust
// Connect to backend with auth
pub async fn connect_authenticated(token: &str) -> WebSocket {
    let url = format!("wss://api.openagents.com/ws?token={}", token);
    // or use Sec-WebSocket-Protocol header
}
```

---

## Phase 4: MechaCoder Web Port

### Step 4.1: Abstract Platform-Specific Code

Current MechaCoder uses:
- `gpui` → Replace with `wgpui`
- `tokio` → Use `wasm-bindgen-futures` on web
- File system → Not available on web (use API)
- Claude CLI subprocess → Use WebSocket to backend proxy

### Step 4.2: Backend AI Proxy (Claude API Only)

**New service on AWS:**
```rust
// crates/api-server/src/routes/ai.rs

// WebSocket endpoint for AI conversation
WS /ai/conversation
  → Authenticate via token
  → Proxy to Anthropic Claude API (using ANTHROPIC_API_KEY)
  → Stream responses back to client via Server-Sent Events or WebSocket

// Uses anthropic crate or direct HTTP to api.anthropic.com/v1/messages
```

This replaces the local `claude` CLI subprocess with a server-side proxy to Anthropic's API.

### Step 4.3: Conditional Compilation

```rust
// crates/mechacoder/src/lib.rs

#[cfg(not(target_arch = "wasm32"))]
mod native {
    // Local file access, subprocess spawning
}

#[cfg(target_arch = "wasm32")]
mod web {
    // WebSocket to backend, no file access
}
```

### Step 4.4: Simplified Web UI

For initial web version, support:
- Conversation thread view
- Message input
- Basic panels (Claude panel for model selection)
- OAuth login

Defer to native:
- Local file browsing
- Docker/Harbor execution
- Terminal-Bench (requires local environment)

---

## Phase 5: AWS Deployment

### Step 5.1: Infrastructure

```
┌─────────────────────────────────────────────────────────┐
│                      AWS                                │
├─────────────────────────────────────────────────────────┤
│  CloudFront (CDN)                                       │
│  └── S3 Bucket (static hosting)                         │
│      ├── index.html                                     │
│      ├── mechacoder.wasm                               │
│      └── mechacoder.js                                 │
├─────────────────────────────────────────────────────────┤
│  API Gateway / ALB                                      │
│  └── ECS / Lambda (Rust API Server)                     │
│      ├── /auth/* (OAuth endpoints)                      │
│      ├── /api/* (REST endpoints)                        │
│      └── /ws (WebSocket for AI streaming)              │
├─────────────────────────────────────────────────────────┤
│  RDS PostgreSQL (user DB)                               │
│  Secrets Manager (API keys, OAuth secrets)              │
└─────────────────────────────────────────────────────────┘
```

### Step 5.2: New API Server Crate

```
crates/api-server/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── auth.rs      # GitHub OAuth
│   │   ├── ai.rs        # AI conversation proxy (WebSocket)
│   │   └── user.rs      # User management
│   ├── db.rs            # Database connection
│   └── config.rs        # Environment config
```

**Dependencies:**
```toml
# crates/api-server/Cargo.toml
[package]
name = "api-server"
version = "0.1.0"
edition = "2024"

[dependencies]
actix-web = "4"
actix-web-actors = "4"  # For WebSocket
actix-cors = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
jsonwebtoken = "9"
uuid = { version = "1", features = ["v4"] }
dotenvy = "0.15"
```

### Step 5.3: Build & Deploy

**WASM Build:**
```bash
# Build MechaCoder for web
cd crates/mechacoder
trunk build --release
# Outputs to dist/: index.html, mechacoder.wasm, mechacoder.js

# Upload to S3
aws s3 sync dist/ s3://mechacoder-web/
```

**API Server:**
```bash
# Build for Linux (AWS)
cargo build --release -p api-server --target x86_64-unknown-linux-gnu

# Deploy to ECS/Lambda
```

---

## Implementation Order

### Milestone 1: WGPUI Foundation
1. Create `crates/wgpui/` with basic structure
2. Port Element/Component traits from GPUI
3. Implement Taffy layout integration
4. Port text rendering from openagents-web
5. Basic div, text elements
6. Web platform with wgpu renderer
7. Simple demo app proving it works in browser

### Milestone 2: Components & Styling
1. Port Button, Input, Card components
2. Implement Styled trait (Tailwind-like API)
3. Theme system (theme_oa colors)
4. Event handling (click, keyboard, focus)

### Milestone 3: Authentication Backend
1. Create `crates/api-server/` with Actix Web
2. GitHub OAuth endpoints (redirect flow)
3. Token generation/validation (JWT or custom)
4. PostgreSQL user database

### Milestone 4: MechaCoder Web
1. Create simplified web version of MechaCoder
2. Thread view + message input using WGPUI
3. WebSocket connection to api-server for Claude API proxy
4. OAuth login flow integration

### Milestone 5: AWS Deployment
1. S3 + CloudFront for static WASM hosting
2. ECS or Lambda for api-server
3. RDS PostgreSQL for user data
4. Secrets Manager for API keys

---

## Key Files to Create

| File | Purpose |
|------|---------|
| `crates/wgpui/Cargo.toml` | New UI framework crate |
| `crates/wgpui/Trunk.toml` | WASM build config |
| `crates/wgpui/src/lib.rs` | Public API |
| `crates/wgpui/src/element.rs` | Element trait |
| `crates/wgpui/src/styled.rs` | Tailwind-like styling |
| `crates/wgpui/src/layout.rs` | Taffy integration |
| `crates/wgpui/src/platform/web.rs` | Web platform (primary) |
| `crates/api-server/Cargo.toml` | Backend API server |
| `crates/api-server/src/routes/auth.rs` | GitHub OAuth |
| `crates/api-server/src/routes/ai.rs` | Claude API proxy |

## Key Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Add wgpui, api-server members |

## Reference Files (Copy/Adapt From)

| File | Use For |
|------|---------|
| `crates/openagents-web/src/lib.rs` | Working wgpu WASM renderer |
| `crates/openagents-web/src/text.rs` | cosmic-text integration |
| `crates/openagents-web/src/theme.rs` | Theme colors |
| `crates/gpui/src/element.rs` | Element trait patterns |
| `crates/gpui/src/styled.rs` | Styling API design |
| `crates/gpui/src/taffy.rs` | Layout engine integration |
| `crates/collab/src/auth.rs` | Token generation patterns |
| `crates/anthropic/src/` | Claude API client
kjgg
