/**
 * The two documents a cycle review turns into: the prompt the reviewer reads,
 * and the review file that lands in `docs/coder/reviews/`.
 *
 * `docs/coder/runbook.md` §6 writes the prompt as a skeleton for a human to
 * fill in. Filling it in by hand is where the evidence rule goes soft — a
 * reviewer told "cite trajectory steps" with no way to know which steps exist
 * will cite plausible ones, and a hand-assembled prompt has no set to check
 * them against. So this renderer prints the citable refs, in full, next to the
 * evidence they name, and states what happens to a ref that is not on the
 * list: the review is refused, not repaired.
 *
 * The output contract is JSON because the adopt step is meant to become a diff
 * (autoimprove §7.5), and because a prose review cannot be checked. Everything
 * the reviewer writes is checked by {@link parseCycleReview} before any of it
 * reaches the file this module also renders, so the markdown is never a
 * transcription of what a model said — it is a rendering of what survived.
 */

import type { ReviewBenchRow, ReviewRequest, ReviewStep, ReviewTrial } from "./assemble.js";
import type {
  CoderCandidate,
  CycleReview,
  EvidenceIndex,
  LedgerOperation,
  ReviewRejection,
} from "./candidate.js";
import { LEVER_AXES } from "./candidate.js";

/** How many citable refs are printed before the list says it stopped. */
const MAX_LISTED_REFS = 400;

/**
 * The reviewer prompt for one assembled cycle.
 *
 * Every section is derived from {@link ReviewRequest}; nothing here reads the
 * working session, the repository, or the environment. A prompt that could
 * reach past its request would make "the review saw only the artifacts"
 * (autoimprove §3) a claim rather than a property.
 */
export const renderReviewPrompt = (request: ReviewRequest, index: EvidenceIndex): string => {
  const lines: Array<string> = [];

  lines.push(
    "You are reviewing one autoimprovement cycle of `openagents coder`.",
    "",
    "The cycle ran one lever against one suite and left the artifacts below.",
    "Judge the cycle, not the reviewer's own comfort: a review that praises a",
    "wasteful run is itself a defect the next review has to catch. There is no",
    "audience to appease.",
    "",
    `Suite: ${request.suite}. Lane: ${request.lane}. Job: ${request.jobDir}${
      request.jobId === null ? "" : ` (${request.jobId})`
    }.`,
    "",
  );

  lines.push("<lever>", `ref: ${request.lever.ref}`, "");
  if (request.lever.paths.length > 0) {
    lines.push(`paths: ${request.lever.paths.join(", ")}`, "");
  }
  lines.push(
    request.lever.diff === "" ? "(no diff was supplied with this cycle)" : request.lever.diff,
  );
  if (request.lever.truncation.dropped_trailing_lines > 0) {
    lines.push(
      "",
      `[${String(request.lever.truncation.dropped_trailing_lines)} of ${String(
        request.lever.truncation.total_lines,
      )} diff lines dropped from the end]`,
    );
  }
  lines.push("</lever>", "");

  lines.push("<rows>");
  if (request.rows.length === 0) {
    lines.push("(no store rows were supplied; you cannot speak to a delta)");
  } else {
    for (const row of request.rows) lines.push(renderRow(row));
  }
  lines.push("</rows>", "");

  lines.push("<trials>");
  for (const trial of request.trials) lines.push(...renderTrial(trial));
  lines.push("</trials>", "");

  lines.push("<practices>", `source: ${request.practices.path}`, "", request.practices.text);
  lines.push("</practices>", "");

  lines.push(...renderEvidenceGrammar(index));
  lines.push(...renderOutputContract());

  return lines.join("\n");
};

const renderRow = (row: ReviewBenchRow): string =>
  `${row.suite}#${row.recordedAt}  ${JSON.stringify(row.row)}`;

