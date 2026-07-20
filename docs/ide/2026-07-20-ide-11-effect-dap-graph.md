# IDE-11 Effect DAP graph

Date: 2026-07-20
Issue: [#9039](https://github.com/OpenAgentsInc/openagents/issues/9039)
State: evidence contract implemented. Product and packaged evidence not executed
Next packet: IDE-12 Git delivery

## Purpose

IDE-11 adds a supervised Debug Adapter Protocol (DAP) capability to the
Desktop IDE. Effect owns the debug identity, policy, state, receipts, and
cleanup. A debug adapter supplies protocol mechanics. The renderer supplies a
decoded view. An adapter, renderer, Electron process, or native helper does not
own project or debug authority.

This document defines the evidence gate for that result. This document does
not report a successful IDE-11 product run. The source base at the time of this
evidence-lane change does not contain the completed product integration or a
packaged debugger evidence run.

## Authority model

| Part | Authority |
| --- | --- |
| Effect debug service | Debug identities, generations, lifecycle, capability truth, bounded state, policy, receipts, and cleanup |
| Electron main host | Admitted adapter process or transport, admitted configuration sources, secret references, target connection, and process cleanup |
| DAP adapter | Protocol translation and target mechanics only |
| Renderer | Decoded panes and decoded commands only |
| Project and language services | Canonical project, worktree, document, language, and navigation identity |
| Run service | Prelaunch task, postdebug task, process, output, and cancellation mechanics |
| Rust | Not admitted without all Desktop AC-47 evidence |

Launch and attach are different paths. Each path must identify the exact
target, placement, transport, data source, environment reference, and policy.
The evidence gate does not accept a shared implicit path.

## Evidence input

`apps/openagents-desktop/src/ide/debug-evidence-contract.ts` defines the
schema-decoded evidence input and output receipts. The input has two variants:

- `Unexecuted` records why a product evidence run is absent.
- `Captured` records an exact product evidence run.

The committed fixture is:

```text
apps/openagents-desktop/scripts/fixtures/ide-debug-evidence.unexecuted.json
```

The fixture is intentionally not green. The benchmark and packaged scripts
refuse it. A current runner must replace the fixture path with a captured JSON
file through `--input`. The runner must not change the committed placeholder
to claim a run that did not occur.

## Required captured corpus

A captured input must contain all of these corpus facts:

- One or more deterministic fake-adapter journeys.
- Two or more representative real-adapter journeys.
- Two or more representative real languages.
- A launch journey and an attach journey.
- The exact adapter, adapter version, language, language version, Desktop
  target, target kind, transport, and configuration reference for each
  journey.
- The exact effective-configuration digest and all data-source references.
- Project, worktree, attachment, language, target, placement, and service
  generations.
- Supported and unsupported adapter capabilities.
- Breakpoint, thread, stack, scope, variable, watch, console, module, and
  loaded-source projections.
- An exact screenshot, trace, and journey receipt for each journey.

The runner must use real adapter versions and real target facts. It must not
copy the synthetic values from the schema tests into a product receipt.

## Control and source gates

The input must contain one row for each required control:

```text
continue pause step-in step-over step-out evaluate restart disconnect terminate
```

Each row records negotiated capability truth, cancellation support, an exact
receipt, and an honest unsupported state. The gate accepts an unsupported
control when the adapter reports it as unsupported. The gate does not accept a
control that implies support without negotiation.

The input must contain one row for each source state:

```text
source-map changed unavailable remote generated
```

Each row must use canonical source identity. Each row must report an explicit
state. A row fails when it guesses a position.

## Generation and cleanup gates

The runner must cause each of these transitions:

```text
cancel adapter-restart target-loss project-switch app-restart
```

For each transition, the runner must send a late old-generation event. The
runner must show that the service rejects the event and that the current state
does not change. The new generation must be greater than the old generation.
Each row must bind an exact cleanup receipt.

After cleanup, the evidence must report zero active adapter processes, zero
subscriptions, zero queued protocol messages, and zero retained variable bytes.
It must also report zero active debug-owned handles. The receipt includes peak
heap and CPU facts for the measured run.

## Fault matrix

The schema requires all issue faults. It requires configuration, substitution,
secret, adapter, capability, protocol, timeout, cancellation, crash, restart,
target, attach, process identity, breakpoint, thread, variable, evaluation,
source, task, and generation faults. Each row must have an exact evidence
reference and a successful result.

The gate rejects absent rows and duplicate rows. An array with the correct
length is not sufficient.

## Accessibility matrix

The schema requires these accessibility results:

- Keyboard controls and pane navigation.
- Screen-reader state, progress, and error messages.
- Focus restoration.
- Zoom and minimum-window behavior.
- Reduced motion.
- Vim and editor key boundaries.
- Khala-default and Tokyo-Night-fallback contrast with non-color status cues.
- Huge-tree degradation.

Each result must bind an exact evidence reference. The packaged runner must
capture the visible state. A source-code inspection alone is not packaged UI
evidence.

## Performance matrix

The benchmark receipt requires p50, p95, and p99 values and thresholds for:

```text
configuration-validation
adapter-launch
adapter-attach
first-stopped-paint
breakpoint-round-trip
step-continue
stack-scope-variable-expansion
evaluate-watch
source-navigation
restart-terminate
memory-cpu-sample
teardown
```

The captured input must record repetitions and warmup counts. Each measured row
must pass its p50, p95, and p99 thresholds. The gate rejects an absent or
duplicate metric.

## Security and data facts

The captured input must prove these facts:

- Secrets stay as references.
- Projected data is redacted.
- Protocol queues are bounded.
- Console retention is bounded.
- Variable depth and count are bounded.
- Retained data is deleted during cleanup.
- The renderer does not receive credentials.
- Public-safe evidence does not contain forbidden material.

The scripts also scan JSON input and receipts for private absolute paths and
secret-shaped values. The packaged script scans each JSON trace and receipt.
It requires each journey evidence file to exist inside the repository.

## Desktop targets and Rust decision

The target table always has these six rows:

```text
macos-arm64 macos-x64 windows-arm64 windows-x64 linux-arm64 linux-x64
```

A row can be `packaged-journey-passed` or `not-claimed`. A claimed row must
bind an exact packaged-journey reference. An unclaimed row must not contain a
journey reference.

The IDE-11 evidence contract admits no Rust helper. It records
`rustAdmitted: false` and `ac47AdmissionEvidencePresent: false`. A later native
proposal must use a different admitted contract. It must supply all AC-47
evidence for codecs, capabilities, six targets, failures, security,
performance, fallback, and target exclusions.

## Commands and expected files

The integration change must add package commands that call these scripts. The
evidence lane does not edit `package.json` because the root IDE-11 integration
owns that shared file.

The direct commands are:

```sh
node --expose-gc --import tsx apps/openagents-desktop/scripts/ide-debug-benchmark.ts \
  --input <captured-evidence.json>

node --import tsx apps/openagents-desktop/scripts/ide-debug-packaged-journey.ts \
  --input <captured-evidence.json>

node --import tsx apps/openagents-desktop/scripts/ide-debug-acceptance.ts
```

The scripts write these expected receipts:

```text
apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-11-debug.json
apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-11-debug-packaged.json
apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-11-debug-acceptance.json
```

The packaged runner must also write one screenshot, one public-safe trace, and
one journey receipt for each corpus row. Their exact paths come from the
captured input.

The benchmark script requires the candidate SHA to equal current `HEAD`. The
packaged script also verifies the packaged application tree digest, file count,
and byte count. The acceptance script requires the same candidate and artifact
hashes. It also requires an ancestor relation to the evaluation SHA. A clean
Git diff check and a successful IDE authority-boundary check are also necessary.

## Review and release state

The owner disposition is `unreviewed`. The Desktop AssuranceSpec lifecycle is
`proposed`. The exact reviewer reference in the acceptance receipt points to
this section until an authorized independent review adds a stronger reference.
This state does not grant release or public-claim authority.

The final issue release comment must record all exact Git and artifact
references. It must also record all receipts, screenshots, traces, target rows,
and corpus facts. It must include capability, performance, security, resource,
Rust, review, owner, and rollback facts.

## Current evidence state

No captured IDE-11 product evidence is committed by this evidence-lane change.
The `Unexecuted` fixture is the current truthful state. The production
integration must run the complete fake and real corpus before it can produce a
green benchmark, packaged, or acceptance receipt.

IDE-11 does not add Git mutation or delivery authority. That work stays in
IDE-12. Windows and Linux packaged claims also stay absent until exact target
journeys pass.
