// The Blueprint contract-export schema types and the security-critical
// `IsPrivateDataSafe` predicate are owned by the canonical
// `@openagentsinc/blueprint-contracts` package (the single drift-free
// authority). This module keeps only the openagents.com-specific export seed
// DATA (`BLUEPRINT_CONTRACT_EXPORT_SEED`) and its app-local coverage/catalog
// helpers, and re-exports the shared contract for existing import sites.
//
// Re-exporting (not redefining) is enforced by scripts/check-contract-drift.mjs.
// All of these are runtime Effect Schema values in the canonical package, so
// they are imported and re-exported as values (not type-only) to preserve the
// original value+type dual binding consumers (e.g. blueprint/index.ts) rely on.
import {
  BlueprintContractConsumer,
  BlueprintContractPrivacyPolicy,
  BlueprintContractExportSeed,
  BlueprintContractStability,
  BlueprintEventCatalogEntry,
  BlueprintJsonSchemaContract,
  BlueprintOpenApiContract,
  BlueprintReceiptCatalogEntry,
  blueprintContractExportSeedIsPrivateDataSafe,
} from '@openagentsinc/blueprint-contracts'

export {
  BlueprintContractConsumer,
  BlueprintContractExportSeed,
  BlueprintContractPrivacyPolicy,
  BlueprintContractStability,
  BlueprintEventCatalogEntry,
  BlueprintJsonSchemaContract,
  BlueprintOpenApiContract,
  BlueprintReceiptCatalogEntry,
  blueprintContractExportSeedIsPrivateDataSafe,
}

export const BLUEPRINT_CONTRACT_CONSUMERS: ReadonlyArray<BlueprintContractConsumer> =
  [
    'ai_agent',
    'nexus',
    'oa_node',
    'oa_workroomd',
    'probe',
    'psionic',
    'pylon',
    'treasury',
  ]

const allConsumers = (): Array<BlueprintContractConsumer> => [
  ...BLUEPRINT_CONTRACT_CONSUMERS,
]

const operatorConsumers = (): Array<BlueprintContractConsumer> => [
  'ai_agent',
  'nexus',
  'oa_node',
  'oa_workroomd',
  'probe',
  'psionic',
  'pylon',
]

const jsonSchema = (
  name: string,
  privacyPolicy: BlueprintContractPrivacyPolicy,
): BlueprintJsonSchemaContract => ({
  consumers:
    privacyPolicy === 'public_refs_only' ? allConsumers() : operatorConsumers(),
  id: `blueprint_json_schema.${name}.v1`,
  jsonSchemaUrl: `https://openagents.com/api/blueprint/contracts/json-schema/${name}.schema.json`,
  name,
  openApiComponentRef: `#/components/schemas/${name}`,
  privacyPolicy,
  schemaRef: `schema.blueprint.${name}.v1`,
  stability: 'seed',
  versionRef: `schema.blueprint.${name}.v1`,
})

