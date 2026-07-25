import { createHash } from "node:crypto"

import { Exit, Schema } from "effect"

import type { FullAutoVerificationResult } from "./full-auto-verification.ts"

/**
 * OMEGA-MOB-31-03 (omega#47) / OMEGA-FA-10 (omega#43): the host's own record of
 * ONE finished Full Auto work unit -- objective, turn, change, project
 * generation, test, typed outcome, host verification, authority decision, and
 * receipt.
 *
 * omega#43 built the reader for this chain (`FullAutoEvidenceView::from_records`
 * and the issue31 adjunct projection) and omega#47 asks a viewer to "follow one
 * finished unit from objective through authority receipt". Against a live
 * `omega-effectd` that walk stopped at the first hop: `get_report` carried no
 * `evidence` block and `get_receipt` carried no `decisionRef` or
 * `authorityReceiptRef`, so the projection reported `unavailable · hop_missing`.
 * That refusal was correct -- an incomplete chain must never be shown as partial
 * proof -- and it is unchanged. The defect was that the host produced no hops.
 * This module produces them.
 *
 * Every hop is a MEASUREMENT the host took, never a claim the run made about
 * itself. Four properties carry that, following the same discipline as the
 * host-recorded numeric run start (`FullAutoRun.startedAtMs`):
 *
 *  - **Host-executed means executed.** The only constructor for a record is
 *    {@link buildFullAutoRunEvidence}, whose verification input is
 *    {@link FullAutoHostExecutedVerification} -- obtainable ONLY from
 *    {@link asHostExecutedVerification}, which admits a verdict solely when the
 *    host itself spawned a command and read its exit code. An `evidence_ref`
 *    check (host-consulted, not host-executed) and a provider self-report cannot
 *    reach the builder at all, and `hostExecuted` is a literal `true` on the
 *    schema, so a self-reported chain is not expressible.
 *  - **Measured from one reading.** The workspace facts (change, project
 *    generation, diff) come from a single {@link FullAutoWorkspaceProbe} call
 *    against the run's bound workspace, taken at the moment the host verified
 *    it, so the change and the generation can never describe two different trees.
 *  - **No input path.** No control-API body, CLI flag, MCP call, or mobile
 *    intent supplies or influences any hop. The refs are derived from host
 *    state and content digests; the workspace facts come from the host's own
 *    probe; the verdict comes from the host's own child process.
 *  - **Never backfilled.** A run that finished before this record existed has
 *    none, decodes fine, and is honestly projected as an unavailable chain. A
 *    backfill would give the hops two provenances nothing on the wire
 *    distinguishes, which is exactly the ambiguity the refusal protects against.
 *
 * The record is also PUBLIC-SAFE by construction. It is built as plain JSON and
 * handed to its own decoder ({@link buildFullAutoRunEvidence} never constructs
 * the typed value directly), and the decoder applies the same bounded-ref and
 * bounded-text rules the phone-side reader applies (`workroom_receipts`'
 * `sanitize_public_ref` / `is_issue31_public_text`). A hop the phone would
 * refuse as private is therefore never written, so the provider boundary --
 * "never send a token, authorization response, private path, or raw credential
 * state to the phone" -- holds at the producer, not only at the reader.
 */
export const FULL_AUTO_RUN_EVIDENCE_SCHEMA = "openagents.desktop.full_auto_run_evidence.v1" as const

/** Mirrors `workroom_receipts::PUBLIC_REF_MAX_LEN`. */
export const FULL_AUTO_EVIDENCE_REF_LIMIT = 256
/** Mirrors the phone reader's bounded command length. */
export const FULL_AUTO_EVIDENCE_COMMAND_LIMIT = 256
/** Mirrors the phone reader's bounded public-text length. */
export const FULL_AUTO_EVIDENCE_TEXT_LIMIT = 512

/**
 * The single authority that decides whether a Full Auto run may complete: the
 * host's own admission gate, which admits completion ONLY on a passed
 * host-executed done-condition verification (`admitFullAutoCompletion`). Naming
 * it on the receipt is what makes "the receipt that says which authority allowed
 * it" a fact rather than a phrase.
 */
export const FULL_AUTO_COMPLETION_AUTHORITY_REF = "authority.omega.host.full_auto_completion" as const

