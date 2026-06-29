import { Context, Effect, Layer } from 'effect'

import { parseJsonUnknown, stringArrayFromUnknown } from './json-boundary'
import type {
  OmniWorkroomKindRequiredArtifact,
  OmniWorkroomKindRequiredEvidence,
  OmniWorkroomKindTemplate,
  OmniWorkroomKindTemplateKind,
} from './omni-workroom-kind-templates'
import type {
  OmniAcceptedOutcomeArtifactKind,
  OmniAcceptedOutcomeCloseoutRequirementKind,
  OmniAcceptedOutcomeProofPolicy,
  OmniAcceptedOutcomeReviewPolicy,
  OmniAcceptedOutcomeWorkKind,
} from './omni-accepted-outcome-contracts'
import type { OmniEvidenceEntryKind } from './omni-evidence-bundles'
import {
  WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY,
  type WorkroomTemplatePackageAuthorityBoundary,
  type WorkroomTemplatePackageState,
} from './workroom-template-packages'

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export class WorkroomTemplateRepositoryError extends Error {
  readonly _tag = 'WorkroomTemplateRepositoryError'

  constructor(
    readonly operation: string,
    readonly reason: unknown,
  ) {
    super(
      `Workroom template repository operation "${operation}" failed: ${String(
        reason,
      )}`,
    )
    this.name = 'WorkroomTemplateRepositoryError'
  }
}

const repositoryErrorFromUnknown = (
  operation: string,
  error: unknown,
): WorkroomTemplateRepositoryError =>
  error instanceof WorkroomTemplateRepositoryError
    ? error
    : new WorkroomTemplateRepositoryError(operation, error)

// ---------------------------------------------------------------------------
// Plain record shapes returned/accepted by the repository
//
// These mirror the type-only definitions in
// omni-workroom-kind-templates.ts and workroom-template-packages.ts, plus the
// created/updated timestamps the DB carries.
// ---------------------------------------------------------------------------

export type WorkroomKindTemplateRow = Readonly<{
  accepted_outcome_work_kind: string
  closeout_requirements_json: string
  created_at: string
  description_ref: string
  kind: string
  privacy_constraint: string
  proof_policy: string
  public_projection_policy: string
  required_artifacts_json: string
  required_evidence_json: string
  review_policy: string
  updated_at: string
}>

export type WorkroomKindTemplateRecord = OmniWorkroomKindTemplate &
  Readonly<{ createdAtIso: string; updatedAtIso: string }>

export type WorkroomTemplatePackageStoredVersion = Readonly<{
  approvalPolicyRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  createdAtIso: string
  evidenceRequirementRefs: ReadonlyArray<string>
  id: string
  outcomeTemplateRefs: ReadonlyArray<string>
  packageId: string
  proofRuleRefs: ReadonlyArray<string>
  requiredArtifactRefs: ReadonlyArray<string>
  runnerNeedRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  templateVersionRef: string
  uiBindingRefs: ReadonlyArray<string>
  updatedAtIso: string
}>

export type WorkroomTemplatePackageStoredRecord = Readonly<{
  approvalPolicyRefs: ReadonlyArray<string>
  authority: Readonly<{
    authorityBoundary: WorkroomTemplatePackageAuthorityBoundary
    noDeployment: boolean
    noExternalRunnerLaunch: boolean
    noMarketplaceListing: boolean
    noPaymentMutation: boolean
    noRuntimePromotion: boolean
  }>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  createdAtIso: string
  displayName: string
  evidenceRequirementRefs: ReadonlyArray<string>
  id: string
  operatorDiagnosticRefs: ReadonlyArray<string>
  orgPrivateEnablementRefs: ReadonlyArray<string>
  outcomeTemplateRefs: ReadonlyArray<string>
  packageRef: string
  proofRuleRefs: ReadonlyArray<string>
  promotionRefs: ReadonlyArray<string>
  publicProjectionRefs: ReadonlyArray<string>
  requiredArtifactRefs: ReadonlyArray<string>
  reviewRefs: ReadonlyArray<string>
  runnerNeedRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  state: WorkroomTemplatePackageState
  templateVersionRefs: ReadonlyArray<string>
  uiBindingRefs: ReadonlyArray<string>
  updatedAtIso: string
  validationRefs: ReadonlyArray<string>
  versionRef: string
}>

// ---------------------------------------------------------------------------
// Row -> record decoders
// ---------------------------------------------------------------------------

