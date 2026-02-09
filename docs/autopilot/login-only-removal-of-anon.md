# Autopilot: Login-Only — Removal of Anonymous Functionality

This document describes the change from **anonymous + authenticated** chat to **login-only**: all autopilot threads are owned by an authenticated user; anonymous threads and `anonKey` are no longer used in the product flow. Another agent (or human) can use this to verify the work.

**Related:** [anon-chat-execution-plane.md](./anon-chat-execution-plane.md) described the original Convex-first MVP with anon + authed; that design is superseded for the **product path** by this login-only behavior.

---

## 1. Decision Summary

- **Before:** Users could use autopilot anonymously (anon thread + `anonKey`) or while logged in (owned thread). Claim flow attached anon thread to user on login.
- **After:** Autopilot requires login. The client uses a single **owned thread** per user (`ensureOwnedThread`). No anon thread creation, no `anonKey` in requests, no claim flow in the app.

---

## 2. Files and Changes (Checklist for Verification)

### 2.1 Convex: Access (Owner-Only)

**File:** `apps/web/convex/autopilot/access.ts`

- **`assertThreadAccess`:** Allows access only when the Convex identity subject matches `thread.ownerId`. No branch for `anonKey`. Unauthenticated or wrong user → `forbidden`.
- **`AutopilotAccessInput`:** Still has optional `anonKey` for type compatibility with legacy callers; it is **not used** in the access check (see `@deprecated` in code).

**Verification:** Grep for `thread.ownerId === subject` and confirm there is no `input.anonKey` branch in `assertThreadAccess`.

---

### 2.2 Convex: Threads

**File:** `apps/web/convex/autopilot/threads.ts`

- **`claimAnonThreadImpl`:** Made no-op on failure (always returns `{ ok: true, threadId }`) so any legacy caller does not throw. Production flow no longer calls it.
- **`ensureAnonThread` / anon helpers:** Still present for tests; production client and Worker do **not** call them.

**Verification:** No production code path should call `ensureAnonThread` or `claimAnonThread` for the main chat flow. Client uses `ensureOwnedThread` only.

---

### 2.3 Worker (Cloudflare): Send, Stream, Cancel

**File:** `apps/web/src/effuse-host/autopilot.ts`

| Location | Change |
|----------|--------|
| **Types** | `SendBody` and `CancelBody`: removed `anonKey`. Request bodies no longer accept or parse `anonKey`. |
| **POST /api/autopilot/send** | No longer reads `body.anonKey`. Calls `convex.mutation(api.autopilot.messages.createRun, { threadId, text })` only (no anonKey). |
| **`runAutopilotStream`** | Input type has no `anonKey`. All calls pass only `threadId`, `runId`, `assistantMessageId`, `controller`, `env`, `request`. |
| **`isCancelRequested`** | Called with `{ convex, threadId, runId }` only (no anonKey). |
| **`layerDsePredictEnvForAutopilotRun`** | Called with `{ threadId, runId, onReceipt }` only (no anonKey). |
| **`flushPartsToConvex`** (and error path) | Called with `threadId`, `runId`, `messageId`, `parts` only (no anonKey). |
| **`runAutopilotStream` call site** | Single call passes no `anonKey` (removed `anonKey: null`). |

**Verification:**

- Grep for `anonKey` in `apps/web/src/effuse-host/autopilot.ts` → should find **zero** matches.
- `createRun` is invoked with only `threadId` and `text`.

---

### 2.4 Worker: DSE Layer

**File:** `apps/web/src/effuse-host/dse.ts`

- **`layerDseReceiptRecorderFromConvex`** and **`layerDsePredictEnvForAutopilotRun`:** No longer take or pass `anonKey`. Convex receipt/record calls (if any) use only `threadId` and `runId` as needed.

**Verification:** Grep for `anonKey` in `apps/web/src/effuse-host/dse.ts` → should find **zero** matches.

---

### 2.5 Client: Chat Service

**File:** `apps/web/src/effect/chat.ts`

- **Removed:** `getOrCreateAnonThreadId`, `getOrCreateAnonKey`, anon storage keys, and any `anonKey` in request bodies or Convex calls.
- **Added:** `getOwnedThreadId()` — calls `api.autopilot.threads.ensureOwnedThread` and returns the owned `threadId`.
- **`open(threadId)`:** No anon creation or claim; subscribes via `getThreadSnapshot({ threadId })` only.
- **`send` / `stop` / `clearHistory`:** Request bodies and Convex usage use only `threadId` (no anonKey).

**Verification:** Grep for `anonKey` or `ensureAnonThread` or `claimAnonThread` in `apps/web/src/effect/chat.ts` → should find **zero** matches. `getOwnedThreadId` should be present and used.

---

### 2.6 Client: Atoms

**File:** `apps/web/src/effect/atoms/chat.ts`

- **Added:** `OwnedThreadIdAtom` — writable atom holding the current user’s owned thread id (or null when not loaded / not logged in).
- No anon-specific atoms.

**Verification:** `OwnedThreadIdAtom` is defined and exported; no anon thread id atom.

---

