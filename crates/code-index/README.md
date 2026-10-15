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

Run `cargo test -p code-index` and `cargo fmt -p code-index`. See the
[roadmap](../../docs/code-graph/task-index-roadmap.md) for experiment scope.
