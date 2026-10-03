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
briefing cannot certify requirement coverage, establish a sufficient test
suite, or replace complete repository instructions. Tests and manifests can be
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

## Opt-in focused evidence pack

Add `--focused` to `preview` or the wrapper to write `focused.md`, an optional
source payload bounded to 16 KiB of UTF-8 text, including headings and
provenance. Use this file when comparing equal optional-context budgets.
`briefing.md` and `briefing.json` also retain the complete issue, coverage
records, and timings; their total size is outside that budget.

Explicit repository paths and line anchors (`path.rs:42`, `path.rs:42-48`, or
`path.rs#L42-L48`) rank before lexical candidates. The pack retains complete
files up to 6 KiB. For larger Rust files, it selects the complete function
containing an explicit anchor or a declaration candidate matching issue
identifiers or terms. A function that exceeds the remaining budget is omitted
without clipping. Invalid anchors and anchors outside a complete Rust function
produce coverage warnings. Other large text files use labeled partial excerpts.

Named documents, nearby tests, and package manifests carry separate roles.
Nearby fixture paths are reported as candidates; their contents remain excluded
from the index. The pack reads at most 24 candidate files and includes at most
16 excerpts. History is omitted. A compatible syntax cache avoids reparsing;
a regular index parses selected committed Rust blobs during assembly. Both
paths verify selected source against the pinned commit and record preparation
time. The default preview remains unchanged.

The optional pack omits `AGENTS.md`, `CLAUDE.md`, and `SKILL.md`. Supply complete
applicable instructions separately and identically to both experiment arms.
The pack does not replace instructions or establish requirement coverage.

## Explicit structural policy

`--explicit-structure` selects `ExplicitStructureV1` and writes its exact
payload to `focused.md`. Choose this flag or `--focused`; the existing focused
policy remains unchanged. This variant reads task-named files and test
filenames under verified package manifests. Only explicit Rust and Cargo paths
introduce automatic test scopes; named documents do not. A committed manifest
must declare a `[package]` table, so virtual workspace roots grant no test scope.
Manifest inspection counts toward the 24-file read bound. The policy does not
search unrelated source files for lexical matches. Inline tests are eligible when their source file is
already named; inline tests elsewhere are outside this file pool.

Explicit anchors select complete Rust declarations with attached attributes and
comments. Without an anchor, identifiers and terms rank declarations within the
named file. Supported same-file calls and type names add dependency candidates.
The traversal stops at 64 declarations, handles cycles, and reports ambiguous
or unsupported links. Imports provide context but do not expand imported code.
This is syntax evidence, not Rust name resolution or a complete call graph.

The policy automatically selects one actual `#[test]` or namespaced test
function. Explicit anchors can select additional tests. Syntactic
module/function references rank first, followed by a labeled term fallback
within the admitted test files. Same-file fixture helpers and local types travel
with that test. Named Markdown documents contribute at most two complete
sections. Anchors select the smallest containing section; term matching ranks
leaf sections. ATX and Setext headings are supported, and fenced headings are
skipped. The section reader is not a full Markdown parser.

The renderer merges adjacent ranges and records each file's digest once.
Complete declaration/dependency bundles are admitted together or omitted
without clipping. The final payload, including coverage warnings, stays within
16 KiB and 16 ranges. Repeated selection-limit omissions are summarized;
rendered warning details use at most 768 bytes plus a summary. Full bounded
coverage records remain in `briefing.json`. Reads remain capped at 24 files. Fresh Rust parsing is
charged to preparation time; this policy does not reuse the older syntax cache.
Complete instructions and the issue remain separate inputs. The lexical and
symbol flags also apply to this policy's ranking and fallback selection.

For example, use the default within-file ranking:

```sh
scripts/briefing-preview.sh --repo /path/to/openagents --rev COMMIT \
  --index /tmp/briefing-example/index.json --issue-file /tmp/issue.json \
  --output-dir /tmp/briefing-structure --explicit-structure
```

For a reproducible preparation, `bench/briefing-replay/prepare_structure.py`
requires a pinned binary SHA-256, a full source commit, an issue file, and an
existing index. It measures a fresh index build and a warm preview separately,
verifies the actual 16 KiB bound, and copies the exact payload to `treatment.md`.
It makes no model calls and appends no Git probe. Default lexical and symbol
ranking stays within the admitted files; disabling those flags also disables
the corresponding test and document fallbacks.

