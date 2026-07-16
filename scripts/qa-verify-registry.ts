// QA-5 (#8910): independent-verifier claim/verdict types and pure logic.
//
// The transcripts are explicit (docs/transcripts/253-notes.md): the agent
// that implements a change must not be the authority that accepts it, and
// "executor self-verification or self-acceptance" is cut. This module owns
// the typed shapes for a verification work unit:
//   claims file  — what the implementer's closing comment claimed, mapped by
//                  a DIFFERENT agent into typed, mechanically runnable claims
//   verdict      — accept | reject | unverifiable-here with per-claim results
// The executor (scripts/qa-verify.ts) re-runs the claims from a clean
// scratch checkout of the claimed commit and writes the verdict artifact.
//
// Honesty contract (mirrors scripts/qa-observer-registry.ts): a claim whose
// precondition is missing (owner-gated env, unbuildable scratch, mutation
// anchor gone) is `unverifiable_here` with the reason. It is NEVER counted
// as verified and never auto-accepted.

export const QA_VERIFIER_CLAIMS_SCHEMA = "openagents.qa_verifier_claims.v1";
export const QA_VERIFIER_VERDICT_SCHEMA = "openagents.qa_verifier_verdict.v1";
export const QA_VERIFIER_ISSUE = "OpenAgentsInc/openagents#8910";
export const QA_VERIFIER_OUTPUT_TAIL_LIMIT = 2_000;

/** Per-claim honest states. */
export type ClaimStatus = "verified" | "failed" | "unverifiable_here";

/** Overall verdict for the work unit. */
export type VerifierVerdict = "accept" | "reject" | "unverifiable-here";

/**
 * A typed claim. `command` re-runs a named proof and expects exit 0.
 * `file_exists` checks a claimed committed artifact. `adversarial`
 * deliberately breaks a guarded behavior in the scratch copy and expects the
 * cited proof to FAIL (exit nonzero). `attested` records a claim that cannot
 * be mechanically re-run here, with the exact reason — always
 * `unverifiable_here`, never a pass.
 */
export type VerifierClaim =
  | Readonly<{
      kind: "command";
      id: string;
      /** The implementer's claim being re-checked, verbatim or near-verbatim. */
      title: string;
      /** argv, executed from the scratch checkout root (or `cwd` under it). */
      command: readonly string[];
      /** Optional working directory relative to the scratch root. */
      cwd?: string;
      timeoutMs?: number;
      /** Env var NAMES that must be present or the claim is unverifiable. */
      requiredEnv?: readonly string[];
    }>
  | Readonly<{
      kind: "file_exists";
      id: string;
      title: string;
      /** Path relative to the scratch checkout root. */
      path: string;
    }>
  | Readonly<{
      kind: "adversarial";
      id: string;
      title: string;
      /** The id of the command claim whose proof this probe stresses. */
      probes: string;
      mutation: Readonly<{
        /** File to mutate, relative to the scratch checkout root. */
        file: string;
        /** Exact source text to replace (first occurrence). */
        find: string;
        replace: string;
      }>;
      /** The cited proof; MUST exit nonzero against the mutated copy. */
      command: readonly string[];
      cwd?: string;
      timeoutMs?: number;
      requiredEnv?: readonly string[];
    }>
  | Readonly<{
      kind: "attested";
      id: string;
      title: string;
      /** Why this claim cannot be re-run in this environment. */
      reason: string;
    }>;

export const VERIFIER_CLAIM_KINDS = ["command", "file_exists", "adversarial", "attested"] as const;

export type SetupStep = Readonly<{
  title: string;
  command: readonly string[];
  cwd?: string;
  timeoutMs?: number;
}>;

export type VerifierClaimsFile = Readonly<{
  schemaVersion: typeof QA_VERIFIER_CLAIMS_SCHEMA;
  /** The issue whose closing claims are being verified. */
  issue: number;
  /** The claimed commit (full sha preferred; resolved by the executor). */
  commit: string;
  /** Where the claims came from, e.g. "issue #8907 closing comment <ts>". */
  source: string;
  /** Implementer actor/session, for the independence check. */
  implementer?: string;
  /** Environment prep run in the scratch after install (recorded, non-fatal). */
  setup?: readonly SetupStep[];
  claims: readonly VerifierClaim[];
}>;

