/**
 * Shared AssuranceSpec tool handlers (AT-1, docs/assurance/AGENT_TOOLING.md).
 *
 * One implementation, two transports: every handler here is an Effect program
 * consumed unchanged by the CLI (src/cli.ts) and the stdio MCP server
 * (src/mcp.ts). Handlers are read-only, deterministic, and never call a model
 * (Law 2). Failures are typed values with stable error codes (§3.2); codes
 * are API here exactly as they are in the validator.
 *
 * Security posture (copied from the upstream ProductSpec server because it is
 * correct): resolve all paths inside `root`, reject `..`, resolve realpaths,
 * and skip symlinks plus `.git`, `node_modules`, `dist` during walks.
 */
import { existsSync, lstatSync, readFileSync, readdirSync, realpathSync } from "node:fs"
import { isAbsolute, join, normalize, relative, resolve } from "node:path"

import { Effect, Schema } from "effect"

import {
  computeProductSpecDocumentDigest,
  validateExecutableProductSpec,
  validateProductSpec,
} from "@openagentsinc/product-spec"

import {
  AGENT_RUN_INGEST_SCHEMA,
  validateAgentRunJson,
  type AgentRunSelfReportEvidence,
} from "./agent-run.ts"
import { projectObligationGraph, type ObligationGraph } from "./graph.ts"
import { inventoryRepository } from "./repository-inventory.ts"
import {
  ASSURANCE_SPEC_EXTENSION,
  type AssuranceDiagnostic,
  type AssuranceSpecDocument,
  type RepositoryInventory,
} from "./schema.ts"
import {
  buildSessionPin,
  classifySessionStatus,
  completionClaimAudit,
  coverageLedgers,
  environmentReport,
  evidenceChecklist,
  gateReport,
  obligationDetail,
  recommendedActionForStatus,
  seamReport,
  sha256Digest,
  summarizeObligations,
  typedGapReport,
  type AssuranceSessionCheck,
  type AssuranceSessionPin,
  type AssuranceSessionStatus,
  type CompletionClaimAudit,
  type CoverageLedgers,
  type EnvironmentReport,
  type EvidenceChecklist,
  type GateReport,
  type ObligationDetail,
  type ObligationFilter,
  type ObligationSummary,
  type SeamReport,
  type SubjectProbe,
  type TypedGapReport,
} from "./tooling.ts"
import { validateAssuranceSpec, type AssuranceStructuralValidation } from "./validator.ts"

// ---------------------------------------------------------------------------
// Typed tool errors (stable codes are API, §3.2)
// ---------------------------------------------------------------------------

export class AssuranceToolError extends Schema.TaggedErrorClass<AssuranceToolError>()(
  "AssuranceToolError",
  {
    code: Schema.String,
    message: Schema.String,
    path: Schema.optionalKey(Schema.String),
    errors: Schema.optionalKey(Schema.Array(Schema.Unknown)),
  },
) {}

const toolError = (
  code: string,
  message: string,
  options: Readonly<{ path?: string; errors?: ReadonlyArray<unknown> }> = {},
): AssuranceToolError =>
  new AssuranceToolError({
    code,
    message,
    ...(options.path === undefined ? {} : { path: options.path }),
    ...(options.errors === undefined ? {} : { errors: options.errors }),
  })

export type ToolFailure = Readonly<{
  ok: false
  code: string
  message: string
  path?: string
  errors?: ReadonlyArray<unknown>
}>

export type ToolOutcome<A> = Readonly<{ ok: true; value: A }> | ToolFailure

/** Run a shared handler synchronously and normalize failures to §3.2 shape. */
export const runTool = <A>(program: Effect.Effect<A, AssuranceToolError>): ToolOutcome<A> => {
  try {
    return Effect.runSync(Effect.match(program, {
      onSuccess: (value): ToolOutcome<A> => ({ ok: true, value }),
      onFailure: (error): ToolOutcome<A> => ({
        ok: false,
        code: error.code,
        message: error.message,
        ...(error.path === undefined ? {} : { path: error.path }),
        ...(error.errors === undefined ? {} : { errors: error.errors }),
      }),
    }))
  } catch (error) {
    return {
      ok: false,
      code: "internal_error",
      message: error instanceof Error ? error.message : String(error),
    }
  }
}

