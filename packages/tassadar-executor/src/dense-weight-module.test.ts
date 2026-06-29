import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { collectInterpreterOutputs } from "./numeric-executor.js"
import {
  TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND,
  TassadarDenseModuleError,
  denseWeightModuleToNumericModel,
  executeTassadarDenseWeightModule,
  tassadarDenseProgramFixture,
  tassadarDenseWeightModuleDigest,
  tassadarDenseWeightModuleTraceDigest,
  type TassadarDenseProgramFixture,
} from "./dense-weight-module.js"

const fixtureFile = JSON.parse(
  readFileSync(
    new URL(
      "../fixtures/tassadar-dense-weight-module-v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
) as TassadarDenseProgramFixture

describe("Tassadar dense weight module", () => {
  test("matches the psionic-generated fixture metadata", () => {
    expect(tassadarDenseProgramFixture).toEqual(fixtureFile)
    expect(tassadarDenseProgramFixture.programId).toBe(
      "tassadar_corpus.loop_sum_v1",
    )
    expect(tassadarDenseProgramFixture.denseModule.moduleKind).toBe(
      TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND,
    )
    expect(tassadarDenseProgramFixture.denseModuleDigest).toBe(
      "cfda0fe5dcf42e16db9e18696731427f0f30915fd3100d38da2dcc8411433e2c",
    )
    expect(tassadarDenseWeightModuleDigest).toBe(
      tassadarDenseProgramFixture.denseModuleDigest,
    )
    expect(tassadarDenseWeightModuleTraceDigest).toBe(
      tassadarDenseProgramFixture.expectedTraceDigest,
    )
  })

  test("executes the dense module through the numeric replay path", async () => {
    const trace = await executeTassadarDenseWeightModule(
      tassadarDenseProgramFixture.denseModule,
      tassadarDenseProgramFixture.steps,
    )
    const { outputs, halted } = collectInterpreterOutputs(trace.stepOutputs)

    expect(trace.traceDigest).toBe(tassadarDenseProgramFixture.expectedTraceDigest)
    expect(trace.traceDigest).toBe(
      "2465d2c2af5077b4cf44c6eddbdc5aba2859029e30062f49a30e669acfc8e9d2",
    )
    expect(halted).toBe(true)
    expect(outputs.map(value => Number(value))).toEqual([15])
  })

  test("decodes loadable dense WQ/WK/WV and FFN matrices", () => {
    const module = tassadarDenseProgramFixture.denseModule
    const dModel = module.dModel
    expect(module.attentionBlocks.length).toBeGreaterThan(0)
    expect(module.ffnBlocks.length).toBeGreaterThan(0)
    for (const block of module.attentionBlocks) {
      expect(block.wQ).toHaveLength(block.heads.length)
      expect(block.wK).toHaveLength(block.heads.length)
      expect(block.wV).toHaveLength(block.heads.length)
      expect(block.wO).toHaveLength(dModel)
      expect(block.wQ.every(row => row.length === dModel)).toBe(true)
      expect(block.wK.every(row => row.length === dModel)).toBe(true)
      expect(block.wV.every(row => row.length === dModel)).toBe(true)
      expect(block.wO.every(row => row.length === block.heads.length)).toBe(
        true,
      )
    }
    const decoded = denseWeightModuleToNumericModel(module)
    expect(decoded.graph_digest).toBe(module.graphDigest)
    expect(module.sourceModelDigest).toBe(
      tassadarDenseProgramFixture.numericModelDigest,
    )
  })

  test("malformed dense matrices refuse before execution", () => {
    const copy = JSON.parse(
      JSON.stringify(tassadarDenseProgramFixture),
    ) as TassadarDenseProgramFixture
    const firstFfnBlock = copy.denseModule.ffnBlocks[0]
    if (firstFfnBlock === undefined) {
      throw new Error("missing FFN block")
    }
    ;(firstFfnBlock.wValue[0] as number[]).push(0)
    expect(() => denseWeightModuleToNumericModel(copy.denseModule)).toThrow(
      TassadarDenseModuleError,
    )
  })
})
