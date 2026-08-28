# Knowledge base

The corpus of reviewed positions and public-doc summaries the coder harness
attaches to a turn, compiled into a WebAssembly plugin the CLI loads.

- `kb/stances.json` — the curated stances. Reviewed like content, edited like
  content.
- `build-kb.mjs` — optional harvest of the served docs site into compact
  entries; `kb.json` is the committed corpus. The script needs a local Node
  binary. This repository does not pin one.
- `src/lib.rs` — the scoring and selection the plugin runs at query time.
- `manifest.json` — the declared interface and the pinned artifact digest.

## Rebuilding the corpus

```sh
node build-kb.mjs
cargo build --manifest-path ../Cargo.toml --release \
  --target wasm32-unknown-unknown -p knowledge-base
```

Then re-pin `artifact.digest` in `manifest.json` to the digest of the rebuilt
`knowledge_base.wasm`. The plugin embeds `kb.json` at build, so a corpus edit
that is not followed by both steps ships nothing.

## Adding a stance

A stance record carries an `id` (lowercase words joined by hyphens, and the only
stable identifier a stance has), a `title`, the `questions` a reader would ask,
a `state`, the `answer`, the `sources` it rests on, and the `date` it was
reviewed. Scoring weighs `questions` and `title` heaviest, `state` next, and the
`answer` last, so write the questions as a reader would ask them.

## Promoting a system memory into a stance

The knowledge base owns what the project has reviewed and decided; system memory
owns what the network has observed and can evidence. When a system memory
stabilizes — admitted, unchallenged for a sustained period, repeatedly recalled
— a steward drains it into a stance here, and the memory row is superseded by a
**promotion tombstone** that names the stance. The claim then has exactly one
live home.

**Promotion is where the boundary between the two rails is enforced**, because
it is the only place either rail records that a memory and a stance are the same
claim. Recall does not compare them: the knowledge base is retrieved here in the
client, memory recall runs server-side inside `POST /api/v1/responses`, and
nothing sees both notes. Do not add a client-side or request-side suppression
rule without reading the reasoning first.

Order matters — add the stance before draining the memory, or the tombstone
points at nothing:

1. Add the stance to `kb/stances.json`, citing the memory among its `sources`.
2. Rebuild and re-pin, as above. Land the change through review.
3. Drain the memory with `OpenAgents.Memories.Promotions.promote/3` in
   `OpenAgentsInc/openagents.com`.

The full procedure, the refusals, and the reasoning behind the enforcement point
are in `docs/memory/knowledge-base-boundary.md` in that repository. The contract
is `docs/memory/2026-08-25-system-memory-spec.md` section 8 here; the invariant
is MEMORY-012 there.

## Tests

`src/tests.rs` runs the ranking against the shipped `kb.json`. Native Coder's
plugin tests load the digest-pinned artifact through the same sandboxed host it
uses in a session. Both checks pin behavior by stance title, so renaming a
title is a test change.
