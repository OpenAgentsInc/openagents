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

/** Reviewed static snapshot. It deliberately reports the current proof gaps. */
export const openAgentsDesktopMvpPublicTrace =
  parseObservatoryPublicTraceProjection({
    assuranceProtocol: 'AssuranceSpec',
    criteria: criterionTitles.map((title, index) => {
      const criterionRef = `CW-AC-${String(index + 1).padStart(2, '0')}`
      return {
        accepted: {
          dispositionRefs: [],
          note: 'No acceptance disposition has been recorded.',
          state: 'pending',
        },
        criterionRef,
        executable: {
          adapterRefs: [],
          falsifierRefs: [],
          note: 'The proposal is not yet an admitted executable proof design.',
          oracleRefs: [],
          state: 'blocked',
        },
        mapped: {
          note: 'A proposal row exists, but no obligation set is admitted.',
          obligationRefs: [],
          state: 'missing',
        },
        observed: {
          note: 'No admitted obligation has run for this criterion.',
          receiptRefs: [],
          state: 'not_run',
        },
        relatedArtifacts: [
          {
            kind: 'product_spec',
            label: `${criterionRef} in ProductSpec`,
            url: productSpecUrl,
          },
          {
            kind: 'assurance_spec',
            label: 'AssuranceSpec proposal',
            url: assuranceSpecUrl,
          },
        ],
        title,
      }
    }),
    generatedAt: '2026-07-13T18:45:00.000Z',
    productSpecRef: 'docs/mvp/openagents-codex-workroom-mvp.product-spec.md',
    projectLabel: 'OpenAgents Desktop Codex Workroom MVP',
    projectRef: 'openagents-desktop-codex-workroom-mvp',
    projectionDigest:
      'sha256:5f4f12c8cb0857f1e135b9ae8e2e59215d74fd24348cd0c7fbd29b8bc0160b56',
    projectionRef: 'trace.openagents-desktop-codex-workroom-mvp.v1',
    publicOptIn: {
      optInRef: 'opt-in.observatory.openagents-desktop-mvp.v1',
      state: 'opted_in',
    },
    publicationReview: {
      reviewRef: 'review.observatory.openagents-desktop-mvp.v1',
      reviewedProjectionDigest:
        'sha256:5f4f12c8cb0857f1e135b9ae8e2e59215d74fd24348cd0c7fbd29b8bc0160b56',
      state: 'approved',
    },
    schema: 'openagents.observatory.public_trace.v1',
    visibility: 'public',
  })
