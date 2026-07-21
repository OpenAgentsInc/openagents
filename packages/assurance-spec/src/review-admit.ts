/**
 * Deterministic independent-admission verifier (root AUTHORITY.md
 * `grant.independent_assurance`, `condition.independence`,
 * `condition.verification`).
 *
 * An AssuranceSpec producer may never admit its own obligation. An owner or an
 * owner-designated independent reviewer distinct from the producer may admit a
 * revision only after reproducing the executable evidence and confirming the
 * document makes NO evidence-tier overclaim. This module encodes that judgment
 * as pure, deterministic functions plus one injected oracle reproducer, so the
 * decision is reproducible and never rounds up a tier.
 *
 * The verifier admits when, and only when:
 *  1. the document is structurally honest (still `proposed`, cannot self-admit,
 *     treats missing evidence as `INCONCLUSIVE`, and names an owner-designated
 *     independent reviewer as an admitter role); and
 *  2. every criterion the document arms at the local-unit tier with a real
 *     oracle test file reproduces GREEN; and
 *  3. every receipt-backed evidence file exists; and
 *  4. no supplied claimed tier is stronger than the tier the document and the
 *     reproduction actually support (no rounding up).
 *
 * Criteria the document HONESTLY discloses as smoke-gated, receipt-backed, or
 * designed-only (its named oracle seam does not exist yet) do NOT block
 * admission: admission means "this revision overclaims nothing", not "all
 * criteria pass". Those criteria stay unobserved and are reported as such.
 */
import { type AssuranceSpecDocument } from "./schema.ts"
import { sha256Digest } from "./tooling.ts"
import {
  AUTHORITY_DECISION_RECEIPT_SCHEMA_ID,
  decodeAuthorityDecisionReceipt,
  type AuthorityConditionResult,
  type AuthorityDecisionReceipt,
} from "./authority-decision-receipt.ts"

export const GATE_LOCAL_UNIT = "GATE-LOCAL-UNIT" as const
export const GATE_DEV_TWO_PROCESS = "GATE-DEV-TWO-PROCESS" as const
export const GATE_OWNER_REAL_PROVIDER = "GATE-OWNER-REAL-PROVIDER" as const
export const GATE_PACKAGED_RELEASE = "GATE-PACKAGED-RELEASE" as const

export const REVIEW_TIERS = [
  "executable",
  "smoke_gated",
  "receipt_backed",
  "designed_only",
  "release_blocked",
  "unclassified",
] as const
export type ReviewTier = (typeof REVIEW_TIERS)[number]

/** Strength ordering: a claim above the observed rank is a round-up. */
const TIER_RANK: Record<ReviewTier, number> = {
  unclassified: 0,
  designed_only: 0,
  release_blocked: 0,
  receipt_backed: 1,
  smoke_gated: 1,
  executable: 2,
}

const TEST_FILE_PATTERN = /\.test\.(?:ts|tsx)$/

export type FileExists = (repoRelativePath: string) => boolean

export interface CriterionClassification {
  readonly criterion_ref: string
  readonly obligation_id: string
  readonly activation_gate: string | undefined
  readonly evaluator_ref: string | undefined
  readonly proof_rung: string | undefined
  readonly oracle_exists: boolean
  readonly oracle_is_test_file: boolean
  readonly tier: ReviewTier
}

