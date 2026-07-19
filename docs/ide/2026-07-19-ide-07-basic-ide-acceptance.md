# IDE-07 packaged daily-use basic IDE acceptance

Date: 2026-07-19
Issue: [#9022](https://github.com/OpenAgentsInc/openagents/issues/9022)
Status: candidate gate implemented; exact packaged receipts are required before acceptance

## Claim boundary

IDE-07 is the first packet allowed to emit exactly one narrow release-rung
claim: **OpenAgents basic IDE**. It does not admit or imply Zed quality, a full
IDE, Cursor parity, a drop-in replacement, an agent IDE, portable placement,
or any IDE-08+ feature.

The claim is tied to one exact macOS arm64 `.app` tree, one reachable `main`
SHA, one public-safe evidence bundle, and one non-overridable deterministic
oracle. Windows, Linux, and macOS x64 remain explicitly unavailable until an
equivalent packaged candidate is evaluated there. Closing #9022 does not close
epic #9014 or replace its owner-acceptance requirement.

## What the gate evaluates

The checked contract is
`apps/openagents-desktop/src/ide/basic-ide-acceptance-contract.ts`. It defines
all boundary values with Effect Schema and derives its TypeScript types. A
valid receipt must contain exactly:

- seven closed child packets, IDE-00 through IDE-06, with evidence refs;
- all fifteen packaged daily-use matrix classes, each passed with evidence;
- an exact candidate SHA, package version, Electron version, OS/hardware
  class, app-tree SHA-256, file count, and byte count;
- checked p50/p95/p99 metrics with repetitions, method, noise, frozen p95/p99
  thresholds, IDE-00 comparison where applicable, and literal pass state;
- a seven-launch chat-only receipt with zero editor assets, renderer workers,
  Monaco hosts, Pierre trees, language placements, or project-index surfaces;
- literal architecture/custody, rollback, public-safety, target-availability,
  evaluator, later-gap, and claim-boundary facts.

There is no permissive `pending`, `warning`, or producer override state in the
accepted schema. Missing evidence fails decoding.

## Exact artifact binding

`scripts/ide-packaged-artifact.ts` resolves the actual Forge-produced macOS
arm64 app. It recursively hashes every directory, regular file content/mode,
and symlink target in stable relative-path order. Both packaged journeys and
the final evaluator independently recompute this tree digest. A receipt from a
different build, a post-build mutation, or a different Git SHA is rejected.

Only relative artifact/evidence refs are serialized. Absolute repository,
workspace, user-data, and home paths are refused by the release oracle.

## Integrated packaged journeys

The editor journey launches the app through macOS LaunchServices with a
supported TypeScript file, not by directly executing Electron. Its disposable
Git worktree proves:

1. Finder cold open into primary Monaco;
2. Tokyo Night on the Monaco surface before readiness is admitted;
3. complete Pierre Explorer availability and root withholding;
4. quick open, preview pinning, editing, Vim toggle, and two editor groups;
5. current local-worker and project TypeScript 6.0.3 evidence;
6. current Problems and Outline projections after an explicit refresh;
7. a real changed-file Pierre review with split layout and context control;
8. recovery after renderer reload;
9. private-scheme/offline editor assets with no legacy textarea; and
10. zero Monaco models, views, workers, and listeners after disposal.

The separate chat-only journey performs seven fresh packaged cold launches.
It waits only for the ordinary chat workspace and never imports the editor
runtime to inspect it. Chromium resource entries, workers, and mounted DOM
surfaces must prove zero IDE activation. It also records shell-ready
p50/p95/p99 and confirms every launched app process stops.

The packaged journey is complemented, not replaced, by the child corpora:

| Matrix concern | Primary evidence |
| --- | --- |
| Explorer scale, watcher, keyboard, reduced motion, zoom | IDE-02 10k-node receipt plus packaged large-repository journey |
| Monaco editing, recovery, conflict, Vim, disposal | IDE-03 receipt plus complete Desktop tests |
| quick open, navigation, settings, keybindings, file operations | IDE-04 receipt plus packaged workbench journey |
| all eight versioned review sources and refusals | IDE-05 500-file receipt plus packaged Git-worktree review |
| all 17 TypeScript capabilities, bursts, crash/restart | IDE-06 real-worker receipt plus packaged language journey |
| rollback and schema compatibility | update/rollback, settings, recovery, package-admission, and dependency-removal corpora |
| accessibility and failure truth | complete Desktop keyboard/DOM/Electron/accessibility tests |

## Performance and resource policy

IDE-07 reruns the IDE-00 deterministic 2,000-file baseline into a separate
current-candidate receipt; it never rewrites the original comparison input.
Filesystem latency metrics use a frozen 2x IDE-00 p95/p99 non-regression
envelope, while process RSS and descriptor counts use 1.5x. These factors are
explicit in every receipt row and cannot auto-expand.

The seven packaged chat-only launches use 1.5x the IDE-00 shell-mounted
p95/p99 as their frozen envelope. Explorer, Monaco, workbench, Pierre review,
and language metrics retain their already-landed packet-owned thresholds;
IDE-07 imports those thresholds without changing them. Linear interpolation
over ascending samples is the single percentile method.

Every metric names repetitions, baseline p95 when applicable, noise, and both
p95 and p99 pass thresholds. A breach fails Schema decoding rather than
silently rewriting the budget.

## Architecture and custody audit

The final evaluator reruns `check:ide-boundaries` and refuses the receipt
unless the existing schema-first Effect authority remains intact:

- Effect owns project, worktree, identity, document, recovery, Git, language,
  policy, persistence, approval, and evidence state;
- services retain `Context.Service`, `Layer.effect`, named `Effect.fn`,
  `Schema.TaggedErrorClass`, decoded inputs, and scoped teardown;
- renderer code, Monaco, Pierre, and native helpers remain projections or
  authority-free mechanics;
- Tokyo Night remains the only active IDE theme and executable theme code is
  absent;
- Vim remains first-party, off by default, persistent, packaged, and fully
  disposable; and
- the gate enables no upload, remote index, embeddings, cloud fallback,
  telemetry expansion, AI edit, terminal expansion, or extension runtime.

The evaluator class is `deterministic_repository_oracle`. It recomputes facts
from the artifact and checked receipts and exposes no producer override. This
is independent mechanical evaluation; it does not impersonate the epic's
human owner acceptance.

## Rollback

The receipt binds the previous IDE-06 evidence commit as the source rollback
target. Rollback is admissible only because:

- editor package/theme/Vim/worker removal is documented in IDE-01;
- settings and recovery schemas decode across the boundary;
- the retained-slot update, interrupted apply, rollback, failure, and cleanup
  corpora pass; and
- canonical project/document state is outside Monaco, Pierre, worker, and
  renderer lifetimes.

Rollback cannot erase or reinterpret owner data to make the old build appear
healthy.

## Commands and evidence

The exact candidate flow is:

```text
pnpm --dir apps/openagents-desktop run typecheck
pnpm --dir apps/openagents-desktop run package:mac
OPENAGENTS_DESKTOP_IDE07_ACCEPTANCE=1 pnpm --dir apps/openagents-desktop run ide:monaco-packaged-journey -- <disposable fixture>
pnpm --dir apps/openagents-desktop run ide:chat-only-packaged-journey
pnpm --dir apps/openagents-desktop run ide-baseline -- --out apps/openagents-desktop/benchmarks/ide/2026-07-19-ide-07-current-baseline.json
pnpm --dir apps/openagents-desktop run verify:ide-07
```

The final checked evidence bundle consists of:

- `2026-07-19-ide-07-packaged-basic-ide.json` and its PNG;
- `2026-07-19-ide-07-chat-only.json`;
- `2026-07-19-ide-07-current-baseline.json` and raw samples;
- `2026-07-19-ide-07-acceptance.json`; and
- every referenced IDE-00 through IDE-06 receipt.

## Explicit residual gaps

IDE-08 through IDE-19 remain incomplete. In particular, this gate adds no
inspectable agent context/proposals, AI completion or multi-file editing,
terminal/tasks/tests/debug integration, broader SCM delivery, additional
themes, portable host placement, mobile portable-IDE control, extension ABI,
complete data migration/export/deletion breadth, Cursor parity closure,
complete cross-platform accessibility, or full-IDE release gate.
