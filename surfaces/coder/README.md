# The coder's staged text surfaces

The optimizable text the coder harness carries into every turn, held as data
rather than as string literals: the system prompt, the tool descriptions, and
the capability catalog lines. Staged so a change to any of it is a diff over an
artifact with a content digest, rather than an edit inside three source files
that no run can afterwards name.

The shape is the `knowledge-base` plugin's, deliberately: a corpus file, a build
step, and a pinned digest, with the build refusing a stale pin.

## The files

| File | What it holds |
| --- | --- |
| `system-prompt.v1.json` | The instructions, the concision sentence, the no-tools and tool-list sentences, and the lane notices |
| `tool-descriptions.v1.json` | Each declared tool's own description, plus the per-model-family emphasis overrides |
| `catalog-lines.v1.json` | Each installed plugin's catalog line, keyed by plugin id |
| `index.json` | Every surface's schema id, key count, and `sha256` content digest |

Each artifact is a flat `text` map of key to string, so a one-sentence change is
a one-line diff. A `{placeholder}` in a value is substituted by the consumer —
`{cwd}`, `{count}`, `{lane}`, `{catalog}`, and so on.

A key containing `rust`, or beginning `coder_lite`, names native Coder text.
A key with no consumer segment is shared data that native Coder also reads.

## Rebuilding

```sh
cargo run -p openagents-cli --bin coder-surfaces -- --write
```

That re-pins `index.json` and regenerates the module the native CLI compiles:

- `crates/openagents-cli/src/surfaces.rs` — read by the current Rust CLI

Do not edit the generated module by hand. It is build output.

```sh
cargo run -p openagents-cli --bin coder-surfaces
cargo test -p openagents-cli --test coder_surfaces_embed
```

refuses a tree where an artifact and its build disagree. The embed test is
part of `cargo test --workspace`. A surface edited without the rebuild is the
knowledge base's failure mode — the edit ships nothing and says nothing while
it does — so it is a named check here rather than a silence.

## The catalog lines are a mirror, not a move

`system-prompt` and `tool-descriptions` were string literals in the former
Coder implementations. Staging them moved the text; the artifact is now where
it is edited.

A plugin's catalog line was never a literal. It is the top-level `description`
of `plugins/<id>/manifest.json`, discovered at runtime by `discover_catalog()`.
The manifest stays where that text is edited, and `catalog-lines.v1.json`
mirrors it into one diffable object with a digest so an optimizer has a single
file to diff and a bench row has a single digest to record. The check fails when
a manifest and the mirror disagree.

## What reads the digests

A `--plain` session announces the staged text it composed from, on stderr,
beside the thread announcement:

```
[oa:surfaces system-prompt=sha256:…,tool-descriptions=sha256:…,catalog-lines=sha256:…]
```

`packages/coder-effectiveness` reads that line out of a trial's `coder.txt`,
folds the digests into the run digest, and records them on the
`openagents.bench_result.v3` row. So two runs that differ only in the prompt are
different runs, a row names exactly which text produced it, and
`pnpm run effectiveness:compare` says "staged text also varies" instead of
letting a text change read as noise.

The pin is read from the trial rather than from this directory at scoring time,
because the repository a week later is not the repository the run happened on.

## Changing the text

Do not edit a sentence here because it reads better. The text is a measured
surface: `docs/coder/runbook.md` owns the cycle, `docs/coder/best-practices.md`
owns the ledger, and `docs/coder/candidate-format.md` owns the object a proposed
change travels as. A wording change belongs in a hillclimb cycle with its own
measured delta.

Issue: OpenAgentsInc/openagents#122.
