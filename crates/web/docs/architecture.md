# OpenAgents Web Architecture

Technical architecture of the full-stack Rust web application running on Cloudflare's edge.

## System Overview

OpenAgents Web consists of two WASM targets:

1. **Client WASM** (wasm-pack → browser) - WGPUI GPU-accelerated frontend
2. **Worker WASM** (worker-build → Cloudflare) - Axum API backend

Both are 100% Rust, compiled to WebAssembly, running at the edge.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLOUDFLARE EDGE                                 │
│                         (300+ global locations)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                         WORKER (Server-side WASM)                        ││
│  │                                                                          ││
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐              ││
│  │  │   Router     │    │   D1         │    │   KV         │              ││
│  │  │   (Axum)     │───▶│   (SQLite)   │    │   (Sessions) │              ││
│  │  │              │    │              │    │              │              ││
│  │  │  Pattern     │    │  users       │    │  session:*   │              ││
│  │  │  matching    │    │  billing     │    │  oauth:*     │              ││
│  │  │  on path     │    │  stripe      │    │              │              ││
│  │  └──────────────┘    └──────────────┘    └──────────────┘              ││
│  │         │                                                                ││
│  │         ▼                                                                ││
│  │  ┌──────────────────────────────────────────────────────────────────┐  ││
│  │  │                        Route Handlers                             │  ││
│  │  │                                                                   │  ││
│  │  │  routes/auth.rs      → GitHub OAuth flow                         │  ││
│  │  │  routes/account.rs   → User settings, API keys                   │  ││
│  │  │  routes/billing.rs   → Credits, plans, packages                  │  ││
│  │  │  routes/stripe.rs    → Payment methods, webhooks                 │  ││
│  │  │  routes/wallet.rs    → Spark wallet summary/send/receive         │  ││
│  │  │  routes/hud.rs       → Personal HUD URLs, embed                  │  ││
│  │  └──────────────────────────────────────────────────────────────────┘  ││
│  │                                                                          ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                      │                                       │
│                                      │ serves static assets                  │
│                                      ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                       STATIC ASSETS (CDN Cache)                          ││
│  │                                                                          ││
│  │  index.html                                                              ││
│  │  pkg/openagents_web_client.js      ← wasm-bindgen glue                  ││
│  │  pkg/openagents_web_client_bg.wasm ← WGPUI client (~4MB)                ││
│  │                                                                          ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       │ downloaded to browser
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                BROWSER                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      CLIENT WASM (WGPUI)                                 ││
│  │                                                                          ││
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐              ││
│  │  │   Scene      │    │   Text       │    │   Renderer   │              ││
│  │  │   Graph      │───▶│   System     │───▶│   (wgpu)     │              ││
│  │  │              │    │              │    │              │              ││
│  │  │  Quads       │    │  cosmic-text │    │  WebGPU or   │              ││
│  │  │  Text runs   │    │  Glyph atlas │    │  WebGL2      │              ││
│  │  └──────────────┘    └──────────────┘    └──────────────┘              ││
│  │                                                    │                     ││
│  │                                                    ▼                     ││
│  │                                          ┌──────────────┐               ││
│  │                                          │   <canvas>   │               ││
│  │                                          │   GPU output │               ││
│  │                                          └──────────────┘               ││
│  │                                                                          ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Worker Architecture

### Entry Point (worker/src/lib.rs)

The worker uses the `#[event(fetch)]` macro from workers-rs:

```rust
#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    match (method, path.as_ref()) {
        // Auth
        (Method::Get, "/api/auth/github/start") => routes::auth::github_start(req, env).await,
        (Method::Get, "/api/auth/github/callback") => routes::auth::github_callback(req, env).await,

        // Protected routes
        (Method::Get, "/api/account") => {
            with_auth(&req, &env, |user| routes::account::get_settings(user, env.clone())).await
        }

        // HUD (shareable paths)
        (Method::Get, path) if path.starts_with("/hud/") || path.starts_with("/repo/") => {
            routes::hud::view_hud(env, parse_hud_path(path)).await
        }

        // Static assets handled by Cloudflare
        _ => Response::error("Not Found", 404),
    }
}
```

