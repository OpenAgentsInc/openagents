import { mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs"
import { dirname, resolve, sep } from "node:path"

import { Schema as S } from "effect"

import { canonicalArtifact } from "./artifact.ts"
import { executeBunTestUnit, type BunTestAdapterResult } from "./bun-test-adapter.ts"
import type { AssuranceEnvironmentProfileDocument } from "./environment.ts"
import type { AssuranceExecutionUnit, AssuranceManifest } from "./manifest.ts"
import { ASSURANCE_RECEIPT_FORMAT_VERSION, makeOracleSensitivityReceipt, type AssuranceReceipt, type OracleSensitivityMutantResult, type OracleSensitivityReceipt } from "./receipt.ts"
import { Digest, NonEmptyString, RelativePath, StableRef } from "./schema.ts"
import { sha256Digest } from "./tooling.ts"

export const OPENAGENTS_MUTATION_ADAPTER_REF = "openagents.mutation.v1" as const
export const OPENAGENTS_MUTATION_ADAPTER_VERSION = "1.0.0" as const
export const OPENAGENTS_MUTATION_MAX_MUTANTS = 16 as const
export const OPENAGENTS_MUTATION_MAX_SOURCE_BYTES = 1024 * 1024
export const OPENAGENTS_MUTATION_MAX_REPLACEMENT_BYTES = 4096

const lexical = (left: string, right: string): number => left < right ? -1 : left > right ? 1 : 0

const mutationSetDigestForDecodedPlan = (plan: MutationPlan): string => canonicalArtifact({
  adapter_ref: OPENAGENTS_MUTATION_ADAPTER_REF,
  adapter_version: OPENAGENTS_MUTATION_ADAPTER_VERSION,
  obligation_id: plan.obligation_id,
  oracle_ref: plan.oracle_ref,
  oracle_unit_ref: plan.oracle_unit_ref,
  subject_relative_path: plan.subject_relative_path,
  subject_source_digest: plan.subject_source_digest,
  mutations: [...plan.mutations].sort((left, right) => lexical(left.mutant_ref, right.mutant_ref)),
}).digest

export const MutationDefinitionSchema = S.Struct({
  mutant_ref: StableRef,
  operator: S.Literal("replace_exact"),
  target: NonEmptyString,
  replacement: S.String,
})
export type MutationDefinition = typeof MutationDefinitionSchema.Type

export const MutationPlanSchema = S.Struct({
  mutation_plan_format_version: S.Literal("0.1"),
  adapter_ref: S.Literal(OPENAGENTS_MUTATION_ADAPTER_REF),
  obligation_id: StableRef,
  oracle_ref: StableRef,
  oracle_unit_ref: StableRef,
  subject_relative_path: RelativePath,
  subject_source_digest: Digest,
  mutations: S.Array(MutationDefinitionSchema),
})
export type MutationPlan = typeof MutationPlanSchema.Type

export class MutationAdapterError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.name = "MutationAdapterError"
    this.code = code
  }
}

export type MutationAdapterResult = Readonly<{
  plan: MutationPlan
  mutationSetDigest: string
  candidateReceipt: AssuranceReceipt
  mutantReceipts: ReadonlyArray<AssuranceReceipt>
  candidateReceiptArtifact: MutationNormalizedReceipt
  mutantReceiptArtifacts: ReadonlyArray<MutationNormalizedReceipt>
  sensitivityReceipt: OracleSensitivityReceipt
  sensitivityReceiptBytes: string
  sensitivityReceiptDigest: string
}>

export type MutationNormalizedReceipt = Readonly<{
  receipt: AssuranceReceipt
  receiptBytes: string
  receiptDigest: string
  receiptPath: string
}>

const fail = (code: string, message: string): never => {
  throw new MutationAdapterError(code, message)
}

const exactOccurrenceCount = (source: string, target: string): number => {
  let count = 0
  let offset = 0
  while (offset <= source.length - target.length) {
    const found = source.indexOf(target, offset)
    if (found < 0) break
    count += 1
    offset = found + target.length
  }
  return count
}