const tierForObligation = (
  gate: string | undefined,
  evaluatorRef: string | undefined,
  fileExists: FileExists,
): { tier: ReviewTier; exists: boolean; isTest: boolean } => {
  const isTest = evaluatorRef !== undefined && TEST_FILE_PATTERN.test(evaluatorRef)
  const exists = evaluatorRef !== undefined && fileExists(evaluatorRef)
  // A real, existing test-file oracle is reproducible in-process at the
  // local-unit tier regardless of a stricter arming gate, so it is executable
  // and must reproduce GREEN. The stricter gate still bounds the SIGNED /
  // cross-process claim, which admission never asserts.
  if (isTest && exists) return { tier: "executable", exists, isTest }
  // A non-test (or absent) oracle takes its tier from the arming gate.
  if (gate === GATE_DEV_TWO_PROCESS) return { tier: "smoke_gated", exists, isTest }
  if (gate === GATE_OWNER_REAL_PROVIDER) return { tier: "receipt_backed", exists, isTest }
  if (gate === GATE_PACKAGED_RELEASE) return { tier: "release_blocked", exists, isTest }
  // A local-unit oracle whose named test file does not exist yet is a design
  // only (the MemoHarness seam): honestly unobserved, never counted observed.
  if (!exists) return { tier: "designed_only", exists, isTest }
  return { tier: "unclassified", exists, isTest }
}

/**
 * One classification per subject criterion. When several obligations cover one
 * criterion, the strongest observable tier wins (a criterion is executable if
 * any covering obligation arms a real local-unit oracle), so a document cannot
 * hide a strong claim behind a weaker sibling obligation.
 */
export const classifyReviewTiers = (
  document: AssuranceSpecDocument,
  fileExists: FileExists,
): ReadonlyArray<CriterionClassification> => {
  const byCriterion = new Map<string, CriterionClassification>()
  for (const obligation of document.obligations) {
    const gate = obligation.activation_gate
    const evaluatorRef = obligation.oracle?.evaluator_ref
    const proofRung = obligation.evidence?.proof_rung
    const { tier, exists, isTest } = tierForObligation(gate, evaluatorRef, fileExists)
    for (const criterion of obligation.criterion_refs) {
      const candidate: CriterionClassification = {
        criterion_ref: criterion,
        obligation_id: obligation.id,
        activation_gate: gate,
        evaluator_ref: evaluatorRef,
        proof_rung: proofRung,
        oracle_exists: exists,
        oracle_is_test_file: isTest,
        tier,
      }
      const existing = byCriterion.get(criterion)
      if (existing === undefined || TIER_RANK[tier] > TIER_RANK[existing.tier]) {
        byCriterion.set(criterion, candidate)
      }
    }
  }
  return document.subject.product_spec.criterion_refs.map((criterion) =>
    byCriterion.get(criterion) ?? {
      criterion_ref: criterion,
      obligation_id: "",
      activation_gate: undefined,
      evaluator_ref: undefined,
      proof_rung: undefined,
      oracle_exists: false,
      oracle_is_test_file: false,
      tier: "unclassified",
    })
}

// ---------------------------------------------------------------------------
// Execution plan (reproduce the executable oracles)
// ---------------------------------------------------------------------------

const DESKTOP_PREFIX = "apps/openagents-desktop/"

export interface OracleBatch {
  readonly batch_id: string
  /** Working directory, relative to the repository root. */
  readonly cwd: string
  /** vp binary, relative to `cwd`. */
  readonly binary: string
  /** `--root` argument passed to vp. */
  readonly root: string
  /** File arguments passed to vp, as vp expects them for this batch. */
  readonly file_args: ReadonlyArray<string>
  /** The repository-relative evaluator files this batch reproduces. */
  readonly evaluator_refs: ReadonlyArray<string>
}

/**
 * Group the executable oracle files into at most two batches that reproduce
 * exactly the way the review packet documents: the desktop suite from
 * `apps/openagents-desktop` with `--root ../..`, and any non-desktop oracle
 * (the assurance-spec self-check) from the repository root with `--root .`
 * using the desktop-installed vp binary.
 */
