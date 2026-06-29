import type { TassadarAlmNumericModel } from "./numeric-executor.js"
import corpusData from "../fixtures/tassadar-compiled-program-corpus-v1.json" with { type: "json" }

export type TassadarProgramInstruction = Readonly<Record<string, unknown>>

export type TassadarProgramPayload = Readonly<{
  program_id: string
  profile_id: string
  local_count: number
  memory_slots: number
  initial_memory?: ReadonlyArray<number>
  instructions: ReadonlyArray<TassadarProgramInstruction>
}>

export type TassadarCompiledProgramFixture = Readonly<{
  fixtureId: string
  programId: string
  programDigest: string
  workloadKind: string
  profileId: string
  program: TassadarProgramPayload
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
  expectedTraceDigest: string
  expectedModelDigest: string
  expectedFinalRow: ReadonlyArray<number> | null
  expectedOutputs: ReadonlyArray<number>
  halted: boolean
  compileReceiptRefs: ReadonlyArray<string>
}>

export type TassadarCompiledProgramCorpus = Readonly<{
  schemaVersion: number
  corpusId: string
  generatedBy: string
  claimBoundary: string
  programCount: number
  fixtures: ReadonlyArray<TassadarCompiledProgramFixture>
  corpusDigest: string
}>

export const tassadarCompiledProgramCorpus: TassadarCompiledProgramCorpus =
  corpusData as unknown as TassadarCompiledProgramCorpus

const stableWorkloadIndex = (seed: string): number => {
  let hash = 2166136261
  for (let index = 0; index < seed.length; index += 1) {
    hash ^= seed.charCodeAt(index)
    hash = Math.imul(hash, 16777619) >>> 0
  }
  return hash % tassadarCompiledProgramCorpus.fixtures.length
}

const explicitWorkloadIndex = (assignmentRef: string): number | null => {
  const marker = ".w"
  const markerIndex = assignmentRef.lastIndexOf(marker)
  if (markerIndex < 0) {
    return null
  }
  const suffix = assignmentRef.slice(markerIndex + marker.length)
  if (!/^[0-9]+$/.test(suffix)) {
    return null
  }
  return Number(suffix) % tassadarCompiledProgramCorpus.fixtures.length
}

export const tassadarCompiledProgramCorpusSize =
  tassadarCompiledProgramCorpus.fixtures.length

export const selectTassadarCompiledProgramFixture = (input: Readonly<{
  assignmentRef: string
}>): TassadarCompiledProgramFixture => {
  const index =
    explicitWorkloadIndex(input.assignmentRef) ??
    (input.assignmentRef.startsWith("assignment.artanis_admin.")
      ? 0
      : stableWorkloadIndex(input.assignmentRef))
  return tassadarCompiledProgramCorpus.fixtures[index] ?? tassadarCompiledProgramCorpus.fixtures[0]!
}
