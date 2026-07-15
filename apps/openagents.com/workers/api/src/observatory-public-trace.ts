import { Schema as S } from 'effect'

export const ObservatoryVisibility = S.Literals([
  'private',
  'unlisted',
  'public',
])
export type ObservatoryVisibility = typeof ObservatoryVisibility.Type

export const ObservatoryMappedFact = S.Struct({
  note: S.String,
  obligationRefs: S.Array(S.String),
  state: S.Literals(['mapped', 'missing']),
})

export const ObservatoryExecutableFact = S.Struct({
  adapterRefs: S.Array(S.String),
  falsifierRefs: S.Array(S.String),
  note: S.String,
  oracleRefs: S.Array(S.String),
  state: S.Literals(['executable', 'blocked']),
})

export const ObservatoryObservedFact = S.Struct({
  note: S.String,
  receiptRefs: S.Array(S.String),
  state: S.Literals(['CONFIRMED', 'REFUTED', 'INCONCLUSIVE', 'not_run']),
})

export const ObservatoryAcceptedFact = S.Struct({
  dispositionRefs: S.Array(S.String),
  note: S.String,
  state: S.Literals(['accepted', 'rejected', 'pending']),
})

export const ObservatoryRelatedArtifact = S.Struct({
  kind: S.Literals(['product_spec', 'assurance_spec', 'evidence']),
  label: S.String,
  url: S.String,
})

export const ObservatoryCriterionTrace = S.Struct({
  accepted: ObservatoryAcceptedFact,
  criterionRef: S.String,
  executable: ObservatoryExecutableFact,
  mapped: ObservatoryMappedFact,
  observed: ObservatoryObservedFact,
  relatedArtifacts: S.Array(ObservatoryRelatedArtifact),
  title: S.String,
})

export const ObservatoryPublicTraceProjection = S.Struct({
  assuranceProtocol: S.Literal('AssuranceSpec'),
  criteria: S.Array(ObservatoryCriterionTrace),
  generatedAt: S.String,
  productSpecRef: S.String,
  projectLabel: S.String,
  projectRef: S.String,
  projectionDigest: S.String,
  projectionRef: S.String,
  publicOptIn: S.Struct({
    optInRef: S.String,
    state: S.Literal('opted_in'),
  }),
  publicationReview: S.Struct({
    reviewRef: S.String,
    reviewedProjectionDigest: S.String,
    state: S.Literal('approved'),
  }),
  schema: S.Literal('openagents.observatory.public_trace.v1'),
  visibility: ObservatoryVisibility,
})
export type ObservatoryPublicTraceProjection =
  typeof ObservatoryPublicTraceProjection.Type

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const SAFE_GITHUB_ARTIFACT_URL =
  /^https:\/\/github\.com\/OpenAgentsInc\/openagents\/(?:blob|tree)\/main\/[A-Za-z0-9_.\-/%]+$/
const PRIVATE_MATERIAL = [
  /(?:^|[\s"':])\/Users\//,
  /(?:api|access|auth|provider)[_-]?token/i,
  /bearer\s+[A-Za-z0-9._~-]+/i,
  /cookie/i,
  /email/i,
  /invoice/i,
  /mnemonic/i,
  /payment[_-]?(?:hash|preimage)/i,
  /private[_-]?(?:key|repo)/i,
  /raw[_-]?(?:log|payload|prompt|receipt|trace)/i,
  /secret/i,
  /sk-[A-Za-z0-9]/,
] as const

export class ObservatoryProjectionUnsafe extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ObservatoryProjectionUnsafe'
  }
}

/** Rejects unsafe source material before a schema decoder can strip it. */
export const assertObservatoryProjectionSourceSafe = (value: unknown): void => {
  const serialized = JSON.stringify(value)
  if (PRIVATE_MATERIAL.some(pattern => pattern.test(serialized))) {
    throw new ObservatoryProjectionUnsafe(
      'Observatory projection contains private or secret-shaped material.',
    )
  }
}

const assertSafeRef = (label: string, value: string): void => {
  if (!SAFE_REF.test(value)) {
    throw new ObservatoryProjectionUnsafe(`${label} is not a public-safe ref.`)
  }
}

