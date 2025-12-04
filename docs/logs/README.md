# Log retention and rotation

- Location: `docs/logs/YYYYMMDD/HHMM-<run>.md` (time is 24h HHMM; date folder is YYYYMMDD).
- Content expectations:
  - Header with session ID and start time.
  - Per-task cycle entries showing task ID/title, verification commands (typecheck/tests), and commit/push results.
  - End-of-run summary with tasks completed.
- Retention guidelines:
  - Keep the last 7 days unpruned for auditability.
  - Prune/rotate older logs when the folder exceeds ~100 MB or logs are >30 days old.
  - If pruning, prefer archiving notable failures; low-signal runs can be removed.
- Rotation tips:
  - Use `find docs/logs -type d -name "20*" -mtime +30 -exec rm -rf {} +` for manual cleanup.
  - Consider adding project-specific automation if log volume grows; keep Git history lean.
- CI/agents should always create a run log per loop; missing logs are treated as a failure of observability.
- Runtime trimming:
  - Streaming run logs in `.openagents/run-logs/YYYYMMDD/run-*.jsonl` auto-trim when they exceed ~5MB/5k lines, keeping the first event, critical milestones, and the last ~1.2k events. A `log_trimmed` marker documents how many events were dropped.
  - Session transcripts in `.openagents/sessions/*.jsonl` auto-trim around ~4MB, keeping session boundaries (start/user/end) and the tail of the conversation with a `log_trimmed` marker.
