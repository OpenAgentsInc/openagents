# SCV: Space Construction Vehicle

## Documents in this directory

| Document | Type |
| --- | --- |
| This file | v0.1 design specification and the worker-to-repair-unit roadmap |
| [`2026-07-31-omega-agent-scv-encoded-practice-analysis.md`](./2026-07-31-omega-agent-scv-encoded-practice-analysis.md) | Strategic-technical analysis: which of the measured costs in the 2026-07-31 Codex overnight-spend audit a runtime we own can refuse mechanically, what SCV should become, and a build order |

Implementation note: the crate shipped as `crates/scv` in the `omega`
repository, binary `scv`, not as the standalone `scv-acp` crate this
specification describes. See §C.5 of the analysis above.

## Overview and vision

SCV ("Space Construction Vehicle") is a hyper-lightweight, standalone Rust
ACP agent. Its name is inspired by the StarCraft worker unit: a small,
dependable worker that starts with one focused job and can gain construction
and repair capabilities only when there is a demonstrated need.

Version 0.1 is deliberately read-only. SCV is an Agent Client Protocol (ACP)
server with exactly one tool, `read`. It reads a bounded range of text-file
lines and returns them with stable line numbers. It does not execute commands,
write files, search paths, list directories, access the network, invoke other
tools, or retain agent state. The narrow surface makes it useful as a
reference ACP worker, a test fixture, and a safe foundation for later work.

The `read` behavior is modeled after Codex's local file-read primitives:
bounded requests, explicit offsets and limits, deterministic EOF handling, and
line-numbered text suitable for precise follow-up requests. SCV specifies a
line-oriented presentation rather than copying Codex implementation code or
its byte-stream handle protocol.

## v0.1 contract

| Area | v0.1 behavior |
| --- | --- |
| Identity | `scv` / `Space Construction Vehicle` |
| Transport | ACP over JSON-RPC 2.0, one JSON message per UTF-8 line on standard input and output |
| Tool surface | One tool only: `read` |
| Filesystem | Read-only regular UTF-8 text files below configured read roots |
| State | Per-request only; no sessions, caches, or persisted data beyond ACP lifecycle state |
| Out of scope | Shells, patches, file writes, directory listing, search, network access, MCP, and custom tools |

## ACP server binding

SCV runs as `scv-acp` and binds ACP to standard input/output. Standard output
is protocol-only: every response or notification is one newline-delimited
JSON-RPC 2.0 object. Diagnostics go to standard error and must never be mixed
into standard output.

At startup, SCV accepts ACP `initialize`, negotiates the ACP version selected
by the client, and identifies itself as `scv` version `0.1.x`. It advertises
only the capability needed to invoke the `read` tool. It must advertise
filesystem, terminal, network, MCP, write/edit, and search capabilities as
false or omit them where ACP defines the capability as optional. The exact ACP
wire types and method names come from the pinned ACP schema used by the host;
SCV must not redefine standard ACP methods.

After initialization, the client can create a read-only session and invoke the
single SCV tool. Tool invocation and result delivery use the ACP tool-call
envelope for the negotiated schema version. `read` is the only tool descriptor
in the advertised tool list. A request for another tool returns JSON-RPC
`-32601` (`Method not found`) or the schema-defined unsupported-tool error.

SCV must reject a request received before successful initialization with the
schema-defined lifecycle error. It must return JSON-RPC `-32600` for an invalid
envelope and `-32602` for invalid `read` parameters. Request IDs are echoed in
responses. Notifications never receive responses.

## `read` tool

### Input schema

`read` accepts this JSON object. No additional properties are permitted.

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["path"],
  "properties": {
    "path": {
      "type": "string",
      "description": "Absolute path of a regular UTF-8 text file below a configured read root."
    },
    "offset": {
      "type": "integer",
      "minimum": 1,
      "default": 1,
      "description": "One-based line number at which to begin reading."
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 2000,
      "default": 2000,
      "description": "Maximum number of lines to return."
    }
  }
}
```

`offset` and `limit` describe lines, not bytes or Unicode code points. The
selected range is `offset` through `offset + limit - 1`, clipped at EOF. An
`offset` past EOF succeeds with an empty result. The server may stop earlier
only when the encoded response would exceed the documented maximum response
size; in that case it sets `truncated` to `true`.

### Output schema

The successful tool result is this JSON object:

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["path", "content", "line_start", "line_end", "truncated"],
  "properties": {
    "path": { "type": "string" },
    "content": {
      "type": "string",
      "description": "Returned lines, each prefixed with its padded one-based line number and a tab."
    },
    "line_start": {
      "type": ["integer", "null"],
      "minimum": 1,
      "description": "First returned line number, or null when no lines were returned."
    },
    "line_end": {
      "type": ["integer", "null"],
      "minimum": 1,
      "description": "Last returned line number, or null when no lines were returned."
    },
    "truncated": {
      "type": "boolean",
      "description": "True when the requested range continues but SCV stopped at a response-size bound."
    }
  }
}
```