export const parseObservatoryPublicTraceProjection = (
  candidate: unknown,
): ObservatoryPublicTraceProjection => {
  assertObservatoryProjectionSourceSafe(candidate)
  const projection = S.decodeUnknownSync(ObservatoryPublicTraceProjection)(
    candidate,
    { onExcessProperty: 'error' },
  )

  for (const [label, ref] of [
    ['productSpecRef', projection.productSpecRef],
    ['projectRef', projection.projectRef],
    ['projectionDigest', projection.projectionDigest],
    ['projectionRef', projection.projectionRef],
    ['publicOptIn.optInRef', projection.publicOptIn.optInRef],
    ['publicationReview.reviewRef', projection.publicationReview.reviewRef],
  ] as const) {
    assertSafeRef(label, ref)
  }
  if (!Number.isFinite(Date.parse(projection.generatedAt))) {
    throw new ObservatoryProjectionUnsafe(
      'generatedAt must be an ISO timestamp.',
    )
  }
  if (!/^sha256:[a-f0-9]{64}$/.test(projection.projectionDigest)) {
    throw new ObservatoryProjectionUnsafe(
      'projectionDigest must be a SHA-256 digest.',
    )
  }
  if (
    projection.projectionDigest !==
    projection.publicationReview.reviewedProjectionDigest
  ) {
    throw new ObservatoryProjectionUnsafe(
      'Publication review does not bind the projected artifact digest.',
    )
  }
  for (const criterion of projection.criteria) {
    assertSafeRef('criterionRef', criterion.criterionRef)
    for (const ref of [
      ...criterion.mapped.obligationRefs,
      ...criterion.executable.adapterRefs,
      ...criterion.executable.oracleRefs,
      ...criterion.executable.falsifierRefs,
      ...criterion.observed.receiptRefs,
      ...criterion.accepted.dispositionRefs,
    ]) {
      assertSafeRef('criterion fact ref', ref)
    }
    for (const artifact of criterion.relatedArtifacts) {
      if (!SAFE_GITHUB_ARTIFACT_URL.test(artifact.url)) {
        throw new ObservatoryProjectionUnsafe(
          'Related artifact URL is outside the reviewed public repository.',
        )
      }
    }
  }

  return projection
}

export type ObservatoryReadKind = 'discovery' | 'exact'

/** Private never leaves the public boundary; unlisted is exact-link only. */
export const admitObservatoryProjectionForPublicRead = (
  candidate: unknown,
  readKind: ObservatoryReadKind,
): ObservatoryPublicTraceProjection | undefined => {
  const projection = parseObservatoryPublicTraceProjection(candidate)
  if (projection.visibility === 'private') return undefined
  if (readKind === 'discovery' && projection.visibility !== 'public') {
    return undefined
  }
  return projection
}

/** Canonical reviewed bytes exclude only the digest and its review envelope. */
export const observatoryProjectionDigestPayload = (
  projection: ObservatoryPublicTraceProjection,
): string => {
  const {
    projectionDigest: _projectionDigest,
    publicationReview: _publicationReview,
    ...reviewedProjection
  } = projection
  return JSON.stringify(reviewedProjection)
}

export const observatoryProjectionDigestMatches = async (
  projection: ObservatoryPublicTraceProjection,
): Promise<boolean> => {
  if (!/^sha256:[a-f0-9]{64}$/.test(projection.projectionDigest)) return false
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(observatoryProjectionDigestPayload(projection)),
  )
  const actual = [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
  return projection.projectionDigest === `sha256:${actual}`
}

const criterionTitles = [
  'Signed release installs and launches',
  'Local-first Codex workroom',
  'Stable repository WorkContext',
  'Validator-clean ProductSpec authoring',
  'Digest-bound spec review',
  'Durable accepted work packets',
  'Pinned ProductSpec work skill',
  'Authority stays outside agent prose',
  'Typed active-spec mismatch',
  'Bounded session rail projection',
  'Durable admission before dispatch',
  'Idempotent retry reconciliation',
  'Complete causal child graph',
  'Bounded repository tools',
  'Host-owned work survives reload',
  'Convergent transport recovery',
  'Private content stays private',
  'Exact release acceptance run',
] as const

const productSpecUrl =
  'https://github.com/OpenAgentsInc/openagents/blob/main/docs/mvp/openagents-codex-workroom-mvp.product-spec.md'
const assuranceSpecUrl =
  'https://github.com/OpenAgentsInc/openagents/blob/main/docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md'
const evidenceIndexUrl =
  'https://github.com/OpenAgentsInc/openagents/blob/main/assurance/openagents-desktop-mvp.evidence-index.json'