// ---------------------------------------------------------------------------
// Root-confined path resolution
// ---------------------------------------------------------------------------

const WALK_SKIP_DIRECTORIES = new Set([".git", "node_modules", "dist"])

const toPosix = (value: string): string => value.replaceAll("\\", "/")

export const resolveRoot = (root?: string): string => resolve(root ?? process.cwd())

type ConfinedPath = Readonly<{ absolute: string; relative: string }>

const confinePath = (
  root: string,
  path: string,
): Effect.Effect<ConfinedPath, AssuranceToolError> =>
  Effect.suspend(() => {
    if (path.trim() === "" || path.includes("\0")) {
      return Effect.fail(toolError("invalid_path", "Path must be a non-empty string without NUL bytes.", { path }))
    }
    if (toPosix(path).split("/").includes("..")) {
      return Effect.fail(toolError("invalid_path", `Path must not contain '..' segments: ${path}`, { path }))
    }
    const absolute = isAbsolute(path) ? normalize(path) : resolve(root, path)
    const relativePath = relative(root, absolute)
    if (relativePath.startsWith("..") || isAbsolute(relativePath)) {
      return Effect.fail(toolError("path_outside_root", `Path must stay inside root: ${path}`, { path }))
    }
    if (!existsSync(absolute)) {
      return Effect.fail(toolError("file_not_found", `File does not exist: ${path}`, { path }))
    }
    let realRoot: string
    let realAbsolute: string
    try {
      realRoot = realpathSync(root)
      realAbsolute = realpathSync(absolute)
    } catch {
      return Effect.fail(toolError("file_not_found", `File is not readable: ${path}`, { path }))
    }
    const realRelative = relative(realRoot, realAbsolute)
    if (realRelative.startsWith("..") || isAbsolute(realRelative)) {
      return Effect.fail(toolError("path_outside_root", `Path resolves outside root: ${path}`, { path }))
    }
    return Effect.succeed({ absolute: realAbsolute, relative: toPosix(relativePath) })
  })

// ---------------------------------------------------------------------------
// Spec loading
// ---------------------------------------------------------------------------

type LoadedSpec = Readonly<{
  absolute: string
  relative: string
  markdown: string
  digest: string
  validation: AssuranceStructuralValidation
}>

type ValidSpec = LoadedSpec & Readonly<{ document: AssuranceSpecDocument }>

const readSpecFile = (
  root: string,
  path: string,
): Effect.Effect<LoadedSpec, AssuranceToolError> =>
  Effect.gen(function* () {
    if (!path.endsWith(ASSURANCE_SPEC_EXTENSION)) {
      return yield* toolError(
        "invalid_assurance_spec_path",
        `AssuranceSpec paths must end in ${ASSURANCE_SPEC_EXTENSION}: ${path}`,
        { path },
      )
    }
    const confined = yield* confinePath(root, path)
    let markdown: string
    try {
      markdown = readFileSync(confined.absolute, "utf8")
    } catch {
      return yield* toolError("file_not_found", `File is not readable: ${path}`, { path })
    }
    return {
      absolute: confined.absolute,
      relative: confined.relative,
      markdown,
      digest: sha256Digest(markdown),
      validation: validateAssuranceSpec(markdown),
    }
  })

const readValidSpec = (
  root: string,
  path: string,
): Effect.Effect<ValidSpec, AssuranceToolError> =>
  Effect.gen(function* () {
    const loaded = yield* readSpecFile(root, path)
    if (!loaded.validation.valid || loaded.validation.document === undefined) {
      const first = loaded.validation.errors[0]
      return yield* toolError(
        first?.code ?? "invalid_assurance_spec",
        `AssuranceSpec is invalid: ${loaded.validation.errors.map((error) => `${error.code}: ${error.message}`).join("; ")}`,
        { path: loaded.relative, errors: loaded.validation.errors },
      )
    }
    return { ...loaded, document: loaded.validation.document }
  })

