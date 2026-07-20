# Apple FM CLI reference

Date: 2026-07-19

The Apple Foundation Models backend has a runnable CLI in the Pylon runtime
(`apps/pylon/packages/runtime/src/cli.ts`). Invoke it through Pylon
(`pylon apple-fm ...`) or `node --import tsx apps/pylon/src/index.ts apple-fm ...`.

All commands run against the local Swift bridge on `http://127.0.0.1:11435` by
default. Add `--auto-launch` to start or adopt the bridge automatically (AFM-1
launcher): a bridge the CLI launches is stopped on exit unless you pass
`--stop-on-exit false`. An already-running bridge is adopted and never stopped.
Add `--json` for a public-safe machine-readable result.

## Commands

| Command | Purpose |
| --- | --- |
| `apple-fm health [--json]` | Typed readiness (`ready` / `unavailable` / `unsupported` / `malformed` / `unreachable`). No inference. |
| `apple-fm status` | Readiness plus the redacted availability receipt (human format). |
| `apple-fm infer --prompt TEXT [--auto-launch] [--stream] [--json]` | One real one-shot completion. Prints the text and the honest usage truth. Alias: `chat`. `--stream` routes through the streaming session path. |
| `apple-fm session --prompt TEXT [--auto-launch] [--stream] [--json]` | A bounded turn through the real session endpoints (`POST /v1/sessions` + `responses/stream`, SSE). The bridge emits real progressive snapshots. The `--stream` flag renders the reconstructed incremental deltas. |
| `apple-fm tool --workspace DIR [--path FILE] --prompt TEXT [--auto-launch] [--json]` | A bounded read-only tool-use turn over a real workspace (Blueprint-selected tools). |
| `apple-fm smoke [--prompt TEXT]` | Require readiness, then complete a short prompt. |
| `apple-fm tool-stream-demo` | The original fixture-driven tool-stream demonstration. |

Shared flags: `--base-url URL`, `--profile apple-fm-local`, `--json`,
`--auto-launch`, `--stop-on-exit false`.

## Examples

```sh
# Start/stop the bridge automatically and run one real inference.
pylon apple-fm infer --auto-launch --prompt "Name three Swift value types." --json

# Bounded read-only tool turn over a workspace.
pylon apple-fm tool --auto-launch --workspace . --path README.md \
  --prompt "Use read_file to read README.md, then name it in one sentence." --json

# Readiness only, no inference.
pylon apple-fm health --json
```

## Honesty and safety

- Usage truth is reported exactly as the bridge provides it: `estimated` for the
  current character-derived counts, `unknown` when unavailable. The CLI never
  synthesises an exact-token claim (see AFM-5).
- Every `--json` result is public-safe: receipts are redacted
  (`contentRedacted: true`), and base URLs, callback URLs, and callback tokens
  are never printed.
- A not-ready or unreachable bridge is surfaced as a typed failure, never masked
  as a fake success.
