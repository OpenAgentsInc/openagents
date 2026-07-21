export const meta = {
  name: "review-round",
  description:
    "One anti-laundering adversarial review round over a frozen commit: N finder lenses, optional per-finding verification, fail-closed fold",
  whenToUse:
    "After landing correctness- or soundness-bearing work. args: {commit, context?, lenses?: [{key, prompt}], verify?: boolean}. Repeat with fresh lenses until a round returns clean.",
  phases: [
    { title: "Attack", detail: "one agent per lens over the frozen commit" },
    { title: "Verify", detail: "adversarial re-check of each finding (when verify is set)" },
  ],
};

// The anti-laundering disciplines, learned from `chenglou/freerange` and
// recorded in docs/teardowns/2026-07-21-freerange-teardown.md. The tested
// authority for the fold is `@openagentsinc/review-round` (packages/review-round);
// the rules below MUST NOT diverge from its `aggregateRound`.
//
//  1. Positive control. A lens with zero findings must report probesRun >= 1,
//     or the empty result cannot prove a sweep happened (lens-unproven).
//  2. Died lens surfaces. A null agent result becomes AGENT-DIED, never a
//     dropped row and never a clean pass. `.filter(Boolean)` is deliberately
//     NOT used on lens results.
//  3. Reproduced contradiction. A finding needs an observed contradiction
//     (failing command, crash, counterexample). No observation =
//     unsubstantiated, not a finding and not a pass.
//  4. Fail-closed status. Any failure row makes the round `failed` (rerun),
//     distinct from `findings` and from `clean`. A round that swept nothing is
//     `failed`, never vacuously clean.

const FINDINGS_SCHEMA = {
  type: "object",
  required: ["probesRun", "findings"],
  additionalProperties: false,
  properties: {
    // The positive control: how many concrete probes this lens executed. An
    // empty findings list is only trustworthy when probesRun >= 1.
    probesRun: { type: "integer", minimum: 0 },
    findings: {
      type: "array",
      items: {
        type: "object",
        required: ["title", "severity", "claim", "probe", "observed"],
        additionalProperties: false,
        properties: {
          title: { type: "string" },
          severity: { type: "string", enum: ["low", "medium", "high", "critical"] },
          claim: { type: "string" },
          probe: { type: "string", description: "the exact input/command run" },
          observed: {
            type: "string",
            description:
              "the reproduced contradiction: wrong output, crash, or counterexample. Empty = unsubstantiated.",
          },
        },
      },
    },
  },
};

const VERDICT_SCHEMA = {
  type: "object",
  required: ["refuted", "reason"],
  additionalProperties: false,
  properties: {
    refuted: { type: "boolean", description: "true if the finding could NOT be reproduced" },
    reason: { type: "string" },
  },
};

const DEFAULT_LENSES = [
  {
    key: "correctness",
    prompt:
      "Hunt for correctness defects: wrong results, off-by-one, unhandled inputs, broken invariants. Prefer a running counterexample over a claim.",
  },
  {
    key: "boundary",
    prompt:
      "Hunt for boundary and failure-path defects: empty/null, overflow, concurrency, error handling, resource limits. Prefer a reproduced failure over a claim.",
  },
  {
    key: "contract",
    prompt:
      "Hunt for contract violations: the change breaks a documented behavior, type, or invariant a caller relies on. Prefer a concrete violating call over a claim.",
  },
];

const PREAMBLE = [
  "You are a finder lens in an adversarial code review round over a FROZEN commit.",
  "Rules you must obey, because this round refuses to launder an unrun review into a clean result:",
  "- Report probesRun: the number of concrete probes you actually ran (files opened, commands executed, counterexamples tried). This is your positive control. If you report zero findings, probesRun MUST be >= 1 or the round treats your lens as unproven.",
  "- A finding requires a REPRODUCED contradiction in `observed`: a failing command, a crash, or a counterexample output. An assumption, a code smell, or an honest imprecision is NOT a finding — leave it out.",
  "- Aim at code people actually write. Do not hunt exotic spellings as if they were exploits.",
  "- Do not claim success you did not observe. If you found nothing after probing, return an empty findings list WITH your probesRun count.",
].join("\n");