### 2.7 Client: Autopilot Controller

**File:** `apps/web/src/effuse-app/controllers/autopilotController.ts`

- **Removed:** Anon storage, `getOrCreateAnonChatId`, `getOrCreateAnonChatKey`, and any `claimAnonThread` call (e.g. after magic-code verify).
- **Chat id source:** `chatId` is derived from `OwnedThreadIdAtom`. When `SessionAtom` has a user, the controller ensures an owned thread via `getOwnedThreadId()` and sets `OwnedThreadIdAtom`; that value is used as `chatId` for subscriptions and send/stop/clear.
- **Subscriptions:** When `OwnedThreadIdAtom` is set, the UI subscribes to `ChatSnapshotAtom(chatId)` and `AutopilotChatIsAtBottomAtom(chatId)`. On signOut, `OwnedThreadIdAtom` is cleared and subscriptions are cleaned up.
- **Send/stop/clear:** All Convex/store calls use only `threadId` (no anonKey). Unused `api` import from Convex removed.

**Verification:** No references to anon storage keys, anon thread id, or `claimAnonThread`. `OwnedThreadIdAtom` is the single source of truth for the active chat thread when logged in.

---

### 2.8 Client: Autopilot Store

**File:** `apps/web/src/effect/autopilotStore.ts`

- **`getBlueprint`**, **`importBlueprint`**, **`resetThread`:** Take only `threadId` (no anonKey). No `ensureAnonThread` usage.

**Verification:** No `anonKey` or anon thread creation in store API or implementations.

---

### 2.9 Boot and Identity

**File:** `apps/web/src/effuse-app/boot.ts`

- Identity pill: shows logged-in state and sign-out; sign-out clears session and **`OwnedThreadIdAtom`**.
- Removed: anon storage key constants and any cleanup of anon keys on signOut.

**Verification:** Sign-out path clears `OwnedThreadIdAtom`; no anon key cleanup.

---

### 2.10 Identity Pill Template

**File:** `apps/web/src/effuse-pages/identityPill.ts`

- Template fix: display label uses `(full || u.email) ?? "Account"` (correct precedence). No anon-specific UI.

---

## 3. What Still Exists (Intentional)

- **Convex schema:** `threads.anonKey` remains optional in the schema for backward compatibility and tests. New product flow does not set or use it.
- **Convex mutations/queries:** Many Convex autopilot functions still accept optional `anonKey` in their args (e.g. messages, blueprint, DSE receipts). Callers in the **product path** do not pass it; owner is derived from Convex auth.
- **Tests:** Some tests in `apps/web/tests/` still use anon threads and `anonKey` (e.g. `ensureAnonThreadImpl`, `createRunImpl` with anonKey) for unit/integration coverage. Those are test-only; production flow is login-only.

---

## 4. Verification Commands

Run from repo root or `apps/web`:

```bash
# Lint and typecheck (must pass)
cd apps/web && npm run lint

# No anonKey in Worker autopilot or dse
rg -n 'anonKey' apps/web/src/effuse-host/autopilot.ts apps/web/src/effuse-host/dse.ts
# Expected: no matches in autopilot.ts or dse.ts

# Client chat and controller: no anon flow
rg -n 'anonKey|ensureAnonThread|claimAnonThread|getOrCreateAnon' apps/web/src/effect/chat.ts apps/web/src/effuse-app/controllers/autopilotController.ts
# Expected: no matches

# Access is owner-only
rg -A 2 'assertThreadAccess' apps/web/convex/autopilot/access.ts
# Expect: only subject && thread.ownerId === subject; no anonKey branch
```

---

## 5. Quick Behavioral Checklist

- **Unauthenticated user:** Cannot create or access autopilot threads; access rules return `forbidden`. UI should show sign-in and not offer chat until logged in.
- **Authenticated user:** Gets exactly one owned thread via `ensureOwnedThread`; that thread id is stored in `OwnedThreadIdAtom` and used for all send/stop/clear and subscriptions.
- **Send request:** POST body to `/api/autopilot/send` contains only `threadId` and `text` (no `anonKey`). Worker calls `createRun` with the same.
- **No claim step:** After magic-code verification, the app does **not** call `claimAnonThread`; it only ensures the owned thread and sets `OwnedThreadIdAtom`.

---

## 6. Summary Table

| Layer        | Anon removed / Login-only behavior |
|-------------|-------------------------------------|
| Convex access | `assertThreadAccess`: owner-only; no anonKey branch. |
| Convex threads | `claimAnonThread` no-op on failure; production does not use ensureAnonThread/claimAnonThread. |
| Worker send/stream/cancel | No anonKey in types, body parsing, or Convex calls. |
| Worker DSE | No anonKey in layer or env. |
| Client chat | `getOwnedThreadId()` + owned thread only; no anon helpers. |
| Client atoms | `OwnedThreadIdAtom`; no anon thread id atom. |
| Controller | chatId from OwnedThreadIdAtom; no anon storage or claim. |
| Store / boot / identity | No anonKey; signOut clears OwnedThreadIdAtom. |

This completes the login-only removal of anonymous functionality for the autopilot product path.