const probeSubject = (root: string, document: AssuranceSpecDocument): SubjectProbe => {
  const declared = document.subject.product_spec
  const base: Pick<SubjectProbe, "declared_path" | "declared_revision" | "declared_digest"> = {
    declared_path: declared.path,
    declared_revision: declared.spec_revision,
    declared_digest: declared.document_digest,
  }
  const outcome = runTool(confinePath(root, declared.path))
  if (!outcome.ok) {
    return {
      ...base,
      status: "missing",
      errors: [{ code: outcome.code, message: outcome.message }],
    }
  }
  let markdown: string
  try {
    markdown = readFileSync(outcome.value.absolute, "utf8")
  } catch {
    return {
      ...base,
      status: "missing",
      errors: [{ code: "file_not_found", message: `Subject is not readable: ${declared.path}` }],
    }
  }
  const currentDigest = sha256Digest(markdown)
  const result = validateExecutableProductSpec(markdown)
  const revision = result.document?.frontmatter.spec_revision
  return {
    ...base,
    status: currentDigest === declared.document_digest ? "bound" : "stale",
    current_digest: currentDigest,
    ...(revision === undefined ? {} : { current_revision: revision }),
    current_executable: result.executable,
    errors: result.executable
      ? []
      : result.errors.map((error) => ({ code: error.code, message: error.message })),
  }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

export type PathArgs = Readonly<{ root?: string; path: string }>

/**
 * Ingest an upstream Agent Run 0.1 as self-reported evidence only. This is a
 * read-only projection: claimed item statuses never become observations.
 */
export const ingestAgentRun = (
  args: PathArgs,
): Effect.Effect<AgentRunSelfReportEvidence, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    if (!args.path.endsWith(".agent-run.json")) {
      return yield* toolError(
        "invalid_agent_run_path",
        `Agent Run paths must end in .agent-run.json: ${args.path}`,
        { path: args.path },
      )
    }
    const sourcePath = yield* confinePath(root, args.path)
    let source: string
    try {
      source = readFileSync(sourcePath.absolute, "utf8")
    } catch {
      return yield* toolError("file_not_found", `File is not readable: ${args.path}`, { path: args.path })
    }
    const validation = validateAgentRunJson(source)
    if (!validation.valid) {
      const first = validation.errors[0]
      return yield* toolError(
        first?.code ?? "invalid_agent_run",
        `Agent Run is invalid: ${validation.errors.map((error) => `${error.code}: ${error.message}`).join("; ")}`,
        { path: sourcePath.relative, errors: validation.errors },
      )
    }
    const run = validation.document
    const productSpecPath = yield* confinePath(root, run.product_spec.path)
    let markdown: string
    try {
      markdown = readFileSync(productSpecPath.absolute, "utf8")
    } catch {
      return yield* toolError("file_not_found", `ProductSpec is not readable: ${run.product_spec.path}`, { path: run.product_spec.path })
    }
    const productSpec = validateProductSpec(markdown, { profile: "upstream" })
    if (!productSpec.valid) {
      const first = productSpec.errors[0]
      return yield* toolError(
        "invalid_pinned_product_spec",
        `Pinned ProductSpec is invalid: ${productSpec.errors.map((error) => `${error.code}: ${error.message}`).join("; ")}`,
        { path: productSpecPath.relative, errors: productSpec.errors },
      )
    }
    const actualRevision = productSpec.document.frontmatter.spec_revision
    if (actualRevision !== run.product_spec.spec_revision) {
      return yield* toolError(
        "product_spec_revision_mismatch",
        `Agent Run pins ProductSpec revision ${run.product_spec.spec_revision}, but ${productSpecPath.relative} is revision ${actualRevision ?? "missing"}.`,
        { path: productSpecPath.relative },
      )
    }
    const computedHash = computeProductSpecDocumentDigest(markdown)
    if (run.product_spec.content_hash !== undefined && run.product_spec.content_hash !== computedHash) {
      return yield* toolError(
        "product_spec_digest_mismatch",
        `Agent Run ProductSpec content_hash does not match ${productSpecPath.relative}.`,
        { path: productSpecPath.relative },
      )
    }
    const itemIds = new Set(productSpec.document.sections.flatMap((section) => [
      ...(section.acceptance_criteria ?? []).map((item) => item.id),
      ...(section.ai_evals ?? []).map((item) => item.id),
      ...(section.success_metrics ?? []).map((item) => item.id),
    ]))
    const unknownItems = run.checked_items.filter((item) => !itemIds.has(item.item_id))
    if (unknownItems.length > 0) {
      return yield* toolError(
        "agent_run_item_not_found",
        `Agent Run cites item ids absent from the pinned ProductSpec: ${unknownItems.map((item) => item.item_id).join(", ")}.`,
        {
          path: productSpecPath.relative,
          errors: unknownItems.map((item) => ({ code: "agent_run_item_not_found", item_id: item.item_id })),
        },
      )
    }
    const identity = {
      name: run.agent.name,
      ...(run.agent.version === undefined ? {} : { version: run.agent.version }),
    }
    return {
      schema: AGENT_RUN_INGEST_SCHEMA,
      source: { path: sourcePath.relative, document_digest: sha256Digest(source) },
      agent_run_format_version: run.agent_run_format_version,
      run_id: run.run_id,
      run_status: run.status,
      started_at: run.started_at,
      ...(run.completed_at === undefined ? {} : { completed_at: run.completed_at }),
      proof_rung: "self_report",
      producer: identity,
      claimant: identity,
      producer_equals_claimant: true,
      independently_verified: false,
      observation_axis: "not_promoted",
      spec_pin: {
        path: productSpecPath.relative,
        spec_revision: run.product_spec.spec_revision,
        declared_content_hash: run.product_spec.content_hash ?? null,
        computed_content_hash: computedHash,
        digest_status: run.product_spec.content_hash === undefined ? "missing" : "matched",
      },
      claimed_items: run.checked_items,
      drift: run.drift,
      ...(run.completion_claim === undefined ? {} : { completion_claim: run.completion_claim }),
      gaps: run.product_spec.content_hash === undefined
        ? [{ code: "missing_product_spec_content_hash", message: "The Agent Run omitted the optional ProductSpec content_hash; revision and item identity were checked, but byte identity was not pinned by the producer." }]
        : [],
      authority: {
        can_promote_observation: false,
        can_verify: false,
        can_satisfy_independent_producer: false,
      },
    }
  })

