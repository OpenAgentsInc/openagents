/**
 * The harness conformance registry (MH-1, issue #8582).
 *
 * `satisfies Record<CodingWorkerHarnessKind, HarnessConformanceEntry>` is the
 * compile-time tooth: every coding-worker harness kind MUST have an entry, so a
 * new coding kind cannot be silently omitted. codex and claude_code ship real
 * fixtures (green). grok_cli is proven as of MH-3/MH-4 fixture fill (#8589).
 */
import type {
  CodingWorkerHarnessKind,
  HarnessConformanceEntry,
} from "./contract.ts"
import { codexHarnessConformanceFixture } from "./fixtures/codex.ts"
import { claudeHarnessConformanceFixture } from "./fixtures/claude.ts"
import { grokHarnessConformanceFixture } from "./fixtures/grok.ts"

export const harnessConformanceRegistry = {
  codex: { status: "proven", fixture: codexHarnessConformanceFixture },
  claude_code: { status: "proven", fixture: claudeHarnessConformanceFixture },
  grok_cli: { status: "proven", fixture: grokHarnessConformanceFixture },
} as const satisfies Record<CodingWorkerHarnessKind, HarnessConformanceEntry>

/**
 * Kinds allowed to be `pending` without reddening the sweep. A NEW pending
 * coding kind that is not on this list fails the coverage gate — that is the
 * run-time tooth mirroring effect-native's "adding a tag reds CI".
 *
 * Empty after MH-3 filled grok_cli fixtures (#8589).
 */
export const knownPendingHarnessKinds: ReadonlyArray<CodingWorkerHarnessKind> =
  []