const FORBIDDEN_TEXT_FRAGMENTS = [
  "bearer ",
  "authorization:",
  "api_key",
  "apikey",
  "access_token",
  "refresh_token",
  "client_secret",
  "private_key",
  "auth.json",
  "id_rsa",
  "begin rsa",
  "begin openssh",
  "begin private key",
  "openagents_agent_token",
] as const

const FORBIDDEN_TEXT_PREFIXES = [
  "sk-",
  "sk_",
  "ghp_",
  "gho_",
  "github_pat_",
  "xox",
  "nsec1",
  "ncryptsec1",
] as const

/**
 * Home and scratch directory segments, stored WITHOUT their surrounding slashes
 * and rejoined at match time.
 *
 * The desktop release preflight fails any bundle containing a literal
 * `/Users/` or `/home/`, because an absolute developer-machine path baked into
 * an artifact is how a build silently keeps working on the build machine only.
 * That check is right and this denylist must not be the reason it is loosened,
 * so the fragments are assembled here rather than written out.
 */
const FORBIDDEN_PATH_SEGMENTS = ["users", "home", "var/folders", "private/tmp"] as const

const carriesForbiddenPath = (lower: string): boolean =>
  lower.startsWith("~/") ||
  FORBIDDEN_PATH_SEGMENTS.some((segment) => lower.includes(`/${segment}/`))

/**
 * True when `value` is a bounded, opaque, public-safe reference -- the producer
 * mirror of `workroom_receipts::sanitize_public_ref`. Deliberately a rejection
 * rather than a redaction: a hop that would have to be scrubbed is not a hop the
 * owner should be shown.
 */
export const isFullAutoEvidenceRef = (value: string): boolean => {
  if (value.trim() !== value || value.length === 0 || value.length > FULL_AUTO_EVIDENCE_REF_LIMIT) {
    return false
  }
  if (!/^[A-Za-z0-9._:-]+$/.test(value)) return false
  const lower = value.toLowerCase()
  if (lower.startsWith("/")) return false
  if (carriesForbiddenPath(lower)) return false
  if (FORBIDDEN_TEXT_FRAGMENTS.some((fragment) => lower.includes(fragment))) return false
  if (FORBIDDEN_TEXT_PREFIXES.some((prefix) => lower.startsWith(prefix))) return false
  // An undotted, unstructured string of any length is an opaque dump, not a ref.
  if (!lower.includes(".") && !lower.includes(":") && !lower.includes("_") && value.length > 64) {
    return false
  }
  return true
}

/**
 * True when `value` is bounded owner-facing text carrying no credential or
 * private-path shape -- the producer mirror of
 * `workroom_receipts::is_issue31_public_text`.
 */
export const isFullAutoEvidenceText = (value: string, maximumLength: number): boolean => {
  if (value.trim() !== value || value.length === 0 || [...value].length > maximumLength) return false
  // Control characters can forge line structure in the owner transcript.
  if ([...value].some((character) => character !== "\t" && character.codePointAt(0)! < 0x20)) return false
  if ([...value].some((character) => character.codePointAt(0) === 0x7f)) return false
  const lower = value.toLowerCase()
  if (FORBIDDEN_TEXT_FRAGMENTS.some((fragment) => lower.includes(fragment))) return false
  if (FORBIDDEN_TEXT_PREFIXES.some((prefix) => lower.startsWith(prefix))) return false
  return !carriesForbiddenPath(lower)
}

const PublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(FULL_AUTO_EVIDENCE_REF_LIMIT),
  Schema.makeFilter(isFullAutoEvidenceRef, {
    message: "evidence hop is not a bounded public-safe reference",
  }),
)

const PublicCommand = Schema.String.check(
  Schema.makeFilter(
    (value: string) => isFullAutoEvidenceText(value, FULL_AUTO_EVIDENCE_COMMAND_LIMIT),
    { message: "verification command is not bounded public-safe text" },
  ),
)

const PublicText = Schema.String.check(
  Schema.makeFilter(
    (value: string) => isFullAutoEvidenceText(value, FULL_AUTO_EVIDENCE_TEXT_LIMIT),
    { message: "evidence detail is not bounded public-safe text" },
  ),
)

