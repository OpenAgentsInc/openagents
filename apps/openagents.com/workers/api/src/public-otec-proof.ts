import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  PublicClaimStateProjection,
  publicClaimStateProjection,
} from './public-claim-state'
import {
  PublicClaimProjection,
  PublicClaimProjectionRecord,
  projectPublicClaimRecord,
} from './public-claim-projections'
import { arrayFromUnknown, parseJsonUnknown } from './json-boundary'
import {
  PublicSiteAgentInstructionCard,
  publicSiteAgentInstructionCard,
} from './public-site-agent-instruction-card'
import {
  PublicSiteReferralCta,
  publicSiteReferralCta,
} from './public-site-referral-cta'
import {
  PublicSiteAgentChallenge,
  publicSiteAgentChallenges,
} from './public-site-agent-challenges'

export const OTEC_SOFTWARE_ORDER_ID =
  'software_order_c34f3a52d60b41d699b71525365b6ee5'
const UNKNOWN_UPDATED_AT = '1970-01-01T00:00:00.000Z'

export class PublicOtecProofOrder extends S.Class<PublicOtecProofOrder>(
  'PublicOtecProofOrder',
)({
  id: S.String,
  status: S.String,
  requestSummary: S.String,
  repositoryFullName: S.NullOr(S.String),
  updatedAt: S.String,
}) {}

export class PublicOtecProofSite extends S.Class<PublicOtecProofSite>(
  'PublicOtecProofSite',
)({
  id: S.NullOr(S.String),
  title: S.NullOr(S.String),
  slug: S.NullOr(S.String),
  status: S.NullOr(S.String),
  activeUrl: S.NullOr(S.String),
  activeVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
}) {}

export class PublicOtecProofAssignment extends S.Class<PublicOtecProofAssignment>(
  'PublicOtecProofAssignment',
)({
  id: S.NullOr(S.String),
  kind: S.NullOr(S.String),
  status: S.NullOr(S.String),
  currentRunId: S.NullOr(S.String),
}) {}

export class PublicOtecProofResearch extends S.Class<PublicOtecProofResearch>(
  'PublicOtecProofResearch',
)({
  runId: S.NullOr(S.String),
  briefId: S.NullOr(S.String),
  status: S.String,
  sourceCount: S.Number,
  approvedSourceCount: S.Number,
  claimState: PublicClaimStateProjection,
}) {}

export class PublicOtecProofVersion extends S.Class<PublicOtecProofVersion>(
  'PublicOtecProofVersion',
)({
  activeVersionId: S.NullOr(S.String),
  latestSavedVersionId: S.NullOr(S.String),
  activeVersionUrl: S.NullOr(S.String),
  latestSavedVersionUrl: S.NullOr(S.String),
  versionRefs: S.Array(S.String),
  latestBuildStatus: S.NullOr(S.String),
  sourceCommitSha: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
}) {}

export class PublicOtecProofCompatibility extends S.Class<PublicOtecProofCompatibility>(
  'PublicOtecProofCompatibility',
)({
  latestCheckId: S.NullOr(S.String),
  status: S.NullOr(S.String),
  blockerCount: S.Number,
  warningCount: S.Number,
  customerSafeStatus: S.NullOr(S.String),
  customerSafeNextAction: S.NullOr(S.String),
}) {}

export class PublicOtecProofBuildValidation extends S.Class<PublicOtecProofBuildValidation>(
  'PublicOtecProofBuildValidation',
)({
  latestValidationId: S.NullOr(S.String),
  status: S.NullOr(S.String),
  sourceHash: S.NullOr(S.String),
  blockerCount: S.Number,
  warningCount: S.Number,
  customerSafeStatus: S.NullOr(S.String),
  customerSafeNextAction: S.NullOr(S.String),
}) {}

export class PublicOtecProofDeployment extends S.Class<PublicOtecProofDeployment>(
  'PublicOtecProofDeployment',
)({
  id: S.NullOr(S.String),
  status: S.NullOr(S.String),
  url: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
}) {}

export class PublicOtecProofReceipts extends S.Class<PublicOtecProofReceipts>(
  'PublicOtecProofReceipts',
)({
  usageReceiptCount: S.Number,
  publicReceiptRefs: S.Array(S.String),
  acceptedWorkSettlementRefs: S.Array(S.String),
  paymentCaveats: S.Array(S.String),
}) {}