export const planOracleReproduction = (
  classifications: ReadonlyArray<CriterionClassification>,
): ReadonlyArray<OracleBatch> => {
  const executableFiles = new Set<string>()
  for (const entry of classifications) {
    if (entry.tier === "executable" && entry.evaluator_ref !== undefined) executableFiles.add(entry.evaluator_ref)
  }
  const desktop = [...executableFiles].filter((path) => path.startsWith(DESKTOP_PREFIX)).sort()
  const other = [...executableFiles].filter((path) => !path.startsWith(DESKTOP_PREFIX)).sort()
  const batches: OracleBatch[] = []
  if (desktop.length > 0) {
    batches.push({
      batch_id: "desktop-oracles",
      cwd: DESKTOP_PREFIX.replace(/\/$/, ""),
      binary: "./node_modules/.bin/vp",
      root: "../..",
      file_args: desktop.map((path) => path.slice(DESKTOP_PREFIX.length)),
      evaluator_refs: desktop,
    })
  }
  if (other.length > 0) {
    batches.push({
      batch_id: "repo-oracles",
      cwd: ".",
      binary: "./apps/openagents-desktop/node_modules/.bin/vp",
      root: ".",
      file_args: other,
      evaluator_refs: other,
    })
  }
  return batches
}

export interface BatchReproduction {
  readonly batch_id: string
  readonly ok: boolean
  readonly exit_code: number
  readonly tests_passed?: number
  readonly tests_failed?: number
  readonly files?: number
  readonly detail?: string
}

/** Injected reproducer. The CLI spawns vp; tests supply a deterministic stub. */
export type OracleReproducer = (batch: OracleBatch) => BatchReproduction

export const batchCommandString = (batch: OracleBatch): string =>
  [batch.binary, "test", "--run", "--root", batch.root, ...batch.file_args].join(" ")

// ---------------------------------------------------------------------------
// Structural honesty (pure, from the parsed document)
// ---------------------------------------------------------------------------

export const INDEPENDENT_REVIEWER_ROLE = "owner_designated_independent_reviewer" as const

export interface HonestyIssue {
  readonly code: string
  readonly message: string
  readonly criterion_ref?: string
}

export interface StructuralHonesty {
  readonly ok: boolean
  readonly issues: ReadonlyArray<HonestyIssue>
}

export const assessStructuralHonesty = (document: AssuranceSpecDocument): StructuralHonesty => {
  const issues: HonestyIssue[] = []
  if (document.frontmatter.lifecycle_state !== "proposed") {
    issues.push({
      code: "not_proposed",
      message: `lifecycle_state is "${document.frontmatter.lifecycle_state}"; only a proposed revision can be admitted.`,
    })
  }
  if (document.authority.proposal_may_self_admit !== false) {
    issues.push({ code: "self_admit_allowed", message: "authority.proposal_may_self_admit must be false." })
  }
  if (document.evidencePolicy.missing_evidence_verdict !== "INCONCLUSIVE") {
    issues.push({ code: "missing_evidence_not_inconclusive", message: "evidence policy must treat missing evidence as INCONCLUSIVE." })
  }
  if (!document.authority.admitted_roles.includes(INDEPENDENT_REVIEWER_ROLE)) {
    issues.push({
      code: "no_independent_reviewer_role",
      message: `authority.admitted_roles must include "${INDEPENDENT_REVIEWER_ROLE}" for an independent reviewer to admit this revision.`,
    })
  }
  return { ok: issues.length === 0, issues }
}

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

/** Optional per-criterion tier assertions to cross-check against reproduction. */
export type ClaimedTiers = Readonly<Record<string, ReviewTier>>

export interface ReviewTierCounts {
  readonly executable: number
  readonly smoke_gated: number
  readonly receipt_backed: number
  readonly designed_only: number
  readonly release_blocked: number
  readonly unclassified: number
  readonly total: number
}

export const countTiers = (classifications: ReadonlyArray<CriterionClassification>): ReviewTierCounts => {
  const counts = { executable: 0, smoke_gated: 0, receipt_backed: 0, designed_only: 0, release_blocked: 0, unclassified: 0 }
  for (const entry of classifications) counts[entry.tier] += 1
  return { ...counts, total: classifications.length }
}