export type ClaimResult = Readonly<{
  id: string;
  kind: VerifierClaim["kind"];
  title: string;
  status: ClaimStatus;
  /** Present when status is not "verified" — or extra context when it is. */
  reason?: string;
  /** Rendered command that was (or would have been) run. */
  command?: string;
  exitCode?: number | null;
  durationMs: number;
  /** Verbatim bounded tail of combined stdout+stderr. Never credentials. */
  outputTail?: string;
}>;

export type SetupResult = Readonly<{
  title: string;
  command: string;
  exitCode: number | null;
  durationMs: number;
  outputTail?: string;
}>;

export type VerdictArtifact = Readonly<{
  schemaVersion: typeof QA_VERIFIER_VERDICT_SCHEMA;
  verifierIssue: typeof QA_VERIFIER_ISSUE;
  workUnit: Readonly<{
    issue: number;
    claimedCommit: string;
    resolvedCommit: string;
    source: string;
  }>;
  implementer?: string;
  verifier: string;
  runAt: string;
  scratch: Readonly<{ installOk: boolean; installTail?: string }>;
  setup: readonly SetupResult[];
  claims: readonly ClaimResult[];
  verdict: VerifierVerdict;
  verdictReasons: readonly string[];
}>;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.length > 0 && value.every((item) => typeof item === "string");

const commandFieldProblems = (claim: Record<string, unknown>, ref: string): string[] => {
  const problems: string[] = [];
  if (!isStringArray(claim.command)) {
    problems.push(`${ref}: command must be a non-empty string argv array`);
  }
  if (claim.cwd !== undefined && typeof claim.cwd !== "string") {
    problems.push(`${ref}: cwd must be a string when present`);
  }
  if (
    claim.timeoutMs !== undefined &&
    (typeof claim.timeoutMs !== "number" || claim.timeoutMs <= 0)
  ) {
    problems.push(`${ref}: timeoutMs must be a positive number when present`);
  }
  if (claim.requiredEnv !== undefined && !isStringArray(claim.requiredEnv)) {
    problems.push(`${ref}: requiredEnv must be a non-empty string array when present`);
  }
  return problems;
};

const claimProblems = (claim: unknown, index: number, seenIds: Set<string>): string[] => {
  const ref =
    isRecord(claim) && typeof claim.id === "string" ? `claim ${claim.id}` : `claim[${index}]`;
  if (!isRecord(claim)) return [`${ref}: must be an object`];
  const problems: string[] = [];
  if (typeof claim.id !== "string" || claim.id === "") {
    problems.push(`${ref}: id must be a non-empty string`);
  } else if (seenIds.has(claim.id)) {
    problems.push(`${ref}: duplicate id`);
  } else {
    seenIds.add(claim.id);
  }
  if (typeof claim.title !== "string" || claim.title === "") {
    problems.push(`${ref}: title must be a non-empty string`);
  }
  switch (claim.kind) {
    case "command":
      problems.push(...commandFieldProblems(claim, ref));
      break;
    case "file_exists":
      if (typeof claim.path !== "string" || claim.path === "") {
        problems.push(`${ref}: path must be a non-empty string`);
      }
      break;
    case "adversarial": {
      problems.push(...commandFieldProblems(claim, ref));
      if (typeof claim.probes !== "string" || claim.probes === "") {
        problems.push(`${ref}: probes must name the command claim it stresses`);
      }
      const mutation = claim.mutation;
      if (
        !isRecord(mutation) ||
        typeof mutation.file !== "string" ||
        mutation.file === "" ||
        typeof mutation.find !== "string" ||
        mutation.find === "" ||
        typeof mutation.replace !== "string" ||
        mutation.find === mutation.replace
      ) {
        problems.push(
          `${ref}: mutation requires file, non-empty find, and a replace different from find`,
        );
      }
      break;
    }
    case "attested":
      if (typeof claim.reason !== "string" || claim.reason === "") {
        problems.push(`${ref}: attested claims require the exact unverifiable reason`);
      }
      break;
    default:
      problems.push(`${ref}: kind must be one of ${VERIFIER_CLAIM_KINDS.join("|")}`);
  }
  return problems;
};

