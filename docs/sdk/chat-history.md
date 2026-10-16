# Saved chat reads

The read-only history API is served at `https://api.openagents.com/v1`.
Use an app session or sign-in key from the existing account. These reads
never start a task, contact a source computer, or change a chat's seen state.
Unsynced local histories are not uploaded by signing in.

| Request | Result |
| --- | --- |
| `GET /history?limit=100&after=ID` | `{data: [Summary], next}`; all owned web, terminal, and phone chats, including archived chats |
| `GET /history/projects?limit=100&after=ID` | `{data: [{id, name, shared}], next}`; current projects without scanning their chats |
| `GET /history?project=ID&limit=100&after=ID` | The shared project's chats, including other members' chats and archived chats |
| `GET /history/{id}/messages?limit=100` | `{thread: Summary, revision, data: [Message], next, captures}`; oldest messages first |
| `GET /history/{id}/messages?after=msg_99&revision=N` | Continue the same saved revision |
| `GET /history/{id}/files/{file}` | Saved input attachment bytes |
| `GET /history/{id}/captures/{capture}` | Saved screenshot or pulled-file bytes |

All limits are 1–200. Follow `next` until null, including on empty pages
whose scanned records were deleted or moved. Discovery uses ascending IDs,
not activity order, and has no 10,000-chat or 2,000-shared-chat cutoff. Merge
owned and shared lists by chat ID, then sort summaries by pin time and last
activity. Keep cursors with their account and project. A deleted cursor still
continues. Discovery isn't a snapshot: a new earlier ID appears on refresh.
Changes to title, pin time, archive state, or activity do not move an ID.

A summary includes `id`, `title`, `revision`, `updated_unix`, nullable
`pinned_unix`, `archived_unix`, and `project`, `source` (`web`, `terminal`,
`phone`), and `messages`. No storage owner or session/host control identifier
is returned. The client decides whether to show archived chats.

Messages include `id` (`msg_INDEX`, valid only at that revision), `role`
(`user`, `assistant`, `tool`, `thinking`), `text`, `files`, and nullable `tool`
(`name`, `summary`, `status`, `input`, `output`). Empty saved messages are
preserved. Files have `id`, `name`, `kind`, and `size`; a reference does not
promise bytes still exist. Generic per-message `generated_files` and
`artifacts` are empty arrays because this store doesn't save those objects.
Saved computer captures are separate transcript-level `captures`, with
`id`, `name`, `state` (`saved`, `unavailable`, `not_saved`), nullable `size`,
and nullable `media`. Unavailable captures don't wake the source computer.
Links in saved Markdown remain text; this API doesn't fetch their targets.

Continue transcripts with the returned revision. `409 changed` means discard
the partially loaded transcript and restart; `400 invalid_cursor` means restart
without a cursor. A saved chat is bounded to 16 MiB; clients must support that
bound rather than silently truncating a message. File limits remain those of
the existing attachment store. `404` covers missing/deleted chats, missing
bytes, and chats no longer shared with the caller. `401` requires login;
`503` means retry later, not an empty account. Every page and byte read checks
account/project access; the existing account access cache expires within
30 seconds. Use no persistent response cache across accounts or sign-out.

The legacy `/threads` offset list, project-filtered `/threads` cursor list,
and newest-200 `/threads/{id}` preview remain distinct and unchanged.
See [the generated OpenAPI](openapi.app.json) for implemented history schemas.