### Authentication Flow

```
┌─────────┐     ┌──────────────────┐     ┌────────────────┐     ┌─────────┐
│ Browser │     │ Worker           │     │ GitHub         │     │ D1/KV   │
└────┬────┘     └────────┬─────────┘     └───────┬────────┘     └────┬────┘
     │                   │                       │                    │
     │  GET /api/auth/   │                       │                    │
     │  github/start     │                       │                    │
     │──────────────────▶│                       │                    │
     │                   │                       │                    │
     │                   │  Store state in KV    │                    │
     │                   │───────────────────────│───────────────────▶│
     │                   │                       │                    │
     │  302 Redirect     │                       │                    │
     │  to GitHub OAuth  │                       │                    │
     │◀──────────────────│                       │                    │
     │                   │                       │                    │
     │  Authorize app    │                       │                    │
     │──────────────────────────────────────────▶│                    │
     │                   │                       │                    │
     │  Redirect with    │                       │                    │
     │  code + state     │                       │                    │
     │◀──────────────────────────────────────────│                    │
     │                   │                       │                    │
     │  GET /api/auth/   │                       │                    │
     │  github/callback  │                       │                    │
     │──────────────────▶│                       │                    │
     │                   │                       │                    │
     │                   │  Verify state in KV   │                    │
     │                   │───────────────────────│───────────────────▶│
     │                   │                       │                    │
     │                   │  Exchange code        │                    │
     │                   │  for token            │                    │
     │                   │──────────────────────▶│                    │
     │                   │                       │                    │
     │                   │  Access token         │                    │
     │                   │◀──────────────────────│                    │
     │                   │                       │                    │
     │                   │  Get user info        │                    │
     │                   │──────────────────────▶│                    │
     │                   │                       │                    │
     │                   │  Upsert user in D1    │                    │
     │                   │───────────────────────│───────────────────▶│
     │                   │                       │                    │
     │                   │  Create session in KV │                    │
     │                   │───────────────────────│───────────────────▶│
     │                   │                       │                    │
     │  302 Redirect     │                       │                    │
     │  + Set-Cookie     │                       │                    │
     │◀──────────────────│                       │                    │
     │                   │                       │                    │
```

### Session Management

Sessions are stored in Cloudflare KV with 30-day TTL:

```rust
// worker/src/db/sessions.rs

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub github_username: String,
    pub github_oauth_state: Option<String>,
    pub created_at: String,
    pub last_active_at: String,
}

impl Session {
    pub async fn create(kv: &KvStore, user_id: &str, username: &str) -> Result<String> {
        let token = generate_secure_token(); // 32 bytes, base64
        let session = Session { /* ... */ };

        kv.put(&format!("session:{}", token), serde_json::to_string(&session)?)
            .expiration_ttl(30 * 24 * 60 * 60) // 30 days
            .execute()
            .await?;

        Ok(token)
    }
}
```

Cookie format:
```
oa_session=<token>; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000; Secure
```

### Wallet Flow (Spark)

```
Client (WGPUI) ──▶ /api/wallet/summary|send|receive
        │
        ▼
Worker routes/wallet.rs
  ├─ load identity material (D1 + session secret)
  ├─ derive SparkSigner from seed entropy
  ├─ build SparkWallet (openagents-spark + Breez SDK)
  └─ return balance, addresses, payments, or send/receive results
```

Environment inputs:
- `SPARK_NETWORK` (mainnet/testnet/signet/regtest)
- `BREEZ_API_KEY` (or `SPARK_API_KEY`) for Breez SDK access

### Database Schema (D1)

D1 is SQLite at the edge. Key differences from PostgreSQL:

