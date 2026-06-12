import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

export const OmniApiSdkSeedEndpoint = '/api/omni/sdk-seed'

export const OmniApiSdkSurface = S.Literals([
  'accepted_outcomes',
  'billing',
  'proof_bundles',
  'program_runs',
  'receipts',
  'webhooks',
  'workrooms',
])
export type OmniApiSdkSurface = typeof OmniApiSdkSurface.Type

export const OmniApiSdkAccessKind = S.Literals([
  'admin_operator_gated',
  'browser_session',
  'contract_only',
  'owner_grant_scoped',
  'planned',
  'public_read',
  'registered_agent_scoped',
])
export type OmniApiSdkAccessKind = typeof OmniApiSdkAccessKind.Type

export const OmniApiSdkEntryStatus = S.Literals([
  'available',
  'contract_only',
  'planned',
])
export type OmniApiSdkEntryStatus = typeof OmniApiSdkEntryStatus.Type

export const OmniApiSdkPrivacyPolicy = S.Literals([
  'operator_refs_only',
  'public_refs_only',
  'scoped_private_refs',
])
export type OmniApiSdkPrivacyPolicy = typeof OmniApiSdkPrivacyPolicy.Type

export class OmniApiSdkSchemaEntry extends S.Class<OmniApiSdkSchemaEntry>(
  'OmniApiSdkSchemaEntry',
)({
  docsUrl: S.String,
  exportName: S.String,
  id: S.String,
  privacyPolicy: OmniApiSdkPrivacyPolicy,
  schemaRef: S.String,
  sourceModule: S.String,
  status: OmniApiSdkEntryStatus,
  surface: OmniApiSdkSurface,
}) {}

export class OmniApiSdkRouteEntry extends S.Class<OmniApiSdkRouteEntry>(
  'OmniApiSdkRouteEntry',
)({
  accessKind: OmniApiSdkAccessKind,
  id: S.String,
  method: S.String,
  notes: S.String,
  operationId: S.String,
  path: S.String,
  requestSchemaRef: S.NullOr(S.String),
  responseSchemaRef: S.String,
  status: OmniApiSdkEntryStatus,
  surface: OmniApiSdkSurface,
}) {}

export class OmniApiSdkSeed extends S.Class<OmniApiSdkSeed>(
  'OmniApiSdkSeed',
)({
  docs: S.Struct({
    agents: S.String,
    capabilityManifest: S.String,
    developerApi: S.String,
    omniDocs: S.String,
    openApi: S.String,
    roadmap: S.String,
  }),
  generatedFromRef: S.String,
  guidance: S.Array(S.String),
  routeCatalog: S.Array(OmniApiSdkRouteEntry),
  schemaCatalog: S.Array(OmniApiSdkSchemaEntry),
  schemaVersion: S.Literal('openagents.omni.sdk_seed.v1'),
  sdkImportHint: S.String,
  status: S.Literal('seed'),
}) {}

export class OmniApiSdkSeedUnsafe extends S.TaggedErrorClass<OmniApiSdkSeedUnsafe>()(
  'OmniApiSdkSeedUnsafe',
  {
    reason: S.String,
  },
) {}

const docs = {
  agents: 'https://openagents.com/AGENTS.md',
  capabilityManifest: 'https://openagents.com/.well-known/openagents.json',
  developerApi: 'https://openagents.com/docs/api',
  omniDocs:
    'https://github.com/OpenAgentsInc/autopilot-omega/tree/main/docs/omni',
  openApi: 'https://openagents.com/api/openapi.json',
  roadmap:
    'https://github.com/OpenAgentsInc/autopilot-omega/blob/main/docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md',
}

const schemaEntry = (
  input: Omit<OmniApiSdkSchemaEntry, 'docsUrl' | 'id'>,
): OmniApiSdkSchemaEntry => ({
  ...input,
  docsUrl: docs.omniDocs,
  id: `omni_sdk_schema.${input.surface}.${input.exportName}`,
})

const routeEntry = (
  input: Omit<OmniApiSdkRouteEntry, 'id'>,
): OmniApiSdkRouteEntry => ({
  ...input,
  id: `omni_sdk_route.${input.operationId}`,
})

