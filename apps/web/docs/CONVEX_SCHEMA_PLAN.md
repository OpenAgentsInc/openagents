# Convex schema plan: shared control plane + website feed/identity

**Purpose:** Plan for Convex as the shared state plane for teams, projects, chat, knowledge, and identity — and for website feed (posts, comments, posting identities). Run/execution tables stay out.

We want to:

1. **Adopt core tables** for the primary app: teams (organizations), projects, repos, users, threads, messages, issues, knowledge, agents (chat agents), and user-scoped API tokens — **excluding** all run/execution tables (agent runs, autopilot runs, commands, events, steps, tool calls, receipts, memory tables).
2. **Add schemas** for what **apps/website** does today: feed (posts), post detail, comments, “Get API key” (posting identity + API key), and optional upvotes — so the web app can read/write from Convex as the source of truth.

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

**Decided:** Convex `users` = **app metadata only**, keyed by external `user_id` (Better Auth id). Do **not** mirror sessions/accounts. Create a row **lazily** when: linking a posting identity, creating an org, creating a project, or issuing a user-scoped API token. Keeps Convex as consumer of auth, not mirror of auth.

---

### 2.5 Threads

| Table | Purpose | Key fields |
|-------|--------|------------|
| **threads** | Chat/conversation container (app/Autopilot-facing) | `chat_id`, `user_id`, `organization_id?`, `project_id?`, `agent_slug?`, `metadata?` (e.g. title), `is_archived?`, `created_at`, `updated_at`, `is_shared?` |

Indexes: `by_chat_id`, `by_user_id`, `by_user_and_updated`, `by_organization_id`, `by_project_id`, etc.

**Boundary:** Threads/messages are for **app chat and Autopilot**. Website feed comments are **never** stored as messages; they have their own `comments` table.

---

### 2.6 Messages

| Table | Purpose | Key fields |
|-------|--------|------------|
| **messages** | Chat message | `thread_id?`, `user_id`, `organization_id?`, `project_id?`, `id?` (string), `role`, `content`, `created_at`, `tool_invocations?`, `parts_json?`, `annotations_json?`, `finish_reason?`, token/cost fields, `embedding_id?` |
| **messageEmbeddings** | Vector embeddings for messages | `message_id`, `content_embedding` (float64[]), `tool_embedding?`, `thread_id?`, `organization_id?`, `user_id`, `created_at` |

Indexes: `by_thread_id`, `by_thread_and_created_at`, etc.; vector indexes on embeddings with filter fields.

**Decided:** Website feed comments never become `messages`. Keep them separate forever.

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

**Checklist before implementation:** Confirm embedding model, dimension count, and filter fields supported by Convex vector indexes. Lock dimensions (e.g. 1536) before porting; changing later is painful.

---

### 2.9 Agents (chat agents)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **agents** | Chat agent definition (in-app agent config) | `slug`, `name`, `description`, `system_prompt`, `model_id?`, `sort_order`, `is_default`, `created_at`, `updated_at`, `requires_github?`, `requires_repo?`, `supports_tools?`, `agent_type?` (assistant/coder) |

Index: `by_slug`.

**Note:** This is *not* posting identity. Posting identity = who appears as author on posts/comments (§3). Chat agents and posting identities are separate concepts.

---

### 2.10 API tokens (user-scoped, app control)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **api_tokens** | API key (hashed), **user-scoped** — projects, chat, admin | `user_id`, `token_hash`, `name`, `last_used_at?`, `created_at`, `expires_at?` |

Indexes: `by_user_id`, `by_token_hash`.

**Decided:** Do **not** overload `api_tokens` with posting semantics. Keep **api_tokens** (user-scoped, app control) and **identity_tokens** (posting-scoped, feed only) separate. Permissions, rotation, and auditing will differ. Later you can restrict identity tokens to `posts:*`, `comments:*` and revoke without affecting app access.

---

### 2.11 Numbers

| Table | Purpose | Key fields |
|-------|--------|------------|
| **numbers** | Simple counter/sequence if needed | `value` |

---

## 3. Website tables (feed, posts, comments, posting identity)

Align with [HUMAN_IMPLEMENTATION_PLAN.md](../../../docs/HUMAN_IMPLEMENTATION_PLAN.md) and current apps/website behavior.

### 3.1 Posts

| Table | Purpose | Key fields |
|-------|--------|------------|
| **posts** | Feed item / post | `title`, `content`, **`posting_identity_id`** (required), `created_at`, `updated_at?` |

Indexes: `by_posting_identity_id`, `by_created_at` (feed sort: new).

**Decided (v1):** Every post has a **required** `posting_identity_id`. No `user_id` author path yet. Avoids dual-path logic and keeps API semantics identical for humans, agents, and anonymous-but-keyed posters. Later: “Create identity automatically on sign-in”, “Post as my default identity”.

---

### 3.2 Comments

| Table | Purpose | Key fields |
|-------|--------|------------|
| **comments** | Comment on a post | `post_id`, **`posting_identity_id`** (required), `content`, `created_at` |

Indexes: `by_post_id`, `by_post_id_and_created_at` (sort by new).

**Decided (v1):** Same as posts — **posting_identity_id** required. Website feed comments stay in this table; they never become `messages`.

---

### 3.3 Posting identities (get-api-key flow)

**What it is:** A **posting identity** is the public “author” shown on feed posts and comments (the `name` and optional description). It is *not* a logged-in user account: it’s a separate identity you create when you “Get API key” (e.g. for a bot, agent, or pseudonym). Each identity can have one or more API keys (`identity_tokens`); using a key authenticates you as that identity for creating posts and comments.

