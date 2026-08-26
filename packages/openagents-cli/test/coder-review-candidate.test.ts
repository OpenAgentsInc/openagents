/**
 * The candidate schema's checks, at the level they are enforced: parse time.
 *
 * Every case here is a way a reviewer can produce a document that reads like a
 * review and is not one. The parser's job is to name which way it was, so
 * these tests assert the reason and the path, not merely that something was
 * refused. A refusal with the wrong name is the same defect as no refusal.
 */

import { describe, expect, it } from "vitest";

import {
  CODER_CANDIDATE_SCHEMA,
  candidateIdOf,
  extractJsonObject,
  MAX_PROPOSALS,
  parseCycleReview,
  resolveEvidenceRef,
  type EvidenceIndex,
  type RejectionReason,
} from "../src/coder-review-candidate.js";

const index: EvidenceIndex = {
  trajectorySteps: new Set(["regex-log#step-1", "regex-log#step-2"]),
  trialOutcomes: new Set(["regex-log"]),
  benchRows: new Set(["tb2-quick#2026-08-26T09:10:00.000Z"]),
  ledgerEntries: new Set(["T1"]),
  diffPaths: new Set(["packages/openagents-cli/src/coder-tools.ts"]),
};

const proposal = (evidence: ReadonlyArray<string>): Record<string, unknown> => ({
  lever: { axis: "harness", summary: "A change." },
  surfaces: [],
  transferLabel: { modelFamily: "fixture-model", lane: "proxy" },
  evidence,
  risk: "Something could go wrong.",
  verification: { suite: "tb2-quick", metric: "successRate", expectedDirection: "up" },
});

const review = (overrides: Record<string, unknown> = {}): string =>
  JSON.stringify({
    score: 5,
    points: [{ point: "A point.", delta: 5, evidence: ["trial:regex-log#step-1"] }],
    causality: "The lever caused it.",
    violations: [],
    proposals: [proposal(["trial:regex-log#step-1"])],
    ledgerOperations: [],
    ...overrides,
  });

const reasons = (raw: string): ReadonlyArray<RejectionReason> => {
  const result = parseCycleReview(raw, index, "test");
  if (result.ok) throw new Error("expected a refusal");
  return result.rejections.map((rejection) => rejection.reason);
};

describe("evidence refs", () => {
  it("resolves each of the five schemes against the request", () => {
    expect(resolveEvidenceRef(index, "trial:regex-log#step-2")).toEqual({
      ok: true,
      kind: "trajectory_step",
    });
    expect(resolveEvidenceRef(index, "trial:regex-log#outcome")).toEqual({
      ok: true,
      kind: "trial_outcome",
    });
    expect(resolveEvidenceRef(index, "row:tb2-quick#2026-08-26T09:10:00.000Z")).toEqual({
      ok: true,
      kind: "bench_row",
    });
    expect(resolveEvidenceRef(index, "ledger:T1")).toEqual({ ok: true, kind: "ledger_entry" });
    expect(
      resolveEvidenceRef(index, "diff:packages/openagents-cli/src/coder-tools.ts"),
    ).toMatchObject({ ok: true, kind: "diff_path" });
  });

  it("separates a malformed ref, an unknown scheme, and one that names nothing", () => {
    expect(resolveEvidenceRef(index, "regex-log")).toMatchObject({
      ok: false,
      reason: "evidence_ref_malformed",
    });
    expect(resolveEvidenceRef(index, "transcript:regex-log")).toMatchObject({
      ok: false,
      reason: "evidence_ref_unknown_scheme",
    });
    expect(resolveEvidenceRef(index, "trial:regex-log#step-99")).toMatchObject({
      ok: false,
      reason: "evidence_ref_unresolved",
    });
  });

  it("says which step and which trial when a step was never in the request", () => {
    const result = resolveEvidenceRef(index, "trial:regex-log#step-99");
    if (result.ok) throw new Error("expected a refusal");
    expect(result.detail).toContain("step-99");
    expect(result.detail).toContain("regex-log");
  });

  it("refuses a ref against an empty index rather than accepting it unchecked", () => {
    const nothing: EvidenceIndex = {
      trajectorySteps: new Set(),
      trialOutcomes: new Set(),
      benchRows: new Set(),
      ledgerEntries: new Set(),
      diffPaths: new Set(),
    };
    expect(resolveEvidenceRef(nothing, "ledger:T1")).toMatchObject({
      ok: false,
      reason: "evidence_ref_unresolved",
    });
  });
});

