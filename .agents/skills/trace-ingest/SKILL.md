---
name: trace-ingest
description: Publish a local Claude, Codex, or OpenAgents conversation as a public OpenAgents trace at openagents.com/trace/{uuid}. Use when the user wants to take a conversation/session id on this machine, sanitize/redact it, ingest it via the OpenAgents API, and get a shareable /trace/{uuid} URL — repeatably and at scale.
---

# Local conversation → public /trace/{uuid}

This skill turns a conversation that already exists on this computer (a Claude
Code session, a Codex rollout, or an OpenAgents Desktop conversation) into a
**public** OpenAgents trace viewable at `https://openagents.com/trace/{uuid}`.

It is a thin CLI over machinery that already exists in `apps/qa-runner`:
the ATIF converters, the `@openagentsinc/atif` public-safety redactor +
tripwire, and the `POST /api/traces` publish transport. Read the audit
`docs/traces/2026-07-19-local-conversation-public-trace-ingest.md` for the full
architecture and the server contract.

## The one command

Run from the repo (uses `tsx`, no build/install step):

```sh
cd /Users/christopherdavid/work/openagents/apps/qa-runner

# 1) ALWAYS dry-run first — build + redact + validate locally, no upload:
pnpm trace:ingest <conversationId> --dry-run --out /tmp/trace.json --json

# 2) Publish (needs an agent token):
export OPENAGENTS_AGENT_TOKEN=oa_agent_...        # never print or commit this
pnpm trace:ingest <conversationId>                # visibility=public by default
```

Success prints the shareable URL:

```
published public trace from claude (303 steps):
  https://openagents.com/trace/1f4c…-…-…
```

`pnpm trace:ingest` = `node --import tsx src/ingest-conversation-cli.ts`.

## What a "conversation id" is

| Source             | id form                                   | where it lives                                                                             |
| ------------------ | ----------------------------------------- | ------------------------------------------------------------------------------------------ |
| Claude Code        | v4 UUID (the session id)                  | `~/.claude/projects/<slug>/<id>.jsonl`                                                     |
| Codex              | UUIDv7 (trailing in the rollout filename) | `~/.codex/sessions/YYYY/MM/DD/rollout-*-<id>.jsonl`                                        |
| OpenAgents Desktop | upper-case UUID                           | array element in `~/Library/Application Support/<Profile>/KhalaDesktop/conversations.json` |

The source is **auto-detected** (claude → codex → openagents). Force it with
`--source claude|codex|openagents` when an id could be ambiguous.

## Flags

- `-s, --source <kind>` — force the source (default `auto`).
- `-v, --visibility <public|unlisted|owner_only>` — stored visibility. The
  default is `public`. `unlisted` is link-only with no listing. `owner_only` is private.
- `--dry-run` — build + redact + validate, do **not** upload.
- `--out <file>` — with `--dry-run`, write the redacted ATIF JSON for inspection.
- `--max-steps <n>` — cap steps with a default and hard max of `2000`. It keeps
  a valid prefix and notes the truncation. Big coding sessions exceed the server cap.
  this keeps them ingestible instead of failing.
- `--agent-name <name>`, `--model <id>` — trajectory header overrides.
- `--base-url <url>` — ingest base (default `$OPENAGENTS_BASE_URL` or
  `https://openagents.com`).
- `--token <oa_agent_…>` — agent bearer (default `$OPENAGENTS_AGENT_TOKEN`).
- `--json` — machine-readable result (uuid, url, redaction counts).

## Safety model (do not weaken)

1. The CLI **deep-redacts** the trajectory with the same `@openagentsinc/atif`
   redactor the ingest API trusts (home paths, emails, tokens, keys, wallet
   material, IPs, phone numbers, long blobs, usernames …).
2. It then runs the **public-safety tripwire** locally and REFUSES to upload if
   anything leaky survives — you get finding codes, never a silent leak.
3. The **server redacts + tripwires again** and rejects (422) anything unsafe.
   A public trace is evidence only: it grants no accepted-work, payout, or
   public-claim authority.

Always `--dry-run --out` and eyeball the JSON before the first public publish of
a sensitive session. Never pass a token on the command line in shared history —
prefer `OPENAGENTS_AGENT_TOKEN` in the environment.

## Scaling / repeated use

- Publishes are **idempotent**: re-running the same conversation returns the
  same uuid (the key is a digest of the redacted trajectory).
- To burn down many at once, loop ids through `pnpm trace:ingest <id> --json`
  and collect the `url` field. Server-side rate limit is 120 traces/hour/owner.

## Troubleshooting

- `No local … conversation found for id` — wrong id or `--source`. Confirm the
  file exists (`ls ~/.claude/projects/**/<id>.jsonl`, `ls ~/.codex/sessions`).
- `Trajectory has no steps` — an aborted/empty session (common for tiny Codex
  rollouts). Nothing to publish.
- `still trips the public-safety tripwire after redaction` — inspect with
  `--dry-run --out` and remove the offending content upstream. Do not force it.
- `no agent token` — set `OPENAGENTS_AGENT_TOKEN` (see the repo Khala runbook).
- `Cannot create a string longer than …` — a pathologically large rollout
  (hundreds of MB). Use `--max-steps` or skip it.
