/**
 * The candidate: the one object a cycle review proposes and an optimizer
 * mutates.
 *
 * `docs/coder/autoimprove.md` §3 types a review proposal as
 * `{lever, evidence, risk, verification}`, and §7.6 says the optimizer lane
 * shares that schema "so a reflection and a mutation are the same object".
 * OpenAgentsInc/openagents#122 names the other half of the same object: a
 * candidate is a diff over the staged text surfaces plus the lineage it came
 * from, the model family it was written or evolved against (ledger O5), and
 * the evidence rows behind it. This file is that single definition. A review
 * proposal is a candidate whose `lineage.origin` is `review`; an optimizer
 * mutation will be one whose origin is `optimizer`, and nothing else about it
 * changes.
 *
 * THE EVIDENCE RULE IS ENFORCED HERE, AT PARSE TIME. `autoimprove.md` §6
 * lists "confident review without understanding" as a failure mode and its
 * control as "a proposal with no evidence pointer is rejected in the adopt
 * step". Rejecting it in the adopt step means a human notices, or does not.
 * {@link parseCycleReview} refuses the whole review instead: every evidence
 * ref must resolve against the artifacts the review was actually given, and
 * every proposal must cite at least one trajectory step. A reviewer that
 * invents a step number produces a named rejection, not a plausible document.
 *
 * The refusals are named because an unnamed refusal is the failure this loop
 * exists to stop: `openagents trace redact` reported "Nothing matched the
 * redaction rules" over a file full of live tokens (#97), and the sentence was
 * true. A reviewer whose citations do not resolve is the same shape of lie one
 * level up, so its rejection says which ref, in which proposal, and why.
 */

/** The schema id a candidate carries on the wire. */
export const CODER_CANDIDATE_SCHEMA = "openagents.coder_candidate.v1";

/** The schema id a parsed cycle review carries. */
export const CODER_REVIEW_SCHEMA = "openagents.coder_review.v1";

/** The improvement axes of `docs/coder/autoimprove.md` §2, plus the ledger. */
export type LeverAxis = "process" | "plugin" | "harness" | "optimizer" | "routing" | "ledger";

export const LEVER_AXES: ReadonlyArray<LeverAxis> = [
  "process",
  "plugin",
  "harness",
  "optimizer",
  "routing",
  "ledger",
];

/** Which way a suite figure has to move for the candidate to be confirmed. */
export type DeltaDirection = "up" | "down" | "unchanged";

export const DELTA_DIRECTIONS: ReadonlyArray<DeltaDirection> = ["up", "down", "unchanged"];

/**
 * One pointer into the artifacts the review was given.
 *
 * The grammar is deliberately small, because every scheme in it has to be
 * resolvable against something the assembler put in the request:
 *
 * - `trial:<task>#step-<id>` — one step of that trial's ATIF trajectory
 * - `trial:<task>#outcome` — that trial's verifier decision
 * - `row:<suite>#<recordedAt>` — one `bench-results` row
 * - `ledger:<id>` — one `docs/coder/best-practices.md` entry, e.g. `ledger:T1`
 * - `diff:<path>` — one file the lever's diff touches
 */
export interface EvidenceRef {
  readonly ref: string;
  /** What this ref is being cited for. Prose, and it may be empty. */
  readonly note: string;
}

/** What kind of artifact a ref resolved to. */
export type EvidenceKind =
  | "trajectory_step"
  | "trial_outcome"
  | "bench_row"
  | "ledger_entry"
  | "diff_path";

/** One staged text surface a candidate changes, and the change to it. */
export interface CandidateSurfaceDiff {
  /**
   * The staged surface's id, from `surfaces/coder/index.json` — `system-prompt`,
   * `tool-descriptions`, or `catalog-lines` as #122 staged them, and whatever a
   * later pass adds. It is not checked here: the vocabulary lives in that
   * artifact and the parser is pure, so the prompt names the keys and the adopt
   * step patches the file. A candidate touching no staged text (a `process`,
   * `plugin`, or `routing` lever) carries an empty `surfaces` array.
   */
  readonly surface: string;
  /** A unified diff where one exists, otherwise the proposed text. */
  readonly diff: string;
}