export const OMNI_API_SDK_SCHEMA_CATALOG:
  ReadonlyArray<OmniApiSdkSchemaEntry> = [
    schemaEntry({
      exportName: 'OmniWorkroomRecord',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.OmniWorkroomRecord.v1',
      sourceModule: 'workers/api/src/omni-workrooms.ts',
      status: 'available',
      surface: 'workrooms',
    }),
    schemaEntry({
      exportName: 'OmniWorkroomSurfaceProjection',
      privacyPolicy: 'public_refs_only',
      schemaRef: 'schema.omni.OmniWorkroomSurfaceProjection.v1',
      sourceModule: 'workers/api/src/omni-workroom-surface-projections.ts',
      status: 'available',
      surface: 'workrooms',
    }),
    schemaEntry({
      exportName: 'OmniAcceptedOutcomeContractRecord',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.OmniAcceptedOutcomeContractRecord.v1',
      sourceModule: 'workers/api/src/omni-accepted-outcome-contracts.ts',
      status: 'available',
      surface: 'accepted_outcomes',
    }),
    schemaEntry({
      exportName: 'AgentRunRecord',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.AgentRunRecord.v1',
      sourceModule: 'workers/api/src/omni-runs.ts',
      status: 'available',
      surface: 'program_runs',
    }),
    schemaEntry({
      exportName: 'OmniPublicProofBundleRecord',
      privacyPolicy: 'public_refs_only',
      schemaRef: 'schema.omni.OmniPublicProofBundleRecord.v1',
      sourceModule: 'workers/api/src/omni-public-proof-bundles.ts',
      status: 'available',
      surface: 'proof_bundles',
    }),
    schemaEntry({
      exportName: 'AdjutantUsageReceipt',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.AdjutantUsageReceipt.v1',
      sourceModule: 'workers/api/src/adjutant-usage-receipts.ts',
      status: 'available',
      surface: 'receipts',
    }),
    schemaEntry({
      exportName: 'BuyerPaymentReceiptRecord',
      privacyPolicy: 'public_refs_only',
      schemaRef: 'schema.omni.BuyerPaymentReceiptRecord.v1',
      sourceModule: 'workers/api/src/buyer-payment-ledger.ts',
      status: 'available',
      surface: 'receipts',
    }),
    schemaEntry({
      exportName: 'BillingSummary',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.BillingSummary.v1',
      sourceModule: 'workers/api/src/billing.ts',
      status: 'available',
      surface: 'billing',
    }),
    schemaEntry({
      exportName: 'WebhookSubscriptionRecord',
      privacyPolicy: 'scoped_private_refs',
      schemaRef: 'schema.omni.WebhookSubscriptionRecord.v1',
      sourceModule: 'workers/api/src/webhook-subscriptions.ts',
      status: 'contract_only',
      surface: 'webhooks',
    }),
    schemaEntry({
      exportName: 'ProgramRunReceiptWebhookSubscriptionContract',
      privacyPolicy: 'scoped_private_refs',
      schemaRef:
        'schema.omni.ProgramRunReceiptWebhookSubscriptionContract.v1',
      sourceModule: 'workers/api/src/webhook-subscriptions.ts',
      status: 'contract_only',
      surface: 'webhooks',
    }),
  ]

export const OMNI_API_SDK_ROUTE_CATALOG:
  ReadonlyArray<OmniApiSdkRouteEntry> = [
    routeEntry({
      accessKind: 'public_read',
      method: 'GET',
      notes:
        'Returns this public-safe SDK seed. It is discovery metadata only.',
      operationId: 'getOmniApiSdkSeed',
      path: OmniApiSdkSeedEndpoint,
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.OmniApiSdkSeed.v1',
      status: 'available',
      surface: 'workrooms',
    }),
    routeEntry({
      accessKind: 'browser_session',
      method: 'GET',
      notes:
        'Lists signed-in owner Program Runs through customer-safe projections.',
      operationId: 'listOmniAgentRuns',
      path: '/api/omni/agent-runs',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.AgentRunListProjection.v1',
      status: 'available',
      surface: 'program_runs',
    }),
    routeEntry({
      accessKind: 'browser_session',
      method: 'POST',
      notes:
        'Creates a signed-in owner Autopilot mission. This is not available to public agents without a matching owner authority path.',
      operationId: 'createOmniAgentRun',
      path: '/api/omni/agent-runs',
      requestSchemaRef: 'schema.omni.AgentRunLaunchSelector.v1',
      responseSchemaRef: 'schema.omni.AgentRunLaunchProjection.v1',
      status: 'available',
      surface: 'program_runs',
    }),
    routeEntry({
      accessKind: 'browser_session',
      method: 'GET',
      notes:
        'Reads a signed-in owner Program Run detail through route access policy.',
      operationId: 'getOmniAgentRun',
      path: '/api/omni/agent-runs/{runId}',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.AgentRunDetailProjection.v1',
      status: 'available',
      surface: 'program_runs',
    }),
    routeEntry({
      accessKind: 'browser_session',
      method: 'GET',
      notes:
        'Streams or lists signed-in owner Program Run events through route access policy.',
      operationId: 'listOmniAgentRunEvents',
      path: '/api/omni/agent-runs/{runId}/events',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.AgentRunEventListProjection.v1',
      status: 'available',
      surface: 'program_runs',
    }),
    routeEntry({
      accessKind: 'browser_session',
      method: 'GET',
      notes:
        'Reads signed-in owner billing and credit projection. It is not a payout or settlement surface.',
      operationId: 'getBillingSummary',
      path: '/api/billing/summary',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.BillingSummary.v1',
      status: 'available',
      surface: 'billing',
    }),
    routeEntry({
      accessKind: 'public_read',
      method: 'GET',
      notes:
        'Reads public-safe commerce discovery for a Site. Payment discovery cannot bypass route auth or owner policy.',
      operationId: 'getSitePaymentDiscovery',
      path: '/api/sites/{siteId}/commerce/discovery',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.sites.SitePaymentDiscovery.v1',
      status: 'available',
      surface: 'billing',
    }),
    routeEntry({
      accessKind: 'public_read',
      method: 'GET',
      notes:
        'Reads a public-safe proof bundle projection for the OTEC Site order.',
      operationId: 'getPublicOtecProof',
      path: '/api/public/proof/otec',
      requestSchemaRef: null,
      responseSchemaRef: 'schema.omni.PublicOtecProof.v1',
      status: 'available',
      surface: 'proof_bundles',
    }),
    routeEntry({
      accessKind: 'public_read',
      method: 'POST',
      notes:
        'Validates a developer package manifest. Validation does not install, deploy, list, promote, or mutate payment state.',
      operationId: 'validateSignaturePackage',
      path: '/api/developer/signature-packages/validate',
      requestSchemaRef: 'schema.developer.SignaturePackageValidationRequest.v1',
      responseSchemaRef: 'schema.developer.SignaturePackageValidationResult.v1',
      status: 'available',
      surface: 'workrooms',
    }),
    routeEntry({
      accessKind: 'contract_only',
      method: 'POST',
      notes:
        'Planned dispatcher route. Current webhook work is schema/projection only and cannot send external webhooks.',
      operationId: 'createProgramRunReceiptWebhookSubscription',
      path: '/api/omni/webhooks/program-run-receipts',
      requestSchemaRef:
        'schema.omni.ProgramRunReceiptWebhookSubscriptionContract.v1',
      responseSchemaRef:
        'schema.omni.ProgramRunReceiptWebhookProjection.v1',
      status: 'planned',
      surface: 'webhooks',
    }),
  ]

