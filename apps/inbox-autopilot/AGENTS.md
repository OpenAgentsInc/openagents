# AGENTS.md — Guide for AI Coding Agents

This file is for **AI coding agents** (e.g. Cursor, Copilot, or other assistants) working in this repo. It explains what the repo is, how it’s structured, how to prioritize work, and how to stay aligned with the product and roadmap.

---

## 1. What this repo is

**Inbox Autopilot** is a **macOS-native** app that:

- Syncs **Gmail** locally
- Learns reply patterns from the user’s sent mail
- Classifies inbound messages (category + risk)
- Generates **draft replies** (local retrieval + optional external LLM in “hybrid/cloud” mode)
- Supports **human approval + send** (no auto-send in MVP)
- Provides an **audit timeline** (“why this draft?” — category, risk, sources, policy)

**Privacy:** Message data stays on device by default. External LLM use (e.g. ChatGPT/OpenAI) is opt-in and only when the user chooses Hybrid or Cloud mode.

The system has **two processes**:

1. **macOS app (Swift)** — UI, onboarding, settings, Gmail/ChatGPT auth flows, Inbox/Thread/Approvals/Settings/Audit screens. Uses Keychain for secrets; never holds long-lived Gmail refresh tokens.
2. **Local daemon (Rust)** — Owns all data, Gmail sync, classification, draft generation, policy, and event log. Talks to the app over **local-only IPC** (HTTP on loopback or Unix socket + SSE for events). The **daemon** performs Gmail OAuth code→token exchange and stores the refresh token; the app only passes the auth code.

**Auth for MVP:** **Gmail** (required) and **ChatGPT** (optional, for hybrid/cloud draft phrasing). No OpenAgents or other account login in MVP.

---

## 2. Repo layout (what exists today)

- **`README.md`** — Product goal, architecture, auth flows, storage, runtime responsibilities, MVP scope, stack choices, high-level build plan. **Source of truth for product and architecture.**
- **`ROADMAP.md`** — Phased build order (Phase 0 → 4) with checkboxes, deliverables, and dependencies. **Source of truth for what to build and in what order.**
- **`AGENTS.md`** — This file; for agents.
- **`Inbox Autopilot/`** — Xcode project for the macOS app. SwiftUI + AppKit. Currently a minimal shell (app entry, `ContentView`, placeholder structure). **No daemon yet** — the Rust daemon is specified in the README/ROADMAP but not yet present in the repo (to be added under a path like `daemon/` or `codex-daemon/`).
- **`docs/`** — Reference and design docs:
  - **`gmail-oauth-without-user-owned-app.md`** — Using a single app-owned OAuth client so users don’t create their own GCP OAuth app.
  - **`openclaw-gmail-auth-and-email-automation.md`** — How OpenClaw does Gmail (gog, gcloud); reference only.
  - **`gogcli-details-and-inlining-pros-cons.md`** — gogcli (Gmail CLI) internals; inlining vs using binary; reference if we ever need Gmail logic details.
  - **`openagents-where-to-put-key-exchange.md`** — Where OAuth/key exchange lives in the OpenAgents Laravel app; relevant if we later integrate with that backend, not required for MVP.

**Not in repo yet (per ROADMAP):** Rust daemon crate, IPC API contract doc, DB schema doc. These are to be created in Phase 0.

---

## 3. Priorities and how to work

### 3.1 Follow the ROADMAP

- **Work in phase order.** Phase N depends on Phase N−1. Do not skip phases or build Phase 2 features before Phase 1 is done.
- **Use the ROADMAP as the task list.** Prefer implementing or completing unchecked items in the current phase. When a phase is complete (deliverables met), move to the next.
- **One vertical slice over broad horizontal work.** Prefer finishing one end-to-end flow (e.g. “Gmail connect → backfill → Inbox list”) before spreading to many half-done areas.

### 3.2 Current phase

- If the **daemon does not exist yet**, the current phase is **Phase 0**: daemon skeleton, app shell (nav, daemon connectivity, Gmail + ChatGPT auth scaffolding), and IPC contract.
- If the daemon exists but Gmail auth is not done, the current phase is **Phase 1** (Gmail OAuth + optional ChatGPT auth + backfill + Inbox UI).
- If Gmail and Inbox work but drafts don’t, the current phase is **Phase 2** (classification, retrieval, draft generation, thread view, “Why this draft?”).
- If drafts work but send/audit don’t, the current phase is **Phase 3** (Approve & Send, approvals queue, audit timeline, hardening).
- If MVP is complete, the current phase is **Phase 4** (polish: Keychain, Touch ID, notifications, menu bar, templates, allowlist, encryption, auto-updates).

When in doubt, open **ROADMAP.md** and work on the **first unchecked item** in the earliest incomplete phase.

### 3.3 Principles (from ROADMAP)

