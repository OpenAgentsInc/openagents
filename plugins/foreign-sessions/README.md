# foreign_sessions

Foreign coding-agent session discovery — the scanner half of
OpenAgentsInc/openagents.com#198, as a `packet-v0` WASM guest on the owned
PDK. Given read-only mounts over `~/.claude` (mount 0) and `~/.codex`
(mount 1), it reports recent session metadata: source, session id, working
directory, mtime, size, and record count. It discovers; it never resumes.
The resume/import half of #198 is later work that builds on this listing.

## What it scans

- **Claude Code** — `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`.
  The working directory and session id come from the first records of each
  file (at most 20 lines inspected); the encoded project directory name is
  used as a cheap prefilter for `cwd_filter` before a file read is spent.
- **Codex CLI** — `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`. The
  working directory and id come from the `session_meta` first line.
- **Out of scope for this slice**: the Codex `~/.codex/state_*.sqlite`
  index and every other SQLite store. Reading SQLite from a
  `wasm32-unknown-unknown` guest is the known open risk (a wasm-sqlite
  spike), deferred deliberately; the rollout files carry enough for
  discovery. Cursor is also not scanned yet.

## Posture and bounds

Foreign state is untrusted input, and the plugin holds no authority of its
own — every access goes through the host's confined capability imports
(`openagents.read_file`, `openagents.list_dir`), which refuse absolute
paths, `..` escapes, and symlinks, and bound every read (1 MiB per file)
and listing (500 entries). On top of that the scanner bounds itself:

- at most 50 sessions reported (`limit` capped at 50, default 50)
- at most 200 file reads and 1500 directory listings per invocation
- at most 5000 candidate files held before sorting
- at most 20 leading JSONL lines inspected per file
- sessions older than `max_age_days` (default 30) are dropped; the sandbox
  has no clock, so the cutoff runs against `now_ms` when given and against
  the newest observed mtime otherwise

Failure is soft everywhere: a missing store is reported in
`missing_sources`, malformed and unreadable files are skipped and counted
in `skipped`, and a session file over the per-file read bound is still
reported from its listing metadata alone, marked `metadata_truncated`
(435 of the 1195 Claude session files on the development machine exceed
the bound, so this path is ordinary, not exceptional). `scan_truncated`
and `read_budget_exhausted` say when the picture may be partial.

## Try it

```
/plugin load plugins/foreign-sessions/manifest.json
```

then ask the coder to list recent sessions. The manifest's mounts are
declared as `~/.claude` and `~/.codex`; both must exist as directories on
the machine, or the load refuses (`mount_invalid`). Tests stage fixture
trees and point a copy of the manifest at them — see
`crates/openagents-cli/tests/foreign_resume_test.rs` for the boundary tests
and `src/tests.rs` for the fake-host scanner tests.