const parseRequiredEvidence = (
  json: string,
): ReadonlyArray<OmniWorkroomKindRequiredEvidence> => {
  const parsed = parseJsonUnknown(json)

  return Array.isArray(parsed)
    ? parsed.map(item => ({
        entryKind: String(item.entryKind) as OmniEvidenceEntryKind,
        publicSafeAllowed: Boolean(item.publicSafeAllowed),
        required: Boolean(item.required),
      }))
    : []
}

const parseRequiredArtifacts = (
  json: string,
): ReadonlyArray<OmniWorkroomKindRequiredArtifact> => {
  const parsed = parseJsonUnknown(json)

  return Array.isArray(parsed)
    ? parsed.map(item => ({
        artifactKind: String(item.artifactKind) as OmniAcceptedOutcomeArtifactKind,
        publicSafeAllowed: Boolean(item.publicSafeAllowed),
        required: Boolean(item.required),
      }))
    : []
}

export const toWorkroomKindTemplateRecord = (
  row: WorkroomKindTemplateRow,
): WorkroomKindTemplateRecord => ({
  acceptedOutcomeWorkKind:
    row.accepted_outcome_work_kind as OmniAcceptedOutcomeWorkKind,
  closeoutRequirements: stringArrayFromUnknown(
    parseJsonUnknown(row.closeout_requirements_json),
  ) as ReadonlyArray<OmniAcceptedOutcomeCloseoutRequirementKind>,
  createdAtIso: row.created_at,
  descriptionRef: row.description_ref,
  kind: row.kind as OmniWorkroomKindTemplateKind,
  privacyConstraint:
    row.privacy_constraint as OmniWorkroomKindTemplate['privacyConstraint'],
  proofPolicy: row.proof_policy as OmniAcceptedOutcomeProofPolicy,
  publicProjectionPolicy:
    row.public_projection_policy as OmniWorkroomKindTemplate['publicProjectionPolicy'],
  requiredArtifacts: parseRequiredArtifacts(row.required_artifacts_json),
  requiredEvidence: parseRequiredEvidence(row.required_evidence_json),
  reviewPolicy: row.review_policy as OmniAcceptedOutcomeReviewPolicy,
  updatedAtIso: row.updated_at,
})

type PackageRow = Readonly<{
  approval_policy_refs_json: string
  authority_boundary: string
  blocker_refs_json: string
  caveat_refs_json: string
  created_at: string
  display_name: string
  evidence_requirement_refs_json: string
  id: string
  no_deployment: number
  no_external_runner_launch: number
  no_marketplace_listing: number
  no_payment_mutation: number
  no_runtime_promotion: number
  operator_diagnostic_refs_json: string
  org_private_enablement_refs_json: string
  outcome_template_refs_json: string
  package_ref: string
  proof_rule_refs_json: string
  promotion_refs_json: string
  public_projection_refs_json: string
  required_artifact_refs_json: string
  review_refs_json: string
  runner_need_refs_json: string
  source_refs_json: string
  state: string
  template_version_refs_json: string
  ui_binding_refs_json: string
  updated_at: string
  validation_refs_json: string
  version_ref: string
}>

type PackageVersionRow = Readonly<{
  approval_policy_refs_json: string
  caveat_refs_json: string
  created_at: string
  evidence_requirement_refs_json: string
  id: string
  outcome_template_refs_json: string
  package_id: string
  proof_rule_refs_json: string
  required_artifact_refs_json: string
  runner_need_refs_json: string
  source_refs_json: string
  template_version_ref: string
  ui_binding_refs_json: string
  updated_at: string
}>

const refs = (json: string): ReadonlyArray<string> =>
  stringArrayFromUnknown(parseJsonUnknown(json))

const toPackageRecord = (
  row: PackageRow,
): WorkroomTemplatePackageStoredRecord => ({
  approvalPolicyRefs: refs(row.approval_policy_refs_json),
  authority: {
    authorityBoundary:
      row.authority_boundary as WorkroomTemplatePackageAuthorityBoundary,
    noDeployment: row.no_deployment === 1,
    noExternalRunnerLaunch: row.no_external_runner_launch === 1,
    noMarketplaceListing: row.no_marketplace_listing === 1,
    noPaymentMutation: row.no_payment_mutation === 1,
    noRuntimePromotion: row.no_runtime_promotion === 1,
  },
  blockerRefs: refs(row.blocker_refs_json),
  caveatRefs: refs(row.caveat_refs_json),
  createdAtIso: row.created_at,
  displayName: row.display_name,
  evidenceRequirementRefs: refs(row.evidence_requirement_refs_json),
  id: row.id,
  operatorDiagnosticRefs: refs(row.operator_diagnostic_refs_json),
  orgPrivateEnablementRefs: refs(row.org_private_enablement_refs_json),
  outcomeTemplateRefs: refs(row.outcome_template_refs_json),
  packageRef: row.package_ref,
  proofRuleRefs: refs(row.proof_rule_refs_json),
  promotionRefs: refs(row.promotion_refs_json),
  publicProjectionRefs: refs(row.public_projection_refs_json),
  requiredArtifactRefs: refs(row.required_artifact_refs_json),
  reviewRefs: refs(row.review_refs_json),
  runnerNeedRefs: refs(row.runner_need_refs_json),
  sourceRefs: refs(row.source_refs_json),
  state: row.state as WorkroomTemplatePackageState,
  templateVersionRefs: refs(row.template_version_refs_json),
  uiBindingRefs: refs(row.ui_binding_refs_json),
  updatedAtIso: row.updated_at,
  validationRefs: refs(row.validation_refs_json),
  versionRef: row.version_ref,
})

