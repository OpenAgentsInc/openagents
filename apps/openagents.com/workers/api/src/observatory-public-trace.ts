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
  'assurance.receipt.24dfeb00c5e6f763ae51f45a15fddc86129dce4c18a1f8baa249f422110fd110',
  'assurance.receipt.1203db184f75dad4df64a8418f2b58ce151d0ae285221126a7c8d6397383dc44',
  'assurance.receipt.9af50c648771a500d98dc48938df4052d3f41cf4b1d9d70742e03c804dda4b7f',
  'assurance.receipt.5bb1083a7a096efca73112fa7baae665dd6effdc255c58ab5e6accb466781b7c',
  'assurance.receipt.faaa2143820e2708bf12fbdd3657c22a4c6581845a24375ea8c02b866a60ad5d',
  'assurance.receipt.91348ebfbe8217d46f60ca9e664b86dbd0be683e9673080027636c8ad36b61e2',
  'assurance.receipt.88d50c291a3403f6eaad261cd2eb53da6a287561d67aeed46e6e71810d21dfdd',
  'assurance.receipt.6cf2c13851ab4060e2a5ca0b3d9ed49394aa11ce6a233c1a16eeb2c3fcdbca99',
  'assurance.receipt.2f50de8ae5bdc501d7db492cd147f80117abe133995caf3b1108c68433b520cc',
  'assurance.receipt.91d533a15bb3c80b1dfadb0723377cb09de7610eb23ffdde2fa4dc2dfed6895f',
  'assurance.receipt.bbfb72594c66683757490c4a1118a7b44996978b300a75a9a1da9e48d52e3792',
  'assurance.receipt.5a68a62451fd1f7ad4263c3b767a16841f29bf473cc2413c62e493a89dfaad61',
  'assurance.receipt.7f656a8e3886c15e57ecaaa13784bb40fe325c93c434a22441855a323c5d4145',
  'assurance.receipt.e9d2f9c8befeaacd3aa7c74d483de70e311a546104d5b20fe988ba8c180bb179',
  'assurance.receipt.c480eb496a511fb3d3c1fd8f75929ee5828234abb3d7231dbbe0c40090e0fd0b',
  'assurance.receipt.112d8f6062ed6d6bebb10ee3b9ddd24842da1b08d82ba8cf2c190c8dd10126d2',
  'assurance.receipt.abac3671bb706ad478efd0e21d2247f844a58c3b0946b5e7f5a88465f4625829',
  'assurance.receipt.77daae5b21404c56fcad17e68dd4e04c7bd45acf57933aef4a8f73a177978d0e',
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
      'sha256:ecd42d941f4c48c7c370e5805960090030f6a0d516cb787a7229e24598a62eb9',
    projectionRef: 'trace.openagents-desktop-codex-workroom-mvp.v1',
    publicOptIn: {
      optInRef: 'opt-in.observatory.openagents-desktop-mvp.v1',
      state: 'opted_in',
    },
    publicationReview: {
      reviewRef: 'review.observatory.openagents-desktop-mvp.v1',
      reviewedProjectionDigest:
        'sha256:ecd42d941f4c48c7c370e5805960090030f6a0d516cb787a7229e24598a62eb9',
      state: 'approved',
    },
    schema: 'openagents.observatory.public_trace.v1',
    visibility: 'public',
  })