export const OMNI_API_SDK_SEED: OmniApiSdkSeed = {
  docs,
  generatedFromRef: 'roadmap.omega.epic_t.omni_api_sdk_seed',
  guidance: [
    'Read AGENTS.md, the capability manifest, and OpenAPI before taking action.',
    'Treat this seed as discovery metadata. It does not grant authority by itself.',
    'Use browser-session routes for signed-in owner work and registered-agent routes only when a server-side grant says the action is available.',
    'Use idempotency keys for writes that require them and keep private payloads out of public posts, docs, issue comments, and prompts.',
    'Payment can satisfy only route-advertised economic requirements. It cannot bypass missing auth, owner scope, moderation, privacy, legal, repository, deployment, or operator policy.',
  ],
  routeCatalog: [...OMNI_API_SDK_ROUTE_CATALOG],
  schemaCatalog: [...OMNI_API_SDK_SCHEMA_CATALOG],
  schemaVersion: 'openagents.omni.sdk_seed.v1',
  sdkImportHint:
    'Use the schemaRef and sourceModule fields as the first generated-SDK seed. Generated clients should be produced from OpenAPI plus these Effect-schema source refs.',
  status: 'seed',
}

const unsafeSeedTextPattern =
  /(access[_-]?token|bearer\s+[A-Za-z0-9._-]{8,}|cookie|customer[_-]?(email|name|value)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access|mnemonic|webhook)|mnemonic|oauth|payment[_-]?(hash|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|response|token)|raw[_-]?(body|email|invoice|payment|payload|prompt|provider|response|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret[_-]?(key|ref|token|value)|sk-[a-z0-9]|source[_-]?archive|wallet)/i

export const omniApiSdkSeedIsPrivateDataSafe = (
  seed: OmniApiSdkSeed,
): boolean => {
  const text = JSON.stringify(seed)

  return !containsProviderSecretMaterial(text) &&
    !unsafeSeedTextPattern.test(text)
}

export const omniApiSdkSeedHasRequiredSurfaces = (
  seed: OmniApiSdkSeed,
): boolean => {
  const surfaces = new Set(seed.schemaCatalog.map(entry => entry.surface))

  return [
    'accepted_outcomes',
    'billing',
    'proof_bundles',
    'program_runs',
    'receipts',
    'webhooks',
    'workrooms',
  ].every(surface => surfaces.has(surface as OmniApiSdkSurface))
}

export const omniApiSdkSeed = (): Effect.Effect<
  OmniApiSdkSeed,
  OmniApiSdkSeedUnsafe
> =>
  Effect.try({
    try: () => {
      const seed = S.decodeUnknownSync(OmniApiSdkSeed)(OMNI_API_SDK_SEED)

      if (!omniApiSdkSeedIsPrivateDataSafe(seed)) {
        throw new OmniApiSdkSeedUnsafe({
          reason: 'Omni API SDK seed contains private or secret-shaped material.',
        })
      }

      if (!omniApiSdkSeedHasRequiredSurfaces(seed)) {
        throw new OmniApiSdkSeedUnsafe({
          reason: 'Omni API SDK seed is missing a required surface.',
        })
      }

      return seed
    },
    catch: error => error instanceof OmniApiSdkSeedUnsafe
      ? error
      : new OmniApiSdkSeedUnsafe({
        reason: 'Omni API SDK seed failed schema validation.',
      }),
  })
