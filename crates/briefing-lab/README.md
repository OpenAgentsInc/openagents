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
No warm-latency claim is made until measured.

The index attempts at most 10,000 source files, 512 KiB per file, and 64 MiB
of source bytes in total. It counts bytes before checking UTF-8 contents.
It caches terms, declaration hints, sizes, and hashes, without full source
text. Preview fetches only selected blobs and verifies their path bindings to
the commit and content digests. Excerpts retain exact source bytes, including
CRLF and terminal newlines. The index prioritizes instructions, manifests,
and Rust source. It excludes generated lockfiles, vendor directories, traces, transcripts, fixtures, binaries, and
symlinks, and reports omitted counts. Preview selects at most eight ranked
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
