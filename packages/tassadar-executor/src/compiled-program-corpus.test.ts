import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import {
  collectInterpreterOutputs,
  executeTassadarNumericModel,
} from "./numeric-executor.js"
import {
  selectTassadarCompiledProgramFixture,
  tassadarCompiledProgramCorpus,
  tassadarCompiledProgramCorpusSize,
  type TassadarCompiledProgramCorpus,
} from "./compiled-program-corpus.js"

const fixtureFile = JSON.parse(
  readFileSync(
    new URL(
      "../fixtures/tassadar-compiled-program-corpus-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as TassadarCompiledProgramCorpus

describe("Tassadar compiled-program corpus", () => {
  test("matches the psionic-generated fixture metadata", () => {
    expect(tassadarCompiledProgramCorpus.schemaVersion).toBe(1)
    expect(tassadarCompiledProgramCorpus.corpusId).toBe(
      "tassadar_alm.numeric_program_corpus.v1",
    )
    expect(tassadarCompiledProgramCorpus.corpusDigest).toBe(
      "386799dbe6f6bd73093a546be16070fed0598e4d568378facccc4c6046ddad83",
    )
    expect(tassadarCompiledProgramCorpus).toEqual(fixtureFile)
    expect(tassadarCompiledProgramCorpusSize).toBe(4)
    expect(
      tassadarCompiledProgramCorpus.fixtures.map(fixture => fixture.programId),
    ).toEqual([
      "tassadar_corpus.loop_sum_v1",
      "tassadar_corpus.mul_add_v1",
      "tassadar_corpus.memory_roundtrip_v1",
      "tassadar_corpus.factorial_loop_v1",
    ])
  })

  test("executes every digest-pinned fixture with the TypeScript executor", async () => {
    for (const fixture of tassadarCompiledProgramCorpus.fixtures) {
      const trace = await executeTassadarNumericModel(
        fixture.model,
        fixture.steps,
      )
      const { outputs, halted } = collectInterpreterOutputs(trace.stepOutputs)

      expect(fixture.halted).toBe(true)
      expect(halted).toBe(true)
      expect(trace.traceDigest).toBe(fixture.expectedTraceDigest)
      expect(trace.graphDigest).toBe(fixture.model.graph_digest)
      expect(outputs.map(value => Number(value))).toEqual(
        [...fixture.expectedOutputs],
      )
      expect(fixture.compileReceiptRefs).toHaveLength(5)
    }
  })

  test("selects explicit workload suffixes as corpus round-robin slots", () => {
    expect(
      [0, 1, 2, 3].map(index =>
        selectTassadarCompiledProgramFixture({
          assignmentRef: `assignment.artanis_admin.2026061800000${index}.w${index}`,
        }).programId,
      ),
    ).toEqual([
      "tassadar_corpus.loop_sum_v1",
      "tassadar_corpus.mul_add_v1",
      "tassadar_corpus.memory_roundtrip_v1",
      "tassadar_corpus.factorial_loop_v1",
    ])
    expect(
      selectTassadarCompiledProgramFixture({
        assignmentRef: "assignment.artanis_admin.legacy",
      }).programId,
    ).toBe("tassadar_corpus.loop_sum_v1")
  })
})
