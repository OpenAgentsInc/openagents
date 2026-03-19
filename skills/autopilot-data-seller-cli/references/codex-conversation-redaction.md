# Codex Conversation Redaction

Use the Codex conversation packager when the material for sale is a bundle of
local Codex sessions rather than an arbitrary file tree.

Primary entrypoint:

```bash
skills/autopilot-data-seller-cli/scripts/package_codex_conversations.sh \
  --limit 5 \
  --output-dir ./tmp/codex-package \
  --title "Redacted Codex conversation bundle" \
  --price-sats 500
```

This wrapper does two jobs in one pass:

1. read rollout JSONL files from `~/.codex/sessions` or explicit `--session`
   paths
2. export a redacted conversation bundle and then package that bundle into the
   normal `listing-template.json` / `grant-template.json` artifacts

## Source format

The current Codex rollout store is append-only JSONL under:

- `~/.codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<thread-id>.jsonl`

The exported bundle intentionally keeps only the useful conversation structure:

- session summary
- selected turn-context posture
- user messages
- assistant messages
- tool calls and tool results unless `--drop-tool-io`

It drops or compresses noisy runtime detail such as the lightweight event
stream.

## Default redaction posture

By default the packager:

- drops developer-role prompt material
- drops `session_meta.payload.base_instructions`
- redacts local paths into stable placeholders
- redacts emails, bearer tokens, API keys, JWTs, GitHub tokens, Nostr secrets,
  Nostr public identities, and Lightning invoices
- redacts URL paths/query strings while preserving the host
- writes a redacted per-session JSON export plus a `conversation-index.json`

The default is `--redaction-tier public`.

## Important switches

- `--session <file-or-dir>`
  - package explicit rollout files or directories
- `--session-root ~/.codex/sessions`
  - alternative root when not using explicit session paths
- `--limit N`
  - export the most recent `N` rollout files from the selected root
- `--drop-tool-io`
  - export only user/assistant messages
- `--include-developer`
  - keep developer-role messages after redaction; default behavior drops them
- `--scrub "literal"`
  - replace a project-specific or user-specific literal everywhere in the
    export with a stable placeholder
- standard listing/grant knobs such as:
  - `--price-sats`
  - `--consumer-id`
  - `--grant-policy-template`
  - `--grant-expires-hours`

The current local username is scrubbed automatically. Use additional `--scrub`
flags for repo names, company names, customer names, or any other literal that
appears in the conversation text outside the default secret/path detectors.

## Emitted files

In the chosen `--output-dir`, the packager writes:

- `redacted-codex-conversations/`
- `redacted-codex-conversations/conversation-index.json`
- `listing-template.json`
- `grant-template.json` unless `--skip-grant-template`
- `packaging-manifest.json`
- `packaging-summary.json`

The listing/grant metadata is automatically tagged with Codex export details so
the seller draft clearly records that the bundle came from redacted Codex
sessions.
