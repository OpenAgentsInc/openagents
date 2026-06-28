import { createHash } from 'node:crypto'

import { isBlueprintProjectionPrivateDataSafe } from '@openagentsinc/blueprint-contracts'
import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Option, Schema as S } from 'effect'

export const ARTANIS_SUB_QUERY_DECOMPOSITION_SCHEMA =
  'openagents.artanis.sub_query_decomposition.v0.1'

export const ArtanisSubQueryKind = S.Literals([
  'collect_context',
  'inspect_blueprint_signature',
  'decompose_task',
  'execute_leaf_query',
  'compose_answer',
  'verify_evidence',
])
export type ArtanisSubQueryKind = typeof ArtanisSubQueryKind.Type

export const ArtanisSubQueryExecutor = S.Literals([
  'local',
  'pylon',
  'swarm',
  'codex',
  'remote',
])
export type ArtanisSubQueryExecutor = typeof ArtanisSubQueryExecutor.Type

export const ArtanisSubQueryCandidate = S.Struct({
  blueprintSignatureRef: S.optional(S.String),
  dependsOn: S.optional(S.Array(S.String)),
  evidenceRefs: S.optional(S.Array(S.String)),
  executor: S.optional(ArtanisSubQueryExecutor),
  kind: ArtanisSubQueryKind,
  objective: S.String,
  subQueryRef: S.optional(S.String),
})
export type ArtanisSubQueryCandidate = typeof ArtanisSubQueryCandidate.Type

const ArtanisSubQueryCandidateList = S.Array(ArtanisSubQueryCandidate)
const ArtanisSubQueryCandidateEnvelope = S.Struct({
  subQueries: ArtanisSubQueryCandidateList,
})

export type ArtanisSubQueryDecompositionBlockerRef =
  | 'blocker.artanis.sub_query_decomposition.root_task_ref_missing'
  | 'blocker.artanis.sub_query_decomposition.blueprint_signature_ref_missing'
  | 'blocker.artanis.sub_query_decomposition.plan_missing'
  | 'blocker.artanis.sub_query_decomposition.parse_failed'
  | 'blocker.artanis.sub_query_decomposition.unsafe_ref'
  | 'blocker.artanis.sub_query_decomposition.unsafe_content'

export type ArtanisParsedSubQuery = Readonly<{
  subQueryRef: string
  parentRef: string
  kind: ArtanisSubQueryKind
  executor: ArtanisSubQueryExecutor
  objectiveDigestRef: string
  blueprintSignatureRef: string
  dependsOn: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
}>

export type ArtanisSubQueryDecompositionProjection = Readonly<{
  schema: typeof ARTANIS_SUB_QUERY_DECOMPOSITION_SCHEMA
  observedAt: string
  rootTaskRef: string | null
  rootObjectiveDigestRef: string | null
  blueprintSignatureRefs: ReadonlyArray<string>
  subQueries: ReadonlyArray<ArtanisParsedSubQuery>
  recursiveSubQueryRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<ArtanisSubQueryDecompositionBlockerRef>
  canSchedule: boolean
  releaseGateRefs: ReadonlyArray<string>
  authorityBoundary: string
  contentRedacted: true
}>

export type ParseArtanisSubQueryDecompositionInput = Readonly<{
  observedAt: string
  rootTaskRef?: string | null
  rootObjective: string
  blueprintSignatureRefs?: ReadonlyArray<string>
  decomposition: unknown
}>