export interface ReviewAdmissionDecision {
  readonly admit: boolean
  readonly outcome: "succeeded" | "refused"
  readonly blockers: ReadonlyArray<HonestyIssue>
  readonly classifications: ReadonlyArray<CriterionClassification>
  readonly counts: ReviewTierCounts
  readonly executable_green: number
  readonly executable_failed: number
  readonly structural: StructuralHonesty
  readonly reproductions: ReadonlyArray<BatchReproduction>
  readonly batches: ReadonlyArray<OracleBatch>
}

export interface DecideReviewAdmissionInput {
  readonly document: AssuranceSpecDocument
  readonly fileExists: FileExists
  readonly reproduce: OracleReproducer
  /** Optional external claim-of-tier to reject overclaims explicitly. */
  readonly claimedTiers?: ClaimedTiers
}

export const decideReviewAdmission = (input: DecideReviewAdmissionInput): ReviewAdmissionDecision => {
  const { document, fileExists, reproduce, claimedTiers } = input
  const classifications = classifyReviewTiers(document, fileExists)
  const counts = countTiers(classifications)
  const structural = assessStructuralHonesty(document)
  const batches = planOracleReproduction(classifications)
  const reproductions = batches.map(reproduce)
  const reproByFile = new Map<string, BatchReproduction>()
  for (const batch of batches) {
    const repro = reproductions.find((entry) => entry.batch_id === batch.batch_id)
    if (repro === undefined) continue
    for (const file of batch.evaluator_refs) reproByFile.set(file, repro)
  }

  const blockers: HonestyIssue[] = [...structural.issues]

  let executableGreen = 0
  let executableFailed = 0
  for (const entry of classifications) {
    if (entry.tier !== "executable" || entry.evaluator_ref === undefined) continue
    const repro = reproByFile.get(entry.evaluator_ref)
    if (repro === undefined) {
      executableFailed += 1
      blockers.push({
        code: "oracle_not_reproduced",
        message: `Executable oracle for ${entry.criterion_ref} (${entry.evaluator_ref}) was not reproduced.`,
        criterion_ref: entry.criterion_ref,
      })
      continue
    }
    if (repro.ok) executableGreen += 1
    else {
      executableFailed += 1
      blockers.push({
        code: "oracle_red",
        message: `Executable oracle for ${entry.criterion_ref} (${entry.evaluator_ref}) reproduced RED (exit ${repro.exit_code}); the document arms it at the local-unit tier, so this is an evidence-tier overclaim.`,
        criterion_ref: entry.criterion_ref,
      })
    }
  }

  // Receipt-backed evidence files must exist to stand as evidence at all.
  for (const entry of classifications) {
    if (entry.tier !== "receipt_backed") continue
    if (entry.evaluator_ref === undefined || !fileExists(entry.evaluator_ref)) {
      blockers.push({
        code: "receipt_evidence_missing",
        message: `Receipt-backed evidence for ${entry.criterion_ref} (${entry.evaluator_ref ?? "unresolved"}) does not exist.`,
        criterion_ref: entry.criterion_ref,
      })
    }
  }

  // Explicit overclaim check: a claimed tier stronger than the observed tier
  // is a round-up (condition.verification: "never round up an evidence tier").
  if (claimedTiers !== undefined) {
    const observed = new Map(classifications.map((entry) => [entry.criterion_ref, entry.tier] as const))
    for (const [criterion, claimed] of Object.entries(claimedTiers)) {
      const actual = observed.get(criterion)
      if (actual === undefined) {
        blockers.push({ code: "claimed_unknown_criterion", message: `Claimed tier for unknown criterion ${criterion}.`, criterion_ref: criterion })
        continue
      }
      if (TIER_RANK[claimed] > TIER_RANK[actual]) {
        blockers.push({
          code: "tier_round_up",
          message: `${criterion} claims tier "${claimed}" but the document and reproduction support only "${actual}".`,
          criterion_ref: criterion,
        })
      }
    }
  }

  const admit = blockers.length === 0
  return {
    admit,
    outcome: admit ? "succeeded" : "refused",
    blockers,
    classifications,
    counts,
    executable_green: executableGreen,
    executable_failed: executableFailed,
    structural,
    reproductions,
    batches,
  }
}