const EpochMs = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/**
 * The durable, host-stamped chain for one finished unit.
 *
 * `hostExecuted` and `allowed` are literal `true` rather than booleans on
 * purpose. The two states the phone must refuse -- a self-reported hop, and a
 * chain that claims an authority decision it never received -- are made
 * UNWRITABLE here rather than merely rejected downstream. The reader still
 * guards both.
 */
export const FullAutoRunEvidenceSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_RUN_EVIDENCE_SCHEMA),
  /** `objective.<sha256(objective)>` -- equals `objective.` + the report's own
   * `objectiveDigest`, so a reader can check the chain names the mission the
   * report is about without either party transmitting the objective text. */
  objectiveRef: PublicRef,
  /** The host-minted ref of the turn whose self-reported completion the host
   * then verified. Read from the host's own turn journal, never supplied. */
  turnRef: PublicRef,
  /** `change.<git HEAD>` in the bound workspace at verification time. */
  changeRef: PublicRef,
  /** `generation.project.<commit count>` at verification time -- the exact
   * generation the change above is bound to. */
  projectGeneration: PublicRef,
  /** `verification.host.<digest of the verdict>` -- identity bound to the run,
   * the command, the exit code, and the instant, so it cannot be re-pointed. */
  verificationRef: PublicRef,
  /** `outcome.test.passed` -- the typed verdict, not prose about it. */
  testOutcome: PublicRef,
  /** The exact command string the host spawned. */
  testCommand: PublicCommand,
  /** `git diff --shortstat` from the run's own baseline commit to the bound
   * workspace as it stood when the host verified it: counts only, no paths. */
  diffSummary: PublicText,
  hostExecuted: Schema.Literal(true),
  /** The authority that decided, and the decision and receipt refs for it. */
  authorityRef: PublicRef,
  decisionRef: PublicRef,
  authorityReceiptRef: PublicRef,
  allowed: Schema.Literal(true),
  recordedAtMs: EpochMs,
})
export type FullAutoRunEvidence = typeof FullAutoRunEvidenceSchema.Type

const decodeFullAutoRunEvidenceExit = Schema.decodeUnknownExit(FullAutoRunEvidenceSchema)

/**
 * The run's own starting point in the bound workspace, measured by the host
 * once, on the first entry into Running. It exists so the change hop is the
 * change THIS RUN made rather than whatever happened to be uncommitted when
 * someone looked. Never supplied by a caller and never rewritten: a run start
 * does not move, and neither does the tree it started from.
 */
export const FullAutoWorkspaceBaselineSchema = Schema.Struct({
  headRef: Schema.String.check(Schema.isLengthBetween(40, 40)),
  generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  recordedAtMs: EpochMs,
})
export type FullAutoWorkspaceBaseline = typeof FullAutoWorkspaceBaselineSchema.Type

/**
 * What the host reads out of a bound workspace in ONE pass. `diffShortstat` is
 * the `git diff --shortstat` counts against the supplied baseline commit (or
 * against HEAD when no baseline is given), and is the empty string when nothing
 * changed -- an honest "no change", never an omission.
 */
export type FullAutoWorkspaceMeasurement = Readonly<{
  headRef: string
  generation: number
  diffShortstat: string
}>

/**
 * The injected workspace reader. The host binds a real Git adapter
 * ({@link makeNodeWorkspaceProbe}); tests inject a deterministic stub. Returns
 * null when the workspace cannot be measured (not a repository, missing, or Git
 * unavailable) so the caller records NO evidence rather than an invented hop.
 */
export type FullAutoWorkspaceProbe = (
  input: Readonly<{ workspaceRef: string; baselineRef?: string }>,
) => Promise<FullAutoWorkspaceMeasurement | null>

const HEX_40 = /^[0-9a-f]{40}$/

/**
 * A Git-backed workspace probe. Runs three read-only plumbing commands with no
 * shell interpolation of the workspace path, and returns null on any failure --
 * a workspace the host cannot measure yields no evidence at all.
 */
