import { Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'

export const ArtanisSubQueryExecutionTarget = S.Literals([
  'blueprint_signature_lookup',
  'context_retrieval',
  'local_rlm_executor',
  'nip90_market',
])
export type ArtanisSubQueryExecutionTarget =
  typeof ArtanisSubQueryExecutionTarget.Type

export class ArtanisSubQueryDecompositionStep extends S.Class<ArtanisSubQueryDecompositionStep>(
  'ArtanisSubQueryDecompositionStep',
)({
  dependsOnSubQueryIds: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  maxTokens: S.optional(S.Int),
  purposeRef: S.String,
  selectedProgramSignatureIds: S.Array(S.String),
  subQueryId: S.String,
  target: ArtanisSubQueryExecutionTarget,
  text: S.String,
}) {}

export class ArtanisSubQueryDecompositionInput extends S.Class<ArtanisSubQueryDecompositionInput>(
  'ArtanisSubQueryDecompositionInput',
)({
  decompositionRef: S.String,
  objectiveRef: S.String,
  rootQuery: S.String,
  sourceRefs: S.Array(S.String),
  subQueries: S.Array(ArtanisSubQueryDecompositionStep),
}) {}

export class ArtanisParsedSubQueryDecomposition extends S.Class<ArtanisParsedSubQueryDecomposition>(
  'ArtanisParsedSubQueryDecomposition',
)({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  decompositionRef: S.String,
  objectiveRef: S.String,
  rootQuery: S.String,
  selectedProgramSignatureIds: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  subQueries: S.Array(ArtanisSubQueryDecompositionStep),
}) {}

export class ArtanisSubQueryDecompositionParseError extends S.TaggedErrorClass<ArtanisSubQueryDecompositionParseError>()(
  'ArtanisSubQueryDecompositionParseError',
  {
    reason: S.String,
  },
) {}

const MAX_ROOT_QUERY_LENGTH = 4_000
const MAX_SUB_QUERY_TEXT_LENGTH = 2_000
const MAX_SUB_QUERY_COUNT = 32
const MAX_REF_COUNT = 64
const MAX_TOKENS_PER_SUB_QUERY = 64_000

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const subQueryIdPattern = /^[a-z][a-z0-9_-]{0,63}$/
const programSignatureRefPattern = /^program_signature\.[A-Za-z0-9_.:-]+$/
const unsafeMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed)|weights\.(bin|gguf|safetensors|pt|pth))/i

export const parseArtanisSubQueryDecomposition = (
  input: unknown,
): ArtanisParsedSubQueryDecomposition => {
  const decoded = decodeDecompositionInput(input)
  validateDecomposition(decoded)

  return new ArtanisParsedSubQueryDecomposition({
    actionSubmissionRequiredForDirectEffects: true,
    decompositionRef: decoded.decompositionRef.trim(),
    objectiveRef: decoded.objectiveRef.trim(),
    rootQuery: decoded.rootQuery.trim(),
    selectedProgramSignatureIds: uniqueRefs(
      decoded.subQueries.flatMap(step => step.selectedProgramSignatureIds),
    ),
    sourceRefs: uniqueRefs(decoded.sourceRefs),
    subQueries: decoded.subQueries.map(normalizeStep),
  })
}

const decodeDecompositionInput = (
  input: unknown,
): ArtanisSubQueryDecompositionInput => {
  const candidate = typeof input === 'string'
    ? parseJsonCandidate(input)
    : input

  try {
    return S.decodeUnknownSync(ArtanisSubQueryDecompositionInput)(candidate)
  } catch {
    throw new ArtanisSubQueryDecompositionParseError({
      reason: 'sub-query decomposition must match the Artanis FRLM JSON schema',
    })
  }
}

const parseJsonCandidate = (input: string): unknown => {
  const text = input.trim()

  if (text === '') {
    throw new ArtanisSubQueryDecompositionParseError({
      reason: 'sub-query decomposition JSON is empty',
    })
  }

  const fenced = text.match(/^```(?:json|JSON)?\s*([\s\S]*?)\s*```$/)
  const jsonText = fenced?.[1] ?? extractJsonObject(text)

  try {
    return parseJsonUnknown(jsonText)
  } catch {
    throw new ArtanisSubQueryDecompositionParseError({
      reason: 'sub-query decomposition is not valid JSON',
    })
  }
}

const extractJsonObject = (text: string): string => {
  const start = text.indexOf('{')
  const end = text.lastIndexOf('}')

  if (start === -1 || end === -1 || end <= start) {
    throw new ArtanisSubQueryDecompositionParseError({
      reason: 'sub-query decomposition must contain one JSON object',
    })
  }

  return text.slice(start, end + 1)
}