export const beginAssuranceSession = (
  args: PathArgs,
): Effect.Effect<AssuranceSessionPin, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    const spec = yield* readValidSpec(root, args.path)
    const subject = probeSubject(root, spec.document)
    if (subject.status === "missing") {
      return yield* toolError(
        "subject_missing",
        `The bound ProductSpec subject is not readable inside root: ${spec.document.subject.product_spec.path}`,
        { path: spec.document.subject.product_spec.path, errors: subject.errors },
      )
    }
    if (subject.current_executable !== true) {
      return yield* toolError(
        "product_spec_not_executable",
        `The bound ProductSpec subject is not an executable ProductSpec: ${spec.document.subject.product_spec.path}`,
        { path: spec.document.subject.product_spec.path, errors: subject.errors },
      )
    }
    return buildSessionPin({
      assuranceSpecPath: spec.relative,
      assuranceSpecDigest: spec.digest,
      document: spec.document,
      subject,
    })
  })

const DIGEST_PATTERN = /^(?:sha256:)?([a-f0-9]{64})$/

const normalizeDigest = (
  value: string,
  label: string,
): Effect.Effect<string, AssuranceToolError> =>
  Effect.suspend(() => {
    const match = DIGEST_PATTERN.exec(value.trim().toLowerCase())
    return match === null
      ? Effect.fail(toolError(
          "invalid_session_pin",
          `${label} must be a sha256 digest (64 hex characters, optionally prefixed with sha256:).`,
        ))
      : Effect.succeed(`sha256:${match[1]}`)
  })

export type SessionCheckArgs = Readonly<{
  root?: string
  path: string
  session_id?: string
  spec_digest?: string
  subject_digest?: string
  pin?: unknown
}>

