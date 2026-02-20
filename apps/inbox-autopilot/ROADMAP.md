# Inbox Autopilot — Detailed Roadmap

This roadmap orders what to build and in what sequence so we hit the MVP from [README.md](./README.md): Mac-native app + local daemon, Gmail sync, classification, draft generation, approve & send, and audit. Each phase builds on the previous; dependencies are called out so nothing is started before its prerequisites exist.

---

## Principles

- **Local-first:** Daemon owns data and workflows; app is UI + auth + user actions. Message corpus stays on device by default.
- **Auth before data:** Gmail connect (and ChatGPT for hybrid/cloud) must work before we backfill or show real threads.
- **Vertical slices:** Prefer “one flow end-to-end” (e.g. backfill → thread list → open thread) over “all backend then all UI.”
- **Audit from day one:** Every daemon action emits events; UI for “why this draft?” can follow once drafts exist.

## Implementation status (updated February 19, 2026)

- End-to-end local daemon + macOS app flow is implemented through Phase 4, including Gmail auth, sync, classification, drafting, approvals, audit timeline, template mining, domain allowlist, retention controls, notifications, menu bar status, keyboard shortcuts, and encryption-at-rest envelope handling.
- Remaining open item: production-grade app auto-update pipeline (Sparkle or equivalent distribution wiring).

---

## Phase 0 — Foundations (Daemon + App Shell + IPC)

**Goal:** Daemon runs, exposes a local API and event stream, and persists minimal state. App shell can talk to the daemon and show login scaffolding. No real Gmail data yet.

### 0.1 Daemon skeleton (Rust)

- [x] **Project layout** — Codex/Rust crate; binary that runs as a local service (no GUI).
- [x] **Local API** — HTTP on `127.0.0.1` (or Unix socket) with a small set of endpoints, e.g.:
  - `GET /health` (or equivalent) for liveness.
  - Placeholder routes for later: `/auth/gmail`, `/auth/chatgpt`, `/sync/backfill`, `/threads`, `/threads/:id`, `/drafts`, `/events` (or SSE endpoint).
- [x] **Event stream** — SSE (or WebSocket) endpoint for timeline/events so the app can subscribe to “new event” updates.
- [x] **Local storage** — SQLite DB created and migrated; minimal schema to start (e.g. `events` table only, or `threads`/`messages` empty). No encryption required in Phase 0.
- [x] **App–daemon auth** — Daemon issues or accepts a short-lived session token; app sends token + nonce on each request (replay protection). Document the IPC auth contract.

**Deliverable:** Daemon starts, responds to health and placeholder routes, app can connect and subscribe to events.

### 0.2 App shell (Swift)

- [x] **Project layout** — macOS app (SwiftUI + AppKit where needed); target macOS version per README.
- [x] **Navigation scaffolding** — Tab or sidebar: placeholders for Inbox, Thread, Approvals, Settings, Audit (screens can be empty or “Coming soon”).
- [x] **Daemon connectivity** — App discovers daemon (e.g. fixed port or config file) and performs IPC auth; show connection status (connected / disconnected) in UI.
- [x] **Login scaffolding** — Two entry points (no real auth yet):
  - “Connect Gmail” — button/screen that will later trigger Google OAuth via `ASWebAuthenticationSession`.
  - “Connect ChatGPT” — button/screen or Settings entry for ChatGPT/OpenAI (OAuth or API key) for hybrid/cloud mode.
- [x] **Secrets** — Keychain usage for any stored tokens (even placeholders) so the pattern is in place.

**Deliverable:** App launches, shows nav and login placeholders, and can connect to the daemon with a clear “no Gmail” / “no ChatGPT (optional)” state.

### 0.3 IPC contract (documentation)

- [x] **API contract** — Document endpoints, request/response shapes, and SSE event types (even if only a subset are implemented). This becomes the single source of truth for app–daemon communication.
- [x] **Auth contract** — How app gets a session token, how it sends token + nonce, how daemon validates.

**Deliverable:** IPC API contract doc (or OpenAPI/spec) that both sides implement against.

---

## Phase 1 — Auth + First Data (Gmail Connect, Backfill, Inbox)

**Goal:** User can connect Gmail (and optionally ChatGPT for hybrid/cloud). Daemon performs token exchange and stores credentials securely, then runs backfill (e.g. 90 days). App shows a real, searchable thread list (Inbox).

### 1.1 Gmail OAuth + token exchange

