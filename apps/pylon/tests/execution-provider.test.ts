import { describe, expect, test } from "bun:test"

import {
  describeExecutionProvider,
  PYLON_EXECUTION_PROVIDER_KINDS,
} from "../src/execution-provider"

describe("Pylon execution providers", () => {
  test("describes every provider kind", () => {
    for (const kind of PYLON_EXECUTION_PROVIDER_KINDS) {
      expect(() => describeExecutionProvider(kind)).not.toThrow()
    }
  })

  test("marks local process as not supporting remote runs", () => {
    expect(describeExecutionProvider("local_process").features.remoteRun).toBe(
      false,
    )
  })

  test("marks OpenAgents Cloud as supporting sync", () => {
    expect(describeExecutionProvider("openagents_cloud").features.sync).toBe(
      true,
    )
  })

  test("exports all provider kinds", () => {
    expect(PYLON_EXECUTION_PROVIDER_KINDS).toHaveLength(3)
  })
})