export const BLUEPRINT_CONTRACT_EXPORT_SEED: BlueprintContractExportSeed = {
  consumers: allConsumers(),
  eventCatalog: [
    {
      consumers: operatorConsumers(),
      eventRef: 'event.blueprint.program_run.recorded.v1',
      id: 'blueprint_event.program_run.recorded.v1',
      payloadSchemaRef:
        'schema.blueprint.BlueprintProgramRunDetailProjection.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRefs: ['receipt.program_run'],
      stability: 'seed',
      topicRef: 'topic.blueprint.program_run.recorded',
    },
    {
      consumers: operatorConsumers(),
      eventRef: 'event.blueprint.action_submission.proposed.v1',
      id: 'blueprint_event.action_submission.proposed.v1',
      payloadSchemaRef: 'schema.blueprint.BlueprintActionSubmission.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRefs: ['receipt.action_submission'],
      stability: 'seed',
      topicRef: 'topic.blueprint.action_submission.proposed',
    },
    {
      consumers: allConsumers(),
      eventRef: 'event.blueprint.release_gate.decided.v1',
      id: 'blueprint_event.release_gate.decided.v1',
      payloadSchemaRef: 'schema.blueprint.BlueprintReleaseGate.v1',
      privacyPolicy: 'public_refs_only',
      receiptRefs: ['receipt.release_gate'],
      stability: 'seed',
      topicRef: 'topic.blueprint.release_gate.decided',
    },
    {
      consumers: operatorConsumers(),
      eventRef: 'event.blueprint.probe.failed.v1',
      id: 'blueprint_event.probe.failed.v1',
      payloadSchemaRef: 'schema.blueprint.BlueprintSmokeProbeResult.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRefs: ['receipt.probe_failure'],
      stability: 'seed',
      topicRef: 'topic.blueprint.probe.failed',
    },
  ],
  id: 'blueprint_contract_export.seed.v1',
  jsonSchemas: [
    jsonSchema('BlueprintObjectiveType', 'public_refs_only'),
    jsonSchema('BlueprintObjectiveRun', 'operator_refs_only'),
    jsonSchema('BlueprintProgramType', 'public_refs_only'),
    jsonSchema('BlueprintProgramSignature', 'public_refs_only'),
    jsonSchema('BlueprintModuleVersion', 'operator_refs_only'),
    jsonSchema('BlueprintProgramRunRecord', 'operator_refs_only'),
    jsonSchema('BlueprintReplayModuleEvidence', 'operator_refs_only'),
    jsonSchema('BlueprintTassadarModuleStepEvidence', 'operator_refs_only'),
    jsonSchema(
      'BlueprintTassadarModuleRegistryProjection',
      'operator_refs_only',
    ),
    jsonSchema('BlueprintChatProgramTurnResult', 'operator_refs_only'),
    jsonSchema('ProbeBlueprintProgramRunEvidence', 'operator_refs_only'),
    jsonSchema(
      'BlueprintProgramRunEvidenceIntakeResponse',
      'operator_refs_only',
    ),
    jsonSchema('BlueprintActionSubmission', 'operator_refs_only'),
    jsonSchema('ProbeBlueprintActionSubmissionProposal', 'operator_refs_only'),
    jsonSchema('BlueprintActionSubmissionIntakeResponse', 'operator_refs_only'),
    jsonSchema('ProbeBlueprintContributionDraft', 'operator_refs_only'),
    jsonSchema('BlueprintProbeContributionRecord', 'operator_refs_only'),
    jsonSchema(
      'BlueprintProbeContributionIntakeResponse',
      'operator_refs_only',
    ),
    jsonSchema('BlueprintSourceAuthority', 'operator_refs_only'),
    jsonSchema('BlueprintContextPack', 'operator_refs_only'),
    jsonSchema('BlueprintReleaseGate', 'public_refs_only'),
    jsonSchema('BlueprintOptimizerRun', 'operator_refs_only'),
    jsonSchema('BlueprintSimulationBranch', 'operator_refs_only'),
    jsonSchema('BlueprintProgramRegistryProjection', 'operator_refs_only'),
    jsonSchema('BlueprintSmokeProbePlan', 'operator_refs_only'),
  ],
  openApi: [
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.program_registry.get.v1',
      method: 'GET',
      operationRef: 'operation.blueprint.program_registry.get',
      path: '/api/blueprint/program-registry',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: null,
      responseSchemaRef:
        'schema.blueprint.BlueprintProgramRegistryProjection.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.contract_export.get.v1',
      method: 'GET',
      operationRef: 'operation.blueprint.contract_export.get',
      path: '/api/blueprint/contracts',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.blueprint.BlueprintContractExportSeed.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.tassadar_modules.get.v1',
      method: 'GET',
      operationRef: 'operation.blueprint.tassadar_modules.get',
      path: '/api/blueprint/tassadar-modules',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: null,
      responseSchemaRef:
        'schema.blueprint.BlueprintTassadarModuleRegistryProjection.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.program_run_evidence.post.v1',
      method: 'POST',
      operationRef: 'operation.blueprint.program_run_evidence.post',
      path: '/api/blueprint/program-runs',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: 'schema.blueprint.ProbeBlueprintProgramRunEvidence.v1',
      responseSchemaRef:
        'schema.blueprint.BlueprintProgramRunEvidenceIntakeResponse.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.action_submissions.get.v1',
      method: 'GET',
      operationRef: 'operation.blueprint.action_submissions.get',
      path: '/api/blueprint/action-submissions',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.blueprint.BlueprintActionSubmission.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.action_submission_proposal.post.v1',
      method: 'POST',
      operationRef: 'operation.blueprint.action_submission_proposal.post',
      path: '/api/blueprint/action-submissions',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef:
        'schema.blueprint.ProbeBlueprintActionSubmissionProposal.v1',
      responseSchemaRef:
        'schema.blueprint.BlueprintActionSubmissionIntakeResponse.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.probe_contributions.get.v1',
      method: 'GET',
      operationRef: 'operation.blueprint.probe_contributions.get',
      path: '/api/blueprint/contributions',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.blueprint.BlueprintProbeContributionRecord.v1',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      id: 'blueprint_openapi.probe_contribution.post.v1',
      method: 'POST',
      operationRef: 'operation.blueprint.probe_contribution.post',
      path: '/api/blueprint/contributions',
      privacyPolicy: 'operator_refs_only',
      requestSchemaRef: 'schema.blueprint.ProbeBlueprintContributionDraft.v1',
      responseSchemaRef:
        'schema.blueprint.BlueprintProbeContributionIntakeResponse.v1',
      stability: 'seed',
    },
  ],
  receiptCatalog: [
    {
      consumers: allConsumers(),
      evidenceSchemaRef:
        'schema.blueprint.BlueprintProgramRunDetailProjection.v1',
      id: 'blueprint_receipt.program_run.v1',
      privacyPolicy: 'public_refs_only',
      receiptRef: 'receipt.program_run',
      retentionPolicyRef: 'retention.blueprint.public_receipt_refs',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      evidenceSchemaRef: 'schema.blueprint.BlueprintActionSubmission.v1',
      id: 'blueprint_receipt.action_submission.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRef: 'receipt.action_submission',
      retentionPolicyRef: 'retention.blueprint.operator_receipt_refs',
      stability: 'seed',
    },
    {
      consumers: allConsumers(),
      evidenceSchemaRef: 'schema.blueprint.BlueprintReleaseGate.v1',
      id: 'blueprint_receipt.release_gate.v1',
      privacyPolicy: 'public_refs_only',
      receiptRef: 'receipt.release_gate',
      retentionPolicyRef: 'retention.blueprint.public_receipt_refs',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      evidenceSchemaRef:
        'schema.blueprint.BlueprintTassadarModuleStepEvidence.v1',
      id: 'blueprint_receipt.tassadar_module_step.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRef: 'receipt.openagents.blueprint_tassadar_step',
      retentionPolicyRef: 'retention.blueprint.operator_receipt_refs',
      stability: 'seed',
    },
    {
      consumers: allConsumers(),
      evidenceSchemaRef: 'schema.blueprint.BlueprintReplayModuleEvidence.v1',
      id: 'blueprint_receipt.public_proof_replay_bundle.v1',
      privacyPolicy: 'public_refs_only',
      receiptRef: 'receipt.public_proof_replay_bundle',
      retentionPolicyRef: 'retention.blueprint.public_receipt_refs',
      stability: 'seed',
    },
    {
      consumers: operatorConsumers(),
      evidenceSchemaRef: 'schema.blueprint.BlueprintSmokeProbeResult.v1',
      id: 'blueprint_receipt.probe_failure.v1',
      privacyPolicy: 'operator_refs_only',
      receiptRef: 'receipt.probe_failure',
      retentionPolicyRef: 'retention.blueprint.retained_failure_refs',
      stability: 'seed',
    },
  ],
  versionRef: 'blueprint_contract_export.seed.v1',
}

export const blueprintContractExportSeedCoversConsumers = (
  seed: BlueprintContractExportSeed,
): boolean =>
  BLUEPRINT_CONTRACT_CONSUMERS.every(consumer =>
    seed.consumers.includes(consumer),
  )

export const blueprintContractExportSeedHasCatalogs = (
  seed: BlueprintContractExportSeed,
): boolean =>
  seed.jsonSchemas.length > 0 &&
  seed.openApi.length > 0 &&
  seed.eventCatalog.length > 0 &&
  seed.receiptCatalog.length > 0
