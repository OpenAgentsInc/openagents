import { createHash } from "node:crypto"
import { readdirSync, readFileSync } from "node:fs"
import type { Dirent } from "node:fs"
import { relative, resolve, sep } from "node:path"

import {
  assessAssuranceSpec,
  decodeAssuranceEvidenceIndex,
  sha256Digest,
  validateAssuranceSpec,
  type AssuranceEvidenceIndex,
} from "@openagentsinc/assurance-spec"
import { validateExecutableProductSpec } from "@openagentsinc/product-spec"

export const SPEC_LANE_MAX_FILES = 32
export const SPEC_LANE_MAX_FILE_BYTES = 512_000
export const SPEC_LANE_MAX_OBLIGATIONS = 128
export const SPEC_LANE_MAX_PROMPT_CHARS = 8_000

type ObligationState = "unmet" | "confirmed"

export type SpecLaneObligation = Readonly<{
  assuranceSpecPath: string
  obligationId: string
  title: string
  criterionRefs: ReadonlyArray<string>
  state: ObligationState
  reason: string
}>

export type SpecLaneSnapshot = Readonly<{
  productSpecs: ReadonlyArray<Readonly<{
    path: string
    title: string
    revision: number
    criteria: ReadonlyArray<Readonly<{ id: string; body: string }>>
  }>>
  assuranceSpecs: ReadonlyArray<Readonly<{
    path: string
    assuranceSpecId: string
    revision: number
    lifecycleState: string
  }>>
  obligations: ReadonlyArray<SpecLaneObligation>
  diagnostics: ReadonlyArray<string>
  truncated: boolean
}>

export type SpecLaneTurnProjection = Readonly<{
  snapshot: SpecLaneSnapshot
  promptContext: string
}>

const clipped = (value: string, limit: number): string =>
  value.length <= limit ? value : `${value.slice(0, Math.max(0, limit - 1))}…`

const boundedText = (value: string, limit: number): string =>
  clipped(value.replace(/[\u0000-\u001f\u007f]+/g, " ").replace(/\s+/g, " ").trim(), limit)

const normalizedRelative = (root: string, path: string): string | null => {
  const projected = relative(root, path)
  return projected === "" || projected === ".." || projected.startsWith(`..${sep}`)
    ? null
    : projected.split(sep).join("/")
}

const inventory = (workspaceRoot: string): Readonly<{ paths: ReadonlyArray<string>; truncated: boolean }> => {
  const root = resolve(workspaceRoot)
  const specsRoot = resolve(root, "specs")
  const pending = [specsRoot]
  const paths: string[] = []
  let truncated = false
  while (pending.length > 0 && paths.length < SPEC_LANE_MAX_FILES) {
    const directory = pending.shift()!
    let entries: Dirent<string>[]
    try {
      entries = readdirSync(directory, { withFileTypes: true, encoding: "utf8" })
    } catch {
      continue
    }
    for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
      const absolute = resolve(directory, entry.name)
      if (normalizedRelative(specsRoot, absolute) === null) continue
      if (entry.isDirectory()) pending.push(absolute)
      else if (entry.isFile() && (
        entry.name.endsWith(".product-spec.md") ||
        entry.name.endsWith(".assurance-spec.md") ||
        entry.name.endsWith(".assurance-evidence-index.json")
      )) paths.push(absolute)
      if (paths.length === SPEC_LANE_MAX_FILES) {
        truncated = pending.length > 0 || entries.indexOf(entry) < entries.length - 1
        break
      }
    }
  }
  return { paths, truncated }
}

type BoundedRead =
  | Readonly<{ ok: true; source: string }>
  | Readonly<{ ok: false; reason: "oversized" | "unreadable" }>

const readBounded = (path: string): BoundedRead => {
  try {
    const bytes = readFileSync(path)
    return bytes.length <= SPEC_LANE_MAX_FILE_BYTES
      ? { ok: true, source: bytes.toString("utf8") }
      : { ok: false, reason: "oversized" }
  } catch {
    return { ok: false, reason: "unreadable" }
  }
}

const evidenceConfirms = (
  evidence: AssuranceEvidenceIndex | undefined,
  obligation: Readonly<{ id: string; criterion_refs: ReadonlyArray<string> }>,
): boolean => {
  const receipt = evidence?.receipts.find(candidate => candidate.obligation_id === obligation.id)
  const axes = receipt?.axes
  return evidence?.gate.admitted === true &&
    evidence.gate.executable === true &&
    receipt?.criterion_refs.length === obligation.criterion_refs.length &&
    receipt.criterion_refs.every((ref, index) => ref === obligation.criterion_refs[index]) &&
    axes?.admission === "admitted" &&
    axes.readiness === "executable" &&
    axes.observation === "CONFIRMED" &&
    axes.infrastructure === "ready" &&
    axes.stability === "stable" &&
    axes.freshness === "current" &&
    axes.disposition === "accepted" &&
    axes.exception === "none"
}