export class PublicOtecProofCloseout extends S.Class<PublicOtecProofCloseout>(
  'PublicOtecProofCloseout',
)({
  slug: S.Literal('ben-otec'),
  orderId: S.String,
  title: S.String,
  customerSafeStatus: S.String,
  nextAction: S.String,
  order: PublicOtecProofOrder,
  site: PublicOtecProofSite,
  assignment: PublicOtecProofAssignment,
  research: PublicOtecProofResearch,
  version: PublicOtecProofVersion,
  compatibility: PublicOtecProofCompatibility,
  buildValidation: PublicOtecProofBuildValidation,
  deployment: PublicOtecProofDeployment,
  receipts: PublicOtecProofReceipts,
  claimProjections: S.Array(PublicClaimProjection),
  siteUrlRefs: S.Array(S.String),
  revisionUrlRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  referralCta: S.NullOr(PublicSiteReferralCta),
  agentInstructionCard: S.NullOr(PublicSiteAgentInstructionCard),
  agentChallenges: S.Array(PublicSiteAgentChallenge),
  claimState: PublicClaimStateProjection,
  caveats: S.Array(S.String),
  updatedAt: S.String,
}) {}

export class PublicOtecProofNotFound extends S.TaggedErrorClass<PublicOtecProofNotFound>()(
  'PublicOtecProofNotFound',
  {},
) {}

export class PublicOtecProofStorageError extends S.TaggedErrorClass<PublicOtecProofStorageError>()(
  'PublicOtecProofStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class PublicOtecProofUnsafe extends S.TaggedErrorClass<PublicOtecProofUnsafe>()(
  'PublicOtecProofUnsafe',
  {},
) {}

type PublicOtecProofError =
  | PublicOtecProofNotFound
  | PublicOtecProofStorageError
  | PublicOtecProofUnsafe

type BaseRow = Readonly<{
  active_deployment_id: string | null
  active_deployment_status: string | null
  active_deployment_updated_at: string | null
  active_deployment_url: string | null
  active_version_id: string | null
  assignment_current_run_id: string | null
  assignment_id: string | null
  assignment_kind: string | null
  assignment_status: string | null
  assignment_updated_at: string | null
  order_id: string
  order_request: string
  order_status: string
  order_updated_at: string
  repository_full_name: string | null
  site_access_mode: string | null
  site_id: string | null
  site_slug: string | null
  site_status: string | null
  site_title: string | null
  site_updated_at: string | null
  site_visibility: string | null
}>

type ResearchRow = Readonly<{
  approved_at: string | null
  approved_source_count: number
  research_brief_id: string | null
  run_id: string
  source_count: number
  status: string
  updated_at: string
}>

type VersionRow = Readonly<{
  build_status: string
  created_at: string
  id: string
  saved_at: string | null
  source_commit_sha: string | null
}>

type CompatibilityRow = Readonly<{
  blockers_json: string
  customer_safe_next_action: string
  customer_safe_status: string
  id: string
  status: string
  warnings_json: string
}>

type BuildValidationRow = Readonly<{
  blockers_json: string
  customer_safe_next_action: string
  customer_safe_status: string
  id: string
  source_hash: string
  status: string
  warnings_json: string
}>

type ReceiptRow = Readonly<{ id: string }>

const unsafeValuePattern =
  /(@|access[_-]?token|api[_-]?token|auth\.json|auth[_-]?token|bearer|callback[_-]?token|checkout_id=|client[_-]?secret|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|runner[_-]?(payload|log)|secret[_-]?(key|ref|token|value)|source[_-]?archive|token[_-]?(hash|ref|value)|wallet[_-]?(key|ref|secret|state|value)|webhook[_-]?secret|workroom[_-]?private)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).some(valueHasPrivateMaterial)
  }

  return false
}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, PublicOtecProofStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new PublicOtecProofStorageError({ operation, error }),
  })

