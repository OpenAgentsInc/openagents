import { describe, expect, test } from "vite-plus/test";

import {
  QA_VERIFIER_CLAIMS_SCHEMA,
  QA_VERIFIER_VERDICT_SCHEMA,
  QA_VERIFIER_ISSUE,
  applyMutation,
  artifactFileName,
  boundTail,
  buildVerdictComment,
  computeVerdict,
  decodeVerifierClaims,
  independenceProblem,
  renderArgv,
  type ClaimResult,
  type VerdictArtifact,
} from "./qa-verify-registry.js";

const validClaimsFile = () => ({
  schemaVersion: QA_VERIFIER_CLAIMS_SCHEMA,
  issue: 8907,
  commit: "08096cae24aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  source: "issue #8907 closing comment 2026-07-16T13:52:33Z",
  implementer: "fable-qa-observer-20260716",
  claims: [
    {
      kind: "command",
      id: "tests.observer",
      title: "24/24 observer tests pass",
      command: ["pnpm", "vp", "test", "--run", "--root", ".", "scripts/qa-observer.test.ts"],
    },
    {
      kind: "file_exists",
      id: "artifact.first-run",
      title: "first production run artifact is committed",
      path: "docs/qa/observer/results/qa-observer-run-2026-07-16T13-49-02Z.json",
    },
    {
      kind: "adversarial",
      id: "adversarial.exit-gate",
      title: "breaking the high-severity exit gate is caught by the cited tests",
      probes: "tests.observer",
      mutation: {
        file: "scripts/qa-observer.ts",
        find: "exitCode: highSeverityDrift > 0 ? 1 : 0,",
        replace: "exitCode: 0,",
      },
      command: ["pnpm", "vp", "test", "--run", "--root", ".", "scripts/qa-observer.test.ts"],
    },
    {
      kind: "attested",
      id: "run.admin-token",
      title: "7/7 pass including capture-health with the admin token",
      reason: "requires OPENAGENTS_ADMIN_API_TOKEN which is owner-gated",
    },
  ],
});

describe("decodeVerifierClaims", () => {
  test("a valid claims file decodes cleanly", () => {
    const decoded = decodeVerifierClaims(validClaimsFile());
    expect("problems" in decoded ? decoded.problems : []).toEqual([]);
    if ("file" in decoded) {
      expect(decoded.file.issue).toBe(8907);
      expect(decoded.file.claims).toHaveLength(4);
    }
  });

  test("wrong schemaVersion, bad issue, and bad commit are all reported", () => {
    const decoded = decodeVerifierClaims({
      ...validClaimsFile(),
      schemaVersion: "nope",
      issue: -1,
      commit: "not-a-sha",
    });
    expect("problems" in decoded).toBe(true);
    if ("problems" in decoded) {
      expect(decoded.problems.join("\n")).toContain("schemaVersion");
      expect(decoded.problems.join("\n")).toContain("issue must be a positive integer");
      expect(decoded.problems.join("\n")).toContain("commit must be a hex sha");
    }
  });

  test("unknown claim kinds, duplicate ids, and empty claims are rejected", () => {
    const base = validClaimsFile();
    const dupe = decodeVerifierClaims({
      ...base,
      claims: [base.claims[0], base.claims[0]],
    });
    expect("problems" in dupe && dupe.problems.some((p) => p.includes("duplicate id"))).toBe(true);

    const unknown = decodeVerifierClaims({
      ...base,
      claims: [{ kind: "vibes", id: "x", title: "y" }],
    });
    expect("problems" in unknown && unknown.problems.some((p) => p.includes("kind must be"))).toBe(
      true,
    );

    const empty = decodeVerifierClaims({ ...base, claims: [] });
    expect("problems" in empty && empty.problems.some((p) => p.includes("non-empty array"))).toBe(
      true,
    );
  });

  test("an adversarial claim must reference an existing command claim and carry a real mutation", () => {
    const base = validClaimsFile();
    const claims = base.claims.map((claim) =>
      claim.id === "adversarial.exit-gate" ? { ...claim, probes: "missing.claim" } : claim,
    );
    const decoded = decodeVerifierClaims({ ...base, claims });
    expect(
      "problems" in decoded &&
        decoded.problems.some((p) => p.includes("references unknown claim id")),
    ).toBe(true);

    const sameFindReplace = decodeVerifierClaims({
      ...base,
      claims: base.claims.map((claim) =>
        claim.id === "adversarial.exit-gate"
          ? { ...claim, mutation: { file: "a.ts", find: "x", replace: "x" } }
          : claim,
      ),
    });
    expect(
      "problems" in sameFindReplace &&
        sameFindReplace.problems.some((p) => p.includes("mutation requires")),
    ).toBe(true);
  });

  test("attested claims require the exact unverifiable reason", () => {
    const base = validClaimsFile();
    const decoded = decodeVerifierClaims({
      ...base,
      claims: [base.claims[0], { kind: "attested", id: "a", title: "b", reason: "" }],
    });
    expect(
      "problems" in decoded &&
        decoded.problems.some((p) => p.includes("exact unverifiable reason")),
    ).toBe(true);
  });
});

