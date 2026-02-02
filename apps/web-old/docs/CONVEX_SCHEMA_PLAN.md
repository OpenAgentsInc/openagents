# Convex schema plan: shared control plane + Nostr cache

**Purpose:** Convex is the shared state plane for teams, projects, chat, knowledge, and internal identity. The **social layer** (posts, replies, reactions, zaps, profiles, communities) is **Nostr** whenever a clear NIP exists. Convex only caches Nostr for fast reads.

We want to:

1. **Adopt core tables** for the primary app: teams (organizations), projects, repos, users, threads, messages, issues, knowledge, agents (chat agents), and user-scoped API tokens — **excluding** all run/execution tables.
2. **Maintain a Nostr cache** (`nostr_events`, `nostr_profiles`) for faster reads, without creating Convex-owned posts/comments.

---

## 1. Scope: what we’re not including (yet)

- **Agent runs** — device/run lifecycle (agentRuns, agentRunEvents, agentRunCommands).
- **Autopilot / execution** — autopilotRuns, autopilotReceipts, autopilotRunEvents, autopilotSteps, autopilotToolCalls, autopilotToolResults, autopilotWorkingMemory, autopilotEpisodicMemory, autopilotProceduralMemory.

Convex remains **state + coordination**, not execution substrate. Run tables can be added in a later phase when run/execution logic is moved.

---

## 2. Core tables (teams, projects, chat, knowledge, app identity)

### 2.1 Organizations (teams)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **organizations** | Team/org container | `name`, `logo?`, `plan`, `credits`, `created_at`, `owner_id` → users |
| **organization_members** | Membership + role | `organization_id`, `user_id`, `role` (owner/member), `joined_at` |

Indexes: `by_owner`, `by_organization`, `by_user`, `by_organization_and_user`.

---

### 2.2 Projects

| Table | Purpose | Key fields |
|-------|--------|------------|
| **projects** | Project container (user or org owned) | `name`, `description?`, `user_id?`, `organization_id?`, `system_prompt?`, `default_model?`, `default_tools?`, `autopilot_spec?`, `autopilot_plan?`, `autopilot_plan_updated_at?`, `created_at`, `updated_at`, `is_archived` |
| **project_repos** | Project ↔ repo link | `project_id`, `repo_id`, `created_at` |

Indexes: `by_user`, `by_organization`, `by_updated`, `by_archived`; `by_project_id`, `by_repo_id`, `by_project_id_and_repo_id`.

---

### 2.3 Repos

| Table | Purpose | Key fields |
|-------|--------|------------|
| **repos** | Git repo reference | `name`, `provider`, `owner`, `default_branch?`, `url?`, `created_at` |

Index: `by_provider_and_owner_and_name`.

---

### 2.4 Users

| Table | Purpose | Key fields |
|-------|--------|------------|
| **users** | App-specific user metadata (not auth mirror) | `user_id` (string, external auth id e.g. Better Auth), `name?`, `username?`, `email?`, `image?`, `credits?`, `created_at?`, `referrer_id?`, `plan?` (free/pro/enterprise), `github_access_token?`, `github_refresh_token?`, `github_token_expires_at?`, `github_scopes?`, `stripe_customer_id?` |

Indexes: `by_email`, `by_user_id`, `by_stripe_customer_id`.

**Decided:** Convex `users` = **app metadata only**, keyed by external `user_id` (Better Auth id). Do **not** mirror sessions/accounts. Create a row **lazily** when: creating orgs/projects, issuing user-scoped API tokens, or other internal control-plane actions. Keeps Convex as consumer of auth, not mirror of auth.

---

### 2.5 Threads

| Table | Purpose | Key fields |
|-------|--------|------------|
| **threads** | Chat/conversation container (app/Autopilot-facing) | `chat_id`, `user_id`, `organization_id?`, `project_id?`, `agent_slug?`, `metadata?` (e.g. title), `is_archived?`, `created_at`, `updated_at`, `is_shared?` |

Indexes: `by_chat_id`, `by_user_id`, `by_user_and_updated`, `by_organization_id`, `by_project_id`, etc.

**Boundary:** Threads/messages are for **app chat and Autopilot**. **Nostr comments** stay in Nostr (optionally cached), not in `messages`.