/** Where a candidate came from, so a pool can be walked backwards. */
export interface CandidateLineage {
  readonly origin: "review" | "optimizer" | "human";
  /** The `candidateId` this one was derived from, or `null` for a root. */
  readonly parent: string | null;
  /** The producer, e.g. `coder-review:<jobDir>:<reviewer lane ref>`. */
  readonly producedBy: string;
}

/**
 * Ledger O5: a candidate carries the model family and lane it was written
 * against, because text tuned on one lane is not evidence for another.
 */
export interface CandidateTransferLabel {
  readonly modelFamily: string;
  readonly lane: string;
}

/** How the candidate would be confirmed or refuted. */
export interface CandidateVerification {
  /** The suite that would run, e.g. `tb2-quick`. */
  readonly suite: string;
  /** The figure to read, e.g. `successRate` or `promptTokens`. */
  readonly metric: string;
  readonly expectedDirection: DeltaDirection;
}

/** A review proposal and an optimizer mutation, as one object. */
export interface CoderCandidate {
  readonly schema: typeof CODER_CANDIDATE_SCHEMA;
  /** A digest over the candidate's own facts. Computed, never supplied. */
  readonly candidateId: string;
  readonly lever: {
    readonly axis: LeverAxis;
    /** One sentence naming the change. */
    readonly summary: string;
  };
  readonly surfaces: ReadonlyArray<CandidateSurfaceDiff>;
  readonly lineage: CandidateLineage;
  readonly transferLabel: CandidateTransferLabel;
  readonly evidence: ReadonlyArray<EvidenceRef>;
  /** What could go wrong if this is adopted. */
  readonly risk: string;
  readonly verification: CandidateVerification;
}

export type LedgerOp = "add" | "promote" | "demote" | "refute";

export const LEDGER_OPS: ReadonlyArray<LedgerOp> = ["add", "promote", "demote", "refute"];

export type LedgerStatus = "adopted" | "proposed" | "refuted";

export const LEDGER_STATUSES: ReadonlyArray<LedgerStatus> = ["adopted", "proposed", "refuted"];

/** The ledger entry an operation adds or moves. */
export interface LedgerEntryProposal {
  /** An existing entry's id (`T1`), or `null` when the operation adds one. */
  readonly id: string | null;
  /** The heading it belongs under, e.g. `Tool habits`. */
  readonly section: string;
  readonly title: string;
  /** The falsifiable claim itself. */
  readonly statement: string;
  /** How a violation is detected. §5: an entry nothing detects is aspiration. */
  readonly detection: string;
  readonly status: LedgerStatus;
}

export interface LedgerOperation {
  readonly op: LedgerOp;
  readonly entry: LedgerEntryProposal;
  readonly provenance: ReadonlyArray<EvidenceRef>;
}

/** One point of the 0-10 score, with what was observed to award or dock it. */
export interface ReviewScorePoint {
  readonly point: string;
  /** Signed, so a reader can add them up and land on the score. */
  readonly delta: number;
  readonly evidence: ReadonlyArray<EvidenceRef>;
}

/** A ledger practice the cycle broke, and the step that shows it. */
export interface PracticeViolation {
  /** The ledger entry id, e.g. `T1`. */
  readonly entry: string;
  readonly note: string;
  readonly evidence: ReadonlyArray<EvidenceRef>;
}

/** One cycle review, parsed and evidence-checked. */
export interface CycleReview {
  readonly schema: typeof CODER_REVIEW_SCHEMA;
  readonly score: number;
  readonly outOf: 10;
  readonly points: ReadonlyArray<ReviewScorePoint>;
  /** Did the lever cause the delta, or does a confounder explain it? */
  readonly causality: string;
  readonly violations: ReadonlyArray<PracticeViolation>;
  readonly proposals: ReadonlyArray<CoderCandidate>;
  readonly ledgerOperations: ReadonlyArray<LedgerOperation>;
}

/** Every way a reviewer's output can be refused, by name. */
export type RejectionReason =
  | "not_json"
  | "not_an_object"
  | "missing_field"
  | "wrong_type"
  | "unknown_lever_axis"
  | "unknown_delta_direction"
  | "unknown_ledger_op"
  | "unknown_ledger_status"
  | "score_out_of_range"
  | "no_proposals"
  | "too_many_proposals"
  | "proposal_without_evidence"
  | "proposal_without_trajectory_evidence"
  | "evidence_ref_malformed"
  | "evidence_ref_unknown_scheme"
  | "evidence_ref_unresolved";

