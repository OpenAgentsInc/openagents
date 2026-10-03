# Briefing lab

This experiment builds a read-only issue briefing from a pinned Git commit.
It does not call a model, run commands from an issue, modify inspected source,
claim an issue, or change production routing.

Build on the approved build host with `cargo build -p briefing-lab --release`.
Set `BRIEFING_LAB_BIN` to the resulting binary. The wrapper never runs Cargo.
Keep output outside the repository:

```sh
export BRIEFING_LAB_BIN=/path/to/briefing-lab
scripts/briefing-preview.sh --repo /path/to/openagents --rev COMMIT \
  --issue https://github.com/OpenAgentsInc/openagents/issues/10253 \
  --output-dir /tmp/briefing-example
```

An issue number needs `--github-repo OpenAgentsInc/openagents`. To avoid a
network request, replace `--issue` with `--issue-file /path/to/issue.json`.
The JSON uses `gh issue view --json number,title,body,url` fields; only
`title` is required. The full title and body survive in both output formats.
Issue comments are outside this initial prototype.

The first invocation builds a bounded index. Reuse it for a warm preview:

```sh
scripts/briefing-preview.sh --repo /path/to/openagents --rev COMMIT \
  --index /tmp/briefing-example/index.json --issue-file /tmp/issue.json \
  --output-dir /tmp/briefing-second
```

The binary also exposes separate `index` and `preview` commands; run
`briefing-lab --help`. A preview refuses an index for a different revision.
It reads committed source only, so dirty and untracked files are absent.
`--no-lexical`, `--no-symbols`, and `--no-history` isolate selection components.
Explicit issue paths and ancestor instructions/manifests remain eligible.

## Evidence and bounds

`briefing.json` records the source commit, Git blob, SHA-256 digests, line
ranges, selection reasons, selected recent commit subjects, and omissions.
`briefing.md` presents the same evidence for review. Timings distinguish index
construction, cache loading, revision validation, assembly, and serialization.
The CLI also prints complete preview time including output writes. Build and
GitHub fetch time are separate; the wrapper prints measured fetch wall time.
Warm timings and coverage limits are recorded in the
[syntax experiment](../../docs/audits/2026-10-03-independent-efficiency/briefing-syntax-results.md).

The index attempts at most 10,000 source files, 512 KiB per file, and 64 MiB
of source bytes in total. It counts bytes before checking UTF-8 contents.
It caches terms, declaration hints, sizes, and hashes, without full source
text. Preview fetches only selected blobs and verifies their path bindings to
the commit and content digests. Excerpts retain exact source bytes, including
CRLF and terminal newlines. The index prioritizes instructions, manifests,
and Rust source. It excludes generated lockfiles, vendor directories, traces,
transcripts, fixtures, binaries, and symlinks, and reports omitted counts. Preview selects at most eight ranked
files plus ancestor context, up to 14 excerpts of 64 lines each. History uses
up to 32 recent subjects and selects up to four. Lexical order breaks ties.

Symbol hints identify Rust declaration names on source lines. They are not
AST ranges, references, or a call graph. Lexical overlap and history subjects
are candidates, not verified relevance. A no-match result is explicit. The
briefing cannot certify requirement coverage, nominate a safe test command,
or replace complete repository instructions. Tests and manifests can be
selected as source evidence; the tool never runs them.

Run focused checks on the approved build host:

```sh
cargo test -p briefing-lab
cargo fmt -p briefing-lab -- --check
bash -n scripts/briefing-preview.sh
```

## Opt-in Rust syntax treatment

Build a separate cache with `index --syntax`, then select it with
`preview --syntax`. The wrapper's `--syntax` forwards to both commands.
An existing baseline cache cannot serve the syntax treatment; rebuild it
with the flag. Without `--syntax`, ranking and excerpts retain baseline
behavior, including when reading a syntax-enriched cache.

```sh
"$BRIEFING_LAB_BIN" index --repo /path/to/openagents --rev COMMIT \
  --output /tmp/briefing-syntax-index.json --syntax
"$BRIEFING_LAB_BIN" preview --repo /path/to/openagents --rev COMMIT \
  --index /tmp/briefing-syntax-index.json --issue-file /tmp/issue.json \
  --output-dir /tmp/briefing-syntax --syntax
```

The treatment changes excerpt selection only. It keeps baseline file
scores, the candidate pool, history, and the maximum of 64 lines per
excerpt. Case-sensitive declaration identifiers or complete qualified names
in the issue select structural declarations within those files. It does not
split `repair_cache` into the words `repair` and `cache`. Equal matches use
source order and are labeled ambiguous. Without a complete matching Rust
declaration, the preview labels its fallback to baseline selection.

The cache records Tree-sitter `0.27.0`, the Rust grammar `0.24.2`, and extractor
`briefing-lab-rust-v1`. Declarations include functions, impl methods, traits,
modules, types, constants, and macro definitions, with declaration, signature,
and optional body spans. Byte ranges are half-open; line ranges are inclusive
and one-based. The selected excerpt retains exact whole source lines and marks
a declaration partial when the 64-line limit clips it. These APIs follow the
[Tree-sitter node contract](https://tree-sitter.github.io/tree-sitter/using-parsers/2-basic-parsing.html)
and [Rust grammar binding](https://docs.rs/tree-sitter-rust/0.24.2/tree_sitter_rust/).

Parse errors and missing nodes are recorded. Declarations containing recovered
errors are not selected structurally; complete declarations elsewhere in the
file remain eligible. Extraction retains at most 1,024 declarations and visits
at most 100,000 syntax nodes per file, with omissions recorded. Names describe
lexical scopes, not resolved compiler identities. This treatment does not
expand macros, evaluate `cfg`, resolve imports/types, or build a call graph.
Separate attributes and comments are outside declaration spans. The parser
can improve boundaries without establishing that the selected declaration
answers the issue.