const publicRefPattern = /^[a-z][a-z0-9._:/-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i

const defaultLeafSignatureRef = 'program_signature.rlm_leaf_executor.v1'
const conductorSignatureRef = 'program_signature.frlm_conductor.v1'

const decodeCandidateList = S.decodeUnknownOption(ArtanisSubQueryCandidateList)
const decodeCandidateEnvelope = S.decodeUnknownOption(
  ArtanisSubQueryCandidateEnvelope,
)

export const parseArtanisSubQueryDecomposition = (
  input: ParseArtanisSubQueryDecompositionInput,
): ArtanisSubQueryDecompositionProjection => {
  const blockerRefs = new Set<ArtanisSubQueryDecompositionBlockerRef>()
  const rootTaskRef = safeRef(input.rootTaskRef)
  const rootObjectiveDigestRef =
    input.rootObjective.trim() === ''
      ? null
      : `digest.artanis.root_objective.${stableHash(input.rootObjective)}`
  const blueprintSignatureRefs = uniqueRefs([
    conductorSignatureRef,
    ...(input.blueprintSignatureRefs ?? []),
  ].map(ref => safeRef(ref)))

  if (rootTaskRef === null) {
    blockerRefs.add('blocker.artanis.sub_query_decomposition.root_task_ref_missing')
  }
  if (blueprintSignatureRefs.length === 0) {
    blockerRefs.add(
      'blocker.artanis.sub_query_decomposition.blueprint_signature_ref_missing',
    )
  }

  const parseResult = parseCandidates(input.decomposition)
  if (!parseResult.ok) {
    blockerRefs.add('blocker.artanis.sub_query_decomposition.parse_failed')
  }

  const candidates = parseResult.ok ? parseResult.candidates : []
  if (
    containsProviderSecretMaterial(JSON.stringify(input.decomposition)) ||
    containsProviderSecretMaterial(input.rootObjective) ||
    !isBlueprintProjectionPrivateDataSafe(input.decomposition)
  ) {
    blockerRefs.add('blocker.artanis.sub_query_decomposition.unsafe_content')
  }

  const subQueries = candidates.flatMap((candidate, index) => {
    const parsed = parseCandidate({
      candidate,
      index,
      rootTaskRef,
      rootObjective: input.rootObjective,
      signatureFallback: blueprintSignatureRefs.includes(defaultLeafSignatureRef)
        ? defaultLeafSignatureRef
        : blueprintSignatureRefs[0] ?? defaultLeafSignatureRef,
    })
    if (!parsed.safe) {
      blockerRefs.add('blocker.artanis.sub_query_decomposition.unsafe_ref')
      return []
    }
    return [parsed.subQuery]
  })

  if (subQueries.length === 0) {
    blockerRefs.add('blocker.artanis.sub_query_decomposition.plan_missing')
  }

  const recursiveSubQueryRefs = subQueries.map(subQuery => subQuery.subQueryRef)
  const evidenceRefs = uniqueRefs([
    rootTaskRef,
    rootObjectiveDigestRef,
    ...blueprintSignatureRefs,
    ...recursiveSubQueryRefs,
    ...subQueries.flatMap(subQuery => [
      subQuery.blueprintSignatureRef,
      subQuery.objectiveDigestRef,
      ...subQuery.dependsOn,
      ...subQuery.evidenceRefs,
    ]),
  ])

  const projection: ArtanisSubQueryDecompositionProjection = {
    authorityBoundary:
      'Read-only Artanis RLM/FRLM sub-query decomposition parser. It normalizes public-safe Blueprint-governed sub-query refs only; it does not dispatch workers, execute code, spend sats, publish claims, or grant settlement authority.',
    blockerRefs: [...blockerRefs].sort(),
    blueprintSignatureRefs,
    canSchedule: blockerRefs.size === 0,
    contentRedacted: true,
    evidenceRefs,
    observedAt: input.observedAt,
    recursiveSubQueryRefs,
    releaseGateRefs: [
      'release_gate.artanis.sub_query_decomposition.public_refs_only',
      'release_gate.blueprint.signature_lookup.safe_projection',
    ],
    rootObjectiveDigestRef,
    rootTaskRef,
    schema: ARTANIS_SUB_QUERY_DECOMPOSITION_SCHEMA,
    subQueries,
  }

  assertProjectionSafe(projection)
  return projection
}

const parseCandidate = (input: {
  candidate: ArtanisSubQueryCandidate
  index: number
  rootTaskRef: string | null
  rootObjective: string
  signatureFallback: string
}): { safe: true; subQuery: ArtanisParsedSubQuery } | { safe: false } => {
  const objective = input.candidate.objective.trim()
  const subQueryRef = safeRef(
    input.candidate.subQueryRef ??
      `subquery.artanis.rlm.${input.candidate.kind}.${stableHash(
        `${input.rootTaskRef ?? 'missing'}:${input.index}:${objective}`,
      )}`,
  )
  const parentRef = input.rootTaskRef
  const blueprintSignatureRef = safeRef(
    input.candidate.blueprintSignatureRef ?? input.signatureFallback,
  )
  const dependsOn = uniqueRefs((input.candidate.dependsOn ?? []).map(safeRef))
  const evidenceRefs = uniqueRefs((input.candidate.evidenceRefs ?? []).map(safeRef))

  if (
    subQueryRef === null ||
    parentRef === null ||
    blueprintSignatureRef === null ||
    objective === '' ||
    containsProviderSecretMaterial(objective) ||
    !isBlueprintProjectionPrivateDataSafe(objective)
  ) {
    return { safe: false }
  }

  return {
    safe: true,
    subQuery: {
      blueprintSignatureRef,
      dependsOn,
      evidenceRefs,
      executor: input.candidate.executor ?? executorForKind(input.candidate.kind),
      kind: input.candidate.kind,
      objectiveDigestRef: `digest.artanis.sub_query_objective.${stableHash(objective)}`,
      parentRef,
      subQueryRef,
    },
  }
}

const parseCandidates = (
  value: unknown,
): { ok: true; candidates: ReadonlyArray<ArtanisSubQueryCandidate> } | { ok: false } => {
  const directList = Option.getOrUndefined(decodeCandidateList(value))
  if (directList !== undefined) return { candidates: directList, ok: true }

  const envelope = Option.getOrUndefined(decodeCandidateEnvelope(value))
  if (envelope !== undefined) return { candidates: envelope.subQueries, ok: true }

  if (typeof value !== 'string') return { ok: false }

  const trimmed = value.trim()
  if (trimmed === '') return { ok: false }

  try {
    const parsed = JSON.parse(trimmed) as unknown
    return parseCandidates(parsed)
  } catch {
    const candidates = parseMarkdownCandidates(trimmed)
    return candidates.length === 0 ? { ok: false } : { candidates, ok: true }
  }
}

const parseMarkdownCandidates = (
  value: string,
): ReadonlyArray<ArtanisSubQueryCandidate> =>
  value
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => /^[-*]\s+/.test(line))
    .map(line => line.replace(/^[-*]\s+/, '').trim())
    .flatMap((line): ReadonlyArray<ArtanisSubQueryCandidate> => {
      const match = /^(?<kind>[a-z_]+)\s*:\s*(?<objective>[^|]+)(?<meta>.*)$/i.exec(line)
      const kindText = match?.groups?.kind
      const objectiveText = match?.groups?.objective
      const metaText = match?.groups?.meta ?? ''
      if (kindText === undefined || objectiveText === undefined) return []
      const kind = normalizeKind(kindText)
      const objective = objectiveText.trim()
      if (kind === null || objective === '') return []
      const meta = parseMetadata(metaText)
      const candidate: ArtanisSubQueryCandidate = {
        dependsOn: meta.depends,
        evidenceRefs: meta.evidence,
        kind,
        objective,
        ...(meta.signature === undefined
          ? {}
          : { blueprintSignatureRef: meta.signature }),
        ...(meta.executor === undefined ? {} : { executor: meta.executor }),
        ...(meta.ref === undefined ? {} : { subQueryRef: meta.ref }),
      }
      return [
        candidate,
      ]
    })

