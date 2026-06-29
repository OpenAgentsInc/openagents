import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OMNI_WORKROOM_KIND_TEMPLATES } from './omni-workroom-kind-templates'
import {
  WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE,
  WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE,
} from './workroom-template-packages'
import {
  makeD1WorkroomTemplateRepository,
  makeWorkroomTemplateRepositoryService,
  type WorkroomTemplatePackageStoredRecord,
  type WorkroomTemplatePackageStoredVersion,
} from './workroom-template-repository'

// ---------------------------------------------------------------------------
// Minimal in-memory D1 fake (modelled on omni-workrooms.test.ts) that stores
// rows in plain arrays and answers the exact statements this repository runs.
// ---------------------------------------------------------------------------

class TemplateStore {
  kindTemplates: Array<Record<string, unknown>> = []
  packages: Array<Record<string, unknown>> = []
  versions: Array<Record<string, unknown>> = []
}

const KIND_TEMPLATE_COLUMNS = [
  'kind',
  'accepted_outcome_work_kind',
  'description_ref',
  'privacy_constraint',
  'proof_policy',
  'public_projection_policy',
  'review_policy',
  'closeout_requirements_json',
  'required_artifacts_json',
  'required_evidence_json',
  'created_at',
  'updated_at',
] as const

const PACKAGE_COLUMNS = [
  'id',
  'package_ref',
  'version_ref',
  'display_name',
  'state',
  'authority_boundary',
  'no_deployment',
  'no_external_runner_launch',
  'no_marketplace_listing',
  'no_payment_mutation',
  'no_runtime_promotion',
  'approval_policy_refs_json',
  'blocker_refs_json',
  'caveat_refs_json',
  'evidence_requirement_refs_json',
  'operator_diagnostic_refs_json',
  'org_private_enablement_refs_json',
  'outcome_template_refs_json',
  'proof_rule_refs_json',
  'promotion_refs_json',
  'public_projection_refs_json',
  'required_artifact_refs_json',
  'review_refs_json',
  'runner_need_refs_json',
  'source_refs_json',
  'template_version_refs_json',
  'ui_binding_refs_json',
  'validation_refs_json',
  'created_at',
  'updated_at',
] as const

const VERSION_COLUMNS = [
  'id',
  'package_id',
  'template_version_ref',
  'approval_policy_refs_json',
  'caveat_refs_json',
  'evidence_requirement_refs_json',
  'outcome_template_refs_json',
  'proof_rule_refs_json',
  'required_artifact_refs_json',
  'runner_need_refs_json',
  'source_refs_json',
  'ui_binding_refs_json',
  'created_at',
  'updated_at',
] as const

const rowFrom = (
  columns: ReadonlyArray<string>,
  values: ReadonlyArray<unknown>,
): Record<string, unknown> =>
  Object.fromEntries(columns.map((column, index) => [column, values[index]]))

class TemplateStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: TemplateStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM workroom_kind_templates')) {
      const kind = String(this.values[0])
      const row =
        this.store.kindTemplates.find(item => item.kind === kind) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM workroom_template_packages')) {
      const packageRef = String(this.values[0])
      const row =
        this.store.packages.find(item => item.package_ref === packageRef) ??
        null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INTO workroom_kind_templates')) {
      const row = rowFrom(KIND_TEMPLATE_COLUMNS, this.values)
      const index = this.store.kindTemplates.findIndex(
        item => item.kind === row.kind,
      )

      if (index === -1) {
        this.store.kindTemplates.push(row)
      } else {
        // ON CONFLICT(kind) DO UPDATE: keep original created_at.
        const existing = this.store.kindTemplates[index]
        this.store.kindTemplates[index] = {
          ...row,
          created_at: existing?.created_at,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INTO workroom_template_packages')) {
      this.store.packages.push(rowFrom(PACKAGE_COLUMNS, this.values))

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INTO workroom_template_package_versions')) {
      this.store.versions.push(rowFrom(VERSION_COLUMNS, this.values))

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM workroom_kind_templates')) {
      const results = [...this.store.kindTemplates].sort((left, right) =>
        String(left.kind).localeCompare(String(right.kind)),
      )

      return Promise.resolve({ results } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM workroom_template_package_versions')) {
      const packageId = String(this.values[0])
      const results = this.store.versions.filter(
        item => item.package_id === packageId,
      )

      return Promise.resolve({ results } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM workroom_template_packages')) {
      return Promise.resolve({
        results: [...this.store.packages],
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const templateDb = (store: TemplateStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new TemplateStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const NOW = '2026-06-14T12:00:00.000Z'

const storedPackage = (): WorkroomTemplatePackageStoredRecord => ({
  approvalPolicyRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.approvalPolicyRefs,
  authority: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.authority,
  blockerRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.blockerRefs,
  caveatRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.caveatRefs,
  createdAtIso: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.createdAtIso,
  displayName: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.displayName,
  evidenceRequirementRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.evidenceRequirementRefs,
  id: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.id,
  operatorDiagnosticRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.operatorDiagnosticRefs,
  orgPrivateEnablementRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.orgPrivateEnablementRefs,
  outcomeTemplateRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.outcomeTemplateRefs,
  packageRef: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.packageRef,
  proofRuleRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.proofRuleRefs,
  promotionRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.promotionRefs,
  publicProjectionRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.publicProjectionRefs,
  requiredArtifactRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.requiredArtifactRefs,
  reviewRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.reviewRefs,
  runnerNeedRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.runnerNeedRefs,
  sourceRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.sourceRefs,
  state: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.state,
  templateVersionRefs:
    WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.templateVersionRefs,
  uiBindingRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.uiBindingRefs,
  updatedAtIso: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.updatedAtIso,
  validationRefs: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.validationRefs,
  versionRef: WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE.versionRef,
})

const storedVersion = (
  packageId: string,
): WorkroomTemplatePackageStoredVersion => ({
  approvalPolicyRefs:
    WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.approvalPolicyRefs,
  caveatRefs: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.caveatRefs,
  createdAtIso: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.createdAtIso,
  evidenceRequirementRefs:
    WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.evidenceRequirementRefs,
  id: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.id,
  outcomeTemplateRefs:
    WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.outcomeTemplateRefs,
  packageId,
  proofRuleRefs: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.proofRuleRefs,
  requiredArtifactRefs:
    WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.requiredArtifactRefs,
  runnerNeedRefs: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.runnerNeedRefs,
  sourceRefs: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.sourceRefs,
  templateVersionRef:
    WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.templateVersionRef,
  uiBindingRefs: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.uiBindingRefs,
  updatedAtIso: WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE.updatedAtIso,
})

describe('Workroom template repository', () => {
  test('creates and reads a workroom kind template round-trip', async () => {
    const repository = makeD1WorkroomTemplateRepository(
      templateDb(new TemplateStore()),
    )
    const codingTemplate = OMNI_WORKROOM_KIND_TEMPLATES.coding

    const created = await repository.upsertKindTemplate(codingTemplate, NOW)
    const found = await repository.findKindTemplate('coding')

    expect(created).toMatchObject({
      acceptedOutcomeWorkKind: 'coding',
      createdAtIso: NOW,
      kind: 'coding',
      privacyConstraint: 'customer_private',
      proofPolicy: 'customer_safe_summary',
      reviewPolicy: 'customer_review',
      updatedAtIso: NOW,
    })
    expect(found).toBeDefined()
    expect(found?.closeoutRequirements).toEqual(
      codingTemplate.closeoutRequirements,
    )
    expect(found?.requiredArtifacts).toEqual(codingTemplate.requiredArtifacts)
    expect(found?.requiredEvidence).toEqual(codingTemplate.requiredEvidence)
  })

  test('lists every persisted kind template sorted by kind', async () => {
    const store = new TemplateStore()
    const repository = makeD1WorkroomTemplateRepository(templateDb(store))

    for (const template of Object.values(OMNI_WORKROOM_KIND_TEMPLATES)) {
      await repository.upsertKindTemplate(template, NOW)
    }

    const listed = await repository.listKindTemplates()

    expect(listed).toHaveLength(
      Object.keys(OMNI_WORKROOM_KIND_TEMPLATES).length,
    )
    expect(listed.map(item => item.kind)).toEqual([
      'coding',
      'crm',
      'document',
      'finance_ops',
      'investor_ops',
      'legal_review',
      'meeting',
      'project_ops',
      'site',
      'support',
    ])
  })

  test('upsert keeps the original created_at on conflict', async () => {
    const repository = makeD1WorkroomTemplateRepository(
      templateDb(new TemplateStore()),
    )

    await repository.upsertKindTemplate(
      OMNI_WORKROOM_KIND_TEMPLATES.site,
      '2026-06-14T10:00:00.000Z',
    )
    const updated = await repository.upsertKindTemplate(
      OMNI_WORKROOM_KIND_TEMPLATES.site,
      '2026-06-14T11:00:00.000Z',
    )

    expect(updated.createdAtIso).toBe('2026-06-14T10:00:00.000Z')
    expect(updated.updatedAtIso).toBe('2026-06-14T11:00:00.000Z')
  })

  test('creates, reads, and lists a template package with a version', async () => {
    const store = new TemplateStore()
    const repository = makeD1WorkroomTemplateRepository(templateDb(store))
    const record = storedPackage()

    const created = await repository.createTemplatePackage(record)
    const version = await repository.createTemplatePackageVersion(
      storedVersion(record.id),
    )

    const found = await repository.findTemplatePackage(record.packageRef)
    const listedPackages = await repository.listTemplatePackages()
    const listedVersions = await repository.listTemplatePackageVersions(
      record.id,
    )

    expect(created.packageRef).toBe(record.packageRef)
    expect(found).toBeDefined()
    expect(found).toMatchObject({
      authority: { authorityBoundary: 'package_review_projection_only' },
      displayName: record.displayName,
      packageRef: record.packageRef,
      state: 'runtime_promotion_requested',
      versionRef: record.versionRef,
    })
    expect(found?.authority.noRuntimePromotion).toBe(true)
    expect(found?.templateVersionRefs).toEqual(record.templateVersionRefs)
    expect(found?.validationRefs).toEqual(record.validationRefs)
    expect(listedPackages).toHaveLength(1)
    expect(listedVersions).toHaveLength(1)
    const [listedVersion] = listedVersions
    expect(listedVersion).toMatchObject({
      id: version.id,
      packageId: record.id,
      templateVersionRef: version.templateVersionRef,
    })
    expect(listedVersion?.outcomeTemplateRefs).toEqual(
      version.outcomeTemplateRefs,
    )
  })

  test('exposes the repository through the Effect service wrapper', async () => {
    const store = new TemplateStore()
    const service = makeWorkroomTemplateRepositoryService(
      makeD1WorkroomTemplateRepository(templateDb(store)),
    )

    const created = await Effect.runPromise(
      service.upsertKindTemplate(OMNI_WORKROOM_KIND_TEMPLATES.support, NOW),
    )
    const found = await Effect.runPromise(service.findKindTemplate('support'))

    expect(created.kind).toBe('support')
    expect(found?.reviewPolicy).toBe('operator_review')
  })
})
