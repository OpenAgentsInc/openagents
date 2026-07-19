# IDE-05 versioned review and Pierre implementation

Date: 2026-07-19

Roadmap packet: IDE-05

Issue: [#9020](https://github.com/OpenAgentsInc/openagents/issues/9020)

Depends on: IDE-03 Monaco document runtime; IDE-04 daily workbench

## Result

Desktop now has one versioned review domain and one production rendering path.
The previous React Git line and hunk loops are gone. Both the adjacent review
drawer and the full review workspace compose an exact source from the current
project/path-index identity, host Git status snapshot, renderer status
generation, and bounded diff result, then render that source through the
app-owned `PierreReviewAdapter`.

The domain is not a generic “diff.” Its Effect Schema tagged union keeps eight
authorities distinct:

| Source tag | Exact comparison | Additional fence | Production mutation |
| --- | --- | --- | --- |
| `GitHeadIndex` | HEAD → index | Git snapshot ref and generation | none; read-only |
| `GitIndexWorktree` | index → working tree | Git snapshot ref and generation | none; read-only |
| `GitHeadWorktree` | HEAD → aggregate working tree | Git snapshot ref and generation | none; read-only |
| `SavedDraft` | saved disk revision → unsaved draft | disk revision and document generation | typed document command only |
| `DraftExternalConflict` | draft → externally changed disk | expected/actual disk refs and draft generation | typed document command only |
| `CheckpointCurrent` | checkpoint → current document/project | checkpoint and attachment/document generations | typed checkpoint/workspace command only |
| `AgentProposal` | exact base → proposed result | proposal, attachment, base/current document generations | contract fixture; IDE-08 owns the real workflow |
| `CandidateComparison` | candidate A → candidate B | two candidate refs and generations | none |

The last six classes are an executable source/adapter corpus in this packet;
they are not claims that checkpoint creation, agent proposal generation, or
candidate selection UI has landed. That separation lets those later producers
join the same review plane without inventing a second diff contract.

## Effect-owned source graph

`IdeReviewSourceSchema` is the sole review-source boundary. Its TypeScript type
is derived from the schema. Every variant carries the shared fields below in
addition to its source-specific refs and generations:

- schema version and opaque review ref;
- exact project, root, and worktree refs;
- nullable exact file, document, and relative path refs for aggregate cases;
- single-file or aggregate scope;
- base and target endpoint labels, opaque version refs, and positive
  generations;
- encoding and line-ending classification for both endpoints;
- an explicit tagged content state: available, binary, secret, too large,
  truncated, or unavailable;
- a bounded patch and language hint only when display is admitted;
- typed origin and allowed actions;
- a tagged ready, stale, or unavailable lifecycle with typed reason and
  refreshability.

Free-form labels cannot grant Git, document, proposal, or checkpoint authority.
Changing `HEAD` text to “proposal” does not change the source tag, generations,
actions, or canonical command route.

```text
host statusRef + repositoryRef + renderer status generation
                         │
path-index identity ─────┼─> GitHeadIndex / GitIndexWorktree
                         │       │
bounded host diff ───────┘       │ exact source + lifecycle
                                 ▼
                    app-owned Pierre projection
                                 │
                   @pierre/diffs rendering mechanics
                                 │ bounded IdeReviewIntent
                                 ▼
                exact-generation action disposition
                                 │
             projection / document / workspace /
                    checkpoint / proposal command
```

The Git panel now increments a monotonic renderer generation when an opaque
host `statusRef` changes. A replaced snapshot clears any displayed diff and
review failure. The Git source builder additionally compares repository and
status refs in the status and diff results. If either differs, the source is
stale, its patch is withheld, and Pierre is not invoked.

## Mutation and staleness law

`IdeReviewIntentSchema` is bounded renderer intent. It carries review ref,
action, exact base/target binding, optional base/target line range, layout, and
bounded context count. It has no mutation function or service handle.

`reviewActionDisposition` applies this order:

1. Refuse an action absent from the source's allowed-action set.
2. Refuse unavailable content.
3. Permit only a typed read-only refresh when a stale source explicitly says
   it is refreshable; refuse every stale mutation.
4. For accept, reject, apply, or undo, compare both endpoint version refs and
   generations again.
5. Refuse a moved base or target before command construction.
6. Construct one schema-derived canonical command for the owning document,
   workspace, checkpoint, or proposal authority. Git and candidate sources
   have no mutation route.

The disposition is not durable success. The owning service must revalidate and
execute the command and return the next canonical version. There is no
line-number patch application, optimistic persistence, or widget-selection
authority.

## Pierre boundary

`projectReviewSourceToPierre` is the only domain-to-library conversion. The
decoded Pierre projection contains exactly:

- review ref and a projection file ref;
- bounded patch;
- unified or split layout;
- context count from 1 through 100;
- optional controlled line selection;
- at most 500 typed diagnostic, conflict, comment, proposal-rationale, stale,
  or unavailable annotations.

The output schema has no field for root, worktree, grant, preload bridge, Git
or filesystem callback, process access, approval policy, document mutation,
persistence, or allowed-action authority. Pierre is invoked with
`disableWorkerPool`; Tokyo Night is registered from the one owned semantic
projection. Annotation kind is rendered as text, and additions/deletions have
line markers and a screen-reader explanation rather than relying on red/green
alone.

The retained Effect Native `DiffView` projection is named
`legacyEffectNativeReviewRows` and documented as the non-React compatibility
test fallback. Shipped React review surfaces contain no custom hunk or line
renderer.

## Review experience

The production Git review surfaces now expose:

- explicit source tag and base → target labels;
- unified/split controls;
- bounded context expansion controls;
- Pierre line/range selection;
- selected-range or whole bounded-diff attachment to the composer;
- a selected-line comment form projected into typed Pierre annotations;
- open-in-editor through the existing grant-scoped canonical open intent;
- changed-file selection, refresh, and close/back controls;
- existing secret, binary, large, invalid-path, stale, unsafe-state, and
  unavailable refusal copy;
- keyboard-focusable native controls and screen-reader source/non-color copy.

Selected composer disclosure does not rediscover selection after the click.
The typed base/target range is sent with the intent, and
`boundedSelectedReviewPatch` extracts only matching patch lines plus necessary
file/hunk headers. The composer records whether it received the whole bounded
diff or an exact selection and continues to label the bytes untrusted data.

## Verification

The focused command is:

```sh
cd apps/openagents-desktop
pnpm run verify:ide-05
```

It builds Desktop, regenerates the schema-decoded review benchmark, typechecks,
runs the project/review/Pierre/Git contract and host corpora, mounts the two
production React review paths, exercises refusals and disclosure, runs
accessibility and behavior-contract tests, and checks IDE boundaries.

The headless public-safe receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-05-review.json`:

| Observation | p50 | p95 | p99 | budget |
| --- | ---: | ---: | ---: | ---: |
| eight-source project + Pierre parse | 0.115 ms | 0.177 ms | 0.231 ms | 20 ms p95 |
| 500-file / 1,647,282-byte aggregate parse | 8.466 ms | 8.766 ms | 9.999 ms | 250 ms p95 |

Thirty samples are recorded. A 100-update generation-fence fixture commits the
newest result once and rejects 99 superseded completions. Two hundred repeated
projection/parse cycles finish with Pierre worker pooling disabled, zero active
workers, zero listener delta, and no positive retained-heap delta after GC.

The existing packaged IDE fixture now adds the eight-source corpus. In both
development-layout and ASAR-layout Electron runs it completes three
create/dispose cycles, renders all eight source tags through
`PierreReviewAdapter`, exercises unified/split selection, annotations, and the
200-file virtualized collection, observes no external URL, and reaches zero
tracked workers and Monaco models after every dispose. The fixture also keeps
the injected worker-construction failure closed and leak-free. The receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-package-spike.json`;
the Tokyo Night visual is
`apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-01-tokyo-night.png`.

The full Desktop regression run closes with 270 test files, 2,645 passing
assertions, and 39 intentional skips. The IDE-05 focused gate closes with 13
test files and 185 passing assertions.

## Explicit non-goals and next packets

- IDE-06 owns TypeScript/LSP lifecycle, symbols, diagnostics, Problems,
  formatting, code actions, and rename.
- IDE-07 owns the packaged daily-editor acceptance matrix over IDE-02–06; it
  does not add unrestricted Git mutation.
- IDE-08 owns real agent proposal creation, review orchestration, application,
  backlinks, and post-apply evidence.
- IDE-10 owns xterm/PTY process mechanics; IDE-12 owns safe Git mutation,
  worktrees, and delivery.
- Cloud review storage, guessed-line application, a second diff authority, and
  direct Pierre mutation remain absent.