- [x] **Who does exchange** — Per README: app gets auth code via `ASWebAuthenticationSession`; **daemon** exchanges code → tokens and stores refresh token. App never holds long-lived Gmail refresh tokens.
- [x] **App** — Google OAuth with scopes: read threads/messages, create drafts; optionally send (can add at “Approve & Send” time). Redirect/callback to app (e.g. custom URL scheme or loopback); app forwards **code** to daemon.
- [x] **Daemon** — Endpoint `POST /auth/gmail` (or similar) accepting `code` + `redirect_uri`. Daemon has Google OAuth client id/secret in config (or env). Exchange code for access + refresh token; store refresh token in encrypted local vault (or Keychain-backed store). Implement token refresh and use access token for Gmail API calls.
- [x] **Storage** — Refresh token encrypted at rest; daemon can read it to refresh and call Gmail API.

**Deliverable:** User can “Connect Gmail”; daemon owns and stores tokens and can call Gmail API (e.g. list threads) on behalf of the user.

### 1.2 ChatGPT auth (optional; for Hybrid / Cloud)

- [x] **App** — “Connect ChatGPT” in onboarding or Settings. MVP: user pastes OpenAI API key; app sends to daemon. Later: OAuth “Sign in with ChatGPT” if available.
- [x] **Daemon** — Endpoint to receive and store ChatGPT/OpenAI credential (API key or token). Store in encrypted vault. Use only when privacy mode is Hybrid or Cloud for draft phrasing.
- [x] **Storage** — Credential encrypted at rest; daemon uses it only for external LLM calls when mode allows.

**Deliverable:** User can connect ChatGPT (API key or OAuth); daemon can call OpenAI (or configured provider) when in hybrid/cloud mode.

### 1.3 Backfill + local storage

- [x] **Daemon** — Gmail sync module:
  - Backfill: fetch threads/messages for last N days (90 default; 12 months optional). Use Gmail API (threads.list, messages.get, etc.).
  - Persist into local SQLite: threads, messages, and any fields needed for list/detail views (ids, subject, snippet, date, labels, etc.). Optional: encrypted blob store for full bodies; MVP can store in DB or skip.
- [x] **Schema** — Tables for `threads`, `messages`, and later `drafts`, `events`. Add FTS or indexed columns for search (e.g. subject, from, snippet).
- [x] **Sync controls** — Daemon exposes “start backfill” (and optionally “sync now”) so app or a scheduler can trigger. MVP can poll on interval for new mail; Gmail “watch” can follow later.

**Deliverable:** Daemon can backfill 90 days of Gmail into local DB; data is queryable.

### 1.4 Inbox UI (thread list)

- [x] **Daemon** — `GET /threads` (or equivalent) with pagination/filters: list threads from local DB (e.g. “Needs reply” or “all”); support search (client name, subject, keywords).
- [x] **App** — Inbox screen:
  - Thread list with category + risk chips (category/risk can be “—” or “unknown” until Phase 2).
  - Search box that calls daemon search.
  - Tapping a thread opens Thread view (can be read-only in Phase 1).
- [x] **Onboarding** — After Gmail connect (and optional ChatGPT for hybrid/cloud), trigger backfill (or show “Backfill 90 days” / “Backfill 12 months”); show progress and then land on Inbox when done.

**Deliverable:** User sees a searchable Inbox of real threads; can open a thread (read-only for now). Acceptance: “App backfills 90 days and shows a searchable Inbox.”

---

## Phase 2 — Classification + Drafts (Thread View, Draft Panel, “Why this draft?”)

**Goal:** For each thread (or new message), daemon classifies category and risk, retrieves similar past threads, and generates a draft. App shows thread view with suggested draft, “Insert Draft,” “Edit,” and the start of “Why this draft?” (audit).

### 2.1 Classification + risk

- [x] **Daemon** — Classifier (v1):
  - Categories: scheduling, report delivery, findings clarification, pricing, complaint/dispute, legal/insurance, other.
  - Risk tiers: low / medium / high.
  - Input: thread (and optionally message) content; output: category + risk + reason codes. Implementation can be rule-based first, then retrieval or small local model if needed.
- [x] **Storage** — Persist classification result on thread (or message); emit event (e.g. `classification_completed`).
- [x] **Policy gating** — Default policy `draft_only`. Evaluate policy after classification (e.g. legal/insurance → hard block for one-click send; unknown pricing → require template). Emit policy decision event.

**Deliverable:** Every synced thread gets a category and risk; policy decision is stored and emitted.

