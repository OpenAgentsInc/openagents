# OpenAgents Web

Full-stack Rust web application on Cloudflare's edge: GPU-accelerated WGPUI frontend + Axum API backend.

**Production URL:** https://openagents-web.openagents.workers.dev

## User Flow

```
┌────────────────────────────────────────────────────────────────────────────┐
│                         ALL ON ROOT URL (/)                                 │
│                                                                             │
│  ┌─────────────┐      ┌─────────────────┐      ┌─────────────────────────┐ │
│  │   Landing   │─────▶│  Repo Selector  │─────▶│       App Shell         │ │
│  │             │      │                 │      │                         │ │
│  │ "OpenAgents"│      │ "Select repo:"  │      │ ┌─────┬───────┬───────┐ │ │
│  │             │      │                 │      │ │Left │Center │Right  │ │ │
│  │ [Login with │      │ ┌─────────────┐ │      │ │Dock │ Pane  │Dock   │ │ │
│  │   GitHub]   │      │ │owner/repo   │ │      │ ├─────┴───────┴───────┤ │ │
│  │             │      │ └─────────────┘ │      │ │    Status Bar       │ │ │
│  └─────────────┘      └─────────────────┘      │ └─────────────────────┘ │ │
│        │                      │                └─────────────────────────┘ │
│        │ OAuth               │ click repo                                  │
│        ▼                      ▼                                             │
│   /api/auth/            Set context +                                      │
│   github/start          switch view                                        │
│                                                                             │
└────────────────────────────────────────────────────────────────────────────┘

Keyboard shortcuts (in App Shell):
  cmd-[     Toggle left dock
  cmd-]     Toggle right dock
  cmd-\     Toggle both docks
  cmd-a     Toggle Full Auto
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Cloudflare Global Edge                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Browser Request                                                            │
│         │                                                                    │
│         ▼                                                                    │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                        Cloudflare Worker                             │   │
│   │                     (Rust/WASM via workers-rs)                       │   │
│   │                                                                       │   │
│   │   ┌───────────────┐   ┌───────────────┐   ┌───────────────┐         │   │
│   │   │  API Routes   │   │   Sessions    │   │   Database    │         │   │
│   │   │               │   │     (KV)      │   │     (D1)      │         │   │
│   │   │               │   │               │   │               │         │   │
│   │   │ /api/auth/*   │   │ 30-day TTL    │   │ SQLite at     │         │   │
│   │   │ /api/repos    │   │ Session data  │   │ edge          │         │   │
│   │   │ /api/billing/*│   │ OAuth state   │   │               │         │   │
│   │   │ /api/stripe/* │   │               │   │ Users         │         │   │
│   │   └───────────────┘   └───────────────┘   │ Billing       │         │   │
│   │           │                   │           │ Stripe        │         │   │
│   │           └───────────────────┴───────────┤               │         │   │
│   │                                           └───────────────┘         │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                      │                                       │
│                                      │ serves                                │
│                                      ▼                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      Static Assets (CDN)                             │   │
│   │                                                                       │   │
│   │   index.html → WGPUI Client (WASM) → WebGPU/WebGL → <canvas>        │   │
│   │                                                                       │   │
│   │   Single-page app with client-side view routing:                     │   │
│   │   • Landing (login)                                                  │   │
│   │   • RepoSelector (pick repo)                                         │   │
│   │   • App Shell (autopilot UI with sidebars)                          │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

External Services:
┌──────────────┐     ┌──────────────┐
│   GitHub     │     │    Stripe    │
│   OAuth      │     │   Payments   │
│   API        │     │   Webhooks   │
└──────────────┘     └──────────────┘
```

## Quick Start