const pinnedDigests = (
  args: SessionCheckArgs,
): Effect.Effect<Readonly<{ spec: string; subject: string }>, AssuranceToolError> =>
  Effect.gen(function* () {
    if (args.pin !== undefined) {
      const pin = args.pin as {
        assurance_spec?: { document_digest?: unknown }
        subject?: { document_digest?: unknown }
      }
      const spec = pin?.assurance_spec?.document_digest
      const subject = pin?.subject?.document_digest
      if (typeof spec !== "string" || typeof subject !== "string") {
        return yield* toolError(
          "invalid_session_pin",
          "pin must be the full record returned by begin_assurance_session (assurance_spec.document_digest and subject.document_digest).",
        )
      }
      return {
        spec: yield* normalizeDigest(spec, "pin.assurance_spec.document_digest"),
        subject: yield* normalizeDigest(subject, "pin.subject.document_digest"),
      }
    }
    if (args.spec_digest !== undefined && args.subject_digest !== undefined) {
      return {
        spec: yield* normalizeDigest(args.spec_digest, "spec_digest"),
        subject: yield* normalizeDigest(args.subject_digest, "subject_digest"),
      }
    }
    return yield* toolError(
      "session_pin_required",
      "Sessions are stateless: pass the full pin returned by begin_assurance_session (pin object, or spec_digest and subject_digest). A session_id alone cannot be resolved because no daemon stores sessions.",
    )
  })

export const checkAssuranceSession = (
  args: SessionCheckArgs,
): Effect.Effect<AssuranceSessionCheck, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    const pinned = yield* pinnedDigests(args)
    const sessionId = args.session_id
    const invalidCurrent = (options: Readonly<{
      specCurrent?: string
      subjectPath?: string
      subjectCurrent?: string
      errors: ReadonlyArray<{ code: string; message: string }>
    }>): AssuranceSessionCheck => ({
      ...(sessionId === undefined ? {} : { session_id: sessionId }),
      status: "invalid_current",
      recommended_action: "resolve_invalid_current",
      assurance_spec: {
        path: args.path,
        pinned_digest: pinned.spec,
        ...(options.specCurrent === undefined ? {} : { current_digest: options.specCurrent }),
        changed: options.specCurrent !== undefined && options.specCurrent !== pinned.spec,
      },
      subject: {
        ...(options.subjectPath === undefined ? {} : { path: options.subjectPath }),
        pinned_digest: pinned.subject,
        ...(options.subjectCurrent === undefined ? {} : { current_digest: options.subjectCurrent }),
        changed: options.subjectCurrent !== undefined && options.subjectCurrent !== pinned.subject,
      },
      errors: options.errors,
    })

    if (!args.path.endsWith(ASSURANCE_SPEC_EXTENSION)) {
      return yield* toolError(
        "invalid_assurance_spec_path",
        `AssuranceSpec paths must end in ${ASSURANCE_SPEC_EXTENSION}: ${args.path}`,
        { path: args.path },
      )
    }
    const confineOutcome = yield* Effect.match(confinePath(root, args.path), {
      onSuccess: (value): ConfinedPath | AssuranceToolError => value,
      onFailure: (error): ConfinedPath | AssuranceToolError => error,
    })
    if (confineOutcome instanceof AssuranceToolError) {
      // Confinement violations stay hard failures; only a missing/unreadable
      // file is a resolvable invalid_current session state.
      if (confineOutcome.code !== "file_not_found") return yield* confineOutcome
      return invalidCurrent({
        errors: [{ code: "file_not_found", message: `AssuranceSpec is not readable inside root: ${args.path}` }],
      })
    }
    const confined: ConfinedPath = confineOutcome
    let markdown: string
    try {
      markdown = readFileSync(confined.absolute, "utf8")
    } catch {
      return invalidCurrent({
        errors: [{ code: "file_not_found", message: `AssuranceSpec is not readable: ${args.path}` }],
      })
    }
    const currentSpecDigest = sha256Digest(markdown)
    const validation = validateAssuranceSpec(markdown)
    if (!validation.valid || validation.document === undefined) {
      return invalidCurrent({
        specCurrent: currentSpecDigest,
        errors: validation.errors.map((error) => ({ code: error.code, message: error.message })),
      })
    }
    const subject = probeSubject(root, validation.document)
    if (subject.status === "missing" || subject.current_digest === undefined) {
      return invalidCurrent({
        specCurrent: currentSpecDigest,
        subjectPath: subject.declared_path,
        errors: [
          { code: "subject_missing", message: `The bound ProductSpec subject is not readable inside root: ${subject.declared_path}` },
          ...subject.errors,
        ],
      })
    }
    const status: AssuranceSessionStatus = classifySessionStatus({
      pinnedSpecDigest: pinned.spec,
      pinnedSubjectDigest: pinned.subject,
      currentSpecDigest,
      currentSubjectDigest: subject.current_digest,
    })
    return {
      ...(sessionId === undefined ? {} : { session_id: sessionId }),
      status,
      recommended_action: recommendedActionForStatus(status),
      assurance_spec: {
        path: confined.relative,
        pinned_digest: pinned.spec,
        current_digest: currentSpecDigest,
        changed: currentSpecDigest !== pinned.spec,
      },
      subject: {
        path: subject.declared_path,
        pinned_digest: pinned.subject,
        current_digest: subject.current_digest,
        changed: subject.current_digest !== pinned.subject,
      },
      errors: [],
    }
  })

