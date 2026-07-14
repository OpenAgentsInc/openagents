import { describe, expect, test } from "bun:test"

import {
  QA_SWARM_RUN_PROJECTION_SCHEMA,
  QaSwarmRegressionCandidate,
  assertResolverBackedQaSwarmProjection,
  buildResolverBackedQaSwarmBoardGraph,
  type QaSwarmProjectionEvidence,
} from "./index"
import { Schema as S } from "effect"

const evidence = (): QaSwarmProjectionEvidence => ({
  coverageFrontier: [],
  distilledTests: [],
  generatedAt: "2026-07-14T00:00:00.000Z",
  opaqueTargetRefs: [],
  perfBudgets: [],
  projectionRef: "projection.qa_swarm.run.example",
  publicSafetyRefs: [],
  runRef: "qa-run.example",
  schemaVersion: QA_SWARM_RUN_PROJECTION_SCHEMA,
  staleness: {
    contractVersion: "projection_staleness.v1",
    maxAgeHours: 24,
    mode: "artifact_snapshot",
  },
  target: { label: "Example", ref: "artifact.qa_swarm.target.example", visibility: "opaque" },
  title: "Example",
  traceRefs: ["trace.qa_swarm.example.observed"],
  verdict: "passed",
  verdictWall: [{
    label: "Scenario",
    receiptRef: "artifact.qa_swarm.verdict.example",
    summary: "Observed",
    verdict: "passed",
  }],
  videoRefs: [],
})

describe("QA Swarm shared projection", () => {
  test("distinguishes validated, proposed, and reviewed-merge-only landed regressions", () => {
    const decode = S.decodeUnknownSync(QaSwarmRegressionCandidate)

    expect(decode({
      _tag: "validated",
      candidateRef: "candidate.example",
      discoveryRef: "discovery.example",
      label: "Example",
      rerunReceiptRef: "receipt.rerun.example",
      testHref: "generated/example.e2e.test.ts",
    })._tag).toBe("validated")
    expect(decode({
      _tag: "proposed",
      candidateRef: "candidate.example",
      commitProposalRef: "commit-proposal:example",
      discoveryRef: "discovery.example",
      issueRef: "github.issue:example",
      label: "Example",
      pullRequestRef: "github.pr:example",
      rerunReceiptRef: "receipt.rerun.example",
      testHref: "generated/example.e2e.test.ts",
    })._tag).toBe("proposed")
    expect(() => decode({
      _tag: "proposed",
      candidateRef: "candidate.example",
      commitProposalRef: "commit-proposal:example",
      discoveryRef: "discovery.example",
      label: "Example",
      pullRequestRef: "github.pr:example",
      rerunReceiptRef: "receipt.rerun.example",
      testHref: "generated/example.e2e.test.ts",
    })).toThrow()
    expect(() => decode({
      _tag: "landed",
      candidateRef: "candidate.example",
      discoveryRef: "discovery.example",
      label: "Example",
      mergedCommitRef: "git.commit:example",
      pullRequestRef: "github.pr:example",
      rerunReceiptRef: "receipt.rerun.example",
      testHref: "generated/example.e2e.test.ts",
    })).toThrow()
  })

  test("missing resolution cannot light an evidence edge and forces inconclusive", () => {
    const source = evidence()
    const resolved = buildResolverBackedQaSwarmBoardGraph(source, {
      resolve: receiptRef => ({
        status: "missing",
        blockerRef: `blocker.qa_swarm.receipt_missing.${receiptRef.split(".").at(-1)}`,
      }),
    })
    const projection = assertResolverBackedQaSwarmProjection({ ...source, ...resolved })

    expect(projection.verdict).toBe("inconclusive")
    expect(projection.evidenceAdmission.admittedReceiptRefs).toEqual([])
    expect(projection.boardGraph.links.every(link => link.status !== "evidence_backed")).toBe(true)
    expect(projection.blockerRefs).toContain("blocker.qa_swarm.receipt_missing.observed")
  })

  test("only exact resolver-admitted receipts can light edges", () => {
    const source = evidence()
    const admittedRef = source.verdictWall[0]!.receiptRef
    const resolved = buildResolverBackedQaSwarmBoardGraph(source, {
      resolve: receiptRef => receiptRef === admittedRef
        ? { status: "admitted", receiptRef }
        : { status: "missing", blockerRef: "blocker.qa_swarm.receipt_missing.trace" },
    })
    const projection = assertResolverBackedQaSwarmProjection({
      ...source,
      ...resolved,
      blockerRefs: resolved.evidenceAdmission.blockerRefs,
    })

    expect(projection.evidenceAdmission.admittedReceiptRefs).toEqual([admittedRef])
    expect(projection.boardGraph.links.some(link => link.status === "evidence_backed")).toBe(true)
  })

  test("rejects a graph whose evidence-looking ref was not admitted", () => {
    const source = evidence()
    const resolved = buildResolverBackedQaSwarmBoardGraph(source, {
      resolve: receiptRef => ({ status: "admitted", receiptRef }),
    })
    const first = resolved.boardGraph.links[0]!
    expect(() => assertResolverBackedQaSwarmProjection({
      ...source,
      ...resolved,
      blockerRefs: [],
      boardGraph: {
        ...resolved.boardGraph,
        links: [{
          ...first,
          evidenceRefs: ["trace.qa_swarm.unresolved"],
          status: "evidence_backed",
        }, ...resolved.boardGraph.links.slice(1)],
      },
    })).toThrow(/not resolver-backed/)
  })

  test("rejects resolver substitution instead of admitting a different receipt", () => {
    const source = evidence()
    const resolved = buildResolverBackedQaSwarmBoardGraph(source, {
      resolve: () => ({ status: "admitted", receiptRef: "trace.qa_swarm.substituted" }),
    })

    expect(resolved.evidenceAdmission.admittedReceiptRefs).toEqual([])
    expect(resolved.blockerRefs.some(ref => ref.includes("ref_mismatch"))).toBe(true)
    expect(resolved.boardGraph.links.every(link => link.status !== "evidence_backed")).toBe(true)
  })
})