| PostgreSQL | D1 (SQLite) |
|------------|-------------|
| `UUID` | `TEXT` (store as string) |
| `TIMESTAMPTZ` | `TEXT` (ISO8601) |
| `BOOLEAN` | `INTEGER` (0/1) |
| `JSONB` | `TEXT` (JSON string) |
| `gen_random_uuid()` | Generate in Rust |
| `now()` | `datetime('now')` |

Schema overview:

```sql
-- Users
CREATE TABLE users (
    user_id TEXT PRIMARY KEY,
    github_id TEXT UNIQUE,
    github_username TEXT,
    nostr_public_key TEXT,
    nostr_npub TEXT,
    nostr_private_key_encrypted TEXT,
    bitcoin_xpriv_encrypted TEXT,
    email TEXT,
    signup_credits INTEGER DEFAULT 100000,
    purchased_credits INTEGER DEFAULT 0,
    credits_balance INTEGER DEFAULT 100000,
    payment_method_status TEXT DEFAULT 'none',
    created_at TEXT DEFAULT (datetime('now'))
);

-- Stripe integration
CREATE TABLE stripe_customers (
    user_id TEXT PRIMARY KEY REFERENCES users(user_id),
    stripe_customer_id TEXT UNIQUE
);

CREATE TABLE stripe_payment_methods (
    stripe_payment_method_id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(user_id),
    brand TEXT,
    last4 TEXT,
    is_default INTEGER DEFAULT 0
);

-- HUD settings (GTM)
CREATE TABLE hud_settings (
    user_id TEXT REFERENCES users(user_id),
    repo TEXT,
    is_public INTEGER DEFAULT 1,
    embed_allowed INTEGER DEFAULT 1,
    PRIMARY KEY (user_id, repo)
);
```

### Stripe Integration

Payment flow:

```
┌─────────┐     ┌──────────────────┐     ┌────────────────┐
│ Browser │     │ Worker           │     │ Stripe         │
└────┬────┘     └────────┬─────────┘     └───────┬────────┘
     │                   │                       │
     │  POST /api/stripe/│                       │
     │  setup-intent     │                       │
     │──────────────────▶│                       │
     │                   │                       │
     │                   │  Create SetupIntent   │
     │                   │──────────────────────▶│
     │                   │                       │
     │                   │  client_secret        │
     │                   │◀──────────────────────│
     │                   │                       │
     │  { client_secret }│                       │
     │◀──────────────────│                       │
     │                   │                       │
     │  Stripe.js        │                       │
     │  confirmSetup()   │                       │
     │──────────────────────────────────────────▶│
     │                   │                       │
     │                   │  Webhook: setup_      │
     │                   │  intent.succeeded     │
     │                   │◀──────────────────────│
     │                   │                       │
     │                   │  Save payment method  │
     │                   │  to D1                │
     │                   │                       │
```

Webhook signature verification:

```rust
pub fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
    // Parse t=timestamp,v1=signature
    let (timestamp, v1_sig) = parse_signature(signature);

    // Compute expected: HMAC-SHA256(timestamp.payload, secret)
    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    expected == v1_sig
}
```

---

## Client Architecture

### Client-Side View Routing

The client is a single-page app that runs entirely on `/`. Views are managed via `AppView` enum:

```rust
enum AppView {
    Landing,      // Login screen (not authenticated)
    RepoSelector, // Repository picker (authenticated)
    RepoView,     // Main app shell with sidebars (repo selected)
}
```

**View transitions:**
```
Landing ──[OAuth success]──▶ RepoSelector ──[click repo]──▶ RepoView
                                  │                             │
                                  └─────[logout]────────────────┘
                                            │
                                            ▼
                                        Landing
```

No URL changes occur during view transitions - all state is client-side.