// ---------------------------------------------------------------------------
// Discovery and document reads
// ---------------------------------------------------------------------------

export type AssuranceSpecListItem = Readonly<{
  path: string
  valid: boolean
  assurance_spec_id?: string
  assurance_revision?: number
  lifecycle_state?: string
  subject_path?: string
  error_count: number
  warning_count: number
}>

const walkAssuranceSpecFiles = (root: string): ReadonlyArray<string> => {
  const results: string[] = []
  const visit = (directory: string): void => {
    let entries
    try {
      entries = readdirSync(directory, { withFileTypes: true })
    } catch {
      return
    }
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue
      const absolute = join(directory, entry.name)
      if (entry.isDirectory()) {
        if (WALK_SKIP_DIRECTORIES.has(entry.name)) continue
        visit(absolute)
      } else if (entry.isFile() && entry.name.endsWith(ASSURANCE_SPEC_EXTENSION)) {
        results.push(absolute)
      }
    }
  }
  visit(root)
  return results.sort()
}

export const listAssuranceSpecs = (
  args: Readonly<{ root?: string }> = {},
): Effect.Effect<ReadonlyArray<AssuranceSpecListItem>, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    if (!existsSync(root)) {
      return yield* toolError("file_not_found", `Root does not exist: ${root}`, { path: root })
    }
    return walkAssuranceSpecFiles(root).map((absolute) => {
      const path = toPosix(relative(root, absolute))
      let markdown: string
      try {
        markdown = readFileSync(absolute, "utf8")
      } catch {
        return { path, valid: false, error_count: 1, warning_count: 0 }
      }
      const validation = validateAssuranceSpec(markdown)
      if (!validation.valid || validation.document === undefined) {
        return {
          path,
          valid: false,
          error_count: validation.errors.length,
          warning_count: validation.warnings.length,
        }
      }
      return {
        path,
        valid: true,
        assurance_spec_id: validation.document.frontmatter.assurance_spec_id,
        assurance_revision: validation.document.frontmatter.assurance_revision,
        lifecycle_state: validation.document.frontmatter.lifecycle_state,
        subject_path: validation.document.subject.product_spec.path,
        error_count: 0,
        warning_count: validation.warnings.length,
      }
    })
  })

export const getAssuranceSpec = (
  args: PathArgs,
): Effect.Effect<AssuranceSpecDocument, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => spec.document)

export type AssuranceValidationReport = Readonly<{
  path: string
  valid: boolean
  errors: ReadonlyArray<AssuranceDiagnostic>
  warnings: ReadonlyArray<AssuranceDiagnostic>
}>

export const validateAssuranceSpecFile = (
  args: PathArgs,
): Effect.Effect<AssuranceValidationReport, AssuranceToolError> =>
  Effect.map(readSpecFile(resolveRoot(args.root), args.path), (loaded) => ({
    path: loaded.relative,
    valid: loaded.validation.valid,
    errors: loaded.validation.errors,
    warnings: loaded.validation.warnings,
  }))

export type SubjectBindingReport = Readonly<{
  subject: AssuranceSpecDocument["subject"]["product_spec"]
  subject_status: SubjectProbe["status"]
  declared_digest: string
  current_digest?: string
  current_revision?: number
  errors: ReadonlyArray<{ code: string; message: string }>
}>

export const getSubjectBinding = (
  args: PathArgs,
): Effect.Effect<SubjectBindingReport, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    const spec = yield* readValidSpec(root, args.path)
    const probe = probeSubject(root, spec.document)
    return {
      subject: spec.document.subject.product_spec,
      subject_status: probe.status,
      declared_digest: probe.declared_digest,
      ...(probe.current_digest === undefined ? {} : { current_digest: probe.current_digest }),
      ...(probe.current_revision === undefined ? {} : { current_revision: probe.current_revision }),
      errors: probe.errors,
    }
  })

