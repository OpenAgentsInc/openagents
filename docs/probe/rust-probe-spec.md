# Probe Rust Rebuild Specification (`probe-rs`)

Date: 2026-07-30

## Purpose

Rebuild Probe as `probe-rs`: a native Rust crate and executable for fast,
low-latency repository investigation and bounded coding work. `probe-rs` is
the Probe implementation that Omega delegates to over ACP. It aligns with the
native architecture of Omega, especially `crates/agent`, while remaining a
separate executable with a clear ACP boundary.

The rebuild replaces the Probe runtime implementation; it does not require
Omega Agent to give up ownership of the interactive conversation, permissions,
thread lifecycle, or user-facing review flow.

## Architecture

`probe-rs` should be organized as a Rust library crate with a thin binary that
starts an ACP session. The library owns workspace policy, repository traversal,
search, structural extraction, tool dispatch, evidence collection, and result
serialization. The binary owns process startup, ACP transport, cancellation,
and streaming.

The implementation uses native primitives:

- Rust `regex` and `ignore` crates, or reusable `ripgrep` primitives where
  they provide the required filtering and search behavior, for fast literal
  and regular-expression search.
- Tree-sitter parsers for structural indexing, outlines, symbol maps, and
  exact source ranges. Index state is optional and revision-aware; structural
  output must identify unsupported or partial language coverage.
- Native async Rust streaming for ACP progress, tool activity, evidence, and
  final results. Cancellation must stop in-flight traversal and indexing work
  promptly.

The first version should prefer deterministic, bounded repository operations
over a broad agent host. It must not introduce a terminal executor, browser,
or unbounded provider-specific tool surface merely to match Omega.

## Operating modes

Probe has two explicit operating modes.

| Mode | Default | Authority | Intended use |
| --- | --- | --- | --- |
| `read_only` | Yes | Read and enumerate only. File mutation tools are unavailable. | Safe exploration, search, repository mapping, and deep analysis without side effects. |
| `read_write` | No | Includes the read-only tools plus narrowly scoped mutation tools. | Targeted patches, probes, and automated trial edits. |

Mode is part of the ACP request metadata and is enforced by the Probe tool
registry, not just suggested in a prompt. `read_write` must remain bounded to
the assigned workspace and must report every mutation as evidence. Omega keeps
the final permission and review authority; receiving `read_write` mode is not
permission to bypass it.

## Minimal toolset

The initial toolset is intentionally small. Tool output is bounded and every
result identifies the workspace-relative path and exact source range when one
exists.

| Tool | Behavior | Modes |
| --- | --- | --- |
| `grep` / `rg` | Fast literal or regular-expression search with ignore-aware file filtering. | `read_only`, `read_write` |
| `outline` / `symbol_map` | Tree-sitter structural AST and symbol-tree extraction with source ranges and partial-coverage status. | `read_only`, `read_write` |
| `file_tree` / `find_paths` | Hierarchical workspace traversal and path discovery subject to allowlists and ignore policy. | `read_only`, `read_write` |
| `read_range` | Line-bounded file reading with requested context padding and byte/output caps. | `read_only`, `read_write` |
| `apply_patch` / `write` | Scoped patch or file-write operations that return changed ranges and content digests. Disabled at registration time in read-only mode. | `read_write` only |

Aliases may map to one implementation, but the ACP-visible names above remain
stable so Omega can request the capability it intends. `apply_patch` should be
preferred for edits; `write` is for explicitly requested whole-file or
generated-file replacement.

## Delegation controls

Omega's delegation interface must make Probe execution parameterized rather
than fixed to a single model profile. A delegation request must be able to
select:

- `model`, such as `"gpt-5.6-terra"`.
- `reasoning_effort`: `low`, `medium`, `high`, or `extreme`.
- `mode`: `read_only` or `read_write`.

Omega validates these values against its available model/provider and
permission policy before invoking Probe. Probe records the accepted values in
its session metadata and result, but does not silently substitute a different
model, reasoning level, or mode. If a requested value is unavailable or
denied, the ACP session must return a structured error before tool execution.

## ACP extension

The existing ACP request envelope carries a versioned Probe-specific metadata
object. This avoids creating an out-of-band configuration channel while
allowing new fields to be negotiated safely.

```json
{
  "probe": {
    "version": 1,
    "model": "gpt-5.6-terra",
    "reasoning_effort": "high",
    "mode": "read_only",
    "workspace_root": ".",
    "repository_revision": "<commit-or-worktree-generation>",
    "allowed_paths": ["crates/agent"],
    "output_budget": { "max_bytes": 131072, "max_files": 200 }
  }
}
```

`model`, `reasoning_effort`, and `mode` are required for Probe-initiated
delegations after the extension is adopted. `version` is required and permits
future additive changes. The metadata must not carry bearer credentials,
unbounded sandbox grants, or authority that Omega has not already approved.

Probe streams typed ACP events for session acceptance, tool start/finish,
evidence, diagnostics, and completion. The completion metadata echoes the
accepted model, reasoning effort, mode, repository revision, structural-index
freshness, and any partial or omitted results. Unknown metadata versions or
invalid enum values fail closed with a structured protocol error.

## Delivery plan

1. Create the `probe-rs` crate and ACP binary with `read_only` mode,
   workspace containment, `grep`, `file_tree`, `find_paths`, and `read_range`.
2. Add tree-sitter-backed `outline` and `symbol_map`, with revision-aware
   incremental indexing only where measurement shows it improves investigations.
3. Add the parameterized Omega delegation interface and ACP metadata extension
   for model, reasoning effort, and mode; validate policy at both boundaries.
4. Enable `read_write` only after read-only evidence, cancellation, and
   containment are reliable. Add `apply_patch` and `write` behind that mode
   with mutation evidence and Omega review.

## Acceptance criteria

- The default Probe session cannot mutate the worktree.
- Omega can select model, reasoning effort, and operating mode for each Probe
  delegation, and the accepted values are visible in ACP completion metadata.
- Search and traversal honor workspace containment, allowlisted paths, and
  ignore policy.
- Structural results include exact ranges and clearly report partial or stale
  index coverage.
- ACP cancellation interrupts search, indexing, and streaming without leaving
  an active mutation operation behind.