/**
 * Decode an unknown value into a claims file. Returns the typed file or the
 * full problem list — never a partially-valid file.
 */
export const decodeVerifierClaims = (
  value: unknown,
): { file: VerifierClaimsFile } | { problems: readonly string[] } => {
  if (!isRecord(value)) return { problems: ["claims file must be a JSON object"] };
  const problems: string[] = [];
  if (value.schemaVersion !== QA_VERIFIER_CLAIMS_SCHEMA) {
    problems.push(`schemaVersion must be ${JSON.stringify(QA_VERIFIER_CLAIMS_SCHEMA)}`);
  }
  if (typeof value.issue !== "number" || !Number.isInteger(value.issue) || value.issue <= 0) {
    problems.push("issue must be a positive integer");
  }
  if (typeof value.commit !== "string" || !/^[0-9a-f]{7,40}$/.test(value.commit)) {
    problems.push("commit must be a hex sha (7-40 chars)");
  }
  if (typeof value.source !== "string" || value.source === "") {
    problems.push("source must describe where the claims came from");
  }
  if (value.implementer !== undefined && typeof value.implementer !== "string") {
    problems.push("implementer must be a string when present");
  }
  if (value.setup !== undefined) {
    if (!Array.isArray(value.setup)) {
      problems.push("setup must be an array when present");
    } else {
      for (const [index, step] of value.setup.entries()) {
        if (
          !isRecord(step) ||
          typeof step.title !== "string" ||
          step.title === "" ||
          !isStringArray(step.command)
        ) {
          problems.push(`setup[${index}]: requires title and a non-empty string argv`);
        }
      }
    }
  }
  if (!Array.isArray(value.claims) || value.claims.length === 0) {
    problems.push("claims must be a non-empty array");
  } else {
    const seenIds = new Set<string>();
    for (const [index, claim] of value.claims.entries()) {
      problems.push(...claimProblems(claim, index, seenIds));
    }
    for (const claim of value.claims) {
      if (isRecord(claim) && claim.kind === "adversarial" && typeof claim.probes === "string") {
        if (!seenIds.has(claim.probes)) {
          problems.push(
            `claim ${String(claim.id)}: probes references unknown claim id ${JSON.stringify(claim.probes)}`,
          );
        }
      }
    }
  }
  return problems.length > 0 ? { problems } : { file: value as unknown as VerifierClaimsFile };
};

/**
 * Independence gate: a verifier must not be the implementer. Returns the
 * problem string, or undefined when independent. Comparison is trimmed and
 * case-insensitive so a cosmetic rename cannot dodge the gate.
 */
export const independenceProblem = (
  implementer: string | undefined,
  verifier: string,
): string | undefined => {
  const verifierNorm = verifier.trim().toLowerCase();
  if (verifierNorm === "") return "verifier actor must be a non-empty string (--actor)";
  if (implementer === undefined) return undefined;
  return implementer.trim().toLowerCase() === verifierNorm
    ? `verifier ${JSON.stringify(verifier)} is the implementer — self-verification is not acceptance (docs/transcripts/253-notes.md)`
    : undefined;
};

/**
 * Verdict rules:
 * - any failed claim => reject (a claim was re-run and contradicted);
 * - accept requires no failures, AND >= 1 verified command re-run, AND >= 1
 *   verified adversarial probe (the cited proof demonstrably catches the
 *   broken behavior);
 * - everything else => unverifiable-here. Owner-gated / env-missing claims
 *   are named in the reasons and never auto-accepted.
 */
