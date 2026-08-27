// The executed-acceptance verdict shape (EPIC #6017).
//
// This module is PURE and Worker-safe (no Playwright, no browser). The headless
// runner (`runner.ts`, runs OUT of the CF Worker) produces an `AcceptanceVerdict`;
// the route consumes it to derive `verified` / `scalarReward` / `verification_receipt`
// when (and only when) a real execution actually ran. Keeping the verdict shape here
// means the route never transitively imports chromium.

import type {
  AcceptanceSpec,
  CrossyRoadAcceptanceCheckId,
} from '../acceptance-spec'

// One executed acceptance check result. `passed` is the verdict of running a PROGRAM
// against the live page — never a regex over source. `detail` is a public-safe,
// human-readable note (measured values / failure reason), never raw artifact bytes.
export type AcceptanceCheckResult = Readonly<{
  id: CrossyRoadAcceptanceCheckId
  passed: boolean
  detail: string
}>

// The full executed-acceptance verdict. `executed` is always true here — this shape
// is only produced AFTER a real headless run. `scalarReward` is the fraction of
// checks passing (a dense, honest signal). `verified` is true ONLY when every check
// passed. `consoleErrors` / `pageErrors` are the captured browser diagnostics that
// caught the "crashed on load" defect.
export type AcceptanceVerdict = Readonly<{
  kind: AcceptanceSpec['kind']
  executed: true
  rubricRef: string
  checks: ReadonlyArray<AcceptanceCheckResult>
  passedChecks: ReadonlyArray<CrossyRoadAcceptanceCheckId>
  failedChecks: ReadonlyArray<CrossyRoadAcceptanceCheckId>
  scalarReward: number
  verified: boolean
  consoleErrors: ReadonlyArray<string>
  pageErrors: ReadonlyArray<string>
}>
