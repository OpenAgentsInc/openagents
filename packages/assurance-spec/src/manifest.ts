import type { AssuranceAdmission } from "./admission.ts"
import { canonicalArtifact } from "./artifact.ts"
import type { AssuranceAdapterLock, AssuranceEnvironmentProfileDocument } from "./environment.ts"
import { validateAdapterLock, validateEnvironmentProfileDigest } from "./environment.ts"
import type { AssuranceSpecDocument } from "./schema.ts"
import { sha256Digest } from "./tooling.ts"

export const ASSURANCE_MANIFEST_FORMAT_VERSION = "0.1" as const
export const ASSURANCE_COMPILER_VERSION = "0.1.0" as const

export type AssuranceExecutionUnit = Readonly<{
  unit_ref: string
  role: "candidate" | "falsifier"
  obligation_id: string
  environment_ref: string
  adapter_ref: string
  argv: ReadonlyArray<string>
  artifact_slots: ReadonlyArray<string>
  expected_observation: "CONFIRMED" | "REFUTED"
}>

export type AssuranceManifest = Readonly<{
  assurance_manifest_format_version: typeof ASSURANCE_MANIFEST_FORMAT_VERSION
  do_not_edit: true
  compiler: Readonly<{ version: typeof ASSURANCE_COMPILER_VERSION; content_digest: string }>
  product_spec: Readonly<{ path: string; revision: number; document_digest: string }>
  assurance_spec: Readonly<{ id: string; revision: number; document_digest: string }>
  admission: Readonly<{ ref: string; digest: string; review_set_digest: string }>
  environment: Readonly<{ profile_id: string; revision: number; digest: string }>
  adapter_lock_digest: string
  gate_refs: ReadonlyArray<string>
  obligation_graph: ReadonlyArray<Readonly<{
    obligation_id: string
    criterion_refs: ReadonlyArray<string>
    dependency_refs: ReadonlyArray<string>
    execution_unit_refs: ReadonlyArray<string>
  }>>
  execution_units: ReadonlyArray<AssuranceExecutionUnit>
  evidence_requirements: ReadonlyArray<string>
  public_safety: Readonly<{ classification: "review_required"; raw_artifacts_public: false }>
}>

export type CompileAssuranceManifestInput = Readonly<{
  assuranceSpec: AssuranceSpecDocument
  assuranceSpecBytes: string
  productSpecBytes: string
  admission: AssuranceAdmission
  admissionBytes: string
  environment: AssuranceEnvironmentProfileDocument
  adapterLock: AssuranceAdapterLock
  adapterLockBytes: string
  compilerContentDigest: string
  executionUnits: ReadonlyArray<AssuranceExecutionUnit>
}>

export class AssuranceCompileError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.name = "AssuranceCompileError"
    this.code = code
  }
}

const fail = (code: string, message: string): never => {
  throw new AssuranceCompileError(code, message)
}

const exact = (actual: unknown, expected: unknown, code: string, label: string): void => {
  if (actual !== expected) fail(code, `${label} does not match the admitted exact input.`)
}