### 2.2 Retrieval + draft generation

- [x] **Daemon** — Draft pipeline (v1):
  1. Retrieve similar past threads + sent replies (from local DB or index).
  2. Extract style signals (tone, formatting, sign-off) from user’s sent mail.
  3. Compose draft: templates + retrieval snippets; optional external LLM for phrasing (hybrid mode). Respect privacy mode (local-only / hybrid / cloud).
- [x] **Storage** — Store draft(s) per thread; link to approval state (pending / approved / rejected). Emit `draft_created` event with params/output hashes.
- [x] **Daemon API** — Endpoints to trigger draft generation for a thread and to fetch “draft for thread X.” Optionally “similar threads” and “sources used” for audit.

**Deliverable:** Daemon produces a draft for scheduling and report-delivery threads (MVP categories); draft is stored and associated with thread. Acceptance: “For new inbound emails, a draft appears in <60 seconds after sync” and “Drafts for scheduling/report delivery require minimal editing at least 60% of the time (initial target).”

### 2.3 Thread view + draft panel

- [x] **App** — Thread view:
  - Conversation timeline (messages in thread).
  - Suggested draft panel: show draft body; buttons: “Insert Draft,” “Edit,” “Approve & Send,” “Mark Needs Human.”
  - “Insert Draft” puts draft into compose area (or inline); “Edit” opens editor; “Approve & Send” and “Mark Needs Human” wired in Phase 3.
- [x] **“Why this draft?” drawer** — Panel or sheet showing:
  - Category, risk tier, policy decision.
  - Similar past threads used (links or IDs).
  - Whether external model was used (for hybrid/cloud).
- [x] **Daemon** — Expose “audit info” for a draft (category, risk, policy, similar threads, model usage) so the app can render “Why this draft?”

**Deliverable:** User opens a thread, sees a suggested draft and can insert/edit; “Why this draft?” shows category, risk, policy, and sources. Acceptance: “Audit shows category/risk and sources used.”

---

## Phase 3 — Approve & Send + Audit Timeline

**Goal:** User can approve a draft and send via Gmail. Daemon records the send and emits events. App shows approvals queue and a full audit timeline per thread/run.

### 3.1 Approve & Send

- [x] **Daemon** — Send flow:
  - Endpoint to “approve and send” draft for thread X (with optional user confirmation token from app).
  - Daemon uses stored Gmail tokens to send (Gmail API: send message or insert draft then send). Record Gmail message id.
  - Emit events: `approval_granted`, `email_sent` (with message id). Policy: only send when policy allows and user approved (no auto-send in MVP except future `auto_send_low_risk`).
- [x] **App** — “Approve & Send” in thread view: confirm dialog, then call daemon; on success, update UI (e.g. “Sent”) and optionally refresh thread.
- [x] **Hard blocks** — Legal threats / unknown pricing: never one-click send unless explicit override (e.g. extra confirmation or admin flag). Enforce in daemon.

**Deliverable:** User can approve a draft and send; email is sent via Gmail; daemon logs message id and events. Acceptance: “Approve & Send sends via Gmail and logs message id.”

### 3.2 Approvals queue

- [x] **Daemon** — List drafts awaiting approval (e.g. `GET /drafts?status=pending` or similar).
- [x] **App** — Approvals queue screen: list all drafts pending approval; tap to open thread and approve/reject from there (or approve from list if UX is defined).

**Deliverable:** Single place to see all drafts waiting for approval.

### 3.3 Audit timeline UI

- [x] **Daemon** — Event log: append-only events per thread (and optionally per run). Event types: ingest, classification, policy, draft_created, approval_granted/denied, email_sent. Expose `GET /threads/:id/events` or `GET /events?thread_id=...`.
- [x] **App** — Audit screen:
  - Per-thread (or per-run) event timeline: what happened and when (ingest → classification → policy → draft → approval → send).
  - Export logs (e.g. for support): export events as file or copy.

**Deliverable:** User can see a full audit timeline for a thread and export logs. Completes “Audit shows category/risk and sources used” and “why this draft?” story.

### 3.4 Hardening + polish

- [x] **Error handling** — Token expiry, Gmail API errors, network failures; clear messages in app and daemon logs.
- [x] **Sync robustness** — Incremental sync (only new mail after backfill); optional Gmail watch later. Poll interval configurable.
- [x] **Onboarding** — “Choose privacy mode” (Local-only / Hybrid / Cloud) and “Select backfill range” (90d / 12mo) during first run; persist in settings.
- [x] **Settings** — Privacy mode, model routing, backfill/sync controls; optional allowed recipient domains and signature/templates (can be minimal in MVP).