const boundedSummary = (value: string, maxLength = 360): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, maxLength - 1).trimEnd()}...`
}

const parseArrayCount = (value: string): number => {
  try {
    const parsed = parseJsonUnknown(value)

    return arrayFromUnknown(parsed)?.length ?? 0
  } catch {
    return 0
  }
}

const newestTimestamp = (
  timestamps: ReadonlyArray<string | null | undefined>,
): string =>
  timestamps
    .filter((timestamp): timestamp is string => timestamp !== null && timestamp !== undefined)
    .sort()
    .at(-1) ?? UNKNOWN_UPDATED_AT

const evidenceRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter(
    (ref): ref is string =>
      ref !== null && ref !== undefined && ref.trim() !== '',
  ))]

const siteVersionUrl = (
  siteSlug: string | null,
  versionId: string | null,
): string | null =>
  siteSlug === null || versionId === null
    ? null
    : `https://sites.openagents.com/${siteSlug}/versions/${versionId}`

const receiptRefs = (
  receiptRows: ReadonlyArray<ReceiptRow>,
): ReadonlyArray<string> =>
  receiptRows.map(receipt => `usage_receipt:${receipt.id}`)

const paymentCaveats = (
  publicReceiptRefs: ReadonlyArray<string>,
): ReadonlyArray<string> => [
  'Usage receipts describe public-beta generation, build, hosting, or storage activity.',
  'Buyer-payment and Site-checkout receipts are separate from accepted-work settlement evidence.',
  ...(publicReceiptRefs.length === 0
    ? ['No public usage receipts have been recorded for this closeout yet.']
    : []),
]

