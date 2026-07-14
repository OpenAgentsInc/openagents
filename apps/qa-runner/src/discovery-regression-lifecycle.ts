import { createHash } from "node:crypto"

import {
  QaSwarmLandedRegressionCandidate,
  QaSwarmProposedRegressionCandidate,
  QaSwarmValidatedRegressionCandidate,
  type QaSwarmDistilledTestRef,
  type QaSwarmRegressionCandidate,
} from "@openagentsinc/qa-swarm-contract"
import { Effect, Schema as S } from "effect"

import { assessCandidate, distill, type E2eScenarioCandidate } from "./distiller"
import { KhalaSessionTrace } from "./session-trace"

export const QA_SWARM_DISCOVERY_SCHEMA =
  "openagents.qa_swarm.observed_discovery.v1" as const

export const QaSwarmObservedDiscovery = S.Struct({
  schemaVersion: S.Literal(QA_SWARM_DISCOVERY_SCHEMA),
  discoveryRef: S.String,
  findingKind: S.Literal("observed_refuted_behavior"),
  label: S.String,
  observationReceiptRef: S.String,
  trace: KhalaSessionTrace,
})
export type QaSwarmObservedDiscovery = typeof QaSwarmObservedDiscovery.Type

export const decodeQaSwarmObservedDiscovery = S.decodeUnknownSync(QaSwarmObservedDiscovery)

export type RegressionRerunOutcome =
  | Readonly<{
      status: "passed"
      candidateDigest: string
      receiptRef: string
    }>
  | Readonly<{
      status: "failed" | "inconclusive"
      blockerRef: string
      candidateDigest: string
    }>

export interface DiscoveryRegressionRerunner {
  readonly rerun: (input: Readonly<{
    candidate: E2eScenarioCandidate
    candidateDigest: string
    candidateRef: string
    discoveryRef: string
  }>) => Effect.Effect<RegressionRerunOutcome, unknown>
}

export type ScmProposalOutcome =
  | Readonly<{
      status: "proposed"
      candidateDigest: string
      commitProposalRef: string
      issueRef: string
      pullRequestRef: string
    }>
  | Readonly<{
      status: "refused"
      blockerRef: string
      candidateDigest: string
    }>

/**
 * The only repository-mutation seam. Merely constructing or running this
 * lifecycle never grants SCM authority; callers must inject it explicitly.
 */
export interface DiscoveryRegressionScmAuthority {
  readonly propose: (input: Readonly<{
    candidateDigest: string
    candidateRef: string
    discoveryRef: string
    issue: Readonly<{ title: string; body: string }>
    proposedFile: Readonly<{ path: string; source: string }>
    rerunReceiptRef: string
  }>) => Effect.Effect<ScmProposalOutcome, unknown>
}

export type ReviewedMergeResolution =
  | Readonly<{
      status: "reviewed_merged"
      candidateDigest: string
      mergedCommitRef: string
      pullRequestRef: string
      reviewedMergeReceiptRef: string
    }>
  | Readonly<{
      status: "pending" | "rejected" | "unavailable"
      blockerRef: string
    }>

export interface ReviewedMergeEvidenceResolver {
  readonly resolve: (input: Readonly<{
    candidateDigest: string
    pullRequestRef: string
  }>) => Effect.Effect<ReviewedMergeResolution, unknown>
}

export interface DiscoveryRegressionLifecycleDependencies {
  readonly rerunner: DiscoveryRegressionRerunner
  readonly scmAuthority?: DiscoveryRegressionScmAuthority
  readonly mergeEvidenceResolver?: ReviewedMergeEvidenceResolver
}

export interface DiscoveryRegressionLifecycleResult {
  readonly blockerRefs: ReadonlyArray<string>
  readonly candidateDigest?: string
  readonly candidateRef?: string
  readonly distilledTest?: QaSwarmDistilledTestRef
  readonly projectionCandidate?: QaSwarmRegressionCandidate
  readonly state: "inconclusive" | "validated" | "proposed" | "landed"
}

const sha256 = (source: string): string =>
  createHash("sha256").update(source).digest("hex")

const mismatchBlocker = (boundary: string): string =>
  `blocker.qa_swarm.regression_lifecycle.${boundary}.candidate_digest_mismatch`

const hasRef = (value: string): boolean => value.trim().length > 0

