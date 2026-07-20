import { describe, expect, test } from "vite-plus/test";

import {
  adjudicateDoneCondition,
  decodeDoneConditionVerdict,
  isObjectiveVerified,
  recordRunTerminalState,
  type DoneConditionEvidence,
  type DoneConditionObjective,
} from "../src/index.ts";

const AT = "2026-07-20T00:00:00Z";
const COMMIT = "a3b02edbdd0000000000000000000000000000000";

const repoObjective: DoneConditionObjective = { objectiveClass: "repository", commit: COMMIT };

const freshMatching: DoneConditionEvidence = {
  present: true,
  errored: false,
  commit: COMMIT,
  fresh: true,
  objectiveMet: true,
  evidenceRef: "sweep-receipt:abc",
};

describe("adjudicateDoneCondition — the happy path is narrow", () => {
  test("fresh, commit-bound, matching evidence yields verified", () => {
    const v = adjudicateDoneCondition(repoObjective, freshMatching, AT);
    expect(v.state).toBe("verified");
    expect(v.objectiveClass).toBe("repository");
    expect(v.evidenceRef).toBe("sweep-receipt:abc");
  });
});

describe("adjudicateDoneCondition — fail-closed rules", () => {
  test("an absent oracle yields unavailable, never verified", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, present: false }, AT);
    expect(v.state).toBe("unavailable");
  });

  test("an oracle error yields unverified", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, errored: true }, AT);
    expect(v.state).toBe("unverified");
    expect(v.reason).toContain("errored");
  });

  test("evidence not bound to the run commit yields unverified", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, commit: "deadbeef" }, AT);
    expect(v.state).toBe("unverified");
    expect(v.reason).toContain("commit");
  });

  test("unbound (null commit) evidence yields unverified", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, commit: null }, AT);
    expect(v.state).toBe("unverified");
  });

  test("stale evidence yields unverified even when it matches", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, fresh: false }, AT);
    expect(v.state).toBe("unverified");
    expect(v.reason).toContain("stale");
  });

  test("objective-not-met yields unverified", () => {
    const v = adjudicateDoneCondition(repoObjective, { ...freshMatching, objectiveMet: false }, AT);
    expect(v.state).toBe("unverified");
  });

  test("an unsupported objective is always unavailable, never verified", () => {
    const v = adjudicateDoneCondition(
      { objectiveClass: "unsupported", commit: COMMIT },
      freshMatching,
      AT,
    );
    expect(v.state).toBe("unavailable");
    expect(v.objectiveClass).toBe("unsupported");
  });

  test("no fail-closed case ever returns verified", () => {
    const failClosed: ReadonlyArray<DoneConditionEvidence> = [
      { ...freshMatching, present: false },
      { ...freshMatching, errored: true },
      { ...freshMatching, commit: null },
      { ...freshMatching, commit: "other" },
      { ...freshMatching, fresh: false },
      { ...freshMatching, objectiveMet: false },
    ];
    for (const e of failClosed) {
      expect(adjudicateDoneCondition(repoObjective, e, AT).state).not.toBe("verified");
    }
  });
});

describe("verdict is a distinct fact from the provider disposition", () => {
  test("a completed provider disposition does not make the objective verified", () => {
    const verdict = adjudicateDoneCondition(repoObjective, { ...freshMatching, present: false }, AT);
    const state = recordRunTerminalState("completed", verdict);
    expect(state.providerDisposition).toBe("completed");
    expect(state.doneCondition.state).toBe("unavailable");
    expect(isObjectiveVerified(state)).toBe(false);
  });

  test("objective completion reads only the oracle verdict, never the disposition", () => {
    const verified = adjudicateDoneCondition(repoObjective, freshMatching, AT);
    expect(isObjectiveVerified(recordRunTerminalState("failed", verified))).toBe(true);
    expect(isObjectiveVerified(recordRunTerminalState("completed", verified))).toBe(true);
  });
});

describe("verdicts decode against their schema", () => {
  test("a produced verdict round-trips through decode", () => {
    const v = adjudicateDoneCondition(repoObjective, freshMatching, AT);
    expect(decodeDoneConditionVerdict(v)).toEqual(v);
  });
});