const candidateReceiptRefs = [
  'assurance.receipt.7fbdc29a318e5889bdf23c121913efb2258fbbe56acae4371b965fd486eef357',
  'assurance.receipt.af14890a67cc5006268666f0d5a711474e2a764aa4f658486994021f9dec8b31',
  'assurance.receipt.78c914c0b138234b9eae7fc905e4850fc66207aa136fad2a7805961bc2bd5384',
  'assurance.receipt.1fccfe2533ef97e5ac567b497b7ee8d95c7a75859dbc203da288737d6748b44d',
  'assurance.receipt.52658c15a0dd52451e1c10d714254d3fd0c18c82cf1dbbff8c43e47946d7526f',
  'assurance.receipt.115e0905092b778c223369ddc0968b20226bcfa9ba02063bf1722d43f3103de0',
  'assurance.receipt.73194324f71aea4e3d813022c7da9449f12f4acb5bc7838852f60da11932abd5',
  'assurance.receipt.d8030ecba265ccb8aae63dc565e9d3df99ec8249a3fc7b89789d21eb47e265e3',
  'assurance.receipt.58d6b651f855c206fc70afa13510a8f3899d613ec476f9cc966e6a6cfaed2cb1',
  'assurance.receipt.57e75095f3f9c9a1dacc051aa6b86d83d9772323be80aaabb879e539fe860d28',
  'assurance.receipt.5b7771054f1280813742ebfc038efe61213e34905762dcb82ff1f7dd400c6a12',
  'assurance.receipt.2d42f0cd756071a6b2c91c639de4cc758880cd049b904bc39a3b6dcba18767ac',
  'assurance.receipt.cbea3de22bb51238b1e173522bb03d3d6f3c3119147e3799641282c1f60626b3',
  'assurance.receipt.a423bdbbf82cb179a0ff3d4b3542a0aecf7060f8f9cde8308ba91bf37979ede1',
  'assurance.receipt.81311ac33581c644ca4c825096d02af606e036db1880701f4b10e136636878d0',
  'assurance.receipt.3ff5f00f81f7147e11f360f88363ec7fcb28cbdf1698632679f84255b4153fa1',
  'assurance.receipt.d576703c171b487a0079ee900ed544f7d06f0cb5151c7bc66698d6b43b8786db',
  'assurance.receipt.6562b029fea3e85695f2cd444a88f68c425e1f8aba51a468a77135cd2fbf9500',
] as const

/** Reviewed static projection of the committed full-MVP Evidence Index. */
export const openAgentsDesktopMvpPublicTrace =
  parseObservatoryPublicTraceProjection({
    assuranceProtocol: 'AssuranceSpec',
    criteria: criterionTitles.map((title, index) => {
      const criterionRef = `CW-AC-${String(index + 1).padStart(2, '0')}`
      const obligationRef = `AO-${criterionRef}-01`
      const receiptRef = candidateReceiptRefs[index]
      if (receiptRef === undefined) {
        throw new ObservatoryProjectionUnsafe(
          `No reviewed receipt ref exists for ${criterionRef}.`,
        )
      }
      return {
        accepted: {
          dispositionRefs: [receiptRef],
          note: 'Independent review recorded an accepted evidence disposition.',
          state: 'accepted',
        },
        criterionRef,
        executable: {
          adapterRefs: ['openagents.bun_test.v1'],
          falsifierRefs: [`${obligationRef}.missing_required_anchor`],
          note: 'The admitted manifest binds a candidate oracle and deterministic falsifier.',
          oracleRefs: [`${obligationRef}.criterion_contract`],
          state: 'executable',
        },
        mapped: {
          note: 'The admitted AssuranceSpec maps this criterion to one exact obligation.',
          obligationRefs: [obligationRef],
          state: 'mapped',
        },
        observed: {
          note: 'The candidate was confirmed and its paired falsifier was rejected.',
          receiptRefs: [receiptRef],
          state: 'CONFIRMED',
        },
        relatedArtifacts: [
          {
            kind: 'product_spec',
            label: `${criterionRef} in ProductSpec`,
            url: productSpecUrl,
          },
          {
            kind: 'assurance_spec',
            label: 'Admitted AssuranceSpec',
            url: assuranceSpecUrl,
          },
          {
            kind: 'evidence',
            label: 'Reviewed Evidence Index',
            url: evidenceIndexUrl,
          },
        ],
        title,
      }
    }),
    generatedAt: '2026-07-14T01:30:00.000Z',
    productSpecRef: 'docs/mvp/openagents-codex-workroom-mvp.product-spec.md',
    projectLabel: 'OpenAgents Desktop Codex Workroom MVP',
    projectRef: 'openagents-desktop-codex-workroom-mvp',
    projectionDigest:
      'sha256:1ff9def30aaf7d97b0de0803c3158aacfeda8330f8d8817a2dedd9e89b6c4ca1',
    projectionRef: 'trace.openagents-desktop-codex-workroom-mvp.v1',
    publicOptIn: {
      optInRef: 'opt-in.observatory.openagents-desktop-mvp.v1',
      state: 'opted_in',
    },
    publicationReview: {
      reviewRef: 'review.observatory.openagents-desktop-mvp.v1',
      reviewedProjectionDigest:
        'sha256:1ff9def30aaf7d97b0de0803c3158aacfeda8330f8d8817a2dedd9e89b6c4ca1',
      state: 'approved',
    },
    schema: 'openagents.observatory.public_trace.v1',
    visibility: 'public',
  })
