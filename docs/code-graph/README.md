# Code Graph

Code Graph is a deterministic map of this repository's Rust code that
agents query before they open source files (#11622). It answers structure
and impact questions, such as "who calls `X`" and "what breaks if `X`
changes", with a short list of `file:line` rows.

- Builder and queries: [`crates/code-graph`](../../crates/code-graph/)
- Coder plugin: [`crates/coder-new/src/code_graph_plugin.rs`](../../crates/coder-new/src/code_graph_plugin.rs)
- Publisher: [`deploy/code-graph/`](../../deploy/code-graph/)
- A/B harness: [`scripts/bench/code-graph-ab/ab.py`](../../scripts/bench/code-graph-ab/ab.py)
- A/B results: [ab-2026-10-14.md](ab-2026-10-14.md). With the tools, Coder
  used 63% fewer tokens to find every caller of a function, at the same
  correctness, and the same tokens as without them to find the files an
  issue's fix changes.
- Proposed next layer: [a task index built from agent navigation](task-index-roadmap.md),
  with a local Claude/Codex navigation census and a roadmap for source-backed
  extension recipes that cheaper agents can retrieve.
- Task-index [edit pilot](evaluation-2026-10-15.md): twelve accepted and
  independently replayed fixture edits, with no promotion or production
  quality claim.

## What the graph holds

**Nodes.** Modules (one per file, plus inline `mod` blocks), functions,
methods, structs, enums, unions, traits, impls, type aliases, consts,
statics, and `macro_rules!` macros. Each node has a qualified name
(`coder_new::plugin_tools::Family::of`), a file, its first and last lines,
whether it is public, its parent, and its signature up to the body.

**Edges.**

| Edge | Meaning |
| --- | --- |
| `calls` | A function or method calls a function, method, tuple-struct constructor, or macro. |
| `uses` | A module imports an item with `use`. |
| `impl` | An `impl` block implements a trait. |
| `impl_for` | An `impl` block is for a type. |
| `refs` | An item names a type, const, or static. |

**Scope.** Rust files only: no documentation, no `target/`, no vendored or
third-party code, and no generated files (a path with `generated` in it, or
`@generated` or `DO NOT EDIT` near the top). Files over 2 MB are skipped.

**Names are resolved from syntax, not by the compiler.** The builder
follows `use` imports (globs included), `crate`, `self`, `super`, `Self`,
and names defined in the same module. A method call on a value
(`x.load()`) has no type information, so it is matched by method name; a
name with more than three candidates gets no edge. An edge that was matched
by name is marked inexact, and answers show it with `?`. Calls inside macro
arguments (`assert!(check(x))`, `format!("{}", f(y))`) are found by a token
pattern. Trait-method dispatch, macro-generated items, and `#[path]`
modules are not modelled.

## The build is deterministic

- **No model runs in the build.**
- **Pinned tools.** `tree-sitter` `=0.27.0` and `tree-sitter-rust` `=0.24.2`,
  the same pins as `crates/briefing-lab`. The header records the builder
  (`code-graph extractor-1`), the parser, the grammar, and the commit.
- **Same input, same bytes.** Files, nodes, and edges are sorted before
  ids are given. A build from a commit reads Git objects (`git ls-tree`,
  `git cat-file --batch`), so the working tree does not affect it, and a
  clean working tree builds to the same bytes as its commit. The tests
  check both.

On `d0dc736500` (this repository, 2026-10-14), one build covers 7,191 files
(148 MB of Rust) and gives 197,475 nodes and 807,390 edges in about 5 s on
an M-series Mac, with 16 parser threads. The graph is 58 MB of JSON lines
and 8.9 MB compressed. Two builds gave the same SHA-256
(`7ea6279c…4c25ff`). A cached graph loads in under a second.

## File format

One JSON-lines file, `openagents.code-graph.v1`:

```text
{"schema":"openagents.code-graph.v1","builder":"code-graph extractor-1","parser":"tree-sitter 0.27.0","grammar":"tree-sitter-rust 0.24.2","commit":"…","dirty":false,"files":7191,"nodes":197475,"edges":807390}
["f",0,"crates/code-graph/src/build.rs"]
["n",12,"fn","build","code_graph::build::build",0,308,349,1,0,"pub fn build(source: &Source) -> Result<Built, String>"]
["e",12,40,"calls",1]
```

Nodes are `["n", id, kind, name, qualified name, file, line, end line,
public, parent (-1 for none), signature]`; edges are `["e", from, to,
kind, exact]`.

## One writer, off `main`

The graph is never committed to `main`. One publisher writes the
`code-graph` branch, whose single commit holds `code-graph.jsonl.gz`,
`COMMIT` (the `main` commit it was built from), and a short `README.md`.
Each publication force-pushes a new parentless commit, so the branch never
grows.

**Where the writer runs.** This repository allows no GitHub workflows or
GitHub-billed automation (`AGENTS.md`), so the rebuild after a push to
`main` is a user timer on one host: `deploy/code-graph/install.sh`
installs `code-graph` and `openagents-code-graph.timer`, which runs
`publish.sh` every five minutes. A pass fetches `main` and publishes only
when a Rust file or a `Cargo.toml` changed since the published commit (the
paths filter); otherwise it does nothing. `code-graph publish` writes the
branch with Git plumbing only (`hash-object`, `mktree`, `commit-tree`), so
it never touches a working tree.

**The host (since 2026-10-14).** The writer is the GCE VM
`oa-coder-worker-1` (project `openagentsgemini`, zone `us-central1-a`,
e2-standard-4, the always-on chat worker host), under its own Unix user
`oa-codegraph` (lingering, so the user timer runs without a login). That
user owns nothing of the chat workers and cannot read their env files.
Its checkout is `~/openagents` (a blobless clone), the publisher's bare
clone `~/.cache/openagents/code-graph/repo.git`, and its Rust toolchain
`~/.cargo` (rustup, stable).

**Credential.** One SSH deploy key with write access to
OpenAgentsInc/openagents only, titled `code-graph publisher
(oa-coder-worker-1, #11622)`; the private half is
`/home/oa-codegraph/.ssh/code_graph_deploy` (0600, on that host only),
selected by that user's `~/.ssh/config`. Deploy keys are scoped to one
repository but not to one branch, so the boundary is the host account:
nothing else on the VM uses the key, and the only push it makes is
`code-graph publish`'s force-push of `code-graph`. The worker's existing
credentials were not reused: the chat worker has no GitHub push
credential, and Solver's are per-requester OAuth tokens and the pool's
GitHub token, both broader than this job.

**Runbook.** With `H=(oa-coder-worker-1 --zone us-central1-a --project
openagentsgemini --tunnel-through-iap)` and the automation gcloud config:

```bash
# status, last runs
gcloud compute ssh "${H[@]}" --command 'sudo -iu oa-codegraph env XDG_RUNTIME_DIR=/run/user/1003 systemctl --user list-timers openagents-code-graph.timer --no-pager'
gcloud compute ssh "${H[@]}" --command 'sudo -iu oa-codegraph env XDG_RUNTIME_DIR=/run/user/1003 journalctl --user -u openagents-code-graph -n 40 --no-pager'
# upgrade the publisher to a newer main (or roll back: pass an older commit)
gcloud compute ssh "${H[@]}" --command 'sudo -iu oa-codegraph env XDG_RUNTIME_DIR=/run/user/1003 PATH=/home/oa-codegraph/.cargo/bin:/usr/bin:/bin ~/install.sh [COMMIT]'
```

`~/install.sh` is a copy of `deploy/code-graph/install.sh`; refresh it
from `origin/main` before an upgrade. **Rollback / turn off:** stop the
timer (`systemctl --user disable --now openagents-code-graph.timer` as
`oa-codegraph`) and delete the deploy key in the repository's
Settings > Deploy keys (or `gh api -X DELETE
repos/OpenAgentsInc/openagents/keys/166468997`). Readers keep working:
with no fresh published graph they build one locally. To restore the last
good branch, force-push its commit back to `code-graph`.

**Readers.** Coder's tools find a checkout's graph in this order:

1. the graph already loaded in the process for the same state;
2. the cached graph in `<git common dir>/code-graph/` (shared by every
   worktree of the repository; the newest four are kept);
3. with the code committed, the published graph, when no Rust file or
   manifest differs between its commit and `HEAD` (fetched with
   `git fetch --depth=1 origin code-graph`, within 20 s);
4. a local build: from `HEAD`'s objects when the code is committed, else
   from the working tree, cached by `HEAD` and the changed files'
   contents.

`CODE_GRAPH_NO_FETCH=1` turns off step 3; `CODE_GRAPH_REMOTE` names
another remote.

## The tools

Coder's built-in **Code Graph** plugin (on by default; the Plugins screen
turns it off) offers six tools in the core tool set, wherever the working
directory is in a Git checkout with a `Cargo.toml` at its top. They are
typed tools the model chooses; nothing routes on keywords.