const renderTrial = (trial: ReviewTrial): ReadonlyArray<string> => {
  const lines: Array<string> = [];
  lines.push(
    "",
    `<trial task="${trial.task}" outcome="${trial.outcome}">`,
    `model: ${trial.modelId ?? "unrecorded"}   agent: ${trial.agentVersion ?? "unrecorded"}`,
    `prompt tokens: ${figure(trial.promptTokens)}   completion tokens: ${figure(
      trial.completionTokens,
    )}   cached input tokens: ${String(trial.cachedInputTokens)}`,
    `tool calls: ${figure(trial.toolCalls)}   wall clock: ${wallClock(trial)}`,
  );
  if (trial.exception !== null) lines.push(`exception: ${trial.exception}`);
  lines.push(
    `instruction (${trial.instructionSource}): ${trial.instruction ?? "(none recorded)"}`,
    `steps: ${String(trial.truncation.kept_steps)} of ${String(trial.truncation.total_steps)}${
      trial.truncation.tail_only
        ? `, tail only — the first ${String(
            trial.truncation.dropped_leading_steps,
          )} steps are not in this prompt and may not be cited`
        : ""
    }`,
    "",
  );
  for (const step of trial.steps) lines.push(...renderStep(trial.task, step));
  lines.push("</trial>");
  return lines;
};

const renderStep = (task: string, step: ReviewStep): ReadonlyArray<string> => {
  const lines: Array<string> = [
    `[trial:${task}#step-${step.stepId}] ${step.source}${
      step.promptTokens === null && step.completionTokens === null
        ? ""
        : `  (prompt ${figure(step.promptTokens)}, completion ${figure(step.completionTokens)})`
    }`,
  ];
  if (step.text !== "") lines.push(step.text);
  for (const call of step.toolCalls) {
    lines.push(`  call ${call.name}: ${call.arguments}`);
    if (call.observation !== "") lines.push(`  → ${call.observation}`);
  }
  lines.push("");
  return lines;
};

const figure = (value: number | null): string => (value === null ? "unrecorded" : String(value));

/** An absent duration says so. `0.0s` would be a measurement, and it is not one. */
const wallClock = (trial: ReviewTrial): string =>
  trial.wallClockSeconds === null ? "unrecorded" : `${trial.wallClockSeconds.toFixed(1)}s`;

/**
 * The ref grammar, and the exact set of refs this review may cite.
 *
 * Printing the set is the difference between an instruction and a contract.
 * The reviewer is not being asked to remember which steps it read; it is being
 * handed the list its citations are checked against, so an unresolvable ref is
 * a choice rather than an accident.
 */
const renderEvidenceGrammar = (index: EvidenceIndex): ReadonlyArray<string> => {
  const lines: Array<string> = [
    "## Evidence refs",
    "",
    "Every claim you make carries refs from this grammar:",
    "",
    "- `trial:<task>#step-<id>` — one step of that trial's trajectory",
    "- `trial:<task>#outcome` — that trial's verifier decision",
    "- `row:<suite>#<recordedAt>` — one store row above",
    "- `ledger:<id>` — one entry of the practices file, such as `ledger:T1`",
    "- `diff:<path>` — one path the lever's diff touches",
    "",
    "A ref that is not in the list below does not resolve, and a review with an",
    "unresolvable ref is refused whole — not partially accepted, not repaired.",
    "The same is true of a proposal that cites no trajectory step: a store row",
    "or a ledger entry says what changed, never what the coder did.",
    "",
  ];

  const listed = [
    ...[...index.trajectorySteps].sort().map((ref) => `trial:${ref}`),
    ...[...index.trialOutcomes].sort().map((task) => `trial:${task}#outcome`),
    ...[...index.benchRows].sort().map((ref) => `row:${ref}`),
    ...[...index.ledgerEntries].sort().map((id) => `ledger:${id}`),
    ...[...index.diffPaths].sort().map((path) => `diff:${path}`),
  ];

  lines.push("Citable refs:", "");
  for (const ref of listed.slice(0, MAX_LISTED_REFS)) lines.push(`- ${ref}`);
  if (listed.length > MAX_LISTED_REFS) {
    lines.push(
      `- [${String(listed.length - MAX_LISTED_REFS)} further refs not listed; they follow the same grammar over the trials above]`,
    );
  }
  lines.push("");
  return lines;
};

/**
 * The JSON the reviewer returns.
 *
 * Spelled as an annotated shape rather than a schema document: a reviewer that
 * has to parse a JSON Schema before writing its answer spends its attention on
 * the wrong artifact, and every field here has a one-line reason attached to
 * it that a schema would strip.
 */
