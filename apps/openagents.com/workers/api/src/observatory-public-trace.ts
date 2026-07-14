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
  'assurance.receipt.c7d0aa02dea8b6d9126bd94fb0aada527b8ec43376b4a4236d5ee86974fc6c4e',
  'assurance.receipt.284fb0d2eb37800acee0b51620c2e369aaaaae0bc09d0048646ce00c886af2c1',
  'assurance.receipt.f5140c692c5ebf49042f46e5102ae616df7919447272c174236de3f556bf9b2a',
  'assurance.receipt.ad9412c3ca6a944387abeb257e7abf8ef8e222b77ed0ab674e6c837c050ee8e8',
  'assurance.receipt.360b84f2cc5219ac1f5847573ae42f7560d9c2ba52ae70456d13ec8e3715705e',
  'assurance.receipt.b7efdd67389093c2a5c581afb6f0590c00b7ac8050f021044cdc19c3cbb95c9e',
  'assurance.receipt.8a5c1cbd228fc5deb95c24e0353d3fd779a0a6ce62f91735f1efc29187d1ea38',
  'assurance.receipt.2682001a7d1833aa3b9e1fbfde9792cac7743bfb9b98a5d8adf076698866c37e',
  'assurance.receipt.91c37d948cf14953645783c01ac486dcd8c38115bd4893abb15d76f3b239764a',
  'assurance.receipt.d9113a79acf44d7d74e2a6ba0b83ec72dd0fd79f01121ceff9cbfe00d27fb81e',
  'assurance.receipt.1ff256914e82ec269038fb9eedf436cf23706bb702702664e270ed38e8b61702',
  'assurance.receipt.eca1784e51ed56b50791006336a9aceef192b38e6067489515efd759c688b1db',
  'assurance.receipt.5cd22951116bed310be4d29325258f0fafa87934ee95367c99d4327438f19242',
  'assurance.receipt.89b0e0251a3d538c97cbb424c2fbdc6078ddf293ae788c3608b8990e755e3ece',
  'assurance.receipt.9a381993568732e6d0068ddfc34362ce4a5ecb45d8209cbbbcfeeb6ddd226cd0',
  'assurance.receipt.51d92c28ec5a0620e3d280695731623e643f74fec8ec25ef17f18c6fe75a1342',
  'assurance.receipt.9b3ffcf9c16cd0d1e185ef06858e620ef8b21305ebe33f0db2cb06e98dda1254',
  'assurance.receipt.5f16c8ef6a5573a8b7061f90700f798171ac29403e13a3f05b6c12686837824c',
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
      'sha256:836da81944eb217cecf59c864c9552413047384aec6dd69a49f431ab7b47caeb',
    projectionRef: 'trace.openagents-desktop-codex-workroom-mvp.v1',
    publicOptIn: {
      optInRef: 'opt-in.observatory.openagents-desktop-mvp.v1',
      state: 'opted_in',
    },
    publicationReview: {
      reviewRef: 'review.observatory.openagents-desktop-mvp.v1',
      reviewedProjectionDigest:
        'sha256:836da81944eb217cecf59c864c9552413047384aec6dd69a49f431ab7b47caeb',
      state: 'approved',
    },
    schema: 'openagents.observatory.public_trace.v1',
    visibility: 'public',
  })