export const makeNodeWorkspaceProbe = (): FullAutoWorkspaceProbe => async ({ workspaceRef, baselineRef }) => {
  const { execFile } = await import("node:child_process")
  const run = (args: ReadonlyArray<string>): Promise<string | null> =>
    new Promise((resolve) => {
      execFile(
        "git",
        ["-C", workspaceRef, ...args],
        { timeout: 30_000, maxBuffer: 1_000_000 },
        (error, stdout) => resolve(error === null ? stdout : null),
      )
    })
  const head = (await run(["rev-parse", "HEAD"]))?.trim() ?? null
  if (head === null || !HEX_40.test(head)) return null
  const count = (await run(["rev-list", "--count", "HEAD"]))?.trim() ?? null
  const generation = count === null ? null : Number.parseInt(count, 10)
  if (generation === null || !Number.isSafeInteger(generation) || generation < 0) return null
  const diff = await run(
    baselineRef === undefined ? ["diff", "--shortstat"] : ["diff", "--shortstat", baselineRef],
  )
  if (diff === null) return null
  return { headRef: head, generation, diffShortstat: diff.trim() }
}

/**
 * Read the bound workspace as it stands right now, for use as a run's baseline.
 *
 * Taken by the host BEFORE the run can dispatch a turn, so the change hop later
 * measured against it is the change this run made and not the tree's prior
 * state. Returns null when the host cannot measure the workspace, in which case
 * the run honestly carries no baseline.
 */
export const measureFullAutoWorkspaceBaseline = async (
  input: Readonly<{
    probe: FullAutoWorkspaceProbe
    workspaceRef: string
    now?: () => Date
  }>,
): Promise<FullAutoWorkspaceBaseline | null> => {
  const measurement = await input.probe({ workspaceRef: input.workspaceRef })
  if (measurement === null) return null
  return {
    headRef: measurement.headRef,
    generation: measurement.generation,
    recordedAtMs: (input.now ?? (() => new Date()))().getTime(),
  }
}

/**
 * A verification verdict the HOST ITSELF produced by running a command and
 * reading its exit code. There is no public constructor: the only way to obtain
 * one is {@link asHostExecutedVerification}, so `hostExecuted: true` on the
 * emitted record cannot be written about anything else.
 */
export type FullAutoHostExecutedVerification = Readonly<{
  readonly _tag: "FullAutoHostExecutedVerification"
  command: string
  exitCode: number
  at: string
}>

/**
 * Narrow a verification result to a host-EXECUTED pass, or null.
 *
 * Three conditions, all required. The spec must be `command` (an `evidence_ref`
 * check is host-consulted, not host-executed, and an owner-gated `none` is
 * neither). The verdict must be `passed` -- the only verdict that admits
 * completion. And `exitCode` must be a real number, which is non-null exactly
 * when a child process actually ran, so a spawn failure or timeout (`error`,
 * exit code null) can never masquerade as execution.
 */
export const asHostExecutedVerification = (
  result: FullAutoVerificationResult,
): FullAutoHostExecutedVerification | null => {
  if (result.spec.kind !== "command") return null
  if (result.status !== "passed") return null
  if (result.exitCode === null) return null
  return {
    _tag: "FullAutoHostExecutedVerification",
    command: result.spec.command,
    exitCode: result.exitCode,
    at: result.at,
  }
}

const sha256Hex = (value: string): string => createHash("sha256").update(value, "utf8").digest("hex")

export type BuildFullAutoRunEvidenceInput = Readonly<{
  runRef: string
  /** The run's objective text as the host holds it AT verification time, so the
   * chain names the objective that was actually verified rather than the one the
   * run happened to start with. */
  objective: string
  /** The host's own journal ref for the turn that triggered verification. */
  turnRef: string
  /** The single workspace reading taken at verification time. */
  measurement: FullAutoWorkspaceMeasurement
  /** The baseline the diff above was measured against, when the host recorded
   * one at run start. */
  baseline: FullAutoWorkspaceBaseline | null
  /** The host's own executed pass. */
  verification: FullAutoHostExecutedVerification
  recordedAtMs: number
}>