const renderOutputContract = (): ReadonlyArray<string> => [
  "## Your answer",
  "",
  "Return one JSON object and nothing else. No prose around it.",
  "",
  "```json",
  "{",
  '  "score": 0,',
  '  "points": [',
  '    { "point": "what was done well or badly", "delta": -1,',
  '      "evidence": ["trial:<task>#step-<id>"] }',
  "  ],",
  '  "causality": "did the lever cause the delta, or does a confounder explain it",',
  '  "violations": [',
  '    { "entry": "T1", "note": "how the cycle broke it",',
  '      "evidence": ["trial:<task>#step-<id>"] }',
  "  ],",
  '  "proposals": [',
  "    {",
  `      "lever": { "axis": "${LEVER_AXES.join('" | "')}", "summary": "one sentence" },`,
  '      "surfaces": [{ "surface": "system-prompt", "diff": "the change, as a diff or as the proposed text" }],',
  '      "transferLabel": { "modelFamily": "the family this was written against", "lane": "the lane" },',
  '      "evidence": ["trial:<task>#step-<id>"],',
  '      "risk": "what goes wrong if this is adopted",',
  '      "verification": { "suite": "tb2-quick", "metric": "successRate", "expectedDirection": "up" }',
  "    }",
  "  ],",
  '  "ledgerOperations": [',
  "    {",
  '      "op": "add" | "promote" | "demote" | "refute",',
  '      "entry": { "id": null, "section": "Tool habits", "title": "short title",',
  '                 "statement": "the falsifiable claim", "detection": "how a violation is detected",',
  '                 "status": "adopted" | "proposed" | "refuted" },',
  '      "provenance": ["trial:<task>#step-<id>"]',
  "    }",
  "  ]",
  "}",
  "```",
  "",
  "Rules the parser enforces, so that writing around them fails rather than passes:",
  "",
  "- `score` is 0–10, and the `points` deltas add up to it.",
  "- `surfaces[].surface` is one of the staged text artifacts —",
  "  `system-prompt`, `tool-descriptions`, `catalog-lines` — and the `diff` is",
  "  over that artifact. A lever that changes code, a plugin, or procedure",
  "  rather than staged text carries an empty `surfaces` array.",
  "- One to three proposals. A cycle with nothing to change says so as a",
  "  proposal whose lever is the practice it would keep, with the evidence for",
  "  keeping it — never as an empty list.",
  "- Every proposal cites at least one `trial:...#step-...` ref.",
  "- Every ledger operation carries provenance, and every entry states how a",
  "  violation is detected. An entry nothing can detect is an aspiration and",
  "  does not get `adopted`.",
  "- Unknown is written as unknown. A figure recorded as `unrecorded` above is",
  "  not zero, and a delta you cannot attribute is a confounder you name.",
];

/**
 * The review file for `docs/coder/reviews/`.
 *
 * Rendered from the parsed review, so every ref in it has already resolved and
 * every enum in it is already one of the accepted values. The header carries
 * what a later reader needs to decide whether the review still applies: the
 * job, the lane, the lever, the reviewer, and the redaction stamp.
 */
export const renderReviewMarkdown = (options: {
  readonly request: ReviewRequest;
  readonly review: CycleReview;
  readonly reviewerRef: string;
  readonly recordedAt: string;
  readonly title: string;
}): string => {
  const { request, review, reviewerRef, recordedAt, title } = options;
  const lines: Array<string> = [
    `# ${title}`,
    "",
    `**Score ${String(review.score)}/10.** Cycle on \`${request.suite}\`, lane \`${request.lane}\`,`,
    `job \`${request.jobDir}\`${request.jobId === null ? "" : ` (\`${request.jobId}\`)`},`,
    `lever \`${request.lever.ref}\`. Reviewed ${recordedAt} by \`${reviewerRef}\`.`,
    "",
    `Redaction: \`${request.redaction.serviceRef}\` on surface \`${request.redaction.surface}\`,`,
    `${String(request.redaction.total)} replacement${request.redaction.total === 1 ? "" : "s"}`,
    `${
      request.redaction.total === 0
        ? "(nothing in the artifacts matched the rule list)"
        : `(${Object.entries(request.redaction.counts)
            .map(([category, count]) => `${category}: ${String(count)}`)
            .join(", ")})`
    } applied before the artifacts left this process.`,
    "",
    "## What ran",
    "",
    "| Trial | Outcome | Steps | Tool calls | Prompt | Completion | Wall clock |",
    "| --- | --- | --- | --- | --- | --- | --- |",
  ];

  for (const trial of request.trials) {
    lines.push(
      `| \`${trial.task}\` | ${trial.outcome} | ${String(trial.truncation.total_steps)}${
        trial.truncation.tail_only ? ` (tail ${String(trial.truncation.kept_steps)} reviewed)` : ""
      } | ${figure(trial.toolCalls)} | ${figure(trial.promptTokens)} | ${figure(
        trial.completionTokens,
      )} | ${wallClock(trial)} |`,
    );
  }

  lines.push("", "## Score", "");
  for (const point of review.points) {
    lines.push(
      `- **${point.delta >= 0 ? "+" : ""}${String(point.delta)}** ${point.point}${refs(
        point.evidence.map((entry) => entry.ref),
      )}`,
    );
  }

  lines.push("", "## Causality", "", review.causality, "", "## Practice violations", "");
  if (review.violations.length === 0) {
    lines.push("None found.");
  } else {
    for (const violation of review.violations) {
      lines.push(
        `- \`${violation.entry}\` — ${violation.note}${refs(
          violation.evidence.map((entry) => entry.ref),
        )}`,
      );
    }
  }

  lines.push("", "## Proposals", "");
  review.proposals.forEach((proposal, position) => {
    lines.push(...renderProposal(proposal, position + 1));
  });

  lines.push("## Ledger operations", "");
  if (review.ledgerOperations.length === 0) {
    lines.push("None proposed.");
  } else {
    for (const operation of review.ledgerOperations)
      lines.push(...renderLedgerOperation(operation));
  }

  lines.push(
    "",
    "---",
    "",
    "Adopting a proposal is a separate act from recording this review. Each one",
    "enters the runbook at §3 as a lever with its measuring suite already named;",
    "a rejected proposal stays here with the one-line reason it was rejected.",
    "",
  );

  return lines.join("\n");
};