**Deliverable:** Stable MVP: login, Gmail, backfill, Inbox, classification, drafts, approve & send, and audit all working end-to-end. Meets README acceptance criteria 1–6.

---

## Phase 4 — Polish + NICE to Have

**Goal:** Premium feel and optional features from README “NICE to have.”

### 4.1 Native integrations

- [x] **Keychain** — All secrets (daemon session, Gmail handled by daemon; app may hold session; ChatGPT API key if stored in app) in Keychain; no plaintext on disk.
- [x] **Touch ID / Passcode** — Optional app unlock (e.g. on launch or when opening sensitive screens).
- [x] **Notifications** — “Draft ready” and “Needs human” when new draft appears or thread needs attention.
- [x] **Menu bar** — Indicator for sync status (idle / syncing / error).
- [x] **Keyboard** — Great keyboard navigation (shortcuts for Inbox, thread, approve, etc.).

### 4.2 Optional features

- [x] **Template mining UI** — “We found 12 common replies—approve these” to bootstrap templates from history.
- [x] **Domain allowlist** — Only generate drafts (or only send) for certain recipient domains; settings UI.
- [x] **Signature and templates** — User-defined signature and reply templates; used in draft generation.
- [x] **Data retention** — Settings: backfill duration, attachment storage (none / metadata / full), “Delete local corpus” + “Factory reset.”

### 4.3 Reliability and ops

- [ ] **Auto-updates** — Sparkle or equivalent so users get new versions without manual download.
- [x] **Encryption at rest** — Per-device master key in Keychain; DB encrypted (SQLCipher or envelope encryption) so local corpus is protected if device is lost.

**Deliverable:** Product feels premium and supports power users; optional features documented and configurable.

---

## Dependency summary

```
Phase 0 (daemon + app shell + IPC)
    ↓
Phase 1 (Gmail OAuth → ChatGPT auth (optional) → backfill → Inbox UI)
    ↓
Phase 2 (classification → retrieval → draft gen → thread view + draft panel + “why this draft?”)
    ↓
Phase 3 (approve & send → approvals queue → audit timeline → hardening)
    ↓
Phase 4 (Keychain/Touch ID, notifications, menu bar, templates, allowlist, encryption, auto-updates)
```

- **Phase 1** depends on Phase 0 (IPC, auth contract, storage).
- **Phase 2** depends on Phase 1 (real threads in DB, Gmail tokens for any future “send” test).
- **Phase 3** depends on Phase 2 (drafts exist, policy and events in place).
- **Phase 4** can be parallelized after Phase 3; individual items (e.g. Touch ID) can be picked up in any order.

---

## README acceptance criteria (checklist)

From README §9:

1. [x] **A user can install app, connect Gmail (and optionally ChatGPT for hybrid/cloud) in <10 minutes.** — Covered by Phase 0 (shell) + Phase 1 (Gmail + optional ChatGPT auth + onboarding).
2. [x] **App backfills 90 days and shows a searchable Inbox.** — Phase 1 (backfill + Inbox UI).
3. [x] **For new inbound emails, a draft appears in <60 seconds after sync.** — Phase 2 (sync + classification + draft pipeline).
4. [ ] **Drafts for scheduling/report delivery require minimal editing at least 60% of the time (initial target).** — Phase 2 (retrieval + style + templates).
5. [x] **“Approve & Send” sends via Gmail and logs message id.** — Phase 3 (approve & send + events).
6. [x] **Audit shows category/risk and sources used.** — Phase 2 (“Why this draft?”) + Phase 3 (audit timeline).

---

## Suggested build order (one-line view)

1. **Phase 0:** Daemon skeleton + app shell + IPC contract.
2. **Phase 1:** Gmail OAuth (daemon exchange) → ChatGPT auth (optional) → backfill → Inbox + search.
3. **Phase 2:** Classification + risk → retrieval + draft generation → thread view + draft panel + “Why this draft?”.
4. **Phase 3:** Approve & Send → approvals queue → audit timeline → hardening.
5. **Phase 4:** Keychain/Touch ID, notifications, menu bar, templates, allowlist, encryption, auto-updates.

This order keeps foundations first, then one vertical slice (auth → data → drafts → send → audit), then polish and NICE-to-have.