```bash
cd crates/web

# Install dependencies
bun install

# First-time setup (creates D1 database and KV namespace)
bun run setup
# Copy the IDs from output into wrangler.toml

# Set secrets
wrangler secret put GITHUB_CLIENT_SECRET
wrangler secret put STRIPE_SECRET_KEY
wrangler secret put STRIPE_WEBHOOK_SECRET

# Edit wrangler.toml with your GitHub/Stripe public keys
# [vars]
# GITHUB_CLIENT_ID = "your_client_id"
# STRIPE_PUBLISHABLE_KEY = "pk_..."

# Run migrations
bun run db:migrate

# Local development
bun run dev              # Client-only dev server on :3000
bun run dev:worker       # Full worker dev on :8787

# Deploy to production
bun run deploy
```

## Project Structure

```
crates/web/
├── client/                     # WGPUI Frontend (Client-side WASM)
│   ├── Cargo.toml              # wasm-bindgen, wgpui dependencies
│   └── src/
│       └── lib.rs              # Single-page app: Landing → RepoSelector → App Shell
│
├── worker/                     # Axum API (Server-side WASM)
│   ├── Cargo.toml              # workers-rs, serde, chrono
│   └── src/
│       ├── lib.rs              # Worker entry point & router
│       ├── db/
│       │   ├── mod.rs
│       │   ├── sessions.rs     # KV session management
│       │   └── users.rs        # D1 user operations
│       ├── middleware/
│       │   ├── mod.rs
│       │   └── auth.rs         # Session validation extractor
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── auth.rs         # GitHub OAuth flow
│       │   ├── account.rs      # User settings, API keys
│       │   ├── billing.rs      # Credits, plans, packages
│       │   ├── stripe.rs       # Payment methods, webhooks
│       │   └── hud.rs          # HUD page rendering
│       └── services/
│           ├── mod.rs
│           ├── github.rs       # GitHub API client (OAuth, repos)
│           └── stripe.rs       # Stripe API client
│
├── migrations/                 # D1 SQL migrations
│   └── 0001_initial.sql        # Users, billing, Stripe tables
│
├── static/                     # Static assets
├── pkg/                        # wasm-pack output (git-ignored)
├── dist/                       # Deployment bundle (git-ignored)
│
├── wrangler.toml               # Cloudflare Workers configuration
├── package.json                # Build scripts
├── build.ts                    # Dist builder
├── serve.ts                    # Local dev server
│
└── docs/
    ├── README.md               # This file
    ├── client-ui.md            # Client UI architecture & views
    ├── architecture.md         # Technical deep-dive
    └── deployment.md           # Deployment guide
```

## Key Commands

```bash
# Development
bun run dev              # Client-only local server (:3000)
bun run dev:worker       # Full worker with wrangler (:8787)

# Building
bun run build            # Full build (client + optimize + dist)
bun run build:client     # Client WASM only
bun run build:worker     # Worker WASM only

# Database
bun run db:create        # Create D1 database
bun run db:migrate       # Apply migrations locally
bun run db:migrate:prod  # Apply migrations in production

# Deployment
bun run deploy           # Build + deploy to Cloudflare
bun run deploy:preview   # Deploy to preview environment

# Monitoring
bun run cf:tail          # Live logs from production
```

## API Reference

### Authentication

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/auth/github/start` | Initiate GitHub OAuth flow |
| GET | `/api/auth/github/callback` | OAuth callback handler |
| GET | `/api/auth/me` | Get current user info |
| POST | `/api/auth/logout` | Clear session and logout |

### Repositories (requires auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/repos` | List user's GitHub repositories |

### Account (requires auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/account` | Get account settings |
| POST | `/api/account/api-key` | Generate new API key |
| POST | `/api/account/delete` | Soft delete account |

### Billing (requires auth)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/billing/balance` | Get credit balance |
| GET | `/api/billing/plans` | List subscription plans |
| GET | `/api/billing/credits` | List credit packages |
| POST | `/api/billing/credits/purchase` | Purchase credits |