export interface ReviewRejection {
  readonly reason: RejectionReason;
  /** Where in the reviewer's output, e.g. `proposals[0].evidence[1].ref`. */
  readonly path: string;
  readonly detail: string;
}

export type ParseCycleReviewResult =
  | { readonly ok: true; readonly review: CycleReview }
  | { readonly ok: false; readonly rejections: ReadonlyArray<ReviewRejection> };

/**
 * What the review was actually given, as the set of refs that can resolve.
 *
 * Built by the assembler from the request it sent, so "resolves" means "is in
 * the artifacts this reviewer read" rather than "exists somewhere on disk".
 */
export interface EvidenceIndex {
  /** `<task>#step-<id>` for every trajectory step that survived truncation. */
  readonly trajectorySteps: ReadonlySet<string>;
  /** Task names with a verifier decision in the request. */
  readonly trialOutcomes: ReadonlySet<string>;
  /** `<suite>#<recordedAt>` for every bench row in the request. */
  readonly benchRows: ReadonlySet<string>;
  /** Ledger entry ids parsed out of the practices file. */
  readonly ledgerEntries: ReadonlySet<string>;
  /** Paths named by the lever's diff. */
  readonly diffPaths: ReadonlySet<string>;
}

export type ResolveEvidenceResult =
  | { readonly ok: true; readonly kind: EvidenceKind }
  | {
      readonly ok: false;
      readonly reason: Extract<
        RejectionReason,
        "evidence_ref_malformed" | "evidence_ref_unknown_scheme" | "evidence_ref_unresolved"
      >;
      readonly detail: string;
    };

const STEP_SUFFIX = /^step-(.+)$/u;

/**
 * Resolve one evidence ref against the artifacts the reviewer was given.
 *
 * An unresolvable ref is the laundering move this whole command exists to
 * block, so the three ways it can fail are separate names: the ref did not
 * parse, its scheme is not one of the five, or it parsed and named nothing
 * that was in the request.
 */
export const resolveEvidenceRef = (index: EvidenceIndex, ref: string): ResolveEvidenceResult => {
  const separator = ref.indexOf(":");
  if (separator <= 0 || separator === ref.length - 1) {
    return {
      ok: false,
      reason: "evidence_ref_malformed",
      detail: `"${ref}" is not <scheme>:<target>. Use trial:<task>#step-<id>, trial:<task>#outcome, row:<suite>#<recordedAt>, ledger:<id>, or diff:<path>.`,
    };
  }
  const scheme = ref.slice(0, separator);
  const target = ref.slice(separator + 1);

  switch (scheme) {
    case "trial": {
      const hash = target.indexOf("#");
      if (hash <= 0 || hash === target.length - 1) {
        return {
          ok: false,
          reason: "evidence_ref_malformed",
          detail: `"${ref}" needs a #step-<id> or #outcome part.`,
        };
      }
      const task = target.slice(0, hash);
      const part = target.slice(hash + 1);
      if (part === "outcome") {
        return index.trialOutcomes.has(task)
          ? { ok: true, kind: "trial_outcome" }
          : {
              ok: false,
              reason: "evidence_ref_unresolved",
              detail: `no trial named "${task}" is in this review's request.`,
            };
      }
      const step = STEP_SUFFIX.exec(part);
      if (step === null) {
        return {
          ok: false,
          reason: "evidence_ref_malformed",
          detail: `"${ref}" names neither a step (#step-<id>) nor the verifier decision (#outcome).`,
        };
      }
      return index.trajectorySteps.has(target)
        ? { ok: true, kind: "trajectory_step" }
        : {
            ok: false,
            reason: "evidence_ref_unresolved",
            detail: `no step "${part}" of trial "${task}" is in this review's request. The request lists the steps it kept, and truncation is named there.`,
          };
    }
    case "row": {
      return index.benchRows.has(target)
        ? { ok: true, kind: "bench_row" }
        : {
            ok: false,
            reason: "evidence_ref_unresolved",
            detail: `no bench-results row "${target}" is in this review's request.`,
          };
    }
    case "ledger": {
      return index.ledgerEntries.has(target)
        ? { ok: true, kind: "ledger_entry" }
        : {
            ok: false,
            reason: "evidence_ref_unresolved",
            detail: `no ledger entry "${target}" is in the practices file this review was given.`,
          };
    }
    case "diff": {
      return index.diffPaths.has(target)
        ? { ok: true, kind: "diff_path" }
        : {
            ok: false,
            reason: "evidence_ref_unresolved",
            detail: `the lever's diff in this review's request touches no path "${target}".`,
          };
    }
    default:
      return {
        ok: false,
        reason: "evidence_ref_unknown_scheme",
        detail: `"${scheme}" is not one of trial, row, ledger, diff.`,
      };
  }
};