/**
 * Projects spec intent without granting a provider any spec authority. Every
 * semantic fact comes from the ProductSpec/AssuranceSpec packages; this host
 * only bounds and formats their output for a turn.
 */
export const projectSpecLaneTurn = (workspaceRoot: string): SpecLaneTurnProjection => {
  const root = resolve(workspaceRoot)
  const discovered = inventory(root)
  const diagnostics: string[] = []
  const productSpecs: SpecLaneSnapshot["productSpecs"][number][] = []
  let criteriaTruncated = false
  let oversizedFileTruncated = false
  const assuranceSources: Array<Readonly<{ path: string; relativePath: string; source: string }>> = []
  const evidenceByDigest = new Map<string, Readonly<{
    evidence: AssuranceEvidenceIndex
    relativePath: string
  }>>()

  for (const path of discovered.paths) {
    const relativePath = normalizedRelative(root, path)
    if (relativePath === null) continue
    const read = readBounded(path)
    if (!read.ok) {
      if (read.reason === "oversized") {
        oversizedFileTruncated = true
        diagnostics.push(`${relativePath}: exceeds ${SPEC_LANE_MAX_FILE_BYTES} bytes`)
      } else {
        diagnostics.push(`${relativePath}: unreadable`)
      }
      continue
    }
    const source = read.source
    if (path.endsWith(".product-spec.md")) {
      const result = validateExecutableProductSpec(source)
      if (!result.executable) {
        diagnostics.push(`${relativePath}: ProductSpec is not executable (${result.errors.length} errors)`)
        continue
      }
      if (result.criteria.length > 64) criteriaTruncated = true
      productSpecs.push({
        path: relativePath,
        title: boundedText(result.document.frontmatter.title, 160),
        revision: result.document.frontmatter.spec_revision!,
        criteria: result.criteria.slice(0, 64).map(criterion => ({
          id: boundedText(criterion.id, 120),
          body: boundedText(criterion.body, 320),
        })),
      })
    } else if (path.endsWith(".assurance-spec.md")) {
      assuranceSources.push({ path, relativePath, source })
    } else {
      try {
        const evidence = decodeAssuranceEvidenceIndex(JSON.parse(source))
        evidenceByDigest.set(evidence.subject.assurance_spec_digest, { evidence, relativePath })
      } catch {
        diagnostics.push(`${relativePath}: evidence index is not schema-valid`)
      }
    }
  }

  const assuranceSpecs: SpecLaneSnapshot["assuranceSpecs"][number][] = []
  const obligations: SpecLaneObligation[] = []
  let obligationTruncated = false
  for (const candidate of assuranceSources) {
    const validation = validateAssuranceSpec(candidate.source)
    if (!validation.valid || validation.document === undefined) {
      diagnostics.push(`${candidate.relativePath}: AssuranceSpec is invalid (${validation.errors.length} errors)`)
      continue
    }
    const document = validation.document
    const assessment = assessAssuranceSpec(document)
    const digest = sha256Digest(candidate.source)
    const evidenceCandidate = evidenceByDigest.get(digest)
    const evidence = evidenceCandidate?.evidence.subject.product_spec_digest ===
      document.subject.product_spec.document_digest
      ? evidenceCandidate.evidence
      : undefined
    if (evidenceCandidate !== undefined && evidence === undefined) {
      diagnostics.push(`${evidenceCandidate.relativePath}: evidence index does not bind the exact ProductSpec digest`)
    }
    assuranceSpecs.push({
      path: candidate.relativePath,
      assuranceSpecId: boundedText(document.frontmatter.assurance_spec_id, 160),
      revision: document.frontmatter.assurance_revision,
      lifecycleState: document.frontmatter.lifecycle_state,
    })
    for (const obligation of document.obligations) {
      if (obligations.length >= SPEC_LANE_MAX_OBLIGATIONS) {
        obligationTruncated = true
        break
      }
      const confirmation = evidenceConfirms(evidence, obligation)
      const diagnostic = assessment.diagnostics.find(entry => entry.obligation_id === obligation.id)
      obligations.push({
        assuranceSpecPath: candidate.relativePath,
        obligationId: boundedText(obligation.id, 160),
        title: boundedText(obligation.title, 240),
        criterionRefs: obligation.criterion_refs.slice(0, 12).map(ref => boundedText(ref, 200)),
        state: confirmation ? "confirmed" : "unmet",
        reason: confirmation
          ? "qualifying schema-valid evidence receipt is confirmed"
          : diagnostic?.code ?? (evidence === undefined ? "no schema-valid evidence index" : "no qualifying confirmed receipt"),
      })
    }
  }

  const snapshot: SpecLaneSnapshot = {
    productSpecs,
    assuranceSpecs,
    obligations,
    diagnostics: diagnostics.slice(0, 24).map(value => clipped(value, 300)),
    truncated: discovered.truncated || oversizedFileTruncated || obligationTruncated || criteriaTruncated,
  }
  const unmet = obligations.filter(obligation => obligation.state === "unmet")
  if (productSpecs.length === 0 && assuranceSpecs.length === 0 && diagnostics.length === 0) {
    return { snapshot, promptContext: "" }
  }
  const lines = [
    "SPEC WORK CONTEXT (bounded, read-only projection; providers do not own spec validation or verdicts):",
    ...productSpecs.flatMap(spec => [
      `ProductSpec ${spec.path} rev ${spec.revision}: ${spec.title}`,
      ...spec.criteria.slice(0, 12).map(criterion => `  ${criterion.id}: ${criterion.body}`),
      ...(spec.criteria.length > 12 ? [`  … ${spec.criteria.length - 12} more criteria omitted by the prompt bound.`] : []),
    ]),
    ...assuranceSpecs.map(spec =>
      `AssuranceSpec ${spec.path} rev ${spec.revision} (${spec.lifecycleState}): ${spec.assuranceSpecId}`),
    ...unmet.slice(0, 12).map(obligation =>
      `UNMET ${obligation.obligationId} in ${obligation.assuranceSpecPath}: ${obligation.title} [${obligation.reason}]`),
    ...(unmet.length > 12 ? [`… ${unmet.length - 12} more unmet obligations omitted by the prompt bound.`] : []),
    ...(snapshot.truncated ? ["The spec projection was truncated; inspect authoritative files before acting."] : []),
  ]
  const authority = "Treat unmet obligations as candidate work, not permission to alter acceptance, admission, verification, release, or public claims."
  const body = lines.join("\n")
  const availableBody = SPEC_LANE_MAX_PROMPT_CHARS - authority.length - 1
  const promptTruncated = body.length > availableBody
  const truncation = "\nThe prompt projection was truncated; inspect authoritative files before acting."
  const bodyLimit = promptTruncated ? availableBody - truncation.length : availableBody
  return {
    snapshot,
    promptContext: `${clipped(body, bodyLimit)}${promptTruncated ? truncation : ""}\n${authority}`,
  }
}

