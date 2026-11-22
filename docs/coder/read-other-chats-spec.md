# Read-Other-Chats Tool Specification

**Status:** Proposed
**Target:** Agent harness tool surface (`read_chat` / `list_chats` / `search_chats`) — the tools an agent uses to inspect other chats (sessions) it is not currently participating in
**Related docs:** [`session-history-evals-spec.md`](session-history-evals-spec.md), [`claude-history-evals-spec.md`](claude-history-evals-spec.md), [`desktop-evals-spec.md`](desktop-evals-spec.md), [`../chat.md`](../chat.md), [`../transcripts/README.md`](../transcripts/README.md)

---

## 1. Goals and Scope

Today an agent has no tool that lets it read other chats. It can read files,
grep the repo, and run commands, but it cannot ask "what did the other agent
just do?", "what did this chat conclude last week?", or "which chats mention
this issue?". This spec defines the tool that closes that gap.

The tool is **read-only**. It never writes, edits, deletes, or sends anything
in any chat. Changes to other chats go through the editor or the agents that
own them.

### 1.1 Non-Goals

- Not a chat-control surface — no sending, replying, stopping, or deleting other chats.
- Not a transcript-storage system — chats are read from where they already live (the harness session store, the CLI's saved chats, or the web app's transcript store); this spec does not define a new store.
- Not a cross-tenant oracle — an agent can only read chats it is authorized to see (its own account, its own workspace, or explicitly shared chats).
- Not a replacement for `evals/session-history/` — that pipeline ingests and sanitizes histories for evaluation; this tool is for in-session inspection.

---

## 2. Tool Surface

Three tools, all read-only, all scoped to the caller's authorization:

### 2.1 `list_chats`

Lists chats the caller may read.

**Inputs**

| Field | Type | Required | Notes |
|---|---|---|---|
| `limit` | integer | no | Max chats to return (default 20, max 100) |
| `cursor` | string | no | Opaque pagination cursor from a previous response |
| `query` | string | no | Free-text filter over title / first user message (case-insensitive substring) |
| `since` | ISO-8601 timestamp | no | Only chats updated at or after this time |
| `before` | ISO-8601 timestamp | no | Only chats updated at or before this time |

**Output**

```json
{
  "chats": [
    {
      "id": "chat_...",
      "title": "...",
      "created_at": "2026-11-20T10:00:00Z",
      "updated_at": "2026-11-20T10:42:00Z",
      "message_count": 12,
      "owner": "self",
      "source": "harness|cli|web"
    }
  ],
  "next_cursor": "..." | null
}
```

### 2.2 `read_chat`

Reads one chat in full (or a bounded slice of it).

**Inputs**

| Field | Type | Required | Notes |
|---|---|---|---|
| `chat_id` | string | yes | From `list_chats` |
| `from_message` | integer | no | 1-based index of first message to return (default 1) |
| `to_message` | integer | no | 1-based index of last message to return (default: end) |
| `max_chars` | integer | no | Cap on total characters returned (default 20000, max 100000) |
| `include_tool_calls` | boolean | no | Include tool-call rows in the transcript (default true) |

**Output**

```json
{
  "id": "chat_...",
  "title": "...",
  "messages": [
    {
      "index": 1,
      "role": "user|assistant|tool|system",
      "content": "...",
      "created_at": "2026-11-20T10:00:00Z",
      "tool_calls": [
        { "name": "read_file", "args": { "path": "..." }, "result_summary": "..." }
      ]
    }
  ],
  "truncated": false,
  "next_cursor": null
}
```

If the chat exceeds `max_chars`, the response is truncated at a message
boundary and `truncated` is true with a `next_cursor` pointing at the
first omitted message. Tool-call `args` are included but never contain
secrets (see §4).

### 2.3 `search_chats`

Searches across readable chats.

**Inputs**

| Field | Type | Required | Notes |
|---|---|---|---|
| `query` | string | yes | Substring or regex over message content |
| `limit` | integer | no | Max hits (default 10, max 50) |
| `since` / `before` | ISO-8601 timestamp | no | Time window over `updated_at` |

**Output**

```json
{
  "hits": [
    {
      "chat_id": "chat_...",
      "title": "...",
      "message_index": 7,
      "snippet": "...",
      "score": 0.0
    }
  ]
}
```

Hits are ordered by `score` descending. `score` is a harness-defined
relevance number; it is not a probability.

---

## 3. Authorization and Scope

- An agent may only read chats it is authorized to see: its own account's
  chats, chats in its own workspace, and chats explicitly shared with it.
- `list_chats` never returns a chat the caller cannot read. `read_chat` and
  `search_chats` return a `not_authorized` error for any other chat, with no
  partial content.
- The tool never returns chats owned by another tenant, even as a count or a
  title.

---

## 4. Privacy and Secrets

- Tool-call `args` and message content are returned as-is **except** for
  known secret patterns (API keys, tokens, private keys, `.env` values),
  which are redacted before the response is built. Redaction is best-effort
  and is not a guarantee of safety for arbitrary content.
- Raw transcripts are not written to git, logs, or eval fixtures by this
  tool. It is an in-session read surface only.
- The tool does not persist anything. Each call is served from the live
  session store.

---

## 5. Errors

| Code | Meaning |
|---|---|
| `not_found` | `chat_id` does not exist or is not visible to the caller |
| `not_authorized` | Chat exists but is not readable by the caller |
| `invalid_input` | Malformed input (bad timestamp, `from_message > to_message`, etc.) |
| `too_large` | Chat exceeds `max_chars` and truncation is not possible (e.g. single oversized message) |
| `rate_limited` | Too many calls in a window |

Errors are returned as structured JSON with the code and a short message.
No partial content is returned on error.

---

## 6. Acceptance Criteria

1. `list_chats` returns only chats the caller can read, paginated, with a stable cursor.
2. `read_chat` returns a bounded slice of a chat, with `truncated` and `next_cursor` set correctly when the chat exceeds `max_chars`.
3. `search_chats` returns hits ordered by score, scoped to readable chats only.
4. A caller cannot read, list, or search a chat it is not authorized to see — no partial content, no existence leakage beyond `not_authorized`.
5. Secret-like content in tool-call args and message content is redacted before the response is built.
6. The tool is read-only: it has no write, send, delete, or control operations.
7. The tool is served from the live session store and persists nothing.

---

## 7. Open Questions

- Should `read_chat` support a `format` parameter (e.g. `markdown`, `atif`)?
  Leaning toward yes, but out of scope for the first cut.
- Should `search_chats` index tool-call args, or only message content?
  Leaning toward content-only for the first cut.
- Should the tool surface the chat's own tool-call trace as a separate
  `read_tool_trace` tool? Deferred.
