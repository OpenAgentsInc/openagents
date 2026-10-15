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

Prepare a bounded model request with `prepare-build --repo DIR --rev COMMIT
--workspace NAME --task TEXT --model NAME --anchors FILE --out DIR`.
The anchors JSON is an array of `{path,symbol,start,end}` objects; a symbol
selects its own source span. `build --repo DIR --input FILE --out DIR
--endpoint URL` uses an explicitly selected Chat Completions-compatible model
endpoint and `CODE_INDEX_MODEL_TOKEN` when authentication is needed.
`--responses FILE` consumes frozen typed responses instead of calling a model.
The teacher can request additional source within the call and byte budgets.

The saved `build.json` binds requests, responses, model names, prompt version,
source references, and the compiled artifact. `replay-build --repo DIR
--input FILE --out DIR` recompiles it without a model call and refuses changed
requests or artifact digests. Cost remains unknown when the provider does not
report it. Build records remain private; private trace context needs a separate
explicit disclosure choice.

`code-index query --repo DIR --index FILE --workspace NAME --question TEXT`
returns source locations, required roles, examples, checks, and a next step.
Use `--operation NAME` to narrow the operation and `--max-bytes N` to bound
the serialized context. The library also accepts supplied embedding vectors,
file-finder candidates, applicability answers, and a selection bound to the
question and artifact. Retrieval scores are not probabilities. Missing
conditions and required context remain explicit; a new revision or workspace
returns `stale` until the artifact is refreshed.

`bind --repo DIR --rev COMMIT --workspace NAME --input FILE --out DIR`
records source, manifest, directory-discovery, documentation-scope, and graph
impact dependencies in `bundle.json`. It accepts an artifact or a replayed
build; private build context keeps the bundle private. `refresh` with the
same arguments carries forward unchanged recipes and marks changed ones stale.
`query` accepts these bundles and checks dirty paths and graph dependents.

`install --repo DIR --workspace NAME --input FILE --store DIR` saves a
content-addressed bundle and activates it. Subsequent installs and `rollback`
require `--expected HASH`; rollback also takes `--digest HASH`. Stores isolate
repositories and workspaces, check object digests, and serialize writers.
`withdraw --repo DIR --workspace NAME --store DIR --source ID --expected HASH`
removes affected private derived objects, including old rollback targets.

`publish --repo DIR --rev COMMIT --workspace public [--remote origin]`
publishes the source-only pilot to the parentless `code-index` branch.
`fetch --repo DIR --workspace public --store DIR [--remote origin]` reads it,
checks its source dependencies, and installs it locally within a 20-second
fetch limit. No graph or semantic artifact belongs on `main`.

The existing graph publisher can install and run the semantic publisher with
`CODE_INDEX_SEMANTIC=1`. Set that value when running its installer and in the
service's `~/.config/openagents/code-graph.env`; the default is off. Existing
structural graph publication works independently. Model-quality promotion
requires the experiment gate, not source verification.

Coder exposes `code_lookup` through its optional **Code guide** plugin. It
starts off. Enable it in Plugins or with `OPENAGENTS_CODE_INDEX=1` after
installing an index. `CODE_INDEX_STORE` chooses the store base;
`CODE_INDEX_WORKSPACE` chooses its workspace (default `local`). Without an
override the store is under the repository's Git common directory in
`code-index/`. No tool is offered without a readable installed artifact.
Lookups record local hashes, status, coverage, and elapsed time in the store's
private observations directory; they record no question text or source output.

Run `cargo test -p code-index` and `cargo fmt -p code-index`. See the
[roadmap](../../docs/code-graph/task-index-roadmap.md) for experiment scope.
