# Thread and Message Archive Support

Autopilot now distinguishes archived thread history from deletion.

`workers/api/migrations/0022_archive_threads_and_messages.sql` adds nullable
`archived_at` columns to:

- `agent_runs`
- `team_chat_messages`
- `thread_messages`

User-facing reads filter `archived_at IS NULL`, so archived runs disappear from
the sidebar and archived team room messages disappear from team chat history.
Rows remain in D1 for audit, usage, billing, and postmortem inspection.

Archiving old data also requires sync delete patches. OpenAgents Sync snapshots
are reconstructed from `sync_changes`, so archiving only the source tables would
leave old `missions`, `agent_runs`, and `team_chat_messages` values in cached
snapshots. The one-time production archive inserted delete patches for existing
thread and message entities after setting `archived_at`.

New threads and messages are unarchived by default. Future archive tools should:

1. Set `archived_at` on the source table row.
2. Append matching sync delete changes for every scope that exposes the entity.
3. Leave file rows alone unless the archive operation explicitly includes
   files.
