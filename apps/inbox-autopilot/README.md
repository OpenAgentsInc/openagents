# Inbox Autopilot (macOS Native) — MVP Plan

## 1) Product Goal

A Mac-native “Inbox Autopilot” that:

* syncs Gmail locally
* learns reply patterns from historical sent mail
* classifies new inbound messages
* generates high-quality draft replies
* supports human approval + send
* provides an audit timeline (“why this draft?”)

**Privacy posture:** message corpus stays local by default. External LLM usage is opt-in and clearly surfaced.

---

## 2) High-level Architecture

### 2.1 Two-process system

**A) macOS App (Swift)**

* UI/UX, onboarding, settings
* Gmail OAuth login
* ChatGPT auth (for hybrid/cloud draft phrasing)
* “Inbox” and “Thread” views
* Approve/Send actions
* Audit & spend screens

**B) Local Codex Daemon (Rust)**

* Owns data + indexing + workflows
* Gmail sync + watch processing
* Local object model (EmailThread, Draft, Approval, Event)
* Policy engine (draft-only / approval-required / blocked)
* Tool execution & receipts
* Optional model routing (local or cloud) with sanitization

### 2.2 Communication (App ↔ Daemon)

Use **local-only** IPC:

* Prefer: **Unix domain socket** (best for local-only & avoids port collisions)
* MVP acceptable: `http://127.0.0.1:<random-port>` bound to loopback only

Streaming:

* **SSE** (Server-Sent Events) for event timeline updates
* Or WebSocket if you prefer bidirectional

Auth between them:

* App obtains a short-lived session token from daemon (or vice versa)
* All requests include token + nonce (replay protection)

---

## 3) macOS App: UX Requirements (Premium)

### 3.1 UI Surfaces

1. **Onboarding**

   * “Connect Gmail”
   * “Connect ChatGPT” (optional; for Hybrid / Cloud mode)
   * “Choose privacy mode” (Local-only / Hybrid / Cloud)
   * “Select backfill range” (90d default; 12mo optional)
2. **Inbox**

   * Focused queue: “Needs reply”
   * Thread list with category + risk chips
   * Search (client name, subject, address keywords)
3. **Thread view**

   * Conversation timeline
   * Suggested draft panel with:

     * “Insert Draft”
     * “Edit”
     * “Approve & Send”
     * “Mark Needs Human”
   * “Why this draft?” drawer:

     * category, risk tier, policy decision
     * similar past threads used
     * whether external model used
4. **Approvals queue**

   * All drafts awaiting approval
5. **Settings**

   * Privacy mode & model routing
   * Allowed recipient domains (optional)
   * Signature and templates
   * Backfill/sync controls
6. **Audit**

   * event timeline per thread/run
   * export logs (for support)

### 3.2 Native integrations (worth doing for premium feel)

* Store secrets in **Keychain**
* Optional **Touch ID / Passcode** app unlock
* **Notifications** for “Draft ready” or “Needs human”
* Menu bar indicator for sync status
* Great keyboard navigation

---

## 4) Auth Flows

### 4.1 Gmail (required)

Implementation:

* Use **ASWebAuthenticationSession** for Google OAuth consent
* Request scopes for:

  * read threads/messages
  * create drafts
  * send messages (optional; can delay until “send” is enabled)
* App passes auth code to daemon
* **Daemon exchanges code → tokens** and stores refresh token securely (Keychain access or encrypted local vault)
* Daemon handles refresh and API calls

**Important:** app should never need long-lived Gmail refresh tokens in memory.

### 4.2 ChatGPT (optional; for Hybrid / Cloud mode)

When privacy mode is Hybrid or Cloud, the daemon may call an external LLM (e.g. OpenAI) for draft phrasing. User must connect a ChatGPT/OpenAI account or provide an API key.

Implementation:

* **Option A:** OAuth-style “Sign in with ChatGPT” via **ASWebAuthenticationSession** (if OpenAI supports it); app passes code or token to daemon; daemon stores and uses for API calls.
* **Option B:** User pastes API key in Settings; app sends to daemon; daemon stores in encrypted vault and uses for OpenAI API. Simpler for MVP.
* Daemon uses stored credential only when mode is Hybrid or Cloud; Local-only never calls external APIs.

---

## 5) Local Storage & Encryption

### 5.1 Storage choices (MVP)

