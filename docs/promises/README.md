# Product promises

A product promise is one thing we say OpenAgents does, with the proof that
it does. The registry is
[`crates/openagents-web/content/promises.toml`](../../crates/openagents-web/content/promises.toml).
The website renders it:

- [openagents.com/promises](https://openagents.com/promises): what works
  today, each line with the one link or command to try it and its proof.
- [openagents.com/roadmap](https://openagents.com/roadmap): what comes next
  and later, each line linked to its GitHub issue.
- Both have Markdown twins (`/promises.md`, `/roadmap.md`) for agents, and
  both are in `sitemap.xml` and `llms.txt`.

The code is [`crates/openagents-web/src/promises.rs`](../../crates/openagents-web/src/promises.rs).

## A promise

| Field | What it holds |
| --- | --- |
| `id` | A stable name, never reused. |
| `group` | The heading it sits under. |
| `statement` | One short sentence a user understands. No internal names. |
| `surfaces` | `web`, `terminal`, `mobile`, `iphone`, `android`, `desktop`, `api`, `network`, `verse`. |
| `status` | `shipped` (works today), `launching` (new in today's release), `partial` (works in part), `next`, `later`, or `dropped` (on purpose). The Kitchen Sink ledger's `live` reads as `shipped` and `missing` as `later` (#11125). |
| `link` or `command` | The one place to try it. Required when it works (`shipped`, `launching`, `partial`). |
| `issues` | The GitHub issues behind it. |
| `episodes` | The show episodes that promised it. |
| `posts` | Posts on X that promised it, for a promise no episode made. `next`, `later`, and `dropped` need an issue, an episode, or a post. |
| `note` | One short line: what part works, or why it was dropped (required for `dropped`). |
| `evidence` | What proves it. At least one when it works. |

Evidence is one of:

- `{ test = "crates/x/src/y.rs::test_fn" }`: a test function in that file.
- `{ smoke = "check name" }`: a check in the web smoke suite,
  [`scripts/smoke/web.py`](../../scripts/smoke/web.py), run by
  `scripts/smoke/staging.sh`.
- `{ golden = "flow.id" }`: a web chat golden in
  [`bench/web-chat/goldens-v1.json`](../../bench/web-chat/goldens-v1.json).
- `{ doc = "path" }`: a document in this repository.

## When an issue ships

In the same change that ships it:

1. Find the promise that names the issue in `content/promises.toml`, or add
   one.
2. Set its `status` to `shipped` (or `launching` on a release day), give it
   the `link` or `command` to try it, and name its `evidence`.
3. Run `cargo test -p openagents-web --lib promises`.

After a release day, move every `launching` promise to `shipped`.

`every_working_promise_names_evidence_that_exists` fails when a working
promise names a test, smoke check, golden, or document that is gone. When
that happens, fix the promise: point it at the new evidence, or move it
back to `next` with its issue. Never delete the check to make the test pass.
Other tests keep each statement one plain sentence and keep both pages
rendered from the registry.

## The Kitchen Sink ledger

Project Kitchen Sink (#11125, `docs/kitchen-sink/`) compresses every
promise from the show's episodes into one OpenAgents 1.0. The registry was
imported from its [ledger](../kitchen-sink/ledger.md) at `c46aee749a`, and
keeps the ledger's ids (A1 to M7, X1 to X16; W1 to W5 are this registry's
own additions). The import followed the ledger's own status table:

- Live and Launching rows became `shipped` and `launching`, each with the
  evidence that proves it and the link or command to try it. C4 (edit queued
  messages) stays `next` while #11121 is open. G6 (this page) is `launching`.
- Partial and Missing rows aren't in front of users yet. `next` holds only
  what's planned for the next two weeks: a row in the spec's V1 core or V1
  complete, or one with an open issue on the V1 board (project 22). Every
  other row is `later` (owner, 2026-10-09). Use `partial` only for something
  people can use today, in part, with evidence for that part.
- Dropped rows became `dropped`, with the ledger's reason as the `note`.
- Surfaces: W `web`, T `terminal`, M `mobile`, D `desktop`, V `verse`,
  N `network`, and "all" is web, terminal, mobile, and desktop.

From now on the registry is the source of truth. When the ledger changes,
change the matching row here in the same change.

## History

The first product promises shipped in June 2026, in
[Episode 234](../transcripts/234.md): "Product Promises". The idea: not
everything said on the show had reached production reliably, so make
"what's actually live" something an agent can check.

- **Registry.** A TypeScript module in the Cloudflare Worker,
  `apps/openagents.com/workers/api/src/product-promises.ts` (schema
  `openagents.product_promises.v1`), about 150 records by its last version,
  `2026-08-27.1`. Each record had a `promiseId` (like
  `repo.open_source_code_map.v1`), a product area, an audience, a `state`,
  the `claim`, `safeCopy` and `unsafeCopy`, `evidenceRefs`, `blockerRefs`, a
  `verification` note, and an authority boundary.
- **States.** `green` (live with current evidence), `yellow` (partly live,
  gated, or needing caveats), `red` (blocked from public copy), `degraded`,
  `planned`, and `withdrawn`.
- **Where it was served.** The page at `openagents.com/promises` (later
  `openagents.com/docs/product-promises`) and the JSON at
  `openagents.com/api/public/product-promises`. Mismatches were reported in
  the Product Promises forum, and concrete bugs through a strict GitHub issue
  form.
- **Process.** Claims were added from transcript and launch audits under
  `docs/promises/`. A state changed only through an owner-authorized
  registry version, recorded as a promise transition receipt (a D1 table),
  and ADR 0007 required public product copy to pass through a promise first.
- **Why it went.** The registry grew to cover planned training, payout, and
  market claims more than working product, and most records were `planned`
  or `withdrawn`. The TypeScript Worker and its promise routes were deleted
  with the TypeScript product roots on 2026-08-28 (`d613b8ea22`), and the old
  `docs/promises/` audits went with the repository reset on 2026-09-18
  (`dabc08102f`). The full record is in Git history.

This version keeps the idea and drops the weight: four plain statuses
instead of six colors, one sentence per promise, and evidence that a test
checks on every run instead of a human-maintained list of references.