/** The largest number of proposals `autoimprove.md` §3 allows in one review. */
export const MAX_PROPOSALS = 3;

interface Collector {
  readonly rejections: Array<ReviewRejection>;
}

const record = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const requireString = (
  collector: Collector,
  source: Record<string, unknown>,
  key: string,
  path: string,
): string => {
  const value = source[key];
  if (value === undefined || value === null) {
    collector.rejections.push({
      reason: "missing_field",
      path: `${path}.${key}`,
      detail: `a review needs ${path}.${key}.`,
    });
    return "";
  }
  if (typeof value !== "string") {
    collector.rejections.push({
      reason: "wrong_type",
      path: `${path}.${key}`,
      detail: `${path}.${key} must be a string, got ${typeof value}.`,
    });
    return "";
  }
  return value;
};

const optionalString = (source: Record<string, unknown>, key: string): string => {
  const value = source[key];
  return typeof value === "string" ? value : "";
};

const requireNumber = (
  collector: Collector,
  source: Record<string, unknown>,
  key: string,
  path: string,
): number => {
  const value = source[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    collector.rejections.push({
      reason: value === undefined ? "missing_field" : "wrong_type",
      path: `${path}.${key}`,
      detail: `${path}.${key} must be a finite number.`,
    });
    return 0;
  }
  return value;
};

const requireArray = (
  collector: Collector,
  source: Record<string, unknown>,
  key: string,
  path: string,
): ReadonlyArray<unknown> => {
  const value = source[key];
  if (value === undefined) {
    collector.rejections.push({
      reason: "missing_field",
      path: `${path}.${key}`,
      detail: `a review needs ${path}.${key}.`,
    });
    return [];
  }
  if (!Array.isArray(value)) {
    collector.rejections.push({
      reason: "wrong_type",
      path: `${path}.${key}`,
      detail: `${path}.${key} must be an array.`,
    });
    return [];
  }
  return value;
};

const readEvidence = (
  collector: Collector,
  index: EvidenceIndex,
  raw: ReadonlyArray<unknown>,
  path: string,
): { readonly refs: ReadonlyArray<EvidenceRef>; readonly kinds: ReadonlyArray<EvidenceKind> } => {
  const refs: Array<EvidenceRef> = [];
  const kinds: Array<EvidenceKind> = [];
  raw.forEach((entry, position) => {
    const here = `${path}[${String(position)}]`;
    // A bare string is accepted: a reviewer that cites a step and says nothing
    // about it has still cited the step, and forcing a note would invite a
    // filler sentence rather than a better citation.
    const asRecord = record(entry);
    const ref = typeof entry === "string" ? entry : optionalString(asRecord ?? {}, "ref");
    const note = typeof entry === "string" ? "" : optionalString(asRecord ?? {}, "note");
    if (ref === "") {
      collector.rejections.push({
        reason: "missing_field",
        path: `${here}.ref`,
        detail: `${here} carries no ref.`,
      });
      return;
    }
    const resolved = resolveEvidenceRef(index, ref);
    if (!resolved.ok) {
      collector.rejections.push({
        reason: resolved.reason,
        path: `${here}.ref`,
        detail: resolved.detail,
      });
      return;
    }
    refs.push({ ref, note });
    kinds.push(resolved.kind);
  });
  return { refs, kinds };
};