/**
 * Build the chain for one finished unit, or return null.
 *
 * Following `issue31_host.rs`'s emitter discipline, this assembles plain JSON
 * and hands it to the record's OWN decoder rather than constructing the typed
 * value. There is therefore exactly one place the boundaries live, and the
 * producer structurally cannot emit a record the reader would refuse: an
 * over-long verification command, a diff summary carrying a private path, or a
 * ref that is not bounded and opaque all fail the decode and yield null, which
 * the caller records as no evidence at all.
 */
export const buildFullAutoRunEvidence = (
  input: BuildFullAutoRunEvidenceInput,
): FullAutoRunEvidence | null => {
  const verificationDigest = sha256Hex(
    [
      input.runRef,
      input.verification.at,
      input.verification.command,
      String(input.verification.exitCode),
      input.measurement.headRef,
    ].join("\n"),
  ).slice(0, 32)
  const decisionDigest = sha256Hex(
    [input.runRef, verificationDigest, input.verification.at].join("\n"),
  ).slice(0, 32)
  const baselineTag = input.baseline === null ? null : input.baseline.headRef.slice(0, 7)
  const diffSummary =
    input.measurement.diffShortstat.length > 0
      ? `${baselineTag === null ? "uncommitted" : `since ${baselineTag}`}: ${input.measurement.diffShortstat}`
      : `no files changed since ${baselineTag ?? input.measurement.headRef.slice(0, 7)}`

  const decoded = decodeFullAutoRunEvidenceExit({
    schema: FULL_AUTO_RUN_EVIDENCE_SCHEMA,
    objectiveRef: `objective.${sha256Hex(input.objective)}`,
    turnRef: input.turnRef,
    changeRef: `change.${input.measurement.headRef}`,
    projectGeneration: `generation.project.${String(input.measurement.generation).padStart(5, "0")}`,
    verificationRef: `verification.host.${verificationDigest}`,
    testOutcome: "outcome.test.passed",
    testCommand: input.verification.command,
    diffSummary,
    hostExecuted: true,
    authorityRef: FULL_AUTO_COMPLETION_AUTHORITY_REF,
    decisionRef: `decision.full_auto.completion.${decisionDigest}`,
    authorityReceiptRef: `receipt.authority.full_auto.completion.${decisionDigest}`,
    allowed: true,
    recordedAtMs: input.recordedAtMs,
  })
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * The public-safe receipt's share of the chain: the four hops it must agree
 * with the report on, plus the authority that allowed the completion, its
 * decision, and its receipt.
 *
 * The field schemas are REUSED from the record above rather than restated, so
 * the bounds the phone applies live in exactly one place and cannot drift
 * between what is stored and what is published. Every value is a bounded,
 * opaque, system-minted ref, so the receipt stays structurally incapable of
 * carrying free text.
 */
export const FullAutoEvidenceReceiptBlockSchema = Schema.Struct({
  objectiveRef: FullAutoRunEvidenceSchema.fields.objectiveRef,
  turnRef: FullAutoRunEvidenceSchema.fields.turnRef,
  changeRef: FullAutoRunEvidenceSchema.fields.changeRef,
  verificationRef: FullAutoRunEvidenceSchema.fields.verificationRef,
  authorityRef: FullAutoRunEvidenceSchema.fields.authorityRef,
  decisionRef: FullAutoRunEvidenceSchema.fields.decisionRef,
  authorityReceiptRef: FullAutoRunEvidenceSchema.fields.authorityReceiptRef,
  allowed: FullAutoRunEvidenceSchema.fields.allowed,
})
export type FullAutoEvidenceReceiptBlock = typeof FullAutoEvidenceReceiptBlockSchema.Type

/**
 * Project the receipt's view of the chain from the ONE stored record -- the
 * same object the report publishes whole. Because both reads take that same
 * source, the four hops the two records share cannot tell two stories about one
 * run: there is no code path that could give them different values.
 */
export const projectFullAutoEvidenceReceiptBlock = (
  evidence: FullAutoRunEvidence,
): FullAutoEvidenceReceiptBlock => ({
  objectiveRef: evidence.objectiveRef,
  turnRef: evidence.turnRef,
  changeRef: evidence.changeRef,
  verificationRef: evidence.verificationRef,
  authorityRef: evidence.authorityRef,
  decisionRef: evidence.decisionRef,
  authorityReceiptRef: evidence.authorityReceiptRef,
  allowed: evidence.allowed,
})