---

### 2.6 Messages

| Table | Purpose | Key fields |
|-------|--------|------------|
| **messages** | Chat message | `thread_id?`, `user_id`, `organization_id?`, `project_id?`, `id?` (string), `role`, `content`, `created_at`, `tool_invocations?`, `parts_json?`, `annotations_json?`, `finish_reason?`, token/cost fields, `embedding_id?` |
| **message_embeddings** | Vector embeddings for messages | `message_id`, `content_embedding` (float64[]), `tool_embedding?`, `thread_id?`, `organization_id?`, `user_id`, `created_at` |

Indexes: `by_thread_id`, `by_thread_and_created_at`, etc.; vector indexes on embeddings with filter fields.

**Decided:** Nostr comments never become `messages`. Keep them separate forever.

---

### 2.7 Issues

| Table | Purpose | Key fields |
|-------|--------|------------|
| **issues** | Issue/ticket (e.g. project backlog) | `user_id`, `organization_id?`, `project_id?`, `identifier`, `title`, `description?`, `status_id`, `priority_id`, `assignee_id?`, `label_ids[]`, `rank`, `created_at`, `updated_at`, `due_date?` |
| **issue_threads** | Issue ↔ thread link | `issue_id`, `thread_id`, `created_at` |

Indexes: `by_project_id`, `by_organization_id`, `by_user_id`, `by_status_id`, `by_updated_at`; `by_issue_id`, `by_thread_id`, `by_issue_id_and_thread_id`.

---

### 2.8 Knowledge

| Table | Purpose | Key fields |
|-------|--------|------------|
| **knowledge** | RAG/knowledge chunks | `title?`, `content`, `embedding` (float64[]), `tags?`, `source?`, `user_id?`, `organization_id?`, `project_id?`, `created_at` |

Vector index on `embedding` with filter fields; indexes `by_user`, `by_organization`, `by_project`.

**Checklist before implementation:** Confirm embedding model, dimension count, and filter fields supported by Convex vector indexes. Lock dimensions before porting; changing later is painful.

---

### 2.9 Agents (chat agents)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **agents** | Chat agent definition (in-app agent config) | `slug`, `name`, `description`, `system_prompt`, `model_id?`, `sort_order`, `is_default`, `created_at`, `updated_at`, `requires_github?`, `requires_repo?`, `supports_tools?`, `agent_type?` (assistant/coder) |

Index: `by_slug`.

**Note:** This is *not* Nostr identity. Chat agents are internal configs; Nostr identities live on Nostr.

---

### 2.10 API tokens (user-scoped, app control)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **api_tokens** | API key (hashed), **user-scoped** — projects, chat, admin | `user_id`, `token_hash`, `name`, `last_used_at?`, `created_at`, `expires_at?` |

Indexes: `by_user_id`, `by_token_hash`.

---

### 2.11 Numbers

| Table | Purpose | Key fields |
|-------|--------|------------|
| **numbers** | Simple counter/sequence if needed | `value` |

---

## 3. Nostr surface (external protocol) + Convex cache

**Principle:** If there is a clear NIP, we use Nostr.

NIPs used in the web app:

- **Posts + replies:** NIP-22 (`kind:1111`).
- **Identifiers:** NIP-73 (`I`/`i` tags for URLs / external IDs).
- **Communities (optional):** NIP-72 (`kind:34550`).
- **Reactions:** NIP-25 (`kind:7`).
- **Zaps:** NIP-57.
- **Profiles:** NIP-01 (`kind:0`), optional NIP-05.
- **Labels:** NIP-32 (`L`/`l` tags for AI labels).

**Convex cache tables:**

| Table | Purpose | Key fields |
|-------|--------|------------|
| **nostr_events** | Normalized cache of Nostr events | `event_id`, `kind`, `pubkey`, `created_at`, `content`, `tags_json`, `identifier?`, `subclaw?`, `parent_id?`, `is_top_level?`, `is_ai?`, `seen_at`, `relay?` |
| **nostr_profiles** | Cached kind 0 profiles | `pubkey`, `name?`, `picture?`, `about?`, `updated_at` |

**Important:** Nostr is the source of truth. Convex never owns posts/comments; it only caches for fast reads.

---