const inconclusive = (
  blockerRef: string,
  identity: Readonly<{ candidateDigest?: string; candidateRef?: string }> = {},
): DiscoveryRegressionLifecycleResult => ({
  ...identity,
  blockerRefs: [blockerRef],
  state: "inconclusive",
})

const qualifyDiscovery = (discovery: QaSwarmObservedDiscovery): string | undefined => {
  if (!discovery.trace.receipts.includes(discovery.observationReceiptRef)) {
    return "blocker.qa_swarm.regression_lifecycle.observation_receipt_not_bound"
  }
  let verdict: Extract<(typeof discovery.trace.beats)[number], { readonly kind: "verdict" }> | undefined
  for (let index = discovery.trace.beats.length - 1; index >= 0; index -= 1) {
    const beat = discovery.trace.beats[index]
    if (beat?.kind === "verdict") {
      verdict = beat
      break
    }
  }
  if (verdict === undefined || !["failed", "none", "seeded"].includes(verdict.verificationClass)) {
    return "blocker.qa_swarm.regression_lifecycle.discovery_not_refuted"
  }
  return undefined
}

const proposedPath = (candidate: E2eScenarioCandidate): string =>
  `apps/qa-runner/generated/${candidate.slug}.e2e.test.ts`

const issueBody = (
  discovery: QaSwarmObservedDiscovery,
  candidateRef: string,
  rerunReceiptRef: string,
): string => [
  "A QA Swarm observed discovery produced a deterministic regression candidate.",
  "",
  `- Discovery: \`${discovery.discoveryRef}\``,
  `- Candidate: \`${candidateRef}\``,
  `- Validating rerun: \`${rerunReceiptRef}\``,
  "",
  "Review the proposed test and its evidence before merge. This proposal is not landed coverage.",
].join("\n")

/**
 * Deterministic discovery -> candidate -> validation -> optional proposal ->
 * reviewed merge lifecycle. Failed/non-replayable findings are INCONCLUSIVE;
 * a missing SCM authority stops safely at `validated` without mutation.
 */