const closeoutEvidenceRefs = (
  base: BaseRow,
  research: ResearchRow | null,
  latestVersion: VersionRow | null,
  buildValidation: BuildValidationRow | null,
  publicReceiptRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  evidenceRefs([
    `order:${base.order_id}`,
    base.site_slug === null ? null : `site:${base.site_slug}`,
    research?.run_id === undefined ? null : `research:${research.run_id}`,
    research?.research_brief_id === undefined
      ? null
      : `research_brief:${research.research_brief_id}`,
    latestVersion?.id === undefined ? null : `version:${latestVersion.id}`,
    buildValidation?.id === undefined
      ? null
      : `build_validation:${buildValidation.id}`,
    base.active_deployment_id === null
      ? null
      : `deployment:${base.active_deployment_id}`,
    base.active_deployment_url,
    siteVersionUrl(base.site_slug, base.active_version_id),
    siteVersionUrl(base.site_slug, latestVersion?.id ?? null),
    ...publicReceiptRefs,
  ])

const customerStatus = (
  base: BaseRow,
  buildValidation: BuildValidationRow | null,
  compatibility: CompatibilityRow | null,
  version: VersionRow | null,
): string => {
  if (
    base.active_deployment_status === 'active' &&
    base.active_deployment_url !== null
  ) {
    return 'OTEC public site is deployed.'
  }

  if (buildValidation?.customer_safe_status !== undefined) {
    return buildValidation.customer_safe_status
  }

  if (compatibility?.customer_safe_status !== undefined) {
    return compatibility.customer_safe_status
  }

  if (version?.build_status === 'saved') {
    return 'OTEC public site has a saved version awaiting deployment approval.'
  }

  if (base.site_id !== null) {
    return 'OTEC public site is being prepared for review.'
  }

  return 'OTEC public site work has an accepted order but no public site project yet.'
}

const nextAction = (
  base: BaseRow,
  buildValidation: BuildValidationRow | null,
  compatibility: CompatibilityRow | null,
  version: VersionRow | null,
): string => {
  if (
    base.active_deployment_status === 'active' &&
    base.active_deployment_url !== null
  ) {
    return 'Inspect the deployed site and keep proof receipts current as follow-up work lands.'
  }

  if (buildValidation?.customer_safe_next_action !== undefined) {
    return buildValidation.customer_safe_next_action
  }

  if (compatibility?.customer_safe_next_action !== undefined) {
    return compatibility.customer_safe_next_action
  }

  if (version?.build_status === 'saved') {
    return 'Review the saved version, then deploy it when the customer-visible proof is approved.'
  }

  if (base.site_id !== null) {
    return 'Run compatibility and build validation, then save a reviewable version.'
  }

  return 'Create the public site project and attach an Adjutant assignment.'
}

const caveats = (
  base: BaseRow,
  research: ResearchRow | null,
  buildValidation: BuildValidationRow | null,
): ReadonlyArray<string> => [
  'This proof page only exposes public-safe OpenAgents records.',
  'Raw Exa payloads, private source notes, provider account details, and runner payloads are intentionally omitted.',
  ...(research === null || research.approved_source_count === 0
    ? ['Research sources are not represented as approved public evidence yet.']
    : []),
  ...(buildValidation === null
    ? ['Build validation has not been recorded for this public proof yet.']
    : []),
  ...(base.active_deployment_status === 'active'
    ? []
    : ['No active deployment is claimed until a deployment receipt and URL are present.']),
]

const claimFromRows = (
  base: BaseRow,
  research: ResearchRow | null,
  version: VersionRow | null,
  buildValidation: BuildValidationRow | null,
  receiptRefs: ReadonlyArray<string>,
) =>
  publicClaimStateProjection({
    desiredState:
      base.active_deployment_status === 'active'
        ? 'verified'
        : buildValidation?.status === 'passed' || version?.build_status === 'saved'
          ? 'measured'
          : 'planned',
    evidenceRefs: evidenceRefs([
      `order:${base.order_id}`,
      base.site_slug === null ? null : `site:${base.site_slug}`,
      research?.run_id === undefined ? null : `research:${research.run_id}`,
      version?.id === undefined ? null : `version:${version.id}`,
      buildValidation?.id === undefined
        ? null
        : `build_validation:${buildValidation.id}`,
      ...receiptRefs,
      base.active_deployment_url,
    ]),
    kind: 'deployment',
  })

const claimRecord = (
  input: Readonly<{
    claimId: string
    claimKind: PublicClaimProjectionRecord['claimKind']
    claimRef: string
    desiredState: PublicClaimProjectionRecord['desiredState']
    evidenceRefs: ReadonlyArray<string>
    subjectRef: string
    surface: PublicClaimProjectionRecord['surface']
    titleRef: string
    updatedAt: string
  }>,
): PublicClaimProjectionRecord => ({
  caveatRefs: ['caveat.public_otec_proof.public_safe_only'],
  claimId: input.claimId,
  claimKind: input.claimKind,
  claimRef: input.claimRef,
  customerRefs: [],
  desiredState: input.desiredState,
  evidenceRefs: input.evidenceRefs,
  operatorRefs: [],
  sourceRefs: ['source.public_otec_proof.closeout'],
  subjectRef: input.subjectRef,
  surface: input.surface,
  teamRefs: [],
  titleRef: input.titleRef,
  updatedAt: input.updatedAt,
})

const claimProjectionRecordsFromRows = (
  base: BaseRow,
  research: ResearchRow | null,
  latestVersion: VersionRow | null,
  buildValidation: BuildValidationRow | null,
  publicReceiptRefs: ReadonlyArray<string>,
  updatedAt: string,
): ReadonlyArray<PublicClaimProjectionRecord> => [
  claimRecord({
    claimId: 'claim_otec_closeout_overall',
    claimKind: 'deployment',
    claimRef: 'claim.otec.closeout.overall',
    desiredState:
      base.active_deployment_status === 'active'
        ? 'verified'
        : buildValidation?.status === 'passed' || latestVersion?.build_status === 'saved'
          ? 'measured'
          : 'planned',
    evidenceRefs: closeoutEvidenceRefs(
      base,
      research,
      latestVersion,
      buildValidation,
      publicReceiptRefs,
    ),
    subjectRef: 'site:ben-otec',
    surface: 'site',
    titleRef: 'title.otec.closeout.overall',
    updatedAt,
  }),
  claimRecord({
    claimId: 'claim_otec_site_url',
    claimKind: 'site_url',
    claimRef: 'claim.otec.site_url',
    desiredState: base.active_deployment_status === 'active'
      ? 'verified'
      : 'planned',
    evidenceRefs: evidenceRefs([
      base.site_slug === null ? null : `site:${base.site_slug}`,
      base.active_deployment_url,
    ]),
    subjectRef: 'site:ben-otec',
    surface: 'site',
    titleRef: 'title.otec.site_url',
    updatedAt,
  }),
  claimRecord({
    claimId: 'claim_otec_research',
    claimKind: 'research',
    claimRef: 'claim.otec.research',
    desiredState:
      research !== null && research.approved_source_count > 0
        ? 'verified'
        : 'planned',
    evidenceRefs: evidenceRefs([
      research?.run_id === undefined ? null : `research:${research.run_id}`,
      research?.research_brief_id === undefined
        ? null
        : `research_brief:${research.research_brief_id}`,
    ]),
    subjectRef: 'research:otec',
    surface: 'public_agent',
    titleRef: 'title.otec.research',
    updatedAt,
  }),
  claimRecord({
    claimId: 'claim_otec_latest_saved_version',
    claimKind: 'saved_version',
    claimRef: 'claim.otec.latest_saved_version',
    desiredState:
      latestVersion?.build_status === 'saved' ? 'verified' : 'planned',
    evidenceRefs: evidenceRefs([
      latestVersion?.id === undefined ? null : `version:${latestVersion.id}`,
      siteVersionUrl(base.site_slug, latestVersion?.id ?? null),
    ]),
    subjectRef: 'site:ben-otec',
    surface: 'site',
    titleRef: 'title.otec.latest_saved_version',
    updatedAt,
  }),
  claimRecord({
    claimId: 'claim_otec_active_deployment',
    claimKind: 'deployment',
    claimRef: 'claim.otec.active_deployment',
    desiredState: base.active_deployment_status === 'active'
      ? 'verified'
      : 'planned',
    evidenceRefs: evidenceRefs([
      base.active_deployment_id === null
        ? null
        : `deployment:${base.active_deployment_id}`,
      base.active_deployment_url,
    ]),
    subjectRef: 'site:ben-otec',
    surface: 'site',
    titleRef: 'title.otec.active_deployment',
    updatedAt,
  }),
  claimRecord({
    claimId: 'claim_otec_public_receipts',
    claimKind: 'fulfillment_receipt',
    claimRef: 'claim.otec.public_receipts',
    desiredState: publicReceiptRefs.length > 0 ? 'verified' : 'planned',
    evidenceRefs: publicReceiptRefs,
    subjectRef: 'order:ben-otec',
    surface: 'order',
    titleRef: 'title.otec.public_receipts',
    updatedAt,
  }),
]

const publicClaimProjectionsFromRows = (
  base: BaseRow,
  research: ResearchRow | null,
  latestVersion: VersionRow | null,
  buildValidation: BuildValidationRow | null,
  publicReceiptRefs: ReadonlyArray<string>,
  updatedAt: string,
): Effect.Effect<ReadonlyArray<PublicClaimProjection>, PublicOtecProofUnsafe> =>
  Effect.try({
    try: () =>
      claimProjectionRecordsFromRows(
        base,
        research,
        latestVersion,
        buildValidation,
        publicReceiptRefs,
        updatedAt,
      ).map(record => projectPublicClaimRecord(record, 'public')),
    catch: () => new PublicOtecProofUnsafe(),
  })

const proofFromRows = (
  base: BaseRow,
  research: ResearchRow | null,
  latestVersion: VersionRow | null,
  compatibility: CompatibilityRow | null,
  buildValidation: BuildValidationRow | null,
  receiptRows: ReadonlyArray<ReceiptRow>,
): Effect.Effect<PublicOtecProofCloseout, PublicOtecProofUnsafe> =>
  Effect.gen(function* () {
  const publicReceiptRefs = receiptRefs(receiptRows)
  const updatedAt = newestTimestamp([
    base.order_updated_at,
    base.site_updated_at,
    base.assignment_updated_at,
    base.active_deployment_updated_at,
    research?.updated_at,
    latestVersion?.saved_at,
    latestVersion?.created_at,
  ])
  const closeoutRefs = closeoutEvidenceRefs(
    base,
    research,
    latestVersion,
    buildValidation,
    publicReceiptRefs,
  )
  const claimProjections = yield* publicClaimProjectionsFromRows(
    base,
    research,
    latestVersion,
    buildValidation,
    publicReceiptRefs,
    updatedAt,
  )
  const overallClaimState = claimFromRows(
    base,
    research,
    latestVersion,
    buildValidation,
    publicReceiptRefs,
  )
  const proof = new PublicOtecProofCloseout({
    slug: 'ben-otec',
    orderId: base.order_id,
    title: 'Ben OTEC Site Proof Closeout',
    customerSafeStatus: customerStatus(
      base,
      buildValidation,
      compatibility,
      latestVersion,
    ),
    nextAction: nextAction(base, buildValidation, compatibility, latestVersion),
    order: new PublicOtecProofOrder({
      id: base.order_id,
      status: base.order_status,
      requestSummary: boundedSummary(base.order_request),
      repositoryFullName: base.repository_full_name,
      updatedAt: base.order_updated_at,
    }),
    site: new PublicOtecProofSite({
      id: base.site_id,
      title: base.site_title,
      slug: base.site_slug,
      status: base.site_status,
      activeUrl:
        base.active_deployment_status === 'active'
          ? base.active_deployment_url
          : null,
      activeVersionId: base.active_version_id,
      activeDeploymentId: base.active_deployment_id,
      claimState: publicClaimStateProjection({
        desiredState: base.active_deployment_status === 'active' ? 'verified' : 'planned',
        evidenceRefs: evidenceRefs([
          base.site_slug === null ? null : `site:${base.site_slug}`,
          base.active_deployment_url,
        ]),
        kind: 'site_url',
      }),
    }),
    assignment: new PublicOtecProofAssignment({
      id: base.assignment_id,
      kind: base.assignment_kind,
      status: base.assignment_status,
      currentRunId: base.assignment_current_run_id,
    }),
    research: new PublicOtecProofResearch({
      runId: research?.run_id ?? null,
      briefId: research?.research_brief_id ?? null,
      status: research?.status ?? 'not_recorded',
      sourceCount: research?.source_count ?? 0,
      approvedSourceCount: research?.approved_source_count ?? 0,
      claimState: publicClaimStateProjection({
        desiredState:
          research !== null && research.approved_source_count > 0
            ? 'verified'
            : 'planned',
        evidenceRefs: evidenceRefs([
          research?.run_id === undefined ? null : `research:${research.run_id}`,
          research?.research_brief_id === undefined
            ? null
            : `research_brief:${research.research_brief_id}`,
        ]),
        kind: 'research',
      }),
    }),
    version: new PublicOtecProofVersion({
      activeVersionId: base.active_version_id,
      latestSavedVersionId:
        latestVersion?.build_status === 'saved' ? latestVersion.id : null,
      activeVersionUrl: siteVersionUrl(base.site_slug, base.active_version_id),
      latestSavedVersionUrl: latestVersion?.build_status === 'saved'
        ? siteVersionUrl(base.site_slug, latestVersion.id)
        : null,
      versionRefs: evidenceRefs([
        base.active_version_id === null
          ? null
          : `version:${base.active_version_id}`,
        latestVersion?.id === undefined ? null : `version:${latestVersion.id}`,
        siteVersionUrl(base.site_slug, base.active_version_id),
        siteVersionUrl(base.site_slug, latestVersion?.id ?? null),
      ]),
      latestBuildStatus: latestVersion?.build_status ?? null,
      sourceCommitSha: latestVersion?.source_commit_sha ?? null,
      claimState: publicClaimStateProjection({
        desiredState:
          latestVersion?.build_status === 'saved' ? 'verified' : 'planned',
        evidenceRefs: evidenceRefs([
          latestVersion?.id === undefined ? null : `version:${latestVersion.id}`,
        ]),
        kind: 'saved_version',
      }),
    }),
    compatibility: new PublicOtecProofCompatibility({
      latestCheckId: compatibility?.id ?? null,
      status: compatibility?.status ?? null,
      blockerCount:
        compatibility === null ? 0 : parseArrayCount(compatibility.blockers_json),
      warningCount:
        compatibility === null ? 0 : parseArrayCount(compatibility.warnings_json),
      customerSafeStatus: compatibility?.customer_safe_status ?? null,
      customerSafeNextAction: compatibility?.customer_safe_next_action ?? null,
    }),
    buildValidation: new PublicOtecProofBuildValidation({
      latestValidationId: buildValidation?.id ?? null,
      status: buildValidation?.status ?? null,
      sourceHash: buildValidation?.source_hash ?? null,
      blockerCount:
        buildValidation === null
          ? 0
          : parseArrayCount(buildValidation.blockers_json),
      warningCount:
        buildValidation === null
          ? 0
          : parseArrayCount(buildValidation.warnings_json),
      customerSafeStatus: buildValidation?.customer_safe_status ?? null,
      customerSafeNextAction: buildValidation?.customer_safe_next_action ?? null,
    }),
    deployment: new PublicOtecProofDeployment({
      id: base.active_deployment_id,
      status: base.active_deployment_status,
      url:
        base.active_deployment_status === 'active'
          ? base.active_deployment_url
          : null,
      claimState: publicClaimStateProjection({
        desiredState: base.active_deployment_status === 'active' ? 'verified' : 'planned',
        evidenceRefs: evidenceRefs([
          base.active_deployment_id === null
            ? null
            : `deployment:${base.active_deployment_id}`,
          base.active_deployment_url,
        ]),
        kind: 'deployment',
      }),
    }),
    receipts: new PublicOtecProofReceipts({
      usageReceiptCount: publicReceiptRefs.length,
      publicReceiptRefs,
      acceptedWorkSettlementRefs: [],
      paymentCaveats: paymentCaveats(publicReceiptRefs),
    }),
    claimProjections,
    siteUrlRefs: evidenceRefs([base.active_deployment_url]),
    revisionUrlRefs: evidenceRefs([
      siteVersionUrl(base.site_slug, base.active_version_id),
      siteVersionUrl(base.site_slug, latestVersion?.id ?? null),
    ]),
    evidenceRefs: closeoutRefs,
    referralCta: publicSiteReferralCta({
      publicSourceRef: 'site_ref_otec_ben',
      siteSlug: base.site_slug,
      siteTitle: base.site_title,
    }),
    agentInstructionCard: publicSiteAgentInstructionCard({
      preset: 'proof_and_challenge',
      proofUrl: 'https://openagents.com/api/public/proof/otec',
      publicSourceRef: 'site_ref_otec_ben',
      siteSlug: base.site_slug,
      siteTitle: base.site_title,
      siteUrl:
        base.active_deployment_status === 'active'
          ? base.active_deployment_url
          : null,
    }),
    agentChallenges: publicSiteAgentChallenges({
      proofUrl: 'https://openagents.com/api/public/proof/otec',
      siteSlug: base.site_slug,
      siteTitle: base.site_title,
    }),
    claimState: overallClaimState,
    caveats: caveats(base, research, buildValidation),
    updatedAt,
  })

  if (valueHasPrivateMaterial(proof)) {
    return yield* new PublicOtecProofUnsafe()
  }

  return proof
})

export const publicOtecProofCloseout = (
  db: D1Database,
  softwareOrderId = OTEC_SOFTWARE_ORDER_ID,
): Effect.Effect<
  PublicOtecProofCloseout,
  PublicOtecProofError
> =>
  Effect.gen(function* () {
    const base = yield* d1Effect('publicOtecProof.base', () =>
    db
      .prepare(
        `SELECT software_orders.id AS order_id,
                software_orders.status AS order_status,
                software_orders.request AS order_request,
                software_orders.repository_full_name AS repository_full_name,
                software_orders.updated_at AS order_updated_at,
                site_projects.id AS site_id,
                site_projects.slug AS site_slug,
                site_projects.title AS site_title,
                site_projects.status AS site_status,
                site_projects.visibility AS site_visibility,
                site_projects.access_mode AS site_access_mode,
                site_projects.active_version_id AS active_version_id,
                site_projects.active_deployment_id AS active_deployment_id,
                site_projects.updated_at AS site_updated_at,
                active_deployments.url AS active_deployment_url,
                active_deployments.status AS active_deployment_status,
                active_deployments.updated_at AS active_deployment_updated_at,
                adjutant_assignments.id AS assignment_id,
                adjutant_assignments.assignment_kind AS assignment_kind,
                adjutant_assignments.status AS assignment_status,
                adjutant_assignments.current_run_id AS assignment_current_run_id,
                adjutant_assignments.updated_at AS assignment_updated_at
           FROM software_orders
           LEFT JOIN site_projects
             ON site_projects.software_order_id = software_orders.id
            AND site_projects.archived_at IS NULL
            AND site_projects.visibility = 'public'
            AND site_projects.access_mode = 'public'
           LEFT JOIN site_deployments AS active_deployments
             ON active_deployments.id = site_projects.active_deployment_id
           LEFT JOIN adjutant_assignments
             ON adjutant_assignments.software_order_id = software_orders.id
            AND adjutant_assignments.visibility = 'public'
            AND adjutant_assignments.archived_at IS NULL
          WHERE software_orders.id = ?
            AND software_orders.visibility = 'public'
            AND software_orders.archived_at IS NULL
          ORDER BY adjutant_assignments.updated_at DESC
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<BaseRow>(),
    )

    if (base === null) {
      return yield* new PublicOtecProofNotFound()
    }

    const research = yield* d1Effect('publicOtecProof.research', () =>
      db
        .prepare(
          `SELECT exa_enrichment_runs.id AS run_id,
                  exa_enrichment_runs.status AS status,
                  exa_enrichment_runs.source_count AS source_count,
                  exa_enrichment_runs.approved_source_count AS approved_source_count,
                  exa_enrichment_runs.updated_at AS updated_at,
                  adjutant_assignment_enrichments.research_brief_id AS research_brief_id,
                  adjutant_assignment_enrichments.approved_at AS approved_at
             FROM exa_enrichment_runs
             LEFT JOIN adjutant_assignment_enrichments
               ON adjutant_assignment_enrichments.enrichment_run_id = exa_enrichment_runs.id
            WHERE exa_enrichment_runs.software_order_id = ?
              AND exa_enrichment_runs.archived_at IS NULL
            ORDER BY COALESCE(adjutant_assignment_enrichments.approved_at, exa_enrichment_runs.completed_at, exa_enrichment_runs.updated_at) DESC
            LIMIT 1`,
        )
        .bind(base.order_id)
        .first<ResearchRow>(),
    )

    const latestVersion =
      base.site_id === null
        ? null
        : yield* d1Effect('publicOtecProof.latestVersion', () =>
            db
              .prepare(
                `SELECT id,
                        build_status,
                        source_commit_sha,
                        saved_at,
                        created_at
                   FROM site_versions
                  WHERE site_id = ?
                  ORDER BY created_at DESC
                  LIMIT 1`,
              )
              .bind(base.site_id)
              .first<VersionRow>(),
          )

    const compatibility =
      base.site_id === null
        ? null
        : yield* d1Effect('publicOtecProof.compatibility', () =>
            db
              .prepare(
                `SELECT id,
                        status,
                        blockers_json,
                        warnings_json,
                        customer_safe_status,
                        customer_safe_next_action
                   FROM site_compatibility_checks
                  WHERE site_id = ?
                    AND archived_at IS NULL
                  ORDER BY created_at DESC
                  LIMIT 1`,
              )
              .bind(base.site_id)
              .first<CompatibilityRow>(),
          )

    const buildValidation =
      base.site_id === null
        ? null
        : yield* d1Effect('publicOtecProof.buildValidation', () =>
            db
              .prepare(
                `SELECT id,
                        status,
                        source_hash,
                        blockers_json,
                        warnings_json,
                        customer_safe_status,
                        customer_safe_next_action
                   FROM site_build_validations
                  WHERE site_id = ?
                    AND archived_at IS NULL
                  ORDER BY created_at DESC
                  LIMIT 1`,
              )
              .bind(base.site_id)
              .first<BuildValidationRow>(),
          )

    const receiptRows = yield* d1Effect('publicOtecProof.receipts', () =>
      db
        .prepare(
          `SELECT id
             FROM adjutant_usage_receipts
            WHERE software_order_id = ?
              AND visibility = 'public'
            ORDER BY created_at DESC
            LIMIT 25`,
        )
        .bind(base.order_id)
        .all<ReceiptRow>(),
    ).pipe(Effect.map(result => result.results))

    return yield* proofFromRows(
      base,
      research,
      latestVersion,
      compatibility,
      buildValidation,
      receiptRows,
    )
  })
