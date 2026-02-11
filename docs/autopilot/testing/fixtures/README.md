# Autopilot Stream Fixtures

These files are **golden “wire transcripts”** for Autopilot chat streaming.

They are intended to be used by:

- `apps/web` worker tests (validate what we write into Convex `messageParts`)
- `apps/web` storybook stories (validate what the UI renders from those parts)
- visual regression (`docs/STORYBOOK.md`) once we add a story that renders a transcript

## Format

Each fixture is **JSONL** (one JSON object per line).

Each line is a **single `messageParts` record** as it would appear on the UI wire:

```json
{ "seq": 12, "part": { "type": "text-delta", "id": "t1", "delta": "hello" } }
```

Notes:

- `seq` MUST be an integer and MUST be monotonic (Convex floors `seq` and de-dupes by `(runId, seq)`).
- `part` is a union:
  - `@effect/ai` stream parts (`text-*`, `finish`, `tool-call`, `tool-result`, etc.)
  - custom DSE action parts (`type: "dse.*"`) as specified in `docs/autopilot/runbooks/SELF_IMPROVE_PLAN.md` (“DSE chat-part schema”)

## Viewing Raw Data

Pretty print:

```bash
cat docs/autopilot/testing/fixtures/<file>.jsonl | jq .
```

Just the `part.type` stream:

```bash
cat docs/autopilot/testing/fixtures/<file>.jsonl | jq -r '.part.type'
```

## Hygiene

- Do not put secrets, real tokens, or real user identifiers in fixtures.
- Keep payloads bounded. Large blobs should be referenced (e.g. `blobRef`) rather than inlined.
- Prefer stable timestamps (`tsMs`) and stable ids (`id`, `toolCallId`) so fixtures are deterministic.