const validateDecomposition = (
  decomposition: ArtanisSubQueryDecompositionInput,
): void => {
  assertSafeRef('decompositionRef', decomposition.decompositionRef)
  assertSafeRef('objectiveRef', decomposition.objectiveRef)
  assertSafeText('rootQuery', decomposition.rootQuery, MAX_ROOT_QUERY_LENGTH)
  assertRefList('sourceRefs', decomposition.sourceRefs, 1)

  if (
    decomposition.subQueries.length === 0 ||
    decomposition.subQueries.length > MAX_SUB_QUERY_COUNT
  ) {
    throw parseError(
      `subQueries must contain between 1 and ${MAX_SUB_QUERY_COUNT} steps`,
    )
  }

  const ids = new Set<string>()

  for (const step of decomposition.subQueries) {
    const id = step.subQueryId.trim()
    if (!subQueryIdPattern.test(id)) {
      throw parseError('subQueryId must be a stable lowercase scheduler id')
    }

    if (ids.has(id)) {
      throw parseError('subQueryId values must be unique')
    }

    ids.add(id)
    assertSafeRef('purposeRef', step.purposeRef)
    assertSafeText('subQuery.text', step.text, MAX_SUB_QUERY_TEXT_LENGTH)
    assertRefList('subQuery.evidenceRefs', step.evidenceRefs, 1)
    assertProgramSignatureRefs(step.selectedProgramSignatureIds)

    if (
      step.maxTokens !== undefined &&
      (step.maxTokens < 1 || step.maxTokens > MAX_TOKENS_PER_SUB_QUERY)
    ) {
      throw parseError(
        `maxTokens must be between 1 and ${MAX_TOKENS_PER_SUB_QUERY}`,
      )
    }
  }

  for (const step of decomposition.subQueries) {
    for (const dependencyId of step.dependsOnSubQueryIds) {
      if (dependencyId === step.subQueryId) {
        throw parseError('subQueries must not depend on themselves')
      }

      if (!ids.has(dependencyId)) {
        throw parseError('subQuery dependency ids must reference known steps')
      }
    }
  }

  assertAcyclic(decomposition.subQueries)
}

const normalizeStep = (
  step: ArtanisSubQueryDecompositionStep,
): ArtanisSubQueryDecompositionStep =>
  new ArtanisSubQueryDecompositionStep({
    dependsOnSubQueryIds: uniqueRefs(step.dependsOnSubQueryIds),
    evidenceRefs: uniqueRefs(step.evidenceRefs),
    maxTokens: step.maxTokens,
    purposeRef: step.purposeRef.trim(),
    selectedProgramSignatureIds: uniqueRefs(step.selectedProgramSignatureIds),
    subQueryId: step.subQueryId.trim(),
    target: step.target,
    text: step.text.trim(),
  })

const assertRefList = (
  label: string,
  refs: ReadonlyArray<string>,
  minimumCount: number,
): void => {
  if (refs.length < minimumCount || refs.length > MAX_REF_COUNT) {
    throw parseError(
      `${label} must contain between ${minimumCount} and ${MAX_REF_COUNT} refs`,
    )
  }

  for (const ref of refs) {
    assertSafeRef(label, ref)
  }
}

const assertProgramSignatureRefs = (
  refs: ReadonlyArray<string>,
): void => {
  assertRefList('selectedProgramSignatureIds', refs, 1)

  for (const ref of refs) {
    if (!programSignatureRefPattern.test(ref.trim())) {
      throw parseError(
        'selectedProgramSignatureIds must be Blueprint Program Signature refs',
      )
    }
  }
}

const assertSafeRef = (label: string, ref: string): void => {
  const trimmed = ref.trim()
  if (
    trimmed === '' ||
    !safeRefPattern.test(trimmed) ||
    unsafeMaterialPattern.test(trimmed)
  ) {
    throw parseError(
      `${label} contains unsafe private, secret, wallet, provider, raw trace, or local path material`,
    )
  }
}

const assertSafeText = (
  label: string,
  text: string,
  maxLength: number,
): void => {
  const trimmed = text.trim()
  if (
    trimmed === '' ||
    trimmed.length > maxLength ||
    unsafeMaterialPattern.test(trimmed)
  ) {
    throw parseError(
      `${label} must be non-empty, bounded, and free of private or secret material`,
    )
  }
}

const assertAcyclic = (
  steps: ReadonlyArray<ArtanisSubQueryDecompositionStep>,
): void => {
  const dependenciesById = new Map(
    steps.map(step => [
      step.subQueryId,
      new Set(step.dependsOnSubQueryIds),
    ]),
  )
  const visiting = new Set<string>()
  const visited = new Set<string>()

  const visit = (id: string): void => {
    if (visited.has(id)) {
      return
    }

    if (visiting.has(id)) {
      throw parseError('subQuery dependencies must form an acyclic graph')
    }

    visiting.add(id)
    for (const dependencyId of dependenciesById.get(id) ?? []) {
      visit(dependencyId)
    }
    visiting.delete(id)
    visited.add(id)
  }

  for (const step of steps) {
    visit(step.subQueryId)
  }
}

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))]

const parseError = (
  reason: string,
): ArtanisSubQueryDecompositionParseError =>
  new ArtanisSubQueryDecompositionParseError({ reason })
