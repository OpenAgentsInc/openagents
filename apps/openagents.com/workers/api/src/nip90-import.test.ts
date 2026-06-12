import { describe, expect, test } from "vitest"
import { KIND_JOB_TEXT_GENERATION, getResultKind, jobInput, makeJobRequest } from "@openagentsinc/nip90"

describe("API Worker NIP-90 package import", () => {
  test("can build a kind 5050 request through the shared protocol package", () => {
    const request = makeJobRequest({
      kind: KIND_JOB_TEXT_GENERATION,
      inputs: [jobInput.text("hello")],
    })

    expect(getResultKind(request.kind)).toBe(6050)
  })
})