- **Local-first** — Daemon owns data and workflows; message corpus stays on device by default.
- **Auth before data** — Gmail (and optional ChatGPT for hybrid/cloud) must work before backfill or real thread list.
- **Vertical slices** — One flow end-to-end (e.g. backfill → thread list → open thread) over “all backend then all UI.”
- **Audit from day one** — Every daemon action should emit events; the “why this draft?” UI can be built once drafts exist.

### 3.4 Conventions and constraints

- **Gmail tokens:** The **daemon** performs the OAuth code→token exchange and stores the refresh token. The **app** only obtains the auth code (e.g. via `ASWebAuthenticationSession`) and sends it to the daemon. The app must **not** hold long-lived Gmail refresh tokens in memory or in the UI process.
- **ChatGPT:** Optional. MVP can be “user pastes API key in Settings”; app sends it to daemon; daemon stores it encrypted and uses it only when privacy mode is Hybrid or Cloud.
- **IPC:** Local-only (127.0.0.1 or Unix socket). App and daemon authenticate with a short-lived session token + nonce (replay protection). Document the IPC contract (endpoints, request/response shapes, SSE events) as soon as the daemon has a minimal API.
- **Secrets:** Keychain for app-side secrets; daemon uses an encrypted local vault (or Keychain-backed store) for Gmail refresh token and ChatGPT API key. No plaintext secrets on disk.

---

## 4. Stack and tooling

- **macOS app:** Swift, SwiftUI, AppKit where needed. `ASWebAuthenticationSession` for OAuth. Keychain for secrets. Target macOS version as specified in the Xcode project.
- **Daemon:** Rust (Codex). SQLite for local DB. Local API: HTTP on loopback or Unix socket; SSE (or WebSocket) for event stream.
- **IPC:** Prefer Unix domain socket for local-only; MVP may use `http://127.0.0.1:<port>` bound to loopback only.

---

## 5. Acceptance criteria (README §9)

Use these to know when the MVP is “done”:

1. User can install app, connect Gmail (and optionally ChatGPT for hybrid/cloud) in **&lt;10 minutes**.
2. App **backfills 90 days** and shows a **searchable Inbox**.
3. For new inbound emails, a **draft appears in &lt;60 seconds** after sync.
4. Drafts for **scheduling/report delivery** require **minimal editing at least 60% of the time** (initial target).
5. **“Approve & Send”** sends via Gmail and **logs message id**.
6. **Audit** shows **category/risk and sources used**.

---

## 6. Docs quick reference

| Doc | Use when |
|-----|----------|
| **README.md** | You need product goal, architecture, auth flows, storage, runtime duties, MVP scope, or stack. |
| **ROADMAP.md** | You need the next task, phase order, deliverables, or dependency between phases. |
| **docs/gmail-oauth-without-user-owned-app.md** | You need to implement Gmail OAuth with a single app-owned client (no per-user OAuth app). |
| **docs/openclaw-gmail-auth-and-email-automation.md** | You want a reference for how another project (OpenClaw) does Gmail. |
| **docs/gogcli-details-and-inlining-pros-cons.md** | You are considering inlining Gmail/watch logic vs calling an external binary. |
| **docs/openagents-where-to-put-key-exchange.md** | You are integrating with the OpenAgents Laravel backend for key exchange. |

---

## 7. What to do when stuck

- **“What should I build next?”** → Open **ROADMAP.md**; pick the first unchecked item in the earliest incomplete phase.
- **“How should X work?”** → Check **README.md** (architecture, auth, storage, runtime) and **ROADMAP.md** (deliverables and constraints).
- **“Where does the daemon live?”** → It may not exist yet; if so, add a Rust crate under e.g. `daemon/` and implement Phase 0.1. If it exists, follow the IPC contract and existing layout.
- **“Can I add OpenAgents login?”** → No for MVP. MVP auth is **Gmail + ChatGPT only**.
- **“Should the app or daemon do token exchange?”** → **Daemon** for Gmail (app sends code only). For ChatGPT, MVP can be API key sent from app to daemon; daemon stores and uses it.
- **“Should I emit events for this action?”** → Yes. Daemon should emit append-only events for ingest, classification, policy, draft creation, approval, and send. Event log is required for audit.

---

## 8. Summary for agents

- **Product:** Mac app + local Rust daemon; Gmail sync, classification, drafts, approve & send, audit; privacy local-first, optional cloud LLM.
- **Auth:** Gmail (required), ChatGPT (optional). No OpenAgents in MVP.
- **Task source:** **ROADMAP.md** (phases 0–4, checkboxes). Work in phase order; prefer vertical slices.
- **Contract source:** **README.md** (architecture, auth, storage, stack). Daemon owns data and Gmail tokens; app is UI and auth code handoff.
- **Current focus:** If no daemon exists → Phase 0 (daemon skeleton, app shell, IPC contract). Else → first incomplete phase in ROADMAP.
- **Done:** When all six acceptance criteria in README §9 are met and ROADMAP Phase 3 (and optionally Phase 4) is complete.