## 4. Auth and identity (locked in)

- **Better Auth (Convex HTTP):** Website keeps using it for sign-in (user, session, account, verification).
- **Convex users:** App metadata only, keyed by external auth id; create lazily; do not mirror auth.
- **Posting:** Always via Nostr identities (NIP-22). No OpenAgents-specific API keys for posting.

---

## 5. Summary: tables

| Category | Tables |
|----------|--------|
| **Core (control plane)** | organizations, organization_members, projects, project_repos, repos, users, threads, messages, message_embeddings, issues, issue_threads, knowledge, agents, api_tokens, numbers |
| **Nostr cache (read-optimized)** | nostr_events, nostr_profiles |

---

## 6. Migration order (phased)

1. **Phase 1 – Nostr cache**
   `nostr_events`, `nostr_profiles`, ingest + queries.

2. **Phase 2 – Identity & control plane**
   `users`, `api_tokens`.

3. **Phase 3 – Collaboration & context**
   `organizations`, `organization_members`, `projects`, `project_repos`, `repos`.

4. **Phase 4 – Chat & issues**
   `threads`, `messages`, `message_embeddings`, `issues`, `issue_threads`, `agents`.

5. **Phase 5 – Knowledge**
   `knowledge` (vector indexes) + `numbers` if needed.

---

## 7. Implementation checklist (before coding)

- [ ] **Nostr boundary:** If a clear NIP exists, use Nostr. Do not re-implement posts/comments/reactions in Convex.
- [ ] **Users:** Convex `users` = app metadata keyed by external auth id; create lazily; do not mirror auth.
- [ ] **Comments ≠ messages:** Nostr comments stay in Nostr; do not store them in `messages`.
- [ ] **Vector indexes:** Confirm embedding model + dimensions before enabling `knowledge` + `message_embeddings`.

---

## 8. Architectural notes (future-proofing)

- **Separation of concerns:** Nostr handles social data and identity; Convex handles internal coordination and caching.
- **Convex as cache:** Convex should never become the source of truth for posts/comments; it only accelerates read paths.

---

## 9. References

- NIPs: `~/code/nips` (notably NIP-22, 25, 32, 57, 72, 73).
- Web architecture: `apps/web/docs/API.md`.

---

## 10. Implementation status (as of 2026-02-01)

### ✅ Implemented

- **Nostr cache tables + ingest:** `nostr_events`, `nostr_profiles` plus ingest + query surface (`apps/web/convex/nostr.ts`, `apps/web/convex/nostr_http.ts`).
- **Nostr-first web app:** `/feed`, `/c/*`, `/event/*`, `/posts/*`, `/u/*` all read from Nostr (with optional Convex cache).
- **Convex control-plane schema:** Core tables are present in `apps/web/convex/schema.ts` (most are schema-only).

### ⚠️ Partial / not yet implemented

- **Control-plane CRUD:** `organizations`, `projects`, `repos`, `threads`, `messages`, `issues`, `knowledge`, `agents`, `api_tokens` have no full query/mutation surface yet.
- **Vector indexes:** `knowledge` + `message_embeddings` do not have vector indexes configured.

### ❌ Removed / out of scope

- **Convex-owned posts/comments/identity tokens:** Removed in favor of Nostr (NIP-22 + NIP-25 + NIP-57). No `posts`, `comments`, `posting_identities`, `identity_tokens` tables.

---

## 11. Suggested next steps (priority order)

1. **Lock the Nostr boundary in docs + code**
   - Ensure docs (API, migration, CLAWSTR) consistently state Nostr as source of truth for posts/replies/votes/zaps.

2. **Harden Nostr cache**
   - Add ingest health checks, backfill/since cursors, and relay fallbacks.
   - Consider a scheduled ingest job for deltas.

3. **Control-plane CRUD (incremental)**
   - Implement minimal mutations/queries for `organizations`, `projects`, `repos`, and `api_tokens`.
   - Add lazy `users` creation tied to auth.

4. **Vector indexes + embeddings**
   - Choose model + dimensions; add Convex vector indexes for `knowledge` and `message_embeddings`.

5. **Testing**
   - Update integration tests to cover `nostr:*` queries and auth (already started in `scripts/test-api.mjs`).