export const decodeMutationPlan = (candidate: unknown): MutationPlan => {
  const plan = S.decodeUnknownSync(MutationPlanSchema)(candidate, {
    onExcessProperty: "error",
  })
  if (plan.mutations.length === 0 || plan.mutations.length > OPENAGENTS_MUTATION_MAX_MUTANTS) {
    fail("mutation_set_out_of_bounds", `Mutation plans require 1-${OPENAGENTS_MUTATION_MAX_MUTANTS} deterministic mutants.`)
  }
  if (new Set(plan.mutations.map((mutation) => mutation.mutant_ref)).size !== plan.mutations.length) {
    fail("duplicate_mutant_ref", "Mutation refs must be unique within one bounded plan.")
  }
  for (const mutation of plan.mutations) {
    if (mutation.target === mutation.replacement) {
      fail("mutation_is_noop", `Mutation ${mutation.mutant_ref} does not change source bytes.`)
    }
    if (
      Buffer.byteLength(mutation.target) > OPENAGENTS_MUTATION_MAX_REPLACEMENT_BYTES ||
      Buffer.byteLength(mutation.replacement) > OPENAGENTS_MUTATION_MAX_REPLACEMENT_BYTES
    ) {
      fail("mutation_bytes_out_of_bounds", "Mutation targets and replacements are bounded to 4096 bytes each.")
    }
  }
  return plan
}

export const mutationSetDigestForPlan = (candidate: unknown): string =>
  mutationSetDigestForDecodedPlan(decodeMutationPlan(candidate))

const normalizedMutationReceipt = (
  run: BunTestAdapterResult,
  input: Readonly<{
    manifestDigest: string
    mutationUnitRef: string
    nativeReportRef: string
    sourceDigest: string
    mutationSetDigest: string
    receiptPath: string
  }>,
): MutationNormalizedReceipt => {
  const seed = {
    manifest_digest: input.manifestDigest,
    execution_unit_ref: input.mutationUnitRef,
    native_report_digest: run.receipt.native_report_digest,
    mutation_set_digest: input.mutationSetDigest,
    source_digest: input.sourceDigest,
  }
  const receipt: AssuranceReceipt = {
    ...run.receipt,
    assurance_receipt_format_version: ASSURANCE_RECEIPT_FORMAT_VERSION,
    receipt_ref: `assurance.receipt.${sha256Digest(JSON.stringify(seed)).slice("sha256:".length)}`,
    adapter_ref: OPENAGENTS_MUTATION_ADAPTER_REF,
    execution_unit_ref: input.mutationUnitRef,
    native_report_ref: input.nativeReportRef,
    source_digest: input.sourceDigest,
  }
  const artifact = canonicalArtifact(receipt)
  writeFileSync(input.receiptPath, artifact.bytes, "utf8")
  return {
    receipt,
    receiptBytes: artifact.bytes,
    receiptDigest: artifact.digest,
    receiptPath: input.receiptPath,
  }
}

const mutationResult = (mutantRef: string, receipt: AssuranceReceipt): OracleSensitivityMutantResult => ({
  mutant_ref: mutantRef,
  operator: "replace_exact",
  observation: receipt.axes.observation === "REFUTED" ? "killed" : receipt.axes.observation === "CONFIRMED" ? "survived" : "inconclusive",
  assurance_receipt_ref: receipt.receipt_ref,
})

