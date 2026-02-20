# Inbox Autopilot (macOS + Local Daemon)

Inbox Autopilot is a macOS-native inbox operations product for teams that handle high-volume, repetitive email workflows. It ingests Gmail threads, learns reply style from historical sent mail, classifies inbound requests, creates draft replies, and keeps a human in control of what gets sent.

## What We Are Building

### Core outcome

- Cut response time for repeatable inbox work without sacrificing quality.
- Produce drafts that read like the operator wrote them.
- Preserve human approval for risky or sensitive messages.
- Keep a clear audit trail for every decision and send action.

### Product principles

- Local-first: corpus, classification, policy, and drafting run locally by default.
- Human-in-the-loop: MVP is draft + approval, not autonomous auto-send.
- Explainability: every meaningful action emits events and can be audited.
- Practical quality: optimize first for common categories (scheduling, report delivery).

## Implementation Status (as of February 20, 2026)

Implemented:

- Native macOS app (SwiftUI) with Inbox, Approvals, Settings, and Audit sections.
- Local Rust daemon with authenticated loopback IPC and SSE event stream.
- Gmail OAuth flow (app gets code, daemon exchanges/stores tokens).
- Optional ChatGPT/OpenAI API key connection for hybrid/cloud rewrite mode.
- Backfill + incremental sync from Gmail into local SQLite.
- Thread classification (category + risk + policy) and draft generation pipeline.
- Approve-and-send via Gmail API with policy enforcement.
- Event timeline, per-thread audit view, and JSON audit export.
- Draft-quality evaluation API and UI (minimal-edit scoring for scheduling/report-delivery).
- Settings for privacy mode, sync cadence, templates/signature, allowlist, retention controls.
- Local notifications, menu bar status controls, and optional app unlock.
- In-app update checks with release download/release-notes links (Sparkle-equivalent update path).

Open items:

- Roadmap deliverables are complete; remaining work is ongoing model/policy quality tuning and ops hardening.

## Product Spec

### 1) User workflow

1. Connect Gmail in onboarding.
2. Optionally connect OpenAI key for hybrid/cloud drafting.
3. Choose privacy mode and backfill range.
4. Run backfill (default 90 days) and load Inbox.
5. Open a thread, generate/review draft, and approve send when safe.
6. Review "why this draft" details and full event timeline in Audit.

### 2) Functional requirements

- FR-1 Gmail must be required for production use.
- FR-2 Backfill must ingest enough history to infer style and common templates.
- FR-3 New inbound threads should be classified into category + risk + policy.
- FR-4 Draft generation must combine local retrieval/style signals with optional external rewrite.
- FR-5 Send path must be policy-gated and approval-gated.
- FR-6 Every step (sync, classify, policy, draft, approval, send, settings changes) must emit events.
- FR-7 Operators must be able to delete local corpus or run full factory reset.

### 3) Quality and policy targets

- Draft quality target: for scheduling/report-delivery categories, drafts should need minimal edits at least 60% of the time.
- Timing target: draft available within 60 seconds after sync for new inbound mail.
- Safety baseline:
  - Legal/insurance content defaults to blocked one-click send.
  - Complaint/dispute and pricing stay in draft-first workflows.
  - Allowlist policy can block send based on configured domains.

### 4) Privacy modes

- `local_only`: no external LLM calls.
- `hybrid`: local retrieval/composition plus optional OpenAI rewrite.
- `cloud`: cloud rewrite path enabled, still policy-gated and audit-logged.

## Architecture

### macOS app (Swift)

Responsibilities:

- Onboarding and auth initiation.
- Inbox/thread/approvals/settings/audit UI.
- Session bootstrap with daemon and nonce-authenticated requests.
- Event-stream subscription for live status updates.
- Local UX integrations (notifications, menu bar, optional unlock).

Important boundary:

- App never stores Gmail refresh token directly.
- App forwards OAuth code to daemon for exchange.

### Local daemon (Rust)

Responsibilities:

- OAuth exchange/refresh and Gmail API calls.
- Sync pipeline (backfill + periodic incremental).
- Local persistence and encryption.
- Classification, policy evaluation, and draft generation.
- Approve/send execution and event logging.

Current transport:

- HTTP on `127.0.0.1:8787` (local-only).
- SSE endpoint for live events.

## IPC Contract Summary

Canonical contract: `docs/ipc-contract.md`

### Session/auth

- `POST /session`
- Required headers on authenticated routes:
  - `x-session-token`
  - `x-nonce` (single-use replay protection)

### Main route groups

- Health: `/health`
- Auth: `/auth/gmail/*`, `/auth/chatgpt/*`
- Sync: `/sync/backfill`, `/sync/now`
- Threads/drafts: `/threads/*`, `/drafts/*`
- Audit/events: `/events`, `/events/stream`, `/threads/:id/audit`, `/threads/:id/export-audit`
- Quality: `/quality/draft-edit-rate`
- Settings/lifecycle: `/settings`, `/settings/delete-corpus`, `/settings/factory-reset`

### Event types emitted

- `daemon_started`
- `auth_gmail_connected`
- `auth_chatgpt_connected`
- `sync_backfill_completed`
- `sync_incremental_completed`
- `sync_incremental_failed`
- `classification_completed`
- `policy_evaluated`
- `draft_created`
- `draft_marked_needs_human`
- `approval_granted`
- `email_sent`
- `settings_updated`
- `local_corpus_deleted`
- `factory_reset_completed`
- `draft_quality_evaluated`

## Data and Security

Canonical schema: `docs/db-schema.md`

- SQLite DB at `~/.inbox-autopilot/daemon.sqlite` by default.
- OAuth secrets, message bodies, and draft bodies are encrypted before persistence.
- Daemon vault uses AES-256-GCM with per-device master key material.
- Master key is stored in macOS Keychain when available (fallback local key file).
- App session token is stored in Keychain.

## Repo Layout

- `Inbox Autopilot/Inbox Autopilot/`: macOS app source (SwiftUI).
- `daemon/`: local service (Rust).
- `docs/ipc-contract.md`: app-daemon protocol contract.
- `docs/db-schema.md`: persistent storage schema.
- `ROADMAP.md`: phased plan and implementation status.

## Local Development

### Prerequisites

- macOS with Xcode (for app build/run).
- Rust toolchain (for daemon).
- Google OAuth app credentials for Gmail integration.

### Run daemon

```bash
cd daemon
cargo run
```

Default bind: `127.0.0.1:8787`

### Required daemon env

```bash
export GOOGLE_OAUTH_CLIENT_ID=...
export GOOGLE_OAUTH_CLIENT_SECRET=...
```

### Optional daemon env

```bash
export INBOX_AUTOPILOT_BIND_ADDR=127.0.0.1:8787
export INBOX_AUTOPILOT_DATA_DIR=$HOME/.inbox-autopilot
export INBOX_AUTOPILOT_SESSION_TTL_SECONDS=300
export GOOGLE_OAUTH_SCOPES="https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.compose https://www.googleapis.com/auth/gmail.send"
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL=gpt-4o-mini
```

### Run macOS app

- Open `Inbox Autopilot/Inbox Autopilot.xcodeproj` in Xcode.
- Build and run the `Inbox Autopilot` scheme.
- Ensure custom URL callback (`inboxautopilot://oauth/callback`) is configured in the app target.

## Verification Entry Points

Daemon:

```bash
cd daemon
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

App:

- Run Xcode unit/UI tests for the `Inbox Autopilot` schemes.
- Validate OAuth connect, backfill, draft generation, approve-send, and audit export on a real Gmail account.

## Near-Term Focus

1. Add a robust auto-update channel for production distribution.
2. Add an evaluation harness for draft quality by category.
3. Improve classification/policy from keyword heuristics to stronger learned signals.
4. Add clearer operational telemetry around sync latency and draft turnaround.

## Related Docs

- `AGENTS.md`
- `ROADMAP.md`
- `docs/ipc-contract.md`
- `docs/db-schema.md`
- `daemon/README.md`