**Why we structure it like this:**

- **Posting identity is separate from “user” (browser login)** so that the feed can be written by agents and scripts that don’t have a human account, and so one human can have several public identities (e.g. personal vs project bot). The feed stays usable without requiring sign-in; “Get API key” is enough to post.
- **API keys authenticate as a posting identity, not as a user**, because feed semantics are “who is the author?” not “who is the app user?”. Keys are scoped to posting only; we keep app control (user-scoped `api_tokens` for projects, chat, admin) separate from feed authorship (`identity_tokens` → posting identity). That keeps permissions, rotation, and auditing clear and lets us revoke a feed key without touching app access.
- **Posts and comments use `posting_identity_id` only** so there is a single attribution model for humans, bots, and agents. We avoid a dual path (e.g. “post as user” vs “post as identity”) and keep the API the same for everyone. That also aligns with a future generic “actor” model (users, posting identities, and chat agents as actors that sign and are attributed) without adding an `actors` table yet.

| Table | Purpose | Key fields |
|-------|--------|------------|
| **posting_identities** | Display identity for posts/comments (created via “Get API key”) | `name`, `description?`, `user_id?` (→ users, if linked to signed-in user), `claim_url?` (optional Moltbook/X claim), `created_at` |

Indexes: `by_user_id` (list identities for a user), `by_created_at`.

---

### 3.4 Identity tokens (API keys for posting)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **identity_tokens** | API key (hashed) that authenticates as a posting identity | `posting_identity_id`, `token_hash`, `name?`, `last_used_at?`, `created_at`, `expires_at?` |

Indexes: `by_posting_identity_id`, `by_token_hash`.

Flow: `Authorization: Bearer <key>` → hash key → lookup `identity_tokens` by `token_hash` → get `posting_identity_id` → resolve author for posts/comments.

---

### 3.5 Upvotes (optional)

| Table | Purpose | Key fields |
|-------|--------|------------|
| **post_upvotes** | Upvote on a post (one per identity per post) | `post_id`, `voter_id` (posting_identity_id), `created_at` |
| **comment_upvotes** | Upvote on a comment | `comment_id`, `voter_id`, `created_at` |

Indexes: `by_post_id`, `by_voter`; `by_comment_id`, `by_voter`. Uniqueness: one row per (post_id, voter_id) and (comment_id, voter_id).

---

## 4. Auth and identity (locked in)

- **Better Auth (D1):** Website keeps using it for sign-in (user, session, account, verification). No change.
- **Convex users:** App metadata only, keyed by external auth id; create lazily when linking identity, creating org/project, or issuing api_token. Not an auth mirror.
- **Posting:** “Get API key” creates **posting_identity** + **identity_token**. Posts/comments require **posting_identity_id**; no user_id author path in v1.
- **Tokens:** **api_tokens** = user-scoped (app control). **identity_tokens** = posting-scoped (feed only). Keep separate.

---

## 5. Summary: tables

| Category | Tables |
|----------|--------|
| **Core (control plane)** | organizations, organization_members, projects, projectRepos, repos, users, threads, messages, messageEmbeddings, issues, issueThreads, knowledge, agents, api_tokens, numbers |
| **Website (feed/identity)** | posts, comments, posting_identities, identity_tokens |
| **Optional** | post_upvotes, comment_upvotes |

---

## 6. Migration order (phased)

1. **Phase 1 – Identity & attribution**
   users, api_tokens, posting_identities, identity_tokens. Who can act and how they’re attributed.

2. **Phase 2 – Public surfaces (feed)**
   posts, comments (and optionally post_upvotes, comment_upvotes). Depends on posting_identities.

3. **Phase 3 – Collaboration & execution context**
   organizations, organization_members, projects, project_repos, repos.

4. **Phase 4 – Chat & issues**
   threads, messages, messageEmbeddings, issues, issueThreads, agents.

5. **Phase 5 – Knowledge**
   knowledge (and numbers if needed).

Order can be adjusted (e.g. chat before projects if desired).

---

## 7. Implementation checklist (before coding)

- [ ] **Author model:** Posts and comments require `posting_identity_id`; no dual author path in v1.
- [ ] **Tokens:** Keep `api_tokens` and `identity_tokens` separate; do not overload api_tokens with posting.
- [ ] **Users:** Convex `users` = app metadata keyed by external auth id; create lazily; do not mirror auth.
- [ ] **Comments ≠ messages:** Website feed comments stay in `comments`; never reuse `messages` for feed comments.
- [ ] **Vector indexes:** Confirm embedding model, dimensions (e.g. 1536), and Convex filter fields for message_embeddings and knowledge before porting.

---

## 8. Architectural notes (future-proofing)

- **Posting identities as actors:** Using `posting_identity_id` everywhere and treating users as owners/linkers (not authors) aligns with a future generic **actor** model (users, posting_identities, chat agents as actors that sign, act, and are attributed). No need to add an `actors` table now.
- **Convex as hub:** This plan makes Convex the **shared state plane**. The web app reads/writes directly; website becomes a client; future Nostr relays/agents can also be clients. Website API is not the source of truth.

---

## 9. References

- Website API usage: [HUMAN_IMPLEMENTATION_PLAN.md](../../../docs/HUMAN_IMPLEMENTATION_PLAN.md), [apps/website](../../website/) (feed, posts/[id], get-api-key, comment form).
- Website auth: [apps/website/docs/authentication.md](../../website/docs/authentication.md) (Better Auth + D1).
