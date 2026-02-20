# Autopilot Stream Testing (Wire Transcripts)

- **Status:** Implemented (v1 fixtures + renderer tests; more E2E-style transcript tests can be added)
- **Last updated:** 2026-02-10
- **Scope:** Legacy `apps/web` Khala-first MVP streaming + future DSE action parts
- **If this doc conflicts with code behavior:** code wins

This document defines a test posture where you can run a local test and get:

1. A deterministic **wire transcript** of what the Worker appends to Khala `messageParts`
2. The same transcript rendered by the **same UI code path** used in `/autopilot`
3. A raw JSON dump of every streamed event (text deltas, tool calls, DSE signature/tool/compile/promote events)

## What We Have Today (Reality Check)

Autopilot MVP streaming is already tested at the Worker/Khala boundary. We also have deterministic UI rendering tests
that consume the same wire transcript fixtures the docs describe.

- Worker streaming test (writes chunked parts, cancel behavior):
  - `apps/web/tests/worker/chat-streaming-khala.test.ts`
- Khala canonical-state tests (idempotent parts, transcript persistence):
  - `apps/web/tests/khala/autopilot-mvp.test.ts`
- Wire transcript UI rendering (fixtures -> `messageParts` -> chat template):
  - `apps/web/tests/worker/dse-chat-parts-rendering.test.ts`
- UI template determinism + visual regression harness:
  - Storybook/visual docs: `docs/STORYBOOK.md`
  - Effuse contract tests: `packages/effuse/docs/TESTING.md`

The current Worker chat call is `@effect/ai` streaming with tools disabled:

- `apps/web/src/effuse-host/autopilot.ts` uses `AiLanguageModel.streamText({ toolChoice: "none" })`

So the “Gmail review” example below is a **target transcript fixture** for the future tool-enabled/DSE-integrated path.

## Contract: Wire Transcript V1

Autopilot’s UI wire is Khala `messageParts` records (see `apps/web/khala/autopilot/messages.ts`).

Transcript rules:

- Each transcript event corresponds to a single `appendParts` item:
  - `{ seq: number, part: unknown }`
- `seq` MUST be monotonic integers (Khala floors and de-dupes by `(runId, seq)`).
- `part` is a union:
  - `@effect/ai` stream parts (`text-start`, `text-delta`, `text-end`, `finish`, and eventually `tool-call`/`tool-result`)
  - DSE action parts (`type: "dse.*"`) as specified in `docs/autopilot/runbooks/SELF_IMPROVE_PLAN.md` (“DSE chat-part schema”)

Goal: a transcript file should be usable by both:

- Worker-side tests (validate the Worker emits the right parts in the right order)
- UI-side tests/storybook (validate the UI consumes and renders those parts correctly)

## Fixtures (Raw JSON You Can Inspect)

Fixtures live under:

- `docs/autopilot/testing/fixtures/` (JSONL)

The two canonical fixtures for this contract are:

- “Review my recent gmail things” (tool loop + formatting):
  - `docs/autopilot/testing/fixtures/autopilot-gmail-review.stream.v1.jsonl`
- “Kitchen sink” (every DSE card type we must render):
  - `docs/autopilot/testing/fixtures/dse-kitchen-sink.stream.v1.jsonl`

View raw parts:

```bash
cat docs/autopilot/testing/fixtures/autopilot-gmail-review.stream.v1.jsonl | jq .
```

Just the stream of part types:

```bash
cat docs/autopilot/testing/fixtures/autopilot-gmail-review.stream.v1.jsonl | jq -r '.part.type'
```

## How We Should Test This (Aligned With Existing Testing Docs)

We already have three complementary testing surfaces in this repo:

1. **Worker tests (Vitest + Cloudflare pool)** validate what gets written to Khala (`messageParts` batching, cancellation, etc.).
2. **Khala impl tests** validate canonical transcript persistence and invariants (idempotency, access rules).
3. **Storybook + visual regression** validates UI rendering deterministically.

The missing piece is a single, repeatable “wire transcript test” that ties (1) and (3) together.

### A) Worker contract test: emits a known transcript and dumps it

Target: add a Worker test that runs `/api/autopilot/send` with a deterministic fake LM + deterministic fake tools, then asserts:

- the Worker appended `messageParts` that match a fixture transcript (or at least match a schema + expected type sequence)
- and, on request, prints the raw transcript JSON to stdout (or writes to `output/`)

This should live next to existing worker tests:

- `apps/web/tests/worker/`

### B) UI contract test: consumes a transcript exactly like prod

Target: add a UI-side test that:

- loads a fixture JSONL transcript
- feeds it through the same accumulator used by `/autopilot`:
  - `apps/web/src/effect/chat.ts` (`applyRemoteChunk`)
- and asserts the resulting `ChatMessage.parts` list includes:
  - text parts
  - tool cards
  - DSE signature/tool/compile/promote/budget cards

This stays aligned with `packages/effuse/docs/TESTING.md`: contract tests should assert **observable output**, not internal implementation details.

### C) Storybook story: render the transcript + show raw JSON in the canvas

Target: add an Effuse story that renders:

- the Autopilot chat UI as it would appear for a given transcript
- plus a raw JSON panel (or collapsible block) showing the underlying transcript lines

Then the existing visual suite (`docs/STORYBOOK.md`) can screenshot it, giving us a stable UI contract.

Status: implemented in `apps/web/src/storybook/stories/autopilot.ts`.

## Commands You Can Run Today

Even before DSE/tool-loop integration lands, these validate the current streaming plane:

```bash
cd apps/web
npm test -- chat-streaming-khala
```

```bash
cd apps/web
npm test -- autopilot-mvp
```

And to verify the UI rendering contract against the golden wire transcript fixtures:

```bash
cd apps/web
npm test -- dse-chat-parts-rendering
```

And for UI determinism/visual regression:

```bash
cd apps/web
npm run storybook
```

```bash
cd apps/web
npm run test:visual
```

## Notes On The “Gmail Review” Example

The “Gmail review” fixture is a **target behavior transcript**:

- It intentionally shows a full tool loop (connect, search, get threads, summarize, render).
- Tool names in the fixture are placeholders; when we implement Gmail for real we should align names/schemas with the canonical tool registry.
- No secrets are present; it is safe to commit.
