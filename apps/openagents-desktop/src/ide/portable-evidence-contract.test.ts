import { describe, expect, test } from "vite-plus/test"

import { IdePortableEvidenceReceiptSchema } from "./portable-evidence-contract.ts"

describe("IDE-13 portability evidence contract", () => {
  test("cannot report full acceptance while required real cohorts remain absent", () => {
    expect(IdePortableEvidenceReceiptSchema.fields.acceptancePassed.ast).toBeDefined()
    expect(() => IdePortableEvidenceReceiptSchema.make({ acceptancePassed: true } as never)).toThrow()
  })
})