// ---------------------------------------------------------------------------
// Receipt construction
// ---------------------------------------------------------------------------

export interface BuildReceiptInput {
  readonly decision: ReviewAdmissionDecision
  readonly targetRef: string
  readonly targetDigest: string
  readonly reviewerRef: string
  readonly producerRef: string
  readonly triggerRef: string
  readonly startedAt: string
  readonly settledAt: string
  readonly evidenceRefs: ReadonlyArray<string>
  readonly scopeNotes: ReadonlyArray<string>
  /** Authority program this admission advances (AUTHORITY.md programs list). */
  readonly programRef?: string
}

export const PROFILE_ID = "openagents.owner-delegated-autonomy" as const
export const PROFILE_REVISION = 6
export const DEFAULT_PROGRAM_REF = "program.full_auto_release" as const
/** @deprecated Prefer DEFAULT_PROGRAM_REF or an explicit programRef. */
export const PROGRAM_REF = DEFAULT_PROGRAM_REF
export const GRANT_REF = "grant.independent_assurance" as const
export const ACTOR_ROLE = "independent_reviewer" as const

export const buildAuthorityDecisionReceipt = (input: BuildReceiptInput): AuthorityDecisionReceipt => {
  const { decision } = input
  const programRef = input.programRef ?? DEFAULT_PROGRAM_REF
  const distinct = input.reviewerRef !== input.producerRef
  const conditionResults: ReadonlyArray<AuthorityConditionResult> = [
    {
      condition_ref: "condition.independence",
      result: distinct ? "satisfied" : "not_satisfied",
      statement: distinct
        ? `Reviewer identity ${input.reviewerRef} is a fresh clean session distinct from the obligation producer ${input.producerRef}; evidence was reproduced independently.`
        : `Reviewer identity is not distinct from the producer.`,
    },
    {
      condition_ref: "condition.verification",
      result: decision.admit ? "satisfied" : "not_satisfied",
      statement: decision.admit
        ? `Reproduced ${decision.executable_green} executable oracle criteria GREEN; every armed local-unit oracle passed and no criterion claims a tier above what reproduces. Smoke-gated, receipt-backed, and designed-only criteria stay unobserved and are not claimed observed.`
        : `Refused: ${decision.blockers.map((issue) => issue.code).join(", ")}.`,
    },
    {
      condition_ref: "condition.redaction",
      result: "satisfied",
      statement: "Receipt carries only public-safe counts, refs, and digests; no raw oracle output, secrets, or private prompts.",
    },
  ]
  const summary = {
    executable_criteria: decision.counts.executable,
    executable_green: decision.executable_green,
    executable_failed: decision.executable_failed,
    smoke_gated: decision.counts.smoke_gated,
    receipt_backed: decision.counts.receipt_backed,
    designed_only: decision.counts.designed_only,
    total_criteria: decision.counts.total,
    batches: decision.batches.map((batch) => {
      const repro = decision.reproductions.find((entry) => entry.batch_id === batch.batch_id)
      return {
        batch_id: batch.batch_id,
        command: batchCommandString(batch),
        cwd: batch.cwd,
        exit_code: repro?.exit_code ?? -1,
        ok: repro?.ok ?? false,
        ...(repro?.tests_passed === undefined ? {} : { tests_passed: repro.tests_passed }),
        ...(repro?.tests_failed === undefined ? {} : { tests_failed: repro.tests_failed }),
        ...(repro?.files === undefined ? {} : { files: repro.files }),
      }
    }),
  }
  const seed = {
    schema_id: AUTHORITY_DECISION_RECEIPT_SCHEMA_ID,
    profile_id: PROFILE_ID,
    program_ref: programRef,
    grant_ref: GRANT_REF,
    actor_role: ACTOR_ROLE,
    target_ref: input.targetRef,
    target_digest: input.targetDigest,
    trigger_ref: input.triggerRef,
    outcome: decision.outcome,
    reviewer: input.reviewerRef,
    executable_green: decision.executable_green,
    settled_at: input.settledAt,
  }
  const receipt: AuthorityDecisionReceipt = {
    schema_id: AUTHORITY_DECISION_RECEIPT_SCHEMA_ID,
    receipt_ref: `authority.decision.${sha256Digest(JSON.stringify(seed)).slice("sha256:".length, "sha256:".length + 32)}`,
    profile_id: PROFILE_ID,
    profile_revision: PROFILE_REVISION,
    program_ref: programRef,
    grant_ref: GRANT_REF,
    actor_role: ACTOR_ROLE,
    action: decision.admit
      ? "admit_assurance_revision_when_source_spec_allows_owner_designated_independent_reviewer"
      : "review_assurance_spec",
    target_ref: input.targetRef,
    target_digest: input.targetDigest,
    trigger_ref: input.triggerRef,
    independence: {
      reviewer_ref: input.reviewerRef,
      producer_ref: input.producerRef,
      distinct,
      statement: distinct
        ? "Independent reviewer session distinct from the packet producer; the executable evidence was reproduced, not trusted."
        : "Reviewer is not distinct from producer; independence is unmet.",
    },
    condition_results: conditionResults,
    reproduction_summary: summary,
    scope_notes: input.scopeNotes,
    started_at: input.startedAt,
    settled_at: input.settledAt,
    outcome: decision.outcome,
    evidence_refs: input.evidenceRefs,
    public_safety: { classification: "reviewed_public_safe", contains_raw_output: false },
  }
  // Fail closed if the receipt does not satisfy its own schema.
  return decodeAuthorityDecisionReceipt(receipt)
}

