import { writeFileSync } from "node:fs"
import path from "node:path"

import { Schema as S } from "effect"

import {
  BaselinePointer,
  CandidateArtifact,
  EvaluationReport,
  ReleasedArtifactPointer,
  UncertaintyRecord,
} from "@openagentsinc/dse"

import {
  compileHonestChatArtifact,
  compileTurnRouteArtifact,
  type DseCompileBundle,
} from "../src/turn/dse/compile.ts"

/**
 * Regenerate the checked-in AFS-09 released artifacts.
 *
 * This offline script compiles the Apple FM signatures against the checked-in
 * production-shaped fixtures and writes their immutable released bytes and
 * receipts to `src/turn/dse/artifacts.generated.ts`. The compile is
 * deterministic, so re-running the script on unchanged fixtures reproduces the
 * same digests. Run it with:
 *
 *   node --import tsx apps/openagents-desktop/scripts/compile-dse-artifacts.ts
 */

const encodeCandidate = S.encodeUnknownSync(CandidateArtifact)
const encodePointer = S.encodeUnknownSync(ReleasedArtifactPointer)
const encodeBaseline = S.encodeUnknownSync(BaselinePointer)
const encodeUncertainty = S.encodeUnknownSync(UncertaintyRecord)
const encodeReport = S.encodeUnknownSync(EvaluationReport)

interface EncodedBundle {
  readonly winner: unknown
  readonly pointer: unknown
  readonly baseline: unknown
  readonly uncertainty: unknown
  readonly holdoutReport: unknown
  readonly baselineHoldoutReport: unknown
}

const encode = (bundle: DseCompileBundle): EncodedBundle => ({
  winner: encodeCandidate(bundle.winner),
  pointer: encodePointer(bundle.pointer),
  baseline: encodeBaseline(bundle.baseline),
  uncertainty: encodeUncertainty(bundle.uncertainty),
  holdoutReport: encodeReport(bundle.holdoutReport),
  baselineHoldoutReport: encodeReport(bundle.baselineHoldoutReport),
})

const constBlock = (name: string, value: unknown): string =>
  `export const ${name} = ${JSON.stringify(value, null, 2)} as const\n`

const main = async (): Promise<void> => {
  const honest = await compileHonestChatArtifact()
  const route = await compileTurnRouteArtifact()

  for (const [label, bundle] of [
    ["HonestChatReply.v1", honest],
    ["TurnRoute.v1", route],
  ] as const) {
    const delta = bundle.holdoutReport.aggregateScore - bundle.baselineHoldoutReport.aggregateScore
    process.stdout.write(
      `${label}: baseline holdout ${bundle.baselineHoldoutReport.aggregateScore.toFixed(3)} -> ` +
        `candidate holdout ${bundle.holdoutReport.aggregateScore.toFixed(3)} (delta ${delta.toFixed(3)}, ` +
        `uncertainty ${bundle.uncertainty.method})\n`,
    )
    if (delta <= 0) throw new Error(`${label}: compiled artifact did not beat the baseline on holdout`)
  }

  const honestEnc = encode(honest)
  const routeEnc = encode(route)

  const header = [
    "/**",
    " * AFS-09 CHECKED-IN RELEASED ARTIFACTS — GENERATED, DO NOT EDIT BY HAND.",
    " *",
    " * Regenerate with:",
    " *   node --import tsx apps/openagents-desktop/scripts/compile-dse-artifacts.ts",
    " *",
    " * Each block is the immutable released bytes and receipts of a compiled Apple",
    " * FM signature: the content-addressed winner candidate, the released pointer,",
    " * the hand-written baseline pointer (the shadow and rollback target), the",
    " * uncertainty record, and the holdout evaluation reports. The runtime resolves",
    " * these bytes offline and verifies every digest.",
    " */",
    "",
  ].join("\n")

  const body = [
    constBlock("HONEST_CHAT_WINNER", honestEnc.winner),
    constBlock("HONEST_CHAT_POINTER", honestEnc.pointer),
    constBlock("HONEST_CHAT_BASELINE", honestEnc.baseline),
    constBlock("HONEST_CHAT_UNCERTAINTY", honestEnc.uncertainty),
    constBlock("HONEST_CHAT_HOLDOUT_REPORT", honestEnc.holdoutReport),
    constBlock("HONEST_CHAT_BASELINE_HOLDOUT_REPORT", honestEnc.baselineHoldoutReport),
    constBlock("TURN_ROUTE_WINNER", routeEnc.winner),
    constBlock("TURN_ROUTE_POINTER", routeEnc.pointer),
    constBlock("TURN_ROUTE_BASELINE", routeEnc.baseline),
    constBlock("TURN_ROUTE_UNCERTAINTY", routeEnc.uncertainty),
    constBlock("TURN_ROUTE_HOLDOUT_REPORT", routeEnc.holdoutReport),
    constBlock("TURN_ROUTE_BASELINE_HOLDOUT_REPORT", routeEnc.baselineHoldoutReport),
  ].join("\n")

  const outPath = path.resolve(import.meta.dirname, "..", "src", "turn", "dse", "artifacts.generated.ts")
  writeFileSync(outPath, `${header}\n${body}`, "utf8")
  process.stdout.write(`wrote ${outPath}\n`)
}

main().catch((error: unknown) => {
  process.stderr.write(`${String(error)}\n`)
  process.exitCode = 1
})