const readCandidate = (
  collector: Collector,
  index: EvidenceIndex,
  raw: unknown,
  path: string,
  producedBy: string,
): CoderCandidate | undefined => {
  const source = record(raw);
  if (source === undefined) {
    collector.rejections.push({
      reason: "not_an_object",
      path,
      detail: `${path} must be an object.`,
    });
    return undefined;
  }

  const leverSource = record(source["lever"]);
  const axisRaw =
    leverSource === undefined
      ? optionalString(source, "lever")
      : optionalString(leverSource, "axis");
  const summary =
    leverSource === undefined
      ? requireString(collector, source, "summary", path)
      : requireString(collector, leverSource, "summary", `${path}.lever`);
  if (!LEVER_AXES.includes(axisRaw as LeverAxis)) {
    collector.rejections.push({
      reason: "unknown_lever_axis",
      path: `${path}.lever.axis`,
      detail: `"${axisRaw}" is not one of ${LEVER_AXES.join(", ")}.`,
    });
  }

  const evidenceRaw = requireArray(collector, source, "evidence", path);
  const evidence = readEvidence(collector, index, evidenceRaw, `${path}.evidence`);
  if (evidence.refs.length === 0) {
    collector.rejections.push({
      reason: "proposal_without_evidence",
      path: `${path}.evidence`,
      detail: `${path} cites no evidence that resolves. A proposal with no evidence pointer is refused here rather than in the adopt step.`,
    });
  } else if (!evidence.kinds.includes("trajectory_step")) {
    collector.rejections.push({
      reason: "proposal_without_trajectory_evidence",
      path: `${path}.evidence`,
      detail: `${path} cites no trajectory step. autoimprove.md §3 types a proposal's evidence as specific steps in the trajectory; a row or a ledger entry alone does not say what the coder did.`,
    });
  }

  const risk = requireString(collector, source, "risk", path);

  const verificationSource = record(source["verification"]);
  if (verificationSource === undefined) {
    collector.rejections.push({
      reason: "missing_field",
      path: `${path}.verification`,
      detail: `${path} must say which suite would confirm it and which way the figure should move.`,
    });
  }
  const verificationRecord = verificationSource ?? {};
  const suite = requireString(collector, verificationRecord, "suite", `${path}.verification`);
  const metric = requireString(collector, verificationRecord, "metric", `${path}.verification`);
  const directionRaw = optionalString(verificationRecord, "expectedDirection");
  if (!DELTA_DIRECTIONS.includes(directionRaw as DeltaDirection)) {
    collector.rejections.push({
      reason: "unknown_delta_direction",
      path: `${path}.verification.expectedDirection`,
      detail: `"${directionRaw}" is not one of ${DELTA_DIRECTIONS.join(", ")}.`,
    });
  }

  const surfaces: Array<CandidateSurfaceDiff> = [];
  const surfacesRaw = source["surfaces"];
  if (Array.isArray(surfacesRaw)) {
    surfacesRaw.forEach((entry, position) => {
      const here = `${path}.surfaces[${String(position)}]`;
      const surfaceRecord = record(entry);
      if (surfaceRecord === undefined) {
        collector.rejections.push({
          reason: "not_an_object",
          path: here,
          detail: `${here} must be an object of {surface, diff}.`,
        });
        return;
      }
      surfaces.push({
        surface: requireString(collector, surfaceRecord, "surface", here),
        diff: optionalString(surfaceRecord, "diff"),
      });
    });
  }

  const lineageSource = record(source["lineage"]) ?? {};
  const parent = lineageSource["parent"];
  const originRaw = optionalString(lineageSource, "origin");

  const candidate: Omit<CoderCandidate, "candidateId"> = {
    schema: CODER_CANDIDATE_SCHEMA,
    lever: { axis: axisRaw as LeverAxis, summary },
    surfaces,
    lineage: {
      origin: originRaw === "optimizer" || originRaw === "human" ? originRaw : "review",
      parent: typeof parent === "string" && parent !== "" ? parent : null,
      producedBy,
    },
    transferLabel: {
      modelFamily: optionalString(record(source["transferLabel"]) ?? {}, "modelFamily"),
      lane: optionalString(record(source["transferLabel"]) ?? {}, "lane"),
    },
    evidence: evidence.refs,
    risk,
    verification: {
      suite,
      metric,
      expectedDirection: directionRaw as DeltaDirection,
    },
  };

  return { ...candidate, candidateId: candidateIdOf(candidate) };
};

