import { describe, expect, test } from 'vitest'

import {
  type ArtanisSubQueryExecutionTarget,
  ArtanisSubQueryDecompositionParseError,
  parseArtanisSubQueryDecomposition,
} from './artanis-subquery-decomposition'

type TestSubQuery = {
  dependsOnSubQueryIds: string[]
  evidenceRefs: string[]
  maxTokens?: number
  purposeRef: string
  selectedProgramSignatureIds: string[]
  subQueryId: string
  target: ArtanisSubQueryExecutionTarget
  text: string
}

type TestDecomposition = {
  decompositionRef: string
  objectiveRef: string
  rootQuery: string
  sourceRefs: string[]
  subQueries: TestSubQuery[]
}

const validDecomposition = (): TestDecomposition => ({
  decompositionRef: 'decomposition.public.artanis.frlm.issue_6680',
  objectiveRef: 'objective.public.issue_6680',
  rootQuery:
    'Decompose the public issue into governed Blueprint-signature work steps.',
  sourceRefs: [
    'issue.github.openagents.6680',
    'doc.public.artanis.blueprint_autonomous_ops_v1',
  ],
  subQueries: [
    {
      dependsOnSubQueryIds: [],
      evidenceRefs: ['evidence.public.issue_6680.read'],
      maxTokens: 4096,
      purposeRef: 'purpose.public.frlm.collect_context',
      selectedProgramSignatureIds: [
        'program_signature.artanis.frlm.context_retrieval.v1',
      ],
      subQueryId: 'collect_context',
      target: 'context_retrieval',
      text: 'Collect public issue and Blueprint registry context.',
    },
    {
      dependsOnSubQueryIds: ['collect_context'],
      evidenceRefs: ['evidence.public.blueprint.signature_lookup'],
      purposeRef: 'purpose.public.frlm.select_signatures',
      selectedProgramSignatureIds: [
        'program_signature.artanis.frlm.signature_selection.v1',
      ],
      subQueryId: 'select_signatures',
      target: 'blueprint_signature_lookup',
      text: 'Resolve the governed Blueprint signatures for each sub-query.',
    },
    {
      dependsOnSubQueryIds: ['collect_context', 'select_signatures'],
      evidenceRefs: ['evidence.public.frlm.leaf_executor_ready'],
      purposeRef: 'purpose.public.frlm.execute_leaf',
      selectedProgramSignatureIds: [
        'program_signature.artanis.frlm.leaf_executor.v1',
        'program_signature.artanis.frlm.signature_selection.v1',
      ],
      subQueryId: 'execute_leaf',
      target: 'local_rlm_executor',
      text: 'Run the bounded leaf executor and return redacted evidence only.',
    },
  ],
})

describe('Artanis sub-query decomposition parser', () => {
  test('parses fenced FRLM JSON and preserves Blueprint signature refs', () => {
    const parsed = parseArtanisSubQueryDecomposition(
      `\n\`\`\`json\n${JSON.stringify(validDecomposition())}\n\`\`\`\n`,
    )

    expect(parsed.actionSubmissionRequiredForDirectEffects).toBe(true)
    expect(parsed.decompositionRef).toBe(
      'decomposition.public.artanis.frlm.issue_6680',
    )
    expect(parsed.subQueries.map(step => step.subQueryId)).toEqual([
      'collect_context',
      'select_signatures',
      'execute_leaf',
    ])
    expect(parsed.selectedProgramSignatureIds).toEqual([
      'program_signature.artanis.frlm.context_retrieval.v1',
      'program_signature.artanis.frlm.signature_selection.v1',
      'program_signature.artanis.frlm.leaf_executor.v1',
    ])
  })

  test('accepts object input and normalizes repeated refs', () => {
    const input = validDecomposition()
    input.sourceRefs.push('issue.github.openagents.6680')
    input.subQueries[2]?.selectedProgramSignatureIds.push(
      'program_signature.artanis.frlm.leaf_executor.v1',
    )

    const parsed = parseArtanisSubQueryDecomposition(input)

    expect(parsed.sourceRefs).toEqual([
      'issue.github.openagents.6680',
      'doc.public.artanis.blueprint_autonomous_ops_v1',
    ])
    expect(parsed.subQueries[2]?.selectedProgramSignatureIds).toEqual([
      'program_signature.artanis.frlm.leaf_executor.v1',
      'program_signature.artanis.frlm.signature_selection.v1',
    ])
  })

  test('rejects steps without Blueprint Program Signature refs', () => {
    const input = validDecomposition()
    const firstStep = requiredStep(input, 0)
    input.subQueries[0] = {
      ...firstStep,
      selectedProgramSignatureIds: ['signature.keyword.context'],
    }

    expect(() => parseArtanisSubQueryDecomposition(input)).toThrow(
      ArtanisSubQueryDecompositionParseError,
    )
  })

  test('rejects unknown and cyclic sub-query dependencies', () => {
    const unknownDependency = validDecomposition()
    const secondStep = requiredStep(unknownDependency, 1)
    unknownDependency.subQueries[1] = {
      ...secondStep,
      dependsOnSubQueryIds: ['missing_context'],
    }

    expect(() => parseArtanisSubQueryDecomposition(unknownDependency)).toThrow(
      ArtanisSubQueryDecompositionParseError,
    )

    const cyclic = validDecomposition()
    const firstStep = requiredStep(cyclic, 0)
    cyclic.subQueries[0] = {
      ...firstStep,
      dependsOnSubQueryIds: ['execute_leaf'],
    }

    expect(() => parseArtanisSubQueryDecomposition(cyclic)).toThrow(
      ArtanisSubQueryDecompositionParseError,
    )
  })

  test('rejects private paths, raw traces, and secret-like material', () => {
    const input = validDecomposition()
    const firstStep = requiredStep(input, 0)
    input.subQueries[0] = {
      ...firstStep,
      text: 'Read /Users/operator/.codex/auth.json for the missing token.',
    }

    expect(() => parseArtanisSubQueryDecomposition(input)).toThrow(
      ArtanisSubQueryDecompositionParseError,
    )
  })

  test('rejects invalid JSON before schema validation', () => {
    expect(() => parseArtanisSubQueryDecomposition('not json')).toThrow(
      ArtanisSubQueryDecompositionParseError,
    )
  })
})

const requiredStep = (
  decomposition: TestDecomposition,
  index: number,
): TestSubQuery => {
  const step = decomposition.subQueries[index]
  if (step === undefined) {
    throw new Error(`missing test sub-query at index ${index}`)
  }
  return step
}