export const appendSpecLaneContext = (message: string, projection: SpecLaneTurnProjection): string =>
  projection.promptContext === "" ? message : `${projection.promptContext}\n\nOWNER TURN INSTRUCTION:\n${message}`

const obligationKey = (obligation: SpecLaneObligation): string =>
  `${obligation.assuranceSpecPath}#${obligation.obligationId}`

/** A bounded owner-visible report after the exact same authority packages
 * re-read the workspace. It reports deltas and never promotes them to verdicts. */
export const specLaneRevalidationNote = (
  laneRef: string,
  before: SpecLaneSnapshot,
  after: SpecLaneSnapshot,
): string | null => {
  if (
    before.assuranceSpecs.length === 0 && after.assuranceSpecs.length === 0 &&
    before.diagnostics.length === 0 && after.diagnostics.length === 0
  ) return null
  const beforeByKey = new Map(before.obligations.map(item => [obligationKey(item), item]))
  const changed = after.obligations.filter(item => beforeByKey.get(obligationKey(item))?.state !== item.state)
  const removed = before.obligations.filter(item =>
    !after.obligations.some(candidate => obligationKey(candidate) === obligationKey(item)))
  const unmet = after.obligations.filter(item => item.state === "unmet")
  const details = [
    ...changed.slice(0, 8).map(item => `${item.obligationId} → ${item.state}`),
    ...removed.slice(0, 4).map(item => `${item.obligationId} → no longer present`),
  ]
  return clipped([
    `Spec revalidation · ${laneRef}: AssuranceSpec authority re-read ${after.assuranceSpecs.length} document(s); ${unmet.length}/${after.obligations.length} obligations remain unmet.`,
    details.length === 0 ? "No obligation state changed during this turn." : `Changed: ${details.join(", ")}.`,
    after.diagnostics.length === 0
      ? "Structural validation passed for every projected AssuranceSpec/evidence index."
      : `Diagnostics: ${after.diagnostics.slice(0, 4).join("; ")}.`,
    "This note is evidence status only; it does not admit, verify, release, or change public claims.",
  ].join(" "), 2_000)
}

export const specLaneSnapshotDigest = (snapshot: SpecLaneSnapshot): string =>
  `sha256:${createHash("sha256").update(JSON.stringify(snapshot)).digest("hex")}`