### App Shell Structure (RepoView)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  LEFT DOCK (280px)  │        CENTER PANE         │  RIGHT DOCK (300px)   │
│  - Model selector   │        - Repo name         │  - Full Auto toggle   │
│  - Sessions list    │        - ThreadView        │  - Usage stats        │
│  - Hotkey legend    │          (future)          │  - Token counts       │
├─────────────────────┴────────────────────────────┴───────────────────────┤
│                            STATUS BAR (28px)                              │
│  - Dock toggle hints                                      - Repo path    │
└──────────────────────────────────────────────────────────────────────────┘
```

Dock visibility controlled by keyboard shortcuts:
- `cmd-[` → `left_dock_open`
- `cmd-]` → `right_dock_open`
- `cmd-\` → Toggle both
- `cmd-a` → `full_auto_enabled`

### WGPUI Rendering Pipeline

```
User Input → AppState → Scene → Renderer → GPU → Canvas

1. Event handlers update AppState
   - Keyboard: dock toggles, Full Auto
   - Mouse: track position, hover detection
   - Click: view transitions, toggles
   - Wheel: scroll offset

2. Each frame (60fps):
   a. Match current view (Landing/RepoSelector/RepoView)
   b. Build Scene:
      - draw_quad() for backgrounds, panels, buttons
      - draw_text() for text runs
   c. Renderer.prepare() converts to GPU buffers
   d. Renderer.render() issues draw calls
   e. surface.present() swaps buffers
```

### Text System

```
"Hello" → TextSystem → TextRun → Scene → GPU

TextSystem (cosmic-text):
├── FontDB (embedded Vera Mono)
├── Shaper (rustybuzz)
├── GlyphCache (atlas)
└── Layout engine

TextRun contains:
├── Positioned glyphs
├── Atlas UV coordinates
├── Color
└── Underline/highlight info
```

### Memory Layout

```
┌────────────────────────────────────────────┐
│           WASM Linear Memory               │
├────────────────────────────────────────────┤
│  Stack (grows down)                        │
│    • Local variables                       │
│    • Call frames                           │
├────────────────────────────────────────────┤
│  Heap (grows up)                           │
│    • Scene primitives                      │
│    • Text layouts                          │
│    • Streaming markdown state              │
├────────────────────────────────────────────┤
│  Static Data                               │
│    • Embedded fonts (~800KB)               │
│    • Syntax themes                         │
└────────────────────────────────────────────┘

┌────────────────────────────────────────────┐
│              GPU Memory                    │
├────────────────────────────────────────────┤
│  Textures                                  │
│    • Glyph atlas (2048x2048 R8)           │
├────────────────────────────────────────────┤
│  Buffers                                   │
│    • Vertex buffer (quads, glyphs)        │
│    • Index buffer                          │
│    • Uniform buffer (viewport)            │
├────────────────────────────────────────────┤
│  Pipelines                                 │
│    • Quad pipeline (SDF corners)          │
│    • Text pipeline (alpha blend)          │
└────────────────────────────────────────────┘
```

### Coordinate System

```
Physical Pixels (Canvas buffer)
┌─────────────────────────────────────┐
│ (0,0)                               │
│   ┌───────────────────────────┐     │
│   │ Logical Pixels            │     │
│   │ (CSS units)               │     │
│   │                           │     │
│   │   UI coordinates          │     │
│   │                           │     │
│   └───────────────────────────┘     │
│                    (width * dpr,    │
│                     height * dpr)   │
└─────────────────────────────────────┘

scale_factor = window.devicePixelRatio
Physical = Logical × scale_factor
```

---

## Data Flow

### Request Lifecycle

```
1. Browser sends request to nearest Cloudflare edge

2. Worker receives request:
   a. Parse URL and method
   b. Route to handler
   c. For protected routes: validate session from cookie
      - Extract token from oa_session cookie
      - Look up in KV
      - Check expiry, update last_active_at

3. Handler executes:
   a. Query D1 database
   b. Call external APIs (GitHub, Stripe)
   c. Build response

4. Response returned:
   a. JSON for API routes
   b. HTML for HUD routes (with context injected)
   c. Static assets from CDN cache