describe("parseCycleReview", () => {
  it("accepts a review whose citations all resolve", () => {
    const result = parseCycleReview(review(), index, "coder-review:job:replay:fixture");
    if (!result.ok) throw new Error(JSON.stringify(result.rejections));
    expect(result.review.score).toBe(5);
    expect(result.review.proposals).toHaveLength(1);
    expect(result.review.proposals[0]!.schema).toBe(CODER_CANDIDATE_SCHEMA);
    expect(result.review.proposals[0]!.lineage.producedBy).toBe("coder-review:job:replay:fixture");
    expect(result.review.proposals[0]!.lineage.origin).toBe("review");
  });

  it("refuses a proposal that cites a step the reviewer was never given", () => {
    expect(reasons(review({ proposals: [proposal(["trial:regex-log#step-42"])] }))).toContain(
      "evidence_ref_unresolved",
    );
  });

  it("refuses a proposal whose only citation is a row or a ledger entry", () => {
    expect(reasons(review({ proposals: [proposal(["ledger:T1"])] }))).toContain(
      "proposal_without_trajectory_evidence",
    );
    expect(
      reasons(review({ proposals: [proposal(["row:tb2-quick#2026-08-26T09:10:00.000Z"])] })),
    ).toContain("proposal_without_trajectory_evidence");
  });

  it("refuses a proposal that cites nothing at all", () => {
    expect(reasons(review({ proposals: [proposal([])] }))).toContain("proposal_without_evidence");
  });

  it("refuses an empty proposal list rather than reading it as approval", () => {
    expect(reasons(review({ proposals: [] }))).toContain("no_proposals");
  });

  it("refuses more proposals than one review may carry", () => {
    const many = Array.from({ length: MAX_PROPOSALS + 1 }, () =>
      proposal(["trial:regex-log#step-1"]),
    );
    expect(reasons(review({ proposals: many }))).toContain("too_many_proposals");
  });

  it("names an unknown axis, direction, ledger op, and status", () => {
    expect(
      reasons(
        review({
          proposals: [
            { ...proposal(["trial:regex-log#step-1"]), lever: { axis: "vibes", summary: "x" } },
          ],
        }),
      ),
    ).toContain("unknown_lever_axis");
    expect(
      reasons(
        review({
          proposals: [
            {
              ...proposal(["trial:regex-log#step-1"]),
              verification: {
                suite: "tb2-quick",
                metric: "successRate",
                expectedDirection: "sideways",
              },
            },
          ],
        }),
      ),
    ).toContain("unknown_delta_direction");
    expect(
      reasons(
        review({
          ledgerOperations: [
            {
              op: "bless",
              entry: {
                id: null,
                section: "Tool habits",
                title: "t",
                statement: "s",
                detection: "d",
                status: "believed",
              },
              provenance: ["trial:regex-log#step-1"],
            },
          ],
        }),
      ),
    ).toEqual(expect.arrayContaining(["unknown_ledger_op", "unknown_ledger_status"]));
  });

  it("refuses a ledger operation with no provenance", () => {
    expect(
      reasons(
        review({
          ledgerOperations: [
            {
              op: "add",
              entry: {
                id: null,
                section: "Tool habits",
                title: "t",
                statement: "s",
                detection: "d",
                status: "proposed",
              },
              provenance: [],
            },
          ],
        }),
      ),
    ).toContain("proposal_without_evidence");
  });

  it("refuses a score outside the range, and output that is not JSON at all", () => {
    expect(reasons(review({ score: 11 }))).toContain("score_out_of_range");
    expect(reasons("I would rather not.")).toEqual(["not_json"]);
    expect(reasons("[1, 2, 3]")).toEqual(["not_an_object"]);
  });
});

describe("extractJsonObject", () => {
  it("takes the object out of a fenced block or a sentence around it", () => {
    expect(extractJsonObject('```json\n{"a":1}\n```')).toBe('{"a":1}');
    expect(extractJsonObject('Here it is: {"a":1} — hope that helps.')).toBe('{"a":1}');
    expect(extractJsonObject('{"a":1}')).toBe('{"a":1}');
  });
});

describe("candidateIdOf", () => {
  it("is stable across evidence order and changes with the lever", () => {
    const base = {
      schema: CODER_CANDIDATE_SCHEMA,
      lever: { axis: "harness", summary: "A change." },
      surfaces: [],
      lineage: { origin: "review", parent: null, producedBy: "a" },
      transferLabel: { modelFamily: "m", lane: "proxy" },
      evidence: [{ ref: "trial:regex-log#step-1", note: "" }],
      risk: "r",
      verification: { suite: "tb2-quick", metric: "successRate", expectedDirection: "up" },
    } as const;

    const reordered = {
      ...base,
      // A different producer and a different note are not different candidates.
      lineage: { origin: "review", parent: null, producedBy: "b" },
      evidence: [{ ref: "trial:regex-log#step-1", note: "a note" }],
    } as const;

    expect(candidateIdOf(base)).toBe(candidateIdOf(reordered));
    expect(candidateIdOf({ ...base, lever: { axis: "process", summary: "A change." } })).not.toBe(
      candidateIdOf(base),
    );
  });
});