export const runDiscoveryRegressionLifecycle = Effect.fn("runDiscoveryRegressionLifecycle")(
  function* (
    discovery: QaSwarmObservedDiscovery,
    dependencies: DiscoveryRegressionLifecycleDependencies,
  ) {
    const qualificationBlocker = qualifyDiscovery(discovery)
    if (qualificationBlocker !== undefined) return inconclusive(qualificationBlocker)

    let distilled
    try {
      distilled = distill(discovery.trace)
    } catch {
      return inconclusive("blocker.qa_swarm.regression_lifecycle.not_distillable")
    }
    const assessment = assessCandidate(distilled, discovery.trace)
    if (!assessment.admissible) {
      return inconclusive("blocker.qa_swarm.regression_lifecycle.candidate_not_admissible")
    }

    const candidate = distilled.emitters.e2e
    const candidateDigest = sha256(candidate.source)
    const candidateRef = `candidate.qa_swarm.regression.${candidateDigest}`
    const identity = { candidateDigest, candidateRef }
    const rerun = yield* dependencies.rerunner.rerun({
        candidate,
        candidateDigest,
        candidateRef,
        discoveryRef: discovery.discoveryRef,
      }).pipe(Effect.catch(() => Effect.succeed<RegressionRerunOutcome>({
        status: "inconclusive",
        blockerRef: "blocker.qa_swarm.regression_lifecycle.rerun_unavailable",
        candidateDigest,
      })))
    if (rerun.candidateDigest !== candidateDigest) {
      return inconclusive(mismatchBlocker("rerun"), identity)
    }
    if (rerun.status !== "passed") return inconclusive(rerun.blockerRef, identity)
    if (!hasRef(rerun.receiptRef)) {
      return inconclusive("blocker.qa_swarm.regression_lifecycle.rerun_receipt_missing", identity)
    }

    const validated = new QaSwarmValidatedRegressionCandidate({
      candidateRef,
      discoveryRef: discovery.discoveryRef,
      label: discovery.label,
      rerunReceiptRef: rerun.receiptRef,
      testHref: proposedPath(candidate),
    })
    if (dependencies.scmAuthority === undefined) {
      return {
        ...identity,
        blockerRefs: [],
        projectionCandidate: validated,
        state: "validated",
      } as DiscoveryRegressionLifecycleResult
    }

    const proposal = yield* dependencies.scmAuthority.propose({
        candidateDigest,
        candidateRef,
        discoveryRef: discovery.discoveryRef,
        issue: {
          title: `QA regression: ${discovery.label}`,
          body: issueBody(discovery, candidateRef, rerun.receiptRef),
        },
        proposedFile: { path: proposedPath(candidate), source: candidate.source },
        rerunReceiptRef: rerun.receiptRef,
      }).pipe(Effect.catch(() => Effect.succeed<ScmProposalOutcome>({
        status: "refused",
        blockerRef: "blocker.qa_swarm.regression_lifecycle.scm_unavailable",
        candidateDigest,
      })))
    if (proposal.candidateDigest !== candidateDigest) {
      return inconclusive(mismatchBlocker("scm_proposal"), identity)
    }
    if (proposal.status === "refused") {
      return {
        ...identity,
        blockerRefs: [proposal.blockerRef],
        projectionCandidate: validated,
        state: "validated",
      } as DiscoveryRegressionLifecycleResult
    }
    if (
      !hasRef(proposal.issueRef) ||
      !hasRef(proposal.commitProposalRef) ||
      !hasRef(proposal.pullRequestRef)
    ) {
      return {
        ...identity,
        blockerRefs: ["blocker.qa_swarm.regression_lifecycle.proposal_refs_missing"],
        projectionCandidate: validated,
        state: "validated",
      } as DiscoveryRegressionLifecycleResult
    }

    const proposed = new QaSwarmProposedRegressionCandidate({
      candidateRef,
      commitProposalRef: proposal.commitProposalRef,
      discoveryRef: discovery.discoveryRef,
      issueRef: proposal.issueRef,
      label: discovery.label,
      pullRequestRef: proposal.pullRequestRef,
      rerunReceiptRef: rerun.receiptRef,
      testHref: proposedPath(candidate),
    })
    if (dependencies.mergeEvidenceResolver === undefined) {
      return {
        ...identity,
        blockerRefs: [],
        projectionCandidate: proposed,
        state: "proposed",
      } as DiscoveryRegressionLifecycleResult
    }

    const merge = yield* dependencies.mergeEvidenceResolver.resolve({
        candidateDigest,
        pullRequestRef: proposal.pullRequestRef,
      }).pipe(Effect.catch(() => Effect.succeed<ReviewedMergeResolution>({
        status: "unavailable",
        blockerRef: "blocker.qa_swarm.regression_lifecycle.merge_resolver_unavailable",
      })))
    if (merge.status !== "reviewed_merged") {
      return {
        ...identity,
        blockerRefs: [merge.blockerRef],
        projectionCandidate: proposed,
        state: "proposed",
      } as DiscoveryRegressionLifecycleResult
    }
    if (
      merge.candidateDigest !== candidateDigest ||
      merge.pullRequestRef !== proposal.pullRequestRef
    ) {
      return inconclusive(mismatchBlocker("reviewed_merge"), identity)
    }
    if (
      !hasRef(merge.reviewedMergeReceiptRef) ||
      !hasRef(merge.mergedCommitRef)
    ) {
      return {
        ...identity,
        blockerRefs: ["blocker.qa_swarm.regression_lifecycle.reviewed_merge_refs_missing"],
        projectionCandidate: proposed,
        state: "proposed",
      } as DiscoveryRegressionLifecycleResult
    }

    const landed = new QaSwarmLandedRegressionCandidate({
      candidateRef,
      discoveryRef: discovery.discoveryRef,
      label: discovery.label,
      mergedCommitRef: merge.mergedCommitRef,
      pullRequestRef: proposal.pullRequestRef,
      rerunReceiptRef: rerun.receiptRef,
      reviewedMergeReceiptRef: merge.reviewedMergeReceiptRef,
      testHref: proposedPath(candidate),
    })
    return {
      ...identity,
      blockerRefs: [],
      distilledTest: {
        href: landed.testHref,
        label: landed.label,
        lifecycleState: "landed",
        receiptRef: landed.reviewedMergeReceiptRef,
      },
      projectionCandidate: landed,
      state: "landed",
    } as DiscoveryRegressionLifecycleResult
  },
)