const toPackageVersionRecord = (
  row: PackageVersionRow,
): WorkroomTemplatePackageStoredVersion => ({
  approvalPolicyRefs: refs(row.approval_policy_refs_json),
  caveatRefs: refs(row.caveat_refs_json),
  createdAtIso: row.created_at,
  evidenceRequirementRefs: refs(row.evidence_requirement_refs_json),
  id: row.id,
  outcomeTemplateRefs: refs(row.outcome_template_refs_json),
  packageId: row.package_id,
  proofRuleRefs: refs(row.proof_rule_refs_json),
  requiredArtifactRefs: refs(row.required_artifact_refs_json),
  runnerNeedRefs: refs(row.runner_need_refs_json),
  sourceRefs: refs(row.source_refs_json),
  templateVersionRef: row.template_version_ref,
  uiBindingRefs: refs(row.ui_binding_refs_json),
  updatedAtIso: row.updated_at,
})

const json = (value: ReadonlyArray<unknown>): string => JSON.stringify(value)

// ---------------------------------------------------------------------------
// Repository shape (promise-based; mirrors provider-account-repository.ts)
// ---------------------------------------------------------------------------

export type WorkroomTemplateRepository = Readonly<{
  upsertKindTemplate: (
    template: OmniWorkroomKindTemplate,
    now: string,
  ) => Promise<WorkroomKindTemplateRecord>
  findKindTemplate: (
    kind: OmniWorkroomKindTemplateKind,
  ) => Promise<WorkroomKindTemplateRecord | undefined>
  listKindTemplates: () => Promise<ReadonlyArray<WorkroomKindTemplateRecord>>
  createTemplatePackage: (
    record: WorkroomTemplatePackageStoredRecord,
  ) => Promise<WorkroomTemplatePackageStoredRecord>
  findTemplatePackage: (
    packageRef: string,
  ) => Promise<WorkroomTemplatePackageStoredRecord | undefined>
  listTemplatePackages: () => Promise<
    ReadonlyArray<WorkroomTemplatePackageStoredRecord>
  >
  createTemplatePackageVersion: (
    version: WorkroomTemplatePackageStoredVersion,
  ) => Promise<WorkroomTemplatePackageStoredVersion>
  listTemplatePackageVersions: (
    packageId: string,
  ) => Promise<ReadonlyArray<WorkroomTemplatePackageStoredVersion>>
}>