const readLedgerOperation = (
  collector: Collector,
  index: EvidenceIndex,
  raw: unknown,
  path: string,
): LedgerOperation | undefined => {
  const source = record(raw);
  if (source === undefined) {
    collector.rejections.push({
      reason: "not_an_object",
      path,
      detail: `${path} must be an object.`,
    });
    return undefined;
  }
  const op = optionalString(source, "op");
  if (!LEDGER_OPS.includes(op as LedgerOp)) {
    collector.rejections.push({
      reason: "unknown_ledger_op",
      path: `${path}.op`,
      detail: `"${op}" is not one of ${LEDGER_OPS.join(", ")}.`,
    });
  }
  const entrySource = record(source["entry"]);
  if (entrySource === undefined) {
    collector.rejections.push({
      reason: "missing_field",
      path: `${path}.entry`,
      detail: `${path} must carry the ledger entry it operates on.`,
    });
  }
  const entryRecord = entrySource ?? {};
  const status = optionalString(entryRecord, "status");
  if (!LEDGER_STATUSES.includes(status as LedgerStatus)) {
    collector.rejections.push({
      reason: "unknown_ledger_status",
      path: `${path}.entry.status`,
      detail: `"${status}" is not one of ${LEDGER_STATUSES.join(", ")}.`,
    });
  }
  const id = entryRecord["id"];
  const provenanceRaw = requireArray(collector, source, "provenance", path);
  const provenance = readEvidence(collector, index, provenanceRaw, `${path}.provenance`);
  if (provenance.refs.length === 0) {
    collector.rejections.push({
      reason: "proposal_without_evidence",
      path: `${path}.provenance`,
      detail: `${path} carries no provenance that resolves. autoimprove.md §5 requires every ledger entry to carry the run, review, or postmortem that produced it.`,
    });
  }

  return {
    op: op as LedgerOp,
    entry: {
      id: typeof id === "string" && id !== "" ? id : null,
      section: requireString(collector, entryRecord, "section", `${path}.entry`),
      title: requireString(collector, entryRecord, "title", `${path}.entry`),
      statement: requireString(collector, entryRecord, "statement", `${path}.entry`),
      detection: requireString(collector, entryRecord, "detection", `${path}.entry`),
      status: status as LedgerStatus,
    },
    provenance: provenance.refs,
  };
};

/**
 * Parse a reviewer's raw output into a checked {@link CycleReview}.
 *
 * `producedBy` is stamped into every candidate's lineage so a proposal can be
 * traced back to the review that emitted it without the reviewer being asked
 * to state its own identity, which it has no reliable way to know.
 */