const normalizeKind = (value: string): ArtanisSubQueryKind | null => {
  switch (value.trim().toLowerCase().replaceAll('-', '_')) {
    case 'collect_context':
    case 'context':
      return 'collect_context'
    case 'inspect_blueprint_signature':
    case 'signature':
      return 'inspect_blueprint_signature'
    case 'decompose_task':
    case 'decompose':
      return 'decompose_task'
    case 'execute_leaf_query':
    case 'execute':
    case 'leaf':
      return 'execute_leaf_query'
    case 'compose_answer':
    case 'compose':
      return 'compose_answer'
    case 'verify_evidence':
    case 'verify':
      return 'verify_evidence'
    default:
      return null
  }
}

const parseMetadata = (
  value: string,
): Readonly<{
  depends: ReadonlyArray<string>
  evidence: ReadonlyArray<string>
  executor: ArtanisSubQueryExecutor | undefined
  ref: string | undefined
  signature: string | undefined
}> => {
  const entries = value
    .split('|')
    .map(entry => entry.trim())
    .filter(Boolean)
  const record = new Map<string, string>()
  for (const entry of entries) {
    const separator = entry.indexOf('=')
    if (separator > 0) {
      record.set(
        entry.slice(0, separator).trim().toLowerCase(),
        entry.slice(separator + 1).trim(),
      )
    }
  }
  return {
    depends: splitRefs(record.get('depends') ?? record.get('depends_on')),
    evidence: splitRefs(record.get('evidence') ?? record.get('evidence_refs')),
    executor: parseExecutor(record.get('executor')),
    ref: record.get('ref'),
    signature: record.get('signature') ?? record.get('blueprint_signature'),
  }
}

const parseExecutor = (value: string | undefined): ArtanisSubQueryExecutor | undefined => {
  const normalized = value?.trim()
  switch (normalized) {
    case 'local':
    case 'pylon':
    case 'swarm':
    case 'codex':
    case 'remote':
      return normalized
    default:
      return undefined
  }
}

const splitRefs = (value: string | undefined): ReadonlyArray<string> =>
  value
    ?.split(',')
    .map(ref => ref.trim())
    .filter(Boolean) ?? []

const executorForKind = (kind: ArtanisSubQueryKind): ArtanisSubQueryExecutor => {
  switch (kind) {
    case 'collect_context':
    case 'inspect_blueprint_signature':
    case 'decompose_task':
    case 'compose_answer':
    case 'verify_evidence':
      return 'local'
    case 'execute_leaf_query':
      return 'pylon'
  }
}

const safeRef = (value: string | null | undefined): string | null => {
  if (value === null || value === undefined) return null
  const trimmed = value.trim()
  if (trimmed === '') return null
  return publicRefPattern.test(trimmed) && !unsafeRefPattern.test(trimmed)
    ? trimmed
    : null
}

const uniqueRefs = (refs: ReadonlyArray<string | null>): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== null))].sort()

const stableHash = (value: string): string =>
  createHash('sha256').update(value).digest('hex').slice(0, 20)

const assertProjectionSafe = (
  projection: ArtanisSubQueryDecompositionProjection,
): void => {
  const serialized = JSON.stringify(projection)
  if (
    containsProviderSecretMaterial(serialized) ||
    !isBlueprintProjectionPrivateDataSafe(projection)
  ) {
    throw new Error('Artanis sub-query decomposition projection is not public-safe')
  }
}
