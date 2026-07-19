# IDE-10 Effect run graph

Date: 2026-07-19
Issue: [#9038](https://github.com/OpenAgentsInc/openagents/issues/9038)
State: implemented; owner disposition remains unreviewed
Next packet: IDE-11 debug/DAP

## Result

IDE-10 adds one project-bound graph for terminals, declared tasks, tests, and
Output. Effect owns the graph. Xterm renders a terminal screen. The Electron
renderer does not own a process, environment, result, task, test, file, or
receipt.

The graph uses identified Effect Schemas. It binds each operation to the exact
project, root, worktree, attachment generation, placement generation, and cwd
reference. Main decodes each IPC command and result. A stale project
generation cannot use a prior runtime.

This packet does not add DAP, Git delivery, remote placement, extension
execution, mobile control, or public sharing. Those items remain in IDE-11 and
later packets.

## Authority map

| Part | Owner | Authority |
| --- | --- | --- |
| Run graph | `IdeRunService` | Identities, lifecycle, output sequence, retention, semantic outcomes, receipts, and teardown |
| Host adapter | Electron main | Workspace binding, executable admission, safe environment values, process groups, output decode/redaction, artifact reads, and mode-0600 export |
| Terminal transport | Existing Node child-process backend | Fixed shell spawn, stdin bytes, resize signal, interrupt, and process-group termination |
| Xterm | Renderer projection | Screen, input events, search, serialized screen projection, and admitted local links |
| Tasks, Tests, and Output | Renderer projection | Decoded state, user commands, status, locations, gaps, redaction facts, and export intent |
| Rust | Not admitted | No native helper ships in IDE-10 |

The low-level terminal transport keeps a bounded reconnect tail. This tail is
transport replay data. It is not a second task, test, result, policy, or
receipt authority. `IdeRunService` assigns the canonical output sequence and
loss facts.

## Schema graph

`apps/openagents-desktop/src/ide/run-contract.ts` defines:

- project and worktree run bindings;
- named environment sources and a value-free environment manifest;
- executable admission with separate display text and argv;
- terminal profile, split, session, reconnect, dimensions, shell capability,
  and lifecycle data;
- task definitions, dependencies, readiness, problem matchers, timeouts,
  artifacts, runs, semantic outcomes, and evidence references;
- test controllers, discovery generations, item trees, locations, profiles,
  results, coverage facts, retries, and semantic outcomes;
- Output producers, channels, chunks, sequence numbers, locations, byte limits,
  dropped-byte and gap facts, redaction facts, and disposal; and
- actor-bound public-safe run receipts.

Boundary decoders reject excess properties. A renderer cannot add an
executable or argv to a `StartTask` command. A renderer cannot add environment
values to a profile or snapshot.

## Environment policy

The host does not copy `process.env`. It can admit only these host keys:

```text
HOME USER LOGNAME PATH SHELL LANG LC_ALL LC_CTYPE TERM COLORTERM TMPDIR
```

The host adds controlled terminal keys. It rejects secret-shaped names from
profile values. The renderer receives only admitted key names, source order,
generation, and a digest. It does not receive environment values. Receipts
contain the manifest reference and do not contain secret values.

The precedence order is explicit:

1. `HostSafe` at precedence 10.
2. `Profile` at precedence 20.
3. `Project` at precedence 30 when a future admitted project source exists.
4. `TaskInput` at precedence 40 when a future typed task input exists.

IDE-10 implements the first two sources. The other source variants are frozen
for compatible later work. Their presence does not admit values.

## Declared tasks

The host discovers bounded `package.json` scripts as fixed `pnpm run <name>`
argv. It also accepts a versioned `.openagents/tasks.json` file. The file has a
closed schema and rejects excess fields. A declared task separates its label
from its executable and argv.

Example:

```json
{
  "version": 1,
  "tasks": [
    {
      "id": "build",
      "label": "Build",
      "group": "build",
      "executable": "pnpm",
      "argv": ["run", "build"],
      "dependsOn": [],
      "background": false,
      "readinessPattern": null,
      "timeoutMs": 900000,
      "maxRetries": 0,
      "artifactPaths": ["dist/report.json"]
    }
  ]
}
```

Dependency cycles and missing dependency references fail discovery. The host
runs dependencies before the requested task. Artifact paths must be literal,
relative, inside the project, regular files, and no larger than 64 MiB. A
missing required artifact prevents semantic success. A background task becomes
`Ready` only after its declared readiness expression matches bounded retained
output.

The current runtime records `maxRetries` but does not automatically repeat a
mutating task. A later retry policy must prove idempotency before it can use
that field. Manual rerun keeps a new run identity and receipt.

## Tests

The first test controller discovers bounded `*.test.*` and `*.spec.*` files.
It binds every item to one controller and discovery generation. The run command
uses fixed `pnpm exec vp test --run` argv plus the selected relative paths.

A zero exit code is not sufficient. The controller must observe an assertion
summary. Each requested item must settle as passed or skipped. Cancellation,
missing discovery, a stale item, invalid retry reference, artifact loss, or
missing assertion evidence prevents success.

Coverage is an explicit nullable fact. IDE-10 does not fabricate coverage when
the controller does not report it.

## Output and terminal behavior

Each producer has one bounded Output channel. Each chunk has a monotonic
sequence number. Retention is byte-bounded, including one oversized UTF-8
frame. Eviction records dropped bytes and a gap. Invalid UTF-8, truncation, and
redaction are explicit chunk facts.

The host redacts token-shaped strings and private-key blocks before it sends
output to Effect. Export writes only the retained redacted text to an
owner-private mode-0600 file. Export adds an actor-bound receipt.

The terminal surface provides:

- session tabs, create, close, rename support in the graph, split identity,
  focus, resize, interrupt, restart, and exit state;
- direct PTY input frames without line-to-shell interpolation;
- xterm screen rendering, bounded scrollback, search, copy/select behavior,
  screen-reader mode, and 4.5 minimum contrast;
- only terminal-announced localhost links, with the existing explicit open
  confirmation; and
- Khala colors by default with the built-in Tokyo Night fallback retained.

Close and host disposal signal the owned process group. A grace timer escalates
from `SIGTERM` to `SIGKILL`. A disposed channel rejects late bytes.

## Human and agent parity

`IdeRunActor` has `Human` and `Agent` variants. Both variants use the same task,
test, cancellation, Output, environment, budget, and receipt paths. An agent
does not get a separate shell or hidden execution result.

## Effect and Rust decision

IDE-10 stays in Effect/TypeScript and Node. No Rust helper is admitted.

The existing Node process-group adapter passes the current deterministic
process-tree, cancellation, redaction, output, and packaged macOS arm64 gates.
The benchmark does not show a p95/p99 or platform-correctness failure that
justifies a native helper. Therefore, IDE-10 does not create a codec, binary,
credential boundary, project-path boundary, or six-target native release
claim.

The benchmark receipt lists macOS, Windows, and Linux on arm64 and x64. Each
row records `nativeHelper: false`, `typescriptFallback: true`, and no native
target claim. If a later PTY packet proposes Rust, it must satisfy the complete
six-target admission rule in the roadmap before it can ship.

## Evidence

The packet gate is:

```sh
pnpm --dir apps/openagents-desktop run verify:ide-10
```

The gate runs:

- Desktop typecheck;
- schema, service, host, terminal, renderer, Electron boundary,
  accessibility, behavior-contract, and architecture tests;
- the IDE authority boundary oracle;
- the deterministic p50/p95/p99 run benchmark;
- a current macOS arm64 package;
- the packaged xterm, task dependency, task artifact, redaction, gap, test
  tree, keyboard, theme, and teardown journey; and
- the schema-decoded IDE-10 acceptance evaluator.

Evidence files are under `apps/openagents-desktop/benchmarks/ide/`:

- `2026-07-19-ide-10-run.json`;
- `2026-07-19-ide-10-packaged-run.json`;
- `2026-07-19-ide-10-packaged-run.png`;
- `2026-07-19-ide-10-packaged-run-trace.json`; and
- `2026-07-19-ide-10-acceptance.json`.

These receipts are implementation evidence. The Desktop and Cursor
AssuranceSpecs remain `proposed`, and the owner disposition remains
`unreviewed`. IDE-10 contributes to the integrated-agent-IDE rung. It does not
complete that rung because IDE-11 and IDE-12 remain open.