export const parseCycleReview = (
  raw: string,
  index: EvidenceIndex,
  producedBy: string,
): ParseCycleReviewResult => {
  const collector: Collector = { rejections: [] };

  let parsed: unknown;
  try {
    parsed = JSON.parse(extractJsonObject(raw)) as unknown;
  } catch (cause) {
    return {
      ok: false,
      rejections: [
        {
          reason: "not_json",
          path: "$",
          detail: `the reviewer's output is not JSON: ${cause instanceof Error ? cause.message : String(cause)}`,
        },
      ],
    };
  }

  const source = record(parsed);
  if (source === undefined) {
    return {
      ok: false,
      rejections: [
        { reason: "not_an_object", path: "$", detail: "the reviewer's output is not an object." },
      ],
    };
  }

  const score = requireNumber(collector, source, "score", "$");
  if (score < 0 || score > 10) {
    collector.rejections.push({
      reason: "score_out_of_range",
      path: "$.score",
      detail: `the score is 0-10; got ${String(score)}.`,
    });
  }

  const points: Array<ReviewScorePoint> = [];
  requireArray(collector, source, "points", "$").forEach((entry, position) => {
    const here = `$.points[${String(position)}]`;
    const pointSource = record(entry);
    if (pointSource === undefined) {
      collector.rejections.push({
        reason: "not_an_object",
        path: here,
        detail: `${here} must be an object.`,
      });
      return;
    }
    const evidence = readEvidence(
      collector,
      index,
      requireArray(collector, pointSource, "evidence", here),
      `${here}.evidence`,
    );
    points.push({
      point: requireString(collector, pointSource, "point", here),
      delta: requireNumber(collector, pointSource, "delta", here),
      evidence: evidence.refs,
    });
  });

  const violations: Array<PracticeViolation> = [];
  const violationsRaw = source["violations"];
  if (Array.isArray(violationsRaw)) {
    violationsRaw.forEach((entry, position) => {
      const here = `$.violations[${String(position)}]`;
      const violationSource = record(entry);
      if (violationSource === undefined) {
        collector.rejections.push({
          reason: "not_an_object",
          path: here,
          detail: `${here} must be an object.`,
        });
        return;
      }
      const evidence = readEvidence(
        collector,
        index,
        requireArray(collector, violationSource, "evidence", here),
        `${here}.evidence`,
      );
      violations.push({
        entry: requireString(collector, violationSource, "entry", here),
        note: optionalString(violationSource, "note"),
        evidence: evidence.refs,
      });
    });
  }

  const proposalsRaw = requireArray(collector, source, "proposals", "$");
  if (proposalsRaw.length === 0) {
    collector.rejections.push({
      reason: "no_proposals",
      path: "$.proposals",
      detail:
        "a review proposes one to three changes. A review with nothing to propose says so as a proposal whose lever is the practice it would keep, not as an empty list.",
    });
  }
  if (proposalsRaw.length > MAX_PROPOSALS) {
    collector.rejections.push({
      reason: "too_many_proposals",
      path: "$.proposals",
      detail: `autoimprove.md §3 allows one to three proposals; got ${String(proposalsRaw.length)}.`,
    });
  }
  const proposals: Array<CoderCandidate> = [];
  proposalsRaw.forEach((entry, position) => {
    const candidate = readCandidate(
      collector,
      index,
      entry,
      `$.proposals[${String(position)}]`,
      producedBy,
    );
    if (candidate !== undefined) proposals.push(candidate);
  });

  const ledgerOperations: Array<LedgerOperation> = [];
  const ledgerRaw = source["ledgerOperations"];
  if (Array.isArray(ledgerRaw)) {
    ledgerRaw.forEach((entry, position) => {
      const operation = readLedgerOperation(
        collector,
        index,
        entry,
        `$.ledgerOperations[${String(position)}]`,
      );
      if (operation !== undefined) ledgerOperations.push(operation);
    });
  }

  if (collector.rejections.length > 0) {
    return { ok: false, rejections: collector.rejections };
  }

  return {
    ok: true,
    review: {
      schema: CODER_REVIEW_SCHEMA,
      score,
      outOf: 10,
      points,
      causality: requireString(collector, source, "causality", "$"),
      violations,
      proposals,
      ledgerOperations,
    },
  };
};

/**
 * Take the JSON object out of a model's reply.
 *
 * A model asked for JSON commonly wraps it in a fenced block or a sentence.
 * Refusing that would make the real lane fail for a reason that has nothing to
 * do with the review's content, so the outermost `{...}` span is used when the
 * whole string does not parse. Anything looser would start guessing.
 */
export const extractJsonObject = (raw: string): string => {
  const trimmed = raw.trim();
  if (trimmed.startsWith("{")) return trimmed;
  const fenced = /```(?:json)?\s*([\s\S]*?)```/u.exec(trimmed);
  const body = fenced?.[1]?.trim();
  if (body !== undefined && body.startsWith("{")) return body;
  const open = trimmed.indexOf("{");
  const close = trimmed.lastIndexOf("}");
  return open >= 0 && close > open ? trimmed.slice(open, close + 1) : trimmed;
};

/**
 * A stable id over a candidate's own facts.
 *
 * FNV-1a rather than a crypto hash: this is an identity for a pool entry, not
 * a receipt. `bench-results` owns the tamper-evidence, and borrowing its
 * vocabulary here would suggest this digest carries the same weight.
 */
export const candidateIdOf = (candidate: Omit<CoderCandidate, "candidateId">): string => {
  const source = JSON.stringify({
    axis: candidate.lever.axis,
    summary: candidate.lever.summary,
    surfaces: candidate.surfaces.map((surface) => [surface.surface, surface.diff]),
    evidence: candidate.evidence.map((entry) => entry.ref).sort(),
    verification: candidate.verification,
  });
  let hash = 0x811c9dc5;
  for (let position = 0; position < source.length; position += 1) {
    hash ^= source.charCodeAt(position);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `candidate:${hash.toString(16).padStart(8, "0")}`;
};