// ---------------------------------------------------------------------------
// Lifecycle flip (proposed -> admitted) on the raw markdown frontmatter
// ---------------------------------------------------------------------------

export interface AdmitFrontmatterInput {
  readonly markdown: string
  readonly reviewerRef: string
  readonly receiptRef: string
  readonly receiptPath: string
  readonly admittedAt: string
}

/**
 * Flip `lifecycle_state: "proposed"` to `"admitted"` and record the
 * independent reviewer plus the decision receipt, editing only the YAML
 * frontmatter and leaving every body byte untouched. The added keys are
 * unknown-but-valid metadata that the parser preserves verbatim.
 */
export const admitAssuranceFrontmatter = (input: AdmitFrontmatterInput): string => {
  const match = /^---\n([\s\S]*?)\n---\n?/.exec(input.markdown)
  if (match === null) throw new Error("assurance_spec_missing_frontmatter")
  const frontmatter = match[1]!
  if (!/^lifecycle_state:\s*"proposed"\s*$/m.test(frontmatter)) {
    throw new Error("assurance_spec_not_proposed")
  }
  let updated = frontmatter.replace(/^lifecycle_state:\s*"proposed"\s*$/m, 'lifecycle_state: "admitted"')
  const additions = [
    `admitted_by: "${input.reviewerRef}"`,
    `admitted_at: "${input.admittedAt}"`,
    `admitted_receipt_ref: "${input.receiptRef}"`,
    `admitted_receipt_path: "${input.receiptPath}"`,
  ]
  for (const line of additions) {
    const key = line.slice(0, line.indexOf(":"))
    if (new RegExp(`^${key}:`, "m").test(updated)) {
      updated = updated.replace(new RegExp(`^${key}:.*$`, "m"), line)
    } else {
      updated = `${updated}\n${line}`
    }
  }
  return input.markdown.replace(match[0], `---\n${updated}\n---\n`)
}
