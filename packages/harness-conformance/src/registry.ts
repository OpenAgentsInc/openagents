/**
 * The harness conformance registry (MH-1, issue #8582).
 *
 * `satisfies Record<CodingWorkerHarnessKind, HarnessConformanceEntry>` is the
 * compile-time tooth: every coding-worker harness kind MUST have an entry, so a
 * new coding kind cannot be silently omitted. codex and claude_code ship real
 * fixtures (green); grok_cli is `pending` by design until the Grok MH-3/MH-4
 * lane fills its fixtures.
 */
import type {
  CodingWorkerHarnessKind,
  HarnessConformanceEntry,
} from "./contract.ts"
import { codexHarnessConformanceFixture } from "./fixtures/codex.ts"
import { claudeHarnessConformanceFixture } from "./fixtures/claude.ts"

export const harnessConformanceRegistry = {
  codex: { status: "proven", fixture: codexHarnessConformanceFixture },
  claude_code: { status: "proven", fixture: claudeHarnessConformanceFixture },
  grok_cli: {
    status: "pending",
    reasonRef: "blocker.harness.grok_cli.no_executor_yet",
    ownerLane: "MH-3/MH-4 (Grok lane)",
  },
} as const satisfies Record<CodingWorkerHarnessKind, HarnessConformanceEntry>

/**
 * Kinds allowed to be `pending` without reddening the sweep. A NEW pending
 * coding kind that is not on this list fails the coverage gate — that is the
 * run-time tooth mirroring effect-native's "adding a tag reds CI".
 */
export const knownPendingHarnessKinds: ReadonlyArray<CodingWorkerHarnessKind> = [
  "grok_cli",
]