export const makeD1WorkroomTemplateRepository = (
  db: D1Database,
): WorkroomTemplateRepository => ({
  upsertKindTemplate: async (template, now) => {
    await db
      .prepare(
        `INSERT INTO workroom_kind_templates
          (kind, accepted_outcome_work_kind, description_ref,
           privacy_constraint, proof_policy, public_projection_policy,
           review_policy, closeout_requirements_json, required_artifacts_json,
           required_evidence_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(kind) DO UPDATE SET
           accepted_outcome_work_kind = excluded.accepted_outcome_work_kind,
           description_ref = excluded.description_ref,
           privacy_constraint = excluded.privacy_constraint,
           proof_policy = excluded.proof_policy,
           public_projection_policy = excluded.public_projection_policy,
           review_policy = excluded.review_policy,
           closeout_requirements_json = excluded.closeout_requirements_json,
           required_artifacts_json = excluded.required_artifacts_json,
           required_evidence_json = excluded.required_evidence_json,
           updated_at = excluded.updated_at`,
      )
      .bind(
        template.kind,
        template.acceptedOutcomeWorkKind,
        template.descriptionRef,
        template.privacyConstraint,
        template.proofPolicy,
        template.publicProjectionPolicy,
        template.reviewPolicy,
        json(template.closeoutRequirements),
        json(template.requiredArtifacts),
        json(template.requiredEvidence),
        now,
        now,
      )
      .run()

    const row = await db
      .prepare(`SELECT * FROM workroom_kind_templates WHERE kind = ?`)
      .bind(template.kind)
      .first<WorkroomKindTemplateRow>()

    if (row === null) {
      throw new WorkroomTemplateRepositoryError(
        'upsert_kind_template',
        'Upserted workroom kind template could not be reloaded.',
      )
    }

    return toWorkroomKindTemplateRecord(row)
  },

  findKindTemplate: async kind => {
    const row = await db
      .prepare(`SELECT * FROM workroom_kind_templates WHERE kind = ?`)
      .bind(kind)
      .first<WorkroomKindTemplateRow>()

    return row === null ? undefined : toWorkroomKindTemplateRecord(row)
  },

  listKindTemplates: async () => {
    const rows = await db
      .prepare(
        `SELECT * FROM workroom_kind_templates ORDER BY kind ASC`,
      )
      .all<WorkroomKindTemplateRow>()

    return rows.results.map(toWorkroomKindTemplateRecord)
  },

  createTemplatePackage: async record => {
    await db
      .prepare(
        `INSERT INTO workroom_template_packages
          (id, package_ref, version_ref, display_name, state,
           authority_boundary, no_deployment, no_external_runner_launch,
           no_marketplace_listing, no_payment_mutation, no_runtime_promotion,
           approval_policy_refs_json, blocker_refs_json, caveat_refs_json,
           evidence_requirement_refs_json, operator_diagnostic_refs_json,
           org_private_enablement_refs_json, outcome_template_refs_json,
           proof_rule_refs_json, promotion_refs_json,
           public_projection_refs_json, required_artifact_refs_json,
           review_refs_json, runner_need_refs_json, source_refs_json,
           template_version_refs_json, ui_binding_refs_json,
           validation_refs_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                 ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.packageRef,
        record.versionRef,
        record.displayName,
        record.state,
        record.authority.authorityBoundary,
        record.authority.noDeployment ? 1 : 0,
        record.authority.noExternalRunnerLaunch ? 1 : 0,
        record.authority.noMarketplaceListing ? 1 : 0,
        record.authority.noPaymentMutation ? 1 : 0,
        record.authority.noRuntimePromotion ? 1 : 0,
        json(record.approvalPolicyRefs),
        json(record.blockerRefs),
        json(record.caveatRefs),
        json(record.evidenceRequirementRefs),
        json(record.operatorDiagnosticRefs),
        json(record.orgPrivateEnablementRefs),
        json(record.outcomeTemplateRefs),
        json(record.proofRuleRefs),
        json(record.promotionRefs),
        json(record.publicProjectionRefs),
        json(record.requiredArtifactRefs),
        json(record.reviewRefs),
        json(record.runnerNeedRefs),
        json(record.sourceRefs),
        json(record.templateVersionRefs),
        json(record.uiBindingRefs),
        json(record.validationRefs),
        record.createdAtIso,
        record.updatedAtIso,
      )
      .run()

    return record
  },

  findTemplatePackage: async packageRef => {
    const row = await db
      .prepare(
        `SELECT * FROM workroom_template_packages WHERE package_ref = ?`,
      )
      .bind(packageRef)
      .first<PackageRow>()

    return row === null ? undefined : toPackageRecord(row)
  },

  listTemplatePackages: async () => {
    const rows = await db
      .prepare(
        `SELECT * FROM workroom_template_packages
         ORDER BY updated_at DESC
         LIMIT 200`,
      )
      .all<PackageRow>()

    return rows.results.map(toPackageRecord)
  },

  createTemplatePackageVersion: async version => {
    await db
      .prepare(
        `INSERT INTO workroom_template_package_versions
          (id, package_id, template_version_ref, approval_policy_refs_json,
           caveat_refs_json, evidence_requirement_refs_json,
           outcome_template_refs_json, proof_rule_refs_json,
           required_artifact_refs_json, runner_need_refs_json,
           source_refs_json, ui_binding_refs_json, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        version.id,
        version.packageId,
        version.templateVersionRef,
        json(version.approvalPolicyRefs),
        json(version.caveatRefs),
        json(version.evidenceRequirementRefs),
        json(version.outcomeTemplateRefs),
        json(version.proofRuleRefs),
        json(version.requiredArtifactRefs),
        json(version.runnerNeedRefs),
        json(version.sourceRefs),
        json(version.uiBindingRefs),
        version.createdAtIso,
        version.updatedAtIso,
      )
      .run()

    return version
  },

  listTemplatePackageVersions: async packageId => {
    const rows = await db
      .prepare(
        `SELECT * FROM workroom_template_package_versions
         WHERE package_id = ?
         ORDER BY created_at DESC
         LIMIT 200`,
      )
      .bind(packageId)
      .all<PackageVersionRow>()

    return rows.results.map(toPackageVersionRecord)
  },
})

export { WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY }

// ---------------------------------------------------------------------------
// Effect service wrapper (mirrors provider-account-repository.ts)
// ---------------------------------------------------------------------------

export type WorkroomTemplateRepositoryServiceShape = Readonly<{
  upsertKindTemplate: (
    template: OmniWorkroomKindTemplate,
    now: string,
  ) => Effect.Effect<WorkroomKindTemplateRecord, WorkroomTemplateRepositoryError>
  findKindTemplate: (
    kind: OmniWorkroomKindTemplateKind,
  ) => Effect.Effect<
    WorkroomKindTemplateRecord | undefined,
    WorkroomTemplateRepositoryError
  >
  listKindTemplates: () => Effect.Effect<
    ReadonlyArray<WorkroomKindTemplateRecord>,
    WorkroomTemplateRepositoryError
  >
  createTemplatePackage: (
    record: WorkroomTemplatePackageStoredRecord,
  ) => Effect.Effect<
    WorkroomTemplatePackageStoredRecord,
    WorkroomTemplateRepositoryError
  >
  findTemplatePackage: (
    packageRef: string,
  ) => Effect.Effect<
    WorkroomTemplatePackageStoredRecord | undefined,
    WorkroomTemplateRepositoryError
  >
  listTemplatePackages: () => Effect.Effect<
    ReadonlyArray<WorkroomTemplatePackageStoredRecord>,
    WorkroomTemplateRepositoryError
  >
  createTemplatePackageVersion: (
    version: WorkroomTemplatePackageStoredVersion,
  ) => Effect.Effect<
    WorkroomTemplatePackageStoredVersion,
    WorkroomTemplateRepositoryError
  >
  listTemplatePackageVersions: (
    packageId: string,
  ) => Effect.Effect<
    ReadonlyArray<WorkroomTemplatePackageStoredVersion>,
    WorkroomTemplateRepositoryError
  >
}>

export class WorkroomTemplateRepositoryService extends Context.Service<
  WorkroomTemplateRepositoryService,
  WorkroomTemplateRepositoryServiceShape
>()('openagents/WorkroomTemplateRepositoryService') {}

const repositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, WorkroomTemplateRepositoryError> =>
  Effect.tryPromise({
    try: run,
    catch: error => repositoryErrorFromUnknown(operation, error),
  })

export const makeWorkroomTemplateRepositoryService = (
  repository: WorkroomTemplateRepository,
): WorkroomTemplateRepositoryServiceShape => ({
  upsertKindTemplate: (template, now) =>
    repositoryEffect('upsert_kind_template', () =>
      repository.upsertKindTemplate(template, now),
    ),
  findKindTemplate: kind =>
    repositoryEffect('find_kind_template', () =>
      repository.findKindTemplate(kind),
    ),
  listKindTemplates: () =>
    repositoryEffect('list_kind_templates', () =>
      repository.listKindTemplates(),
    ),
  createTemplatePackage: record =>
    repositoryEffect('create_template_package', () =>
      repository.createTemplatePackage(record),
    ),
  findTemplatePackage: packageRef =>
    repositoryEffect('find_template_package', () =>
      repository.findTemplatePackage(packageRef),
    ),
  listTemplatePackages: () =>
    repositoryEffect('list_template_packages', () =>
      repository.listTemplatePackages(),
    ),
  createTemplatePackageVersion: version =>
    repositoryEffect('create_template_package_version', () =>
      repository.createTemplatePackageVersion(version),
    ),
  listTemplatePackageVersions: packageId =>
    repositoryEffect('list_template_package_versions', () =>
      repository.listTemplatePackageVersions(packageId),
    ),
})

export const makeWorkroomTemplateRepositoryLayer = (
  repository: WorkroomTemplateRepository,
) =>
  Layer.succeed(
    WorkroomTemplateRepositoryService,
    makeWorkroomTemplateRepositoryService(repository),
  )

export const makeD1WorkroomTemplateRepositoryLayer = (db: D1Database) =>
  makeWorkroomTemplateRepositoryLayer(makeD1WorkroomTemplateRepository(db))

// COORDINATOR WIRING: if the API needs runtime access to this repository,
// register makeD1WorkroomTemplateRepositoryLayer(env.DB) (or the equivalent
// D1 binding) in the worker layer composition in index.ts / the shared
// repository layer module. This module intentionally does not edit index.ts.
