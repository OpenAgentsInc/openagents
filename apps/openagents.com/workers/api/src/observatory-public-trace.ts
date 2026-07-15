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
  'assurance.receipt.19b482bbea5bf7801cf449769296cb62f670d8a1572ffed487f9079ac3ed55b2',
  'assurance.receipt.c3a3392dc017eef38079beba2a9e01fa811076837850754418487412b9d1ff59',
  'assurance.receipt.ba2b7c9744cad1c1a99bee11b0b1fc81fb0690d5ee08ea727a761788a2be22ff',
  'assurance.receipt.989464aec5936ab70ebb2991aad714e868f96dd2afd861b126b6b59976257cb6',
  'assurance.receipt.d5b1dfd6d6a5bd224d6269d1b23c8e68d1b5ea0d372cbf0e5150015dc6225a16',
  'assurance.receipt.79dbf5df6d82f7235232b7a2597f1a5c40ab948db1989eccb18c7cb242b080b8',
  'assurance.receipt.e45f2a8bfea9b5c593560b1f36232317c37c79a2e48448eee325476059fe50d6',
  'assurance.receipt.36a94b8864d197db0771cc566697622d524f91db74703bf6acf04b80f87f55c1',
  'assurance.receipt.360b3ccf7d8dd2355688b93da42d75c4396a27a5b005a761dbe886ed37ecf89f',
  'assurance.receipt.4b30450674b96e6af2040cdffd272d12f1376382a559c1120a40061f5d0b776b',
  'assurance.receipt.7644fe0da56482960e0890bf4e51c960f040ec315a371012e0b28c9004ef01ef',
  'assurance.receipt.bd1c2b2b6d2e610bc92c527698a080be7d1a6f85cded9a99cbed2d6f14f7c0d3',
  'assurance.receipt.c3b66773010988342ce7935283d139c421c98972794886c28f6e366acb704ca8',
  'assurance.receipt.976b5d1d619ae95f9f156a9f80cf2c71f4ee30fd3f652e927a60409588540c1a',
  'assurance.receipt.b15bf1251ff3142641b7708de0067b0a33412e0e36a4dc3b6612f71e536cb847',
  'assurance.receipt.7096619aafe6068d023cad74eb33c5ac2d72a692a774874ff06ebb7bcfe70a6f',
  'assurance.receipt.89d13666520b2001eefe1dd4978c3eb67ccfcc5feb53f92e2b459a7b3d566606',
  'assurance.receipt.655c173f78b7b44fcdda19cbb30b4c64b51c6759c0a7099a12f479ab26a758b6',
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
      'sha256:64e4da124597597abeab53f0eeb4a4cf837de52a477d14d343c1d3e3a3a3317e',
    projectionRef: 'trace.openagents-desktop-codex-workroom-mvp.v1',
    publicOptIn: {
      optInRef: 'opt-in.observatory.openagents-desktop-mvp.v1',
      state: 'opted_in',
    },
    publicationReview: {
      reviewRef: 'review.observatory.openagents-desktop-mvp.v1',
      reviewedProjectionDigest:
        'sha256:64e4da124597597abeab53f0eeb4a4cf837de52a477d14d343c1d3e3a3a3317e',
      state: 'approved',
    },
    schema: 'openagents.observatory.public_trace.v1',
    visibility: 'public',
  })