function foldRound(lensResults) {
  // Mirror of `aggregateRound` in @openagentsinc/review-round. Keep in sync.
  const confirmedFindings = [];
  const failures = [];
  let lensesSwept = 0;
  let lensesReported = 0;
  let probesRun = 0;

  for (const { lens, report } of lensResults) {
    if (report === null || report === undefined) {
      failures.push({ lens, kind: "agent-died", detail: "lens runner returned no report" });
      continue;
    }
    if (typeof report.probesRun !== "number" || !Array.isArray(report.findings)) {
      failures.push({
        lens,
        kind: "malformed-report",
        detail: "report did not match the contract",
      });
      continue;
    }
    lensesReported += 1;
    probesRun += Math.max(0, Math.floor(report.probesRun));

    let substantiated = false;
    for (const finding of report.findings) {
      if (typeof finding.observed !== "string" || finding.observed.trim().length === 0) {
        failures.push({
          lens,
          kind: "unsubstantiated-finding",
          detail: `finding "${finding.title}" has no reproduced observation`,
        });
        continue;
      }
      substantiated = true;
      confirmedFindings.push({ ...finding, lens });
    }

    if (!substantiated) {
      if (report.probesRun >= 1) lensesSwept += 1;
      else
        failures.push({
          lens,
          kind: "lens-unproven",
          detail: "no confirmed findings and zero probes (no positive control)",
        });
    } else {
      lensesSwept += 1;
    }
  }

  if (failures.length === 0 && confirmedFindings.length === 0 && lensesSwept === 0) {
    failures.push({ lens: "(round)", kind: "no-sweep", detail: "no lens proved a sweep" });
  }

  const status =
    failures.length > 0 ? "failed" : confirmedFindings.length > 0 ? "findings" : "clean";

  return {
    schemaVersion: "openagents.review-round.v1",
    status,
    confirmedFindings,
    failures,
    lensesSwept,
    lensesReported,
    lensesAttempted: lensResults.length,
    probesRun,
  };
}

// `args` arrives as a parsed value, but some invocation paths deliver it as a
// JSON string. Accept both so the workflow does not reject a well-formed call.
const input = typeof args === "string" ? JSON.parse(args) : (args ?? {});
const commit = input.commit;
if (typeof commit !== "string" || commit.length === 0) {
  throw new Error("review-round requires args.commit (a frozen commit ref or SHA)");
}
const context = typeof input.context === "string" ? input.context : "";
const lenses =
  Array.isArray(input.lenses) && input.lenses.length > 0 ? input.lenses : DEFAULT_LENSES;
const verify = input.verify === true;

log(`review-round over ${commit}: ${lenses.length} lenses${verify ? " + verify" : ""}`);

phase("Attack");
// One agent per lens. A died agent resolves to null (parallel never rejects),
// and foldRound turns that null into an AGENT-DIED row rather than dropping it.
const lensResults = await parallel(
  lenses.map((lens) => async () => {
    const prompt = [
      PREAMBLE,
      "",
      `Frozen commit: ${commit}`,
      context ? `Context: ${context}` : "",
      "",
      `Your lens (${lens.key}): ${lens.prompt}`,
      "",
      `Inspect the diff of ${commit} and its touched files. Return your findings and probesRun.`,
    ]
      .filter((line) => line.length > 0)
      .join("\n");
    const report = await agent(prompt, {
      label: `attack:${lens.key}`,
      phase: "Attack",
      schema: FINDINGS_SCHEMA,
    });
    return { lens: lens.key, report };
  }),
);

let folded = foldRound(lensResults);

// Optional adversarial verification: every confirmed finding gets an
// independent skeptic prompted to REFUTE it. A refuted finding is removed and
// recorded as a failure so the round stays honest about what survived.
if (verify && folded.confirmedFindings.length > 0) {
  phase("Verify");
  const verdicts = await parallel(
    folded.confirmedFindings.map((finding) => async () => {
      const verdict = await agent(
        [
          `Adversarially verify a review finding over frozen commit ${commit}.`,
          `Try to REFUTE it. Default to refuted=true if you cannot reproduce it.`,
          "",
          `Title: ${finding.title}`,
          `Claim: ${finding.claim}`,
          `Probe: ${finding.probe}`,
          `Observed: ${finding.observed}`,
        ].join("\n"),
        {
          label: `verify:${finding.lens}:${finding.title.slice(0, 32)}`,
          phase: "Verify",
          schema: VERDICT_SCHEMA,
        },
      );
      return { finding, verdict };
    }),
  );
  const survived = [];
  const refuted = [];
  for (const { finding, verdict } of verdicts) {
    // A died verifier (null verdict) cannot clear a finding; keep it and flag.
    if (verdict === null || verdict === undefined) {
      survived.push(finding);
      folded.failures.push({
        lens: finding.lens,
        kind: "agent-died",
        detail: "verifier died; finding unverified",
      });
    } else if (verdict.refuted) {
      refuted.push({
        lens: finding.lens,
        kind: "unsubstantiated-finding",
        detail: `refuted on verify: ${verdict.reason}`,
      });
    } else {
      survived.push(finding);
    }
  }
  folded = {
    ...folded,
    confirmedFindings: survived,
    failures: [...folded.failures, ...refuted],
    status:
      folded.failures.length + refuted.length > 0
        ? "failed"
        : survived.length > 0
          ? "findings"
          : folded.lensesSwept > 0
            ? "clean"
            : "failed",
  };
}

log(
  `round ${folded.status}: ${folded.confirmedFindings.length} confirmed, ${folded.failures.length} failure rows, ${folded.lensesSwept}/${folded.lensesAttempted} lenses swept`,
);

return folded;