const renderProposal = (proposal: CoderCandidate, position: number): ReadonlyArray<string> => {
  const lines: Array<string> = [
    `### ${String(position)}. ${proposal.lever.summary}`,
    "",
    `- **Axis** \`${proposal.lever.axis}\``,
    `- **Candidate** \`${proposal.candidateId}\` (${proposal.lineage.origin}, produced by \`${proposal.lineage.producedBy}\`${
      proposal.lineage.parent === null ? "" : `, from \`${proposal.lineage.parent}\``
    })`,
    `- **Written against** ${
      proposal.transferLabel.modelFamily === ""
        ? "an unstated model family"
        : `\`${proposal.transferLabel.modelFamily}\``
    } on ${
      proposal.transferLabel.lane === ""
        ? "an unstated lane"
        : `lane \`${proposal.transferLabel.lane}\``
    }`,
    `- **Verification** \`${proposal.verification.suite}\`, \`${proposal.verification.metric}\` should go ${proposal.verification.expectedDirection}`,
    `- **Risk** ${proposal.risk}`,
    `- **Evidence**${refs(proposal.evidence.map((entry) => entry.ref))}`,
    "",
  ];
  for (const surface of proposal.surfaces) {
    lines.push(`\`${surface.surface}\`:`, "", "```diff", surface.diff, "```", "");
  }
  return lines;
};

const renderLedgerOperation = (operation: LedgerOperation): ReadonlyArray<string> => [
  `- **${operation.op}** ${
    operation.entry.id === null ? "(new entry)" : `\`${operation.entry.id}\``
  } — ${operation.entry.section} / ${operation.entry.title} → \`${operation.entry.status}\``,
  `  - Statement: ${operation.entry.statement}`,
  `  - Detection: ${operation.entry.detection}`,
  `  - Provenance:${refs(operation.provenance.map((entry) => entry.ref))}`,
];

const refs = (values: ReadonlyArray<string>): string =>
  values.length === 0 ? "" : ` (${values.map((value) => `\`${value}\``).join(", ")})`;

/**
 * A refusal, written so the next attempt has somewhere to start.
 *
 * The rejection is named, located, and explained, because the failure this
 * command exists to stop is a document that reads as a review and is not one.
 * "The review was rejected" without the ref that failed is the same defect one
 * level up.
 */
export const renderRejections = (
  rejections: ReadonlyArray<ReviewRejection>,
  reviewerRef: string,
): string => {
  const lines: Array<string> = [
    `The review from \`${reviewerRef}\` was refused. Nothing was written to the reviews directory.`,
    "",
  ];
  for (const rejection of rejections) {
    lines.push(`- ${rejection.reason} at ${rejection.path}: ${rejection.detail}`);
  }
  lines.push(
    "",
    "A refused review is not a failed cycle. Re-run the reviewer, or read the",
    "refusals as what they are: the reviewer wrote about artifacts it was not given.",
  );
  return lines.join("\n");
};
