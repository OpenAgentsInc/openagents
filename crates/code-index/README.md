# Task index

`code-index` keeps local investigation analysis and source-backed task
knowledge separate from the deterministic `code-graph` extractor.

Import explicitly selected history roots into a private directory:

```sh
cargo run -p code-index -- import --repo "$PWD" \
  --claude "$HOME/.claude" --codex "$HOME/.codex" \
  --out "$(openagents scratch)/investigations" \
  --before 2026-10-15T00:00:00Z
```

The importer reads through `coder-history`, includes all recognized tool
blocks, and retains unknown records and partial tails. Saved commands are
data and are never executed. Output can contain private conversation text;
keep it local. Standard output contains aggregate counts only.

Token observations retain the provider's accounting: Claude request updates
contribute only additional tokens; Codex cumulative usage contributes deltas.
Absent usage, base commits, parent links, and independent outcomes remain
unknown. Edit and shell-activity classifications are navigation heuristics.
A transcript's completion message is a claim, not a verified outcome.

`code-index demand --input DIR/episodes.json --out DIR` ranks local task
families. It writes a grouping request that can be reviewed locally; pass a
frozen response with `--grouping FILE`. Different operations stay separate,
related episodes must share a data partition, and delegated copies count
once per family. `--assumptions FILE` supplies future uses, avoidable fraction,
unit navigation cost, build cost, and refresh cost. Missing assumptions
produce no monetary estimate. Grouping and demand files remain private.

`code-index pilot --repo DIR --rev COMMIT --workspace NAME --out DIR`
compiles twenty source-reviewed extension recipes against committed Git
objects. It validates paths, symbol spans, full-file digests, new-file
integration points, and Cargo check packages. Dirty source cannot change the
frozen evidence. Recipes cover 3D assets and authoring, Coder plugins and
tools, CLI wiring, world wire/client/server changes, and agent history.
Source verification does not establish successful edits or model quality.

Run `cargo test -p code-index` and `cargo fmt -p code-index`. See the
[roadmap](../../docs/code-graph/task-index-roadmap.md) for experiment scope.