export const compileAssuranceManifest = (
  input: CompileAssuranceManifestInput,
): Readonly<{ manifest: AssuranceManifest; bytes: string; digest: `sha256:${string}` }> => {
  const spec = input.assuranceSpec
  const specDigest = sha256Digest(input.assuranceSpecBytes)
  const productDigest = sha256Digest(input.productSpecBytes)
  const admissionDigest = sha256Digest(input.admissionBytes)
  const adapterLockDigest = sha256Digest(input.adapterLockBytes)
  const subject = spec.subject.product_spec

  exact(spec.frontmatter.lifecycle_state, "admitted", "assurance_spec_not_admitted", "AssuranceSpec lifecycle")
  exact(input.admission.assurance_spec.id, spec.frontmatter.assurance_spec_id, "admission_spec_mismatch", "AssuranceSpec id")
  exact(input.admission.assurance_spec.revision, spec.frontmatter.assurance_revision, "admission_spec_mismatch", "AssuranceSpec revision")
  exact(input.admission.assurance_spec.document_digest, specDigest, "admission_spec_mismatch", "AssuranceSpec digest")
  exact(input.admission.product_spec.path, subject.path, "admission_subject_mismatch", "ProductSpec path")
  exact(input.admission.product_spec.revision, subject.spec_revision, "admission_subject_mismatch", "ProductSpec revision")
  exact(input.admission.product_spec.document_digest, productDigest, "admission_subject_mismatch", "ProductSpec digest")
  exact(subject.document_digest, productDigest, "subject_document_digest_mismatch", "ProductSpec subject digest")
  if (!validateEnvironmentProfileDigest(input.environment)) {
    fail("environment_profile_digest_mismatch", "Environment Profile payload does not match profile_digest.")
  }
  const lockDiagnostics = validateAdapterLock(input.adapterLock)
  if (lockDiagnostics.length > 0) fail("invalid_adapter_lock", lockDiagnostics.join(", "))
  for (const gate of input.admission.allowed_gate_refs) {
    if (!spec.gates.some((candidate) => candidate.id === gate)) {
      fail("admission_gate_mismatch", `Admission names unknown gate ${gate}.`)
    }
  }

  const unitByObligation = new Map<string, AssuranceExecutionUnit[]>()
  const adapterRefs = new Set(input.adapterLock.adapters.map((adapter) => adapter.adapter_ref))
  for (const unit of input.executionUnits) {
    const obligation = spec.obligations.find((candidate) => candidate.id === unit.obligation_id)
    if (obligation === undefined) throw new AssuranceCompileError(
      "manifest_unknown_obligation",
      `Execution unit ${unit.unit_ref} names an unknown obligation.`,
    )
    if (
      obligation.oracle === undefined || obligation.falsifier === undefined ||
      obligation.evidence === undefined || obligation.independence === undefined ||
      obligation.environment_refs?.includes(input.environment.profile_id) !== true
    ) {
      fail("manifest_obligation_needs_design", `Execution unit ${unit.unit_ref} names an undesigned obligation.`)
    }
    if (unit.environment_ref !== input.environment.profile_id) {
      fail("manifest_environment_mismatch", `Execution unit ${unit.unit_ref} is bound to another environment.`)
    }
    if (!adapterRefs.has(unit.adapter_ref)) {
      fail("manifest_adapter_unlocked", `Execution unit ${unit.unit_ref} names an unlocked adapter.`)
    }
    if (unit.argv.length === 0 || unit.argv.some((argument) => argument.startsWith("/"))) {
      fail("manifest_unsafe_argv", `Execution unit ${unit.unit_ref} must use repository-relative explicit argv.`)
    }
    unitByObligation.set(unit.obligation_id, [...(unitByObligation.get(unit.obligation_id) ?? []), unit])
  }

  const manifest: AssuranceManifest = {
    assurance_manifest_format_version: ASSURANCE_MANIFEST_FORMAT_VERSION,
    do_not_edit: true,
    compiler: { version: ASSURANCE_COMPILER_VERSION, content_digest: input.compilerContentDigest },
    product_spec: { path: subject.path, revision: subject.spec_revision, document_digest: productDigest },
    assurance_spec: {
      id: spec.frontmatter.assurance_spec_id,
      revision: spec.frontmatter.assurance_revision,
      document_digest: specDigest,
    },
    admission: {
      ref: input.admission.admission_ref,
      digest: admissionDigest,
      review_set_digest: input.admission.review_set_digest,
    },
    environment: {
      profile_id: input.environment.profile_id,
      revision: input.environment.revision,
      digest: input.environment.profile_digest,
    },
    adapter_lock_digest: adapterLockDigest,
    gate_refs: [...input.admission.allowed_gate_refs].sort(),
    obligation_graph: spec.obligations.map((obligation) => ({
      obligation_id: obligation.id,
      criterion_refs: [...obligation.criterion_refs],
      dependency_refs: [...(obligation.dependency_refs ?? [])],
      execution_unit_refs: (unitByObligation.get(obligation.id) ?? []).map((unit) => unit.unit_ref),
    })),
    execution_units: [...input.executionUnits],
    evidence_requirements: ["native_report", "normalized_receipt", "oracle_sensitivity_receipt"],
    public_safety: { classification: "review_required", raw_artifacts_public: false },
  }
  const artifact = canonicalArtifact(manifest)
  return { manifest, bytes: artifact.bytes, digest: artifact.digest }
}