## Execution facts and prior attempts

The opt-in execution treatment adds committed Cargo manifest facts, local
prerequisite observations, and selected prior attempts. Choose the package
scope explicitly; the lab does not infer a complete acceptance suite.

```sh
scripts/briefing-preview.sh --repo /path/to/openagents --rev COMMIT \
  --index /tmp/briefing-example/index.json --issue-file /tmp/issue.json \
  --output-dir /tmp/briefing-execution \
  --execution --manifest crates/openagents-mobile/Cargo.toml \
  --environment-id build-slot-3
```

Repeat `--manifest` for more packages, up to 32. The lab reads regular
committed manifest blobs, bounds each to 512 KiB, and proposes structured
argument arrays such as:

```json
["cargo", "test", "--manifest-path", "./crates/openagents-mobile/Cargo.toml"]
```

This works with the mobile package's separate workspace. Workspace declarations
and exact member entries retain blob and content digests. Glob membership,
dependency-based automatic membership, and workspace roots outside the snapshot
remain unresolved. The tool does not run Cargo metadata, expand build scripts,
or evaluate features. Workspace defaults can still affect which tests Cargo
runs. The proposed commands apply to a matching checkout; dirty and untracked
source is absent from the preview.

Prerequisite checks always inspect `cargo` and `git` on this process's `PATH`.
Add explicit requirements with repeated `--require-tool protoc` and
`--require-file /path/to/include/google/protobuf/timestamp.proto` options.
The lab checks executable-file presence and whether required files can be
opened. It does not execute tools or read required file contents. Missing
requirements are visible in the preview. They prevent run allocation.

The environment record includes its observation time and the exact metadata
used in its fingerprint: label, OS, architecture, required inputs, canonical
paths, sizes, modification times, and Unix modes. It describes the computer
running the preview. A label such as `boat` does not verify a remote host.
Metadata checks do not prove tool versions, complete dependencies, compatible
targets, or unchanged content. Recheck before execution.

### Reserve artifacts for one attempt

`prepare-run` creates a unique directory outside the repository, reserves
separate stdout/stderr logs, and writes a request plus preparation facts.
Concurrent requests with identical inputs receive distinct directories.

```sh
"$BRIEFING_LAB_BIN" prepare-run --repo /path/to/openagents --rev COMMIT \
  --issue-file /tmp/issue.json --manifest crates/openagents-mobile/Cargo.toml \
  --environment-id build-slot-3 --artifact-root /tmp/briefing-runs
```

The printed JSON provides `run_dir`, `stdout_log`, `stderr_log`, and
`request_sha256`. No process starts. A separate executor can use those logs
and report the actual exit code, captured before any output pipeline changes
the shell status. For a separately observed failure with exit code 101:

```sh
"$BRIEFING_LAB_BIN" record-result --run-dir PRINTED_RUN_DIR \
  --request-sha256 PRINTED_REQUEST_DIGEST --exit-code 101 \
  --summary 'The check failed; inspect the retained compiler diagnostic.' \
  --next-action 'Resolve the diagnostic before another check.'
```

Results are explicitly `caller_reported`. The lab does not verify what ran,
read logs, or turn an exit code into accepted issue completion. It refuses a
wrong request digest, an existing result, malformed or oversized records,
and symlinked or hard-linked run artifacts. Records are limited to 1 MiB;
summary and next-action text are each limited to 4 KiB. Digests bind records
without authenticating their author.

Pass `--attempt-dir PRINTED_RUN_DIR` to an execution preview, up to eight
directories. It compares source commit, complete parsed issue, observed
environment fingerprint, and proposed test command. Changes remain visible
as historical evidence; matching inputs do not establish that a check passes
now. Pending requests retain a null result. Logs that say “passed” cannot
override the supplied exit code because the reader never parses log text.
Attempt text is evidence, not a new instruction or permission.

Live claim admission, process supervision, runtime version checks, and
deployment stay outside this experiment. The source-only preview remains the
default. [Measurements and isolated checks](../../docs/audits/2026-10-03-independent-efficiency/conversation-briefing-audit.md#implemented-first-step)
record the added preparation cost and current limits.