export const executeMutationPlan = (
  input: Readonly<{
    workspaceRoot: string
    runRoot: string
    manifest: AssuranceManifest
    manifestDigest: string
    environment: AssuranceEnvironmentProfileDocument
    oracleUnit: AssuranceExecutionUnit
    plan: unknown
    producerRef: string
    reviewerRef: string
    bunExecutable?: string
  }>,
): MutationAdapterResult => {
  const plan = decodeMutationPlan(input.plan)
  if (input.environment.mutability !== "isolated_write") {
    fail("environment_not_isolated", "Mutation execution requires an isolated-write Environment Profile.")
  }
  for (const forbidden of ["network", "credentials", "production_mutation"] as const) {
    if (!input.environment.forbidden_actions.includes(forbidden)) {
      fail("environment_authority_too_broad", `Mutation environment must forbid ${forbidden}.`)
    }
  }
  if (plan.obligation_id !== input.oracleUnit.obligation_id || plan.oracle_unit_ref !== input.oracleUnit.unit_ref || input.oracleUnit.role !== "candidate" || input.oracleUnit.expected_observation !== "CONFIRMED") {
    fail("oracle_unit_binding_mismatch", "Mutation plan does not bind the admitted candidate oracle unit.")
  }
  const graphEntry = input.manifest.obligation_graph.find((entry) => entry.obligation_id === plan.obligation_id)
  const manifestUnit = input.manifest.execution_units.find((unit) => unit.unit_ref === input.oracleUnit.unit_ref)
  if (
    graphEntry === undefined ||
    !graphEntry.execution_unit_refs.includes(input.oracleUnit.unit_ref) ||
    manifestUnit === undefined ||
    canonicalArtifact(manifestUnit).digest !== canonicalArtifact(input.oracleUnit).digest ||
    input.manifest.environment.profile_id !== input.environment.profile_id ||
    input.manifest.environment.digest !== input.environment.profile_digest
  ) {
    fail("oracle_unit_not_admitted", "Oracle unit is not present in the admitted manifest graph.")
  }

  const workspaceRoot = realpathSync(resolve(input.workspaceRoot))
  const subjectPath = realpathSync(resolve(workspaceRoot, plan.subject_relative_path))
  if (!subjectPath.startsWith(`${workspaceRoot}${sep}`)) {
    fail("subject_path_escape", "Mutation subject must remain inside the isolated workspace.")
  }
  const source = readFileSync(subjectPath, "utf8")
  if (Buffer.byteLength(source) > OPENAGENTS_MUTATION_MAX_SOURCE_BYTES) {
    fail("mutation_subject_out_of_bounds", "Mutation subjects are bounded to one MiB.")
  }
  if (sha256Digest(source) !== plan.subject_source_digest) {
    fail("subject_source_digest_mismatch", "Mutation subject bytes differ from the reviewed plan.")
  }
  for (const mutation of plan.mutations) {
    if (exactOccurrenceCount(source, mutation.target) !== 1) {
      fail("mutation_target_not_exact", `Mutation ${mutation.mutant_ref} target must occur exactly once in reviewed source bytes.`)
    }
  }

  const sortedMutations = [...plan.mutations].sort((left, right) => lexical(left.mutant_ref, right.mutant_ref))
  const mutationSetDigest = mutationSetDigestForDecodedPlan(plan)
  mkdirSync(input.runRoot, { recursive: true })

  const runOracle = (label: string, sourceBytes: string): MutationNormalizedReceipt => {
    const runRoot = resolve(input.runRoot, label)
    mkdirSync(runRoot, { recursive: true })
    const run = executeBunTestUnit({
      workspaceRoot,
      runRoot,
      manifest: input.manifest,
      manifestDigest: input.manifestDigest,
      environment: input.environment,
      unit: input.oracleUnit,
      producerRef: input.producerRef,
      reviewerRef: input.reviewerRef,
      sourceDigest: sha256Digest(sourceBytes),
      ...(input.bunExecutable === undefined ? {} : { bunExecutable: input.bunExecutable }),
    })
    return normalizedMutationReceipt(run, {
      manifestDigest: input.manifestDigest,
      mutationUnitRef: `mutation.${label}`,
      nativeReportRef: `${label}/${input.oracleUnit.role}.junit.xml`,
      sourceDigest: sha256Digest(sourceBytes),
      mutationSetDigest,
      receiptPath: resolve(runRoot, "assurance-receipt.json"),
    })
  }

  let candidateReceipt: AssuranceReceipt
  let candidateReceiptArtifact: MutationNormalizedReceipt
  const mutantReceipts: AssuranceReceipt[] = []
  const mutantReceiptArtifacts: MutationNormalizedReceipt[] = []
  const results: OracleSensitivityMutantResult[] = []
  try {
    candidateReceiptArtifact = runOracle("candidate", source)
    candidateReceipt = candidateReceiptArtifact.receipt
    if (candidateReceipt.axes.observation !== "CONFIRMED") {
      fail("candidate_oracle_not_confirmed", "The unmutated subject did not confirm under the admitted oracle.")
    }
    for (const mutation of sortedMutations) {
      const mutated = source.replace(mutation.target, mutation.replacement)
      writeFileSync(subjectPath, mutated, "utf8")
      const receiptArtifact = runOracle(mutation.mutant_ref, mutated)
      const receipt = receiptArtifact.receipt
      mutantReceipts.push(receipt)
      mutantReceiptArtifacts.push(receiptArtifact)
      results.push(mutationResult(mutation.mutant_ref, receipt))
      writeFileSync(subjectPath, source, "utf8")
    }
  } finally {
    writeFileSync(subjectPath, source, "utf8")
  }

  const sensitivityReceipt = makeOracleSensitivityReceipt(candidateReceipt!, {
    oracleRef: plan.oracle_ref,
    mutationSetDigest,
    mutantResults: results,
  })
  const artifact = canonicalArtifact(sensitivityReceipt)
  const sensitivityPath = resolve(input.runRoot, "oracle-sensitivity-receipt.json")
  mkdirSync(dirname(sensitivityPath), { recursive: true })
  writeFileSync(sensitivityPath, artifact.bytes, "utf8")
  return {
    plan,
    mutationSetDigest,
    candidateReceipt: candidateReceipt!,
    mutantReceipts,
    candidateReceiptArtifact: candidateReceiptArtifact!,
    mutantReceiptArtifacts,
    sensitivityReceipt,
    sensitivityReceiptBytes: artifact.bytes,
    sensitivityReceiptDigest: artifact.digest,
  }
}