* Local SQLite for metadata + events + drafts
* Local encrypted blob store for raw RFC822 or message bodies (optional v1)
* Search index (can be SQLite FTS to start)

Encryption:

* Per-device master key stored in Keychain
* DB encrypted at rest (SQLCipher or app-level envelope encryption)

### 5.2 Data retention knobs

* backfill duration
* attachment storage: none / metadata-only / full
* delete local corpus button + “factory reset”

---

## 6) Runtime (Daemon) Responsibilities

### 6.1 Gmail Sync

* Backfill last N days/months
* Incremental sync on schedule
* Gmail “watch” support later; MVP can poll every X minutes

### 6.2 Classification & Risk

Classifier categories (v1):

* scheduling
* report delivery
* findings clarification
* pricing
* complaint/dispute
* legal/insurance
* other

Risk tiers:

* low / medium / high

### 6.3 Draft Generation (Local-first)

Draft algorithm (v1):

1. retrieve similar past threads + sent replies
2. extract style signals (tone, formatting, sign-off)
3. compose draft using:

   * templates + retrieval snippets
   * optional external model for phrasing (hybrid mode)

### 6.4 Policy gating

Default:

* `draft_only`

Then:

* `send_with_approval` (human required)
* `auto_send_low_risk` (future)

Hard blocks:

* legal threats → never one-click send unless explicit override
* unknown pricing → require template grounding

### 6.5 Event log + receipts

Every action produces an event (append-only), including:

* ingest events
* classification decision (+ reason codes)
* policy decision (+ evaluation hash)
* draft created (+ params/output hashes)
* approval granted/denied
* email sent (+ Gmail message id)

---

## 7) Cloud usage modes (explicit UX)

Mode A — **Local-only**

* No external LLM calls
* Draft quality may be lower; still strong for repetitive templated replies

Mode B — **Hybrid (recommended default)**

* Local retrieval + policy
* External model only for rewriting/phrasing
* Strict redaction before sending context
* Audit shows exactly what was sent externally

Mode C — **Cloud-first**

* External model drafts from retrieved context
* Still gated by policy + approval

---

## 8) MVP Scope (What we ship first)

### MUST ship

* Mac app with Gmail connect
* ChatGPT auth (for hybrid/cloud; can be API key in Settings for MVP)
* Backfill (90 days default)
* Inbox + thread view
* Draft generation for at least:

  * scheduling
  * report delivery
* Approve & Send (manual)
* Audit “why this draft?”

### NICE to have (if time)

* Template mining UI (“we found 12 common replies—approve these”)
* Domain allowlist
* Touch ID lock

---

## 9) Acceptance Criteria

1. A user can install app, connect Gmail (and optionally ChatGPT for hybrid/cloud) in <10 minutes.
2. App backfills 90 days and shows a searchable Inbox.
3. For new inbound emails, a draft appears in <60 seconds after sync.
4. Drafts for scheduling/report delivery require minimal editing at least 60% of the time (initial target).
5. “Approve & Send” sends via Gmail and logs message id.
6. Audit shows category/risk and sources used.

---

## 10) Build Plan (Engineering)

### Phase 0 (Day 0–1)

* Rust daemon skeleton:

  * local API
  * event stream
  * local storage
* Swift app shell:

  * navigation scaffolding
  * daemon connectivity
  * Gmail + ChatGPT auth scaffolding

### Phase 1 (Day 1–3)

* Gmail OAuth + token exchange
* Backfill + thread list UI

### Phase 2 (Day 3–5)

* Classification + retrieval
* Draft generation
* Thread view with “insert draft”

### Phase 3 (Day 5–7)

* Approve & Send
* Audit timeline UI
* Hardening + polish

---

## 11) Stack choices (explicit)

### macOS App

* **SwiftUI** for main UI
* AppKit where needed (rich text editor, email-like composition, advanced text)
* `ASWebAuthenticationSession` for OAuth
* Keychain for secrets
* Sparkle or equivalent for auto-updates (later if needed)

### Local daemon

* **Rust** (Codex)
* SQLite (or Postgres local if you’re already there, but SQLite is simplest)
* Local IPC: Unix socket + SSE

---

If you want, I’ll also produce:

* the **IPC API contract** (endpoints/events JSON)
* the **local DB schema** (threads/messages/drafts/events)
* and the **onboarding screens copy** so the first-run experience feels Apple-level.