// ---------------------------------------------------------------------------
// Obligations, seams, environments, gates
// ---------------------------------------------------------------------------

export type ObligationsArgs = PathArgs & Readonly<{
  criterion_ref?: string
  status?: string
  technique?: string
}>

export const getObligations = (
  args: ObligationsArgs,
): Effect.Effect<ReadonlyArray<ObligationSummary>, AssuranceToolError> =>
  Effect.gen(function* () {
    if (args.status !== undefined && args.status !== "ready" && args.status !== "needs_design") {
      return yield* toolError(
        "invalid_argument",
        `status must be "ready" or "needs_design", received: ${args.status}`,
      )
    }
    const spec = yield* readValidSpec(resolveRoot(args.root), args.path)
    const filter: ObligationFilter = {
      ...(args.criterion_ref === undefined ? {} : { criterionRef: args.criterion_ref }),
      ...(args.status === undefined ? {} : { status: args.status }),
      ...(args.technique === undefined ? {} : { technique: args.technique }),
    }
    return summarizeObligations(spec.document, filter)
  })

export const getObligation = (
  args: PathArgs & Readonly<{ obligation_id: string }>,
): Effect.Effect<ObligationDetail, AssuranceToolError> =>
  Effect.gen(function* () {
    const spec = yield* readValidSpec(resolveRoot(args.root), args.path)
    const detail = obligationDetail(spec.document, args.obligation_id)
    if (detail === null) {
      return yield* toolError(
        "obligation_not_found",
        `No obligation with ID ${args.obligation_id} exists in ${spec.relative}.`,
        { path: spec.relative },
      )
    }
    return detail
  })

export const getObligationGraph = (
  args: PathArgs,
): Effect.Effect<ObligationGraph, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => projectObligationGraph(spec.document))

export const getSeams = (args: PathArgs): Effect.Effect<SeamReport, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => seamReport(spec.document))

export const getEnvironments = (
  args: PathArgs,
): Effect.Effect<EnvironmentReport, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => environmentReport(spec.document))

export const getGates = (args: PathArgs): Effect.Effect<GateReport, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => gateReport(spec.document))

// ---------------------------------------------------------------------------
// Ledgers, checklist, claim, gaps, inventory
// ---------------------------------------------------------------------------

export const getCoverageLedgers = (
  args: PathArgs,
): Effect.Effect<CoverageLedgers, AssuranceToolError> =>
  Effect.map(readValidSpec(resolveRoot(args.root), args.path), (spec) => coverageLedgers(spec.document))

export const getEvidenceChecklist = (
  args: PathArgs & Readonly<{ criterion_ref?: string }>,
): Effect.Effect<EvidenceChecklist, AssuranceToolError> =>
  Effect.gen(function* () {
    const spec = yield* readValidSpec(resolveRoot(args.root), args.path)
    const checklist = evidenceChecklist(spec.document, args.criterion_ref)
    if (checklist === null) {
      return yield* toolError(
        "criterion_not_found",
        `The subject does not declare criterion ${args.criterion_ref}.`,
        { path: spec.relative },
      )
    }
    return checklist
  })

export const checkCompletionClaim = (
  args: PathArgs & Readonly<{ claim?: string }>,
): Effect.Effect<CompletionClaimAudit, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    const spec = yield* readValidSpec(root, args.path)
    return completionClaimAudit(spec.document, probeSubject(root, spec.document), args.claim)
  })

export const getTypedGaps = (
  args: PathArgs,
): Effect.Effect<TypedGapReport, AssuranceToolError> =>
  Effect.gen(function* () {
    const root = resolveRoot(args.root)
    const spec = yield* readValidSpec(root, args.path)
    return typedGapReport(spec.document, probeSubject(root, spec.document))
  })

export type RepositoryInventoryReport = RepositoryInventory & Readonly<{
  candidates_not_proof: true
}>

export const getRepositoryInventory = (
  args: Readonly<{ root?: string }> = {},
): Effect.Effect<RepositoryInventoryReport, AssuranceToolError> =>
  Effect.sync(() => ({
    ...inventoryRepository(resolveRoot(args.root)),
    candidates_not_proof: true,
  }))