For a file whose first two lines are `alpha` and `beta`, a request with
`offset: 1` and `limit: 2` returns content equivalent to:

```text
     1\talpha
     2\tbeta
```

Each displayed line number is right-aligned, space-padded to
`max(6, decimal digits of line_end)`, then followed by one tab and the source
line. This preserves the familiar Codex-style scanability while allowing
alignment for files past line 999,999. A final source line without a newline is
returned normally. `content` joins returned display lines with `\n` and does
not add a trailing newline.

### File safety and failures

SCV resolves the supplied absolute path against its configured read roots
before opening it. The resolved target must remain below a root after symlink
resolution and must be a regular file. It never follows a path outside a root,
opens a directory, or mutates a file. v0.1 accepts only valid UTF-8 text; it
does not attempt a lossy decode of binary data.

Failures are structured ACP tool errors with a stable machine-readable `code`.
They include a concise, public-safe message and the requested path, but no file
content or operating-system error detail. The initial codes are:

| Code | Meaning |
| --- | --- |
| `invalid_params` | The request failed the input schema or exceeded the line limit. |
| `path_not_allowed` | The path is relative, outside a configured root, or escapes one through a symlink. |
| `not_found` | The target does not exist. |
| `not_regular_file` | The target is a directory, device, socket, or other non-regular file. |
| `invalid_text` | The target is not valid UTF-8 text. |
| `read_failed` | The operating system denied or interrupted the read. |
| `response_too_large` | No complete line fits within the configured response-size bound. |

## Rust project structure

SCV is planned as one standalone crate, `scv-acp`, rather than a workspace or a
dependency on the OpenAgents application monorepo.

```text
scv-acp/
├── Cargo.toml
├── README.md
├── schemas/
│   ├── read.input.schema.json
│   └── read.output.schema.json
└── src/
    ├── main.rs          # process startup, stdio wiring, exit status
    ├── server.rs        # ACP lifecycle and JSON-RPC dispatch
    ├── protocol.rs      # pinned ACP envelope/types adapter
    ├── read.rs          # schema validation, range selection, formatting
    ├── roots.rs         # root confinement and regular-file checks
    └── error.rs         # JSON-RPC and tool-error mapping
```

The initial dependency budget is intentionally small:

- `tokio` for asynchronous standard I/O and filesystem operations.
- `serde` and `serde_json` for JSON-RPC and schema payloads.
- A pinned Rust ACP binding, if a maintained binding is available for the
  selected ACP schema; otherwise a small, tested adapter only for the required
  standard envelopes.
- `thiserror` for typed, non-panicking error conversion.
- `tracing` and `tracing-subscriber` for stderr-only diagnostics.

`Cargo.lock` is committed. The crate uses `#![forbid(unsafe_code)]`, returns
typed errors instead of panicking, and tests line ranges, padding, EOF,
symlink escape rejection, invalid UTF-8, and JSON-RPC error mapping.

## Roadmap: worker to construction and repair unit

1. **v0.1 — Scout:** ACP lifecycle plus the bounded, read-only `read` tool.
2. **v0.2 — Inspect:** Optional directory listing and content search, each
   separately capability-gated and constrained to the same roots.
3. **v0.3 — Construct:** Explicit, reviewable file creation and patch tools
   with a writable-root policy, dry-run support, and structured diffs.
4. **v0.4 — Repair:** Targeted diagnostics and verification commands behind
   an allowlist, time/resource bounds, cancellation, and streamed ACP updates.
5. **v1.0 — Deployable worker:** Stable capability negotiation, durable
   conformance tests, audit events, sandbox integrations, and a documented
   upgrade path for hosts.

Each step is additive: no future construction or repair capability is implied
by the v0.1 server. Any capability that can change files, execute a process,
or cross a trust boundary requires its own specification, explicit ACP
advertisement, policy gate, and tests before it is enabled.