describe("independenceProblem — no agent accepts its own work", () => {
  test("the implementer cannot verify their own work, even with cosmetic renames", () => {
    expect(independenceProblem("fable-x", "fable-x")).toContain("self-verification");
    expect(independenceProblem("Fable-X", "  fable-x ")).toContain("self-verification");
  });

  test("a distinct verifier passes; empty verifier is refused", () => {
    expect(independenceProblem("fable-x", "fable-y")).toBeUndefined();
    expect(independenceProblem(undefined, "fable-y")).toBeUndefined();
    expect(independenceProblem("fable-x", "  ")).toContain("non-empty");
  });
});

const result = (overrides: Partial<ClaimResult>): ClaimResult => ({
  durationMs: 1,
  id: "claim",
  kind: "command",
  status: "verified",
  title: "a claim",
  ...overrides,
});

describe("computeVerdict", () => {
  const verifiedCommand = result({ id: "tests.a", kind: "command", status: "verified" });
  const verifiedAdversarial = result({
    id: "adv.a",
    kind: "adversarial",
    status: "verified",
  });

  test("accept requires a verified command re-run AND a verified adversarial probe", () => {
    const accept = computeVerdict([verifiedCommand, verifiedAdversarial]);
    expect(accept.verdict).toBe("accept");
    expect(accept.reasons.join(" ")).toContain("adversarial probe");
  });

  test("any failed claim is a reject with the failing claims named", () => {
    const rejected = computeVerdict([
      verifiedCommand,
      verifiedAdversarial,
      result({ id: "smoke.b", reason: "command exited 1", status: "failed" }),
    ]);
    expect(rejected.verdict).toBe("reject");
    expect(rejected.reasons.join(" ")).toContain("smoke.b");
  });

  test("no runnable proof means unverifiable-here — env-gated claims never auto-accept", () => {
    const gatedOnly = computeVerdict([
      result({
        id: "gated",
        kind: "command",
        reason: "requires env X",
        status: "unverifiable_here",
      }),
      result({ id: "att", kind: "attested", status: "unverifiable_here" }),
    ]);
    expect(gatedOnly.verdict).toBe("unverifiable-here");
    expect(gatedOnly.reasons.join(" ")).toContain("never auto-accepted");
  });

  test("a verified command without a verified adversarial probe is not an accept", () => {
    const noProbe = computeVerdict([verifiedCommand]);
    expect(noProbe.verdict).toBe("unverifiable-here");
    expect(noProbe.reasons.join(" ")).toContain("no adversarial probe");
  });

  test("file_exists alone can never accept", () => {
    const filesOnly = computeVerdict([
      result({ id: "f", kind: "file_exists", status: "verified" }),
    ]);
    expect(filesOnly.verdict).toBe("unverifiable-here");
  });
});

describe("mutation and output helpers", () => {
  test("applyMutation replaces the first occurrence and reports a missing anchor", () => {
    expect(applyMutation("a b a", "a", "z")).toBe("z b a");
    expect(applyMutation("a b", "missing", "z")).toBeUndefined();
  });

  test("boundTail keeps the verbatim tail within the limit", () => {
    expect(boundTail("short output\n")).toBe("short output");
    const long = `${"x".repeat(3000)}TAIL`;
    const bounded = boundTail(long, 100);
    expect(bounded.startsWith("…")).toBe(true);
    expect(bounded.endsWith("TAIL")).toBe(true);
    expect(bounded.length).toBe(101);
  });

  test("renderArgv shell-quotes only what needs quoting", () => {
    expect(renderArgv(["pnpm", "vp", "test", "--root", "."])).toBe("pnpm vp test --root .");
    expect(renderArgv(["sh", "-c", "echo hi"])).toBe("sh -c 'echo hi'");
  });

  test("artifactFileName pins issue and short commit", () => {
    expect(artifactFileName(8907, "08096cae24f00dabcdef1234567890abcdef1234")).toBe(
      "qa-verify-issue-8907-08096cae24f0.json",
    );
  });
});

describe("buildVerdictComment", () => {
  const artifact: VerdictArtifact = {
    schemaVersion: QA_VERIFIER_VERDICT_SCHEMA,
    verifierIssue: QA_VERIFIER_ISSUE,
    workUnit: {
      issue: 8907,
      claimedCommit: "08096cae24",
      resolvedCommit: "08096cae24f00dabcdef1234567890abcdef1234",
      source: "issue #8907 closing comment",
    },
    implementer: "fable-qa-observer-20260716",
    verifier: "fable-qa-verifier-20260716",
    runAt: "2026-07-16T14:00:00.000Z",
    scratch: { installOk: true },
    setup: [],
    claims: [
      result({ id: "tests.a", reason: "re-ran clean (exit 0)", status: "verified" }),
      result({
        id: "gated",
        kind: "command",
        reason: "requires env OPENAGENTS_ADMIN_API_TOKEN not present in the verifier environment",
        status: "unverifiable_here",
      }),
    ],
    verdict: "accept",
    verdictReasons: ["all re-run claims verified"],
  };

  test("the comment names the verdict, both actors, per-claim states, and the authority boundary", () => {
    const comment = buildVerdictComment(artifact);
    expect(comment).toContain("ACCEPT");
    expect(comment).toContain("#8907");
    expect(comment).toContain("fable-qa-verifier-20260716");
    expect(comment).toContain("independent of implementer");
    expect(comment).toContain("**verified** `tests.a`");
    expect(comment).toContain("**unverifiable-here** `gated`");
    expect(comment).toContain("OPENAGENTS_ADMIN_API_TOKEN");
    expect(comment).toContain("not merge/release/public-claim authority");
    expect(comment).toContain("qa-verify-issue-8907-08096cae24f0.json");
  });
});
