import { describe, expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { Effect } from "effect"

import {
  QA_SWARM_DISCOVERY_SCHEMA,
  runDiscoveryRegressionLifecycle,
  type DiscoveryRegressionRerunner,
  type DiscoveryRegressionScmAuthority,
  type QaSwarmObservedDiscovery,
  type ReviewedMergeEvidenceResolver,
} from "./discovery-regression-lifecycle"
import { makeSessionTrace, type SessionBeat } from "./session-trace"

const observationReceiptRef = "receipt.qa_swarm.discovery.login_redirect"

const observedDiscovery = (): QaSwarmObservedDiscovery => {
  const beats: ReadonlyArray<SessionBeat> = [
    { kind: "browser", action: "navigate", targetHint: "/login", status: "ok" },
    {
      kind: "browser",
      action: "assert",
      targetHint: "stays at /login (no redirect to home)",
      status: "failed",
    },
    { kind: "verdict", verificationClass: "failed" },
  ]
  return {
    schemaVersion: QA_SWARM_DISCOVERY_SCHEMA,
    discoveryRef: "discovery.qa_swarm.login_redirect",
    findingKind: "observed_refuted_behavior",
    label: "Login route must remain visible",
    observationReceiptRef,
    trace: makeSessionTrace({
      goal: "verify login route remains visible",
      target: { name: "openagents.com", baseUrl: "https://openagents.com" },
      model: "openagents/khala",
      beats,
      inputs: [{ name: "target", type: "Target" }],
      outputs: [{ name: "visible", type: "boolean" }],
      receipts: [observationReceiptRef],
    }),
  }
}

const passingRerunner = (): DiscoveryRegressionRerunner => ({
  rerun: input => Effect.succeed({
    status: "passed",
    candidateDigest: input.candidateDigest,
    receiptRef: `receipt.qa_swarm.rerun.${input.candidateDigest}`,
  }),
})

const proposalAuthority = (
  calls: Array<string>,
): DiscoveryRegressionScmAuthority => ({
  propose: input => Effect.sync(() => {
    calls.push(input.candidateDigest)
    expect(createHash("sha256").update(input.proposedFile.source).digest("hex")).toBe(
      input.candidateDigest,
    )
    expect(input.proposedFile.path).toMatch(
      /^apps\/qa-runner\/generated\/[a-z0-9-]+\.e2e\.test\.ts$/,
    )
    return {
      status: "proposed" as const,
      candidateDigest: input.candidateDigest,
      commitProposalRef: `commit-proposal:${input.candidateDigest}`,
      issueRef: "github.issue:OpenAgentsInc/openagents#fixture",
      pullRequestRef: "github.pr:OpenAgentsInc/openagents#fixture",
    }
  }),
})

describe("reviewed discovery-to-regression lifecycle", () => {
  test("defaults to validated with no SCM mutation authority", async () => {
    const result = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      { rerunner: passingRerunner() },
    ))

    expect(result.state).toBe("validated")
    expect(result.projectionCandidate?._tag).toBe("validated")
    expect(result.distilledTest).toBeUndefined()
  })

  test("a failed rerun is INCONCLUSIVE and never reaches SCM", async () => {
    let scmCalls = 0
    const result = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: {
          rerun: input => Effect.succeed({
            status: "failed",
            candidateDigest: input.candidateDigest,
            blockerRef: "blocker.qa_swarm.regression_lifecycle.rerun_failed",
          }),
        },
        scmAuthority: {
          propose: input => {
            scmCalls += 1
            return Effect.succeed({
              status: "refused",
              candidateDigest: input.candidateDigest,
              blockerRef: "must-not-run",
            })
          },
        },
      },
    ))

    expect(result.state).toBe("inconclusive")
    expect(result.blockerRefs).toEqual([
      "blocker.qa_swarm.regression_lifecycle.rerun_failed",
    ])
    expect(scmCalls).toBe(0)
  })

  test("explicit authority creates only a proposal, never landed coverage", async () => {
    const scmCalls: Array<string> = []
    const result = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: proposalAuthority(scmCalls),
      },
    ))

    expect(result.state).toBe("proposed")
    expect(result.projectionCandidate?._tag).toBe("proposed")
    expect(result.distilledTest).toBeUndefined()
    expect(result.candidateDigest).toBeDefined()
    expect(scmCalls[0]).toBe(result.candidateDigest!)
  })

  test("only exact reviewed merge evidence produces landed/distilled state", async () => {
    const scmCalls: Array<string> = []
    const resolver: ReviewedMergeEvidenceResolver = {
      resolve: input => Effect.succeed({
        status: "reviewed_merged",
        candidateDigest: input.candidateDigest,
        pullRequestRef: input.pullRequestRef,
        mergedCommitRef: "git.commit:0123456789abcdef",
        reviewedMergeReceiptRef: "receipt.qa_swarm.reviewed_merge.fixture",
      }),
    }
    const result = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: proposalAuthority(scmCalls),
        mergeEvidenceResolver: resolver,
      },
    ))

    expect(result.state).toBe("landed")
    expect(result.projectionCandidate?._tag).toBe("landed")
    expect(result.distilledTest?.receiptRef).toBe(
      "receipt.qa_swarm.reviewed_merge.fixture",
    )
  })

  test("candidate digest substitution fails closed at rerun and merge boundaries", async () => {
    const discovery = observedDiscovery()
    const rerunMismatch = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      discovery,
      {
        rerunner: {
          rerun: () => Effect.succeed({
            status: "passed",
            candidateDigest: "substituted",
            receiptRef: "receipt.qa_swarm.rerun.substituted",
          }),
        },
      },
    ))
    expect(rerunMismatch.state).toBe("inconclusive")
    expect(rerunMismatch.blockerRefs[0]).toContain("candidate_digest_mismatch")

    const scmCalls: Array<string> = []
    const mergeMismatch = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      discovery,
      {
        rerunner: passingRerunner(),
        scmAuthority: proposalAuthority(scmCalls),
        mergeEvidenceResolver: {
          resolve: input => Effect.succeed({
            status: "reviewed_merged",
            candidateDigest: "substituted",
            pullRequestRef: input.pullRequestRef,
            mergedCommitRef: "git.commit:substituted",
            reviewedMergeReceiptRef: "receipt.qa_swarm.merge.substituted",
          }),
        },
      },
    ))
    expect(mergeMismatch.state).toBe("inconclusive")
    expect(mergeMismatch.distilledTest).toBeUndefined()
  })

  test("dependency failures remain bounded at their last proven state", async () => {
    const rerunFailure = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      { rerunner: { rerun: () => Effect.fail("offline") } },
    ))
    expect(rerunFailure.state).toBe("inconclusive")
    expect(rerunFailure.blockerRefs).toContain(
      "blocker.qa_swarm.regression_lifecycle.rerun_unavailable",
    )

    const scmFailure = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: { propose: () => Effect.fail("offline") },
      },
    ))
    expect(scmFailure.state).toBe("validated")
    expect(scmFailure.blockerRefs).toContain(
      "blocker.qa_swarm.regression_lifecycle.scm_unavailable",
    )

    const scmCalls: Array<string> = []
    const mergeFailure = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: proposalAuthority(scmCalls),
        mergeEvidenceResolver: { resolve: () => Effect.fail("offline") },
      },
    ))
    expect(mergeFailure.state).toBe("proposed")
    expect(mergeFailure.blockerRefs).toContain(
      "blocker.qa_swarm.regression_lifecycle.merge_resolver_unavailable",
    )
  })

  test("empty proposal and reviewed-merge refs cannot advance state", async () => {
    const emptyProposal = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: {
          propose: input => Effect.succeed({
            status: "proposed",
            candidateDigest: input.candidateDigest,
            commitProposalRef: "",
            issueRef: "",
            pullRequestRef: "",
          }),
        },
      },
    ))
    expect(emptyProposal.state).toBe("validated")

    const scmCalls: Array<string> = []
    const emptyMerge = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      observedDiscovery(),
      {
        rerunner: passingRerunner(),
        scmAuthority: proposalAuthority(scmCalls),
        mergeEvidenceResolver: {
          resolve: input => Effect.succeed({
            status: "reviewed_merged",
            candidateDigest: input.candidateDigest,
            pullRequestRef: input.pullRequestRef,
            mergedCommitRef: "",
            reviewedMergeReceiptRef: "",
          }),
        },
      },
    ))
    expect(emptyMerge.state).toBe("proposed")
    expect(emptyMerge.distilledTest).toBeUndefined()
  })

  test("unbound observations are INCONCLUSIVE before distillation", async () => {
    const discovery = observedDiscovery()
    const result = await Effect.runPromise(runDiscoveryRegressionLifecycle(
      {
        ...discovery,
        observationReceiptRef: "receipt.qa_swarm.discovery.not_bound",
      },
      { rerunner: passingRerunner() },
    ))

    expect(result.state).toBe("inconclusive")
    expect(result.candidateRef).toBeUndefined()
  })
})