export const computeVerdict = (
  results: readonly ClaimResult[],
): Readonly<{ verdict: VerifierVerdict; reasons: readonly string[] }> => {
  const failed = results.filter((result) => result.status === "failed");
  if (failed.length > 0) {
    return {
      reasons: failed.map(
        (result) => `claim ${result.id} failed: ${result.reason ?? "(no reason recorded)"}`,
      ),
      verdict: "reject",
    };
  }
  const unverifiable = results.filter((result) => result.status === "unverifiable_here");
  const commandVerified = results.some(
    (result) => result.kind === "command" && result.status === "verified",
  );
  const adversarialVerified = results.some(
    (result) => result.kind === "adversarial" && result.status === "verified",
  );
  const unverifiableNote =
    unverifiable.length > 0
      ? [
          `${unverifiable.length} claim(s) unverifiable here (never auto-accepted): ${unverifiable
            .map((result) => result.id)
            .join(", ")}`,
        ]
      : [];
  if (commandVerified && adversarialVerified) {
    return {
      reasons: [
        "all re-run claims verified from a clean checkout of the claimed commit",
        "at least one adversarial probe confirmed the cited proof catches the deliberately broken behavior",
        ...unverifiableNote,
      ],
      verdict: "accept",
    };
  }
  const missing: string[] = [];
  if (!commandVerified) missing.push("no command claim could be re-run and verified here");
  if (!adversarialVerified) missing.push("no adversarial probe was verified here");
  return { reasons: [...missing, ...unverifiableNote], verdict: "unverifiable-here" };
};

const shellQuote = (part: string): string =>
  /^[A-Za-z0-9._/:=@#-]+$/.test(part) ? part : `'${part.replaceAll("'", String.raw`'\''`)}'`;

export const renderArgv = (command: readonly string[]): string => command.map(shellQuote).join(" ");

/** Bounded VERBATIM tail of a command's combined output. */
export const boundTail = (raw: string, limit = QA_VERIFIER_OUTPUT_TAIL_LIMIT): string => {
  const trimmed = raw.replace(/\s+$/, "");
  return trimmed.length > limit ? `…${trimmed.slice(-limit)}` : trimmed;
};

/** Apply a first-occurrence mutation. Undefined when the anchor is missing. */
export const applyMutation = (content: string, find: string, replace: string): string | undefined =>
  content.includes(find) ? content.replace(find, replace) : undefined;

export const artifactFileName = (issue: number, resolvedCommit: string): string =>
  `qa-verify-issue-${issue}-${resolvedCommit.slice(0, 12)}.json`;

const statusLabel: Record<ClaimStatus, string> = {
  failed: "FAILED",
  unverifiable_here: "unverifiable-here",
  verified: "verified",
};

/**
 * Ready-to-post issue-comment text for the verdict. The verifier never posts
 * it — the coordinator (or accepting maintainer) does. A verdict is evidence
 * for acceptance, not merge/release/promise authority.
 */
export const buildVerdictComment = (artifact: VerdictArtifact): string => {
  const lines: string[] = [
    `## Independent verification verdict: ${artifact.verdict.toUpperCase()}`,
    "",
    `Work unit: #${artifact.workUnit.issue} @ \`${artifact.workUnit.resolvedCommit.slice(0, 12)}\` (${artifact.workUnit.source})`,
    `Verifier: \`${artifact.verifier}\`${
      artifact.implementer === undefined
        ? ""
        : ` — independent of implementer \`${artifact.implementer}\``
    }`,
    `Method: claims re-run from a clean scratch checkout of the claimed commit by \`scripts/qa-verify.ts\` (QA-5 #8910).`,
    "",
    "Per-claim results:",
    ...artifact.claims.map((claim) => {
      const head = `- **${statusLabel[claim.status]}** \`${claim.id}\` (${claim.kind}) — ${claim.title}`;
      const detail =
        claim.status === "verified" && claim.kind !== "file_exists" ? claim.reason : claim.reason;
      return detail === undefined || detail === "" ? head : `${head}\n  - ${detail}`;
    }),
    "",
    `Verdict reasons: ${artifact.verdictReasons.join("; ")}.`,
    "",
    `Artifact: \`docs/qa/verifier/results/${artifactFileName(artifact.workUnit.issue, artifact.workUnit.resolvedCommit)}\` (verbatim output tails inside).`,
    "",
    "_A verdict is acceptance evidence for the maintainer, not merge/release/public-claim authority. Unverifiable-here claims are never auto-accepted._",
  ];
  return lines.join("\n");
};