| Tool | Arguments | Answer |
| --- | --- | --- |
| `graph_symbol` | `symbol`, `kind` | Definition `file:line-end`, signature, parent, caller and callee counts, a type's impls and their methods. |
| `graph_callers` | `symbol`, `limit` | Who calls, names, or imports it. |
| `graph_callees` | `symbol`, `limit` | What it calls and names. |
| `graph_impact` | `symbol`, `depth` (default 3), `limit`, `exact_only` | Transitive callers and dependents, nearest first, grouped by depth, with a count of files and crates. A type's dependents include its impls' methods. |
| `graph_path` | `from`, `to`, `max_depth` | The shortest chain of calls and references. |
| `graph_module` | `module`, `limit` | A module's public items with line numbers and signatures, and its submodules. |

A symbol can be a qualified name, a `::` suffix of one (`Family::of`), a
file path (its module), or `file.rs:LINE` (the innermost item there). An
ambiguous name returns up to 15 candidates to choose from. Answers never
include source.

Coder's guidance, sent with the tools, says to query the graph before
opening source for structure and impact questions, then read only the
lines it points to. `OPENAGENTS_CODE_GRAPH=0` or `1` overrides the plugin
setting (used by the A/B).

From a shell:

```sh
cargo run --release -p code-graph -- build --out /tmp/graph.jsonl.gz
cargo run --release -p code-graph -- query graph_callers '{"symbol":"Family::of"}'
cargo run --release -p code-graph -- publish --rev origin/main            # local branch only
cargo run --release -p code-graph -- publish --rev origin/main --remote origin   # the writer
```

## Module labels

Optional model-written names for top-level modules (Argon, a few calls,
cached by the graph's digest and kept apart from the graph) are not built
yet. The graph and its answers do not depend on them.

## Relation to file finding

[File finding](../inference/file-finding-bench.md) (#11210) maps an issue's
text to the files its fix needs. The graph answers structural questions
once a symbol is known. They are complementary: the finder narrows the
files, and the graph answers what calls what and what a change reaches.