5. Browser processes response:
   a. For API: update client state
   b. For HUD: load WGPUI client, render
```

### HUD Page Flow

```
GET /hud/@username/repo (alias: /repo/:username/:repo)
        │
        ▼
┌───────────────────┐
│ Route Handler     │
│ routes/hud.rs     │
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Check visibility  │
│ D1: hud_settings  │
└────────┬──────────┘
         │
    ┌────┴────┐
    │ Public? │
    └────┬────┘
    Yes  │  No
    ▼    ▼
   OK   Check if
        owner
         │
    ┌────┴────┐
    │ Owner?  │
    └────┬────┘
    Yes  │  No
    ▼    ▼
   OK   403
         │
         ▼
┌───────────────────┐
│ Generate HTML     │
│ Inject context:   │
│ - username        │
│ - repo            │
│ - is_owner        │
│ - embed_mode      │
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Browser loads     │
│ WGPUI client      │
│ Renders HUD       │
└───────────────────┘
```

---

## External Services

### GitHub API

Used for:
- OAuth authentication
- User info (login, email, avatar)
- Repository list (future)

Endpoints:
```
POST https://github.com/login/oauth/access_token
GET  https://api.github.com/user
GET  https://api.github.com/user/emails
GET  https://api.github.com/user/repos
```

### Stripe API

Used for:
- Customer management
- Payment method storage
- Webhook events

Endpoints:
```
POST https://api.stripe.com/v1/customers
POST https://api.stripe.com/v1/setup_intents
GET  https://api.stripe.com/v1/payment_methods/:id
POST https://api.stripe.com/v1/payment_intents
```

Webhooks handled:
- `setup_intent.succeeded` → Save payment method
- `payment_intent.succeeded` → Add credits

---

## Dependencies

### Worker Dependencies

```toml
[dependencies]
worker = { version = "0.4", features = ["http", "d1"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde", "js"] }
chrono = { version = "0.4", features = ["serde", "wasmbind", "clock"] }
hmac = "0.12"
sha2 = "0.10"
base64 = "0.22"
urlencoding = "2"
```

### Client Dependencies

```toml
[dependencies]
wgpui = { path = "../../wgpui" }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [...] }
console_error_panic_hook = "0.1"
```

---

## Security Considerations

### Session Security

- Tokens: 32 bytes, cryptographically random
- Storage: KV with TTL (auto-expiry)
- Cookies: HttpOnly, Secure, SameSite=Lax
- No session fixation (new token on login)

### OAuth Security

- State parameter: Random UUID, 10-minute expiry
- PKCE: Not used (server-side flow)
- Token storage: Encrypted in D1 (production)

### Credential Encryption

- A Nostr/Bitcoin identity is generated at GitHub OAuth and stored as encrypted key material.
- Credential fields (GitHub tokens, API keys) are encrypted with a key derived from the user's identity.
- Identity private keys are encrypted with `SESSION_SECRET` before being stored in D1.

### Stripe Security

- Webhook signatures verified with HMAC-SHA256
- No card data stored (Stripe handles)
- Customer IDs only stored locally

### API Security

- All `/api/*` routes require authentication
- Rate limiting: Cloudflare's built-in
- CORS: Handled by worker headers

---

## Performance Characteristics

### Worker

- Cold start: ~5ms (WASM is pre-compiled)
- D1 query: ~1-5ms (edge-local)
- KV read: ~1ms
- GitHub API: ~100-300ms (external)
- Stripe API: ~200-500ms (external)

### Client

- WASM load: ~500ms (4MB compressed)
- First frame: ~100ms after load
- Frame time: ~16ms (60fps)
- Text layout: ~1ms per paragraph

### Caching

- Static assets: 1 year immutable
- WASM: Streaming compilation while downloading
- D1: Edge-local, sub-millisecond reads
- KV: Global replication, eventual consistency