### Stripe

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/stripe/config` | Get Stripe publishable key |
| GET | `/api/stripe/payment-methods` | List saved payment methods |
| POST | `/api/stripe/setup-intent` | Create SetupIntent for new card |
| POST | `/webhooks/stripe` | Stripe webhook handler |

### HUD (GTM Features)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/hud/:username/:repo` | View personal HUD |
| GET | `/embed/:username/:repo` | Embeddable iframe HUD |
| POST | `/api/hud/settings` | Update HUD visibility |

## Configuration

### Environment Variables

Set in `wrangler.toml` `[vars]`:

```toml
[vars]
GITHUB_CLIENT_ID = "Ov23li..."
STRIPE_PUBLISHABLE_KEY = "pk_live_..."
```

### Secrets

Set via CLI (never in code):

```bash
wrangler secret put GITHUB_CLIENT_SECRET
wrangler secret put STRIPE_SECRET_KEY
wrangler secret put STRIPE_WEBHOOK_SECRET
```

### D1 Database

Binding: `DB`
- Users, credits, payment status
- Stripe customers and payment methods
- Usage events and invoices
- HUD visibility settings

### KV Namespace

Binding: `SESSIONS`
- Session tokens with 30-day TTL
- OAuth state tokens (10-minute TTL)

## App Shell Layout

After logging in and selecting a repository, users see the main Autopilot interface:

```
┌────────────────────────────────────────────────────────────────────────────┐
│ LEFT DOCK (280px)    │      CENTER PANE         │ RIGHT DOCK (300px)       │
│                      │                          │                          │
│ Model: sonnet        │                          │ ○ FULL AUTO OFF          │
│ ───────────────────  │                          │                          │
│ Sessions             │      owner/repo          │ Claude Usage             │
│ ┌──────────────────┐ │                          │ ───────────────────────  │
│ │ Today 14:32      │ │  (ThreadView will go     │ Model: sonnet            │
│ └──────────────────┘ │   here)                  │ Context: ████░░░░░░ 10%  │
│                      │                          │                          │
│ Hotkeys              │                          │ Tokens:  0 / 0           │
│ cmd-[   left dock    │                          │ Cost:    $0.00           │
│ cmd-]   right dock   │                          │                          │
├──────────────────────┴──────────────────────────┴──────────────────────────┤
│ cmd-[ / cmd-] toggle docks                                  owner/repo     │
└────────────────────────────────────────────────────────────────────────────┘
                                STATUS BAR (28px)
```

**Keyboard shortcuts:**
- `cmd-[` Toggle left dock
- `cmd-]` Toggle right dock
- `cmd-\` Toggle both docks
- `cmd-a` Toggle Full Auto mode

See [client-ui.md](./client-ui.md) for detailed UI documentation.

## Database Schema

See `migrations/0001_initial.sql` for full schema. Key tables:

- **users** - Accounts, credits, payment status
- **stripe_customers** - Stripe customer ID mapping
- **stripe_payment_methods** - Saved cards
- **usage_events** - Credit consumption log
- **hud_settings** - Public/private visibility per repo

## Browser Requirements

| Feature | Required | Fallback |
|---------|----------|----------|
| WebAssembly | Yes | None |
| ES Modules | Yes | None |
| WebGPU | No | WebGL2 |
| WebGL2 | No | WebGL1 |

**Minimum browsers:** Chrome 80+, Firefox 80+, Safari 14+, Edge 80+

## Documentation

| Document | Description |
|----------|-------------|
| [client-ui.md](./client-ui.md) | Client UI views, app shell, keyboard shortcuts |
| [architecture.md](./architecture.md) | Technical architecture, data flows, WASM details |
| [deployment.md](./deployment.md) | Build, optimize, deploy to Cloudflare |

## Related

- [WGPUI Crate](../../wgpui/) - GPU-accelerated UI library
- [Autopilot Spec](../../../docs/autopilot/PROJECT-SPEC.md) - Product roadmap
