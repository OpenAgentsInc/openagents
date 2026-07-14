import { describe, expect, test } from "bun:test"

import {
  decodeKhalaCodeQaModelRunReport,
  decodeKhalaCodeQaModelState,
  initialKhalaCodeQaModelReport,
  initialKhalaCodeQaModelState,
} from "./model-based.js"

describe("Khala Code QA model-based tier", () => {
  test("exports Effect Schema-decodable models and reports", () => {
    const model = initialKhalaCodeQaModelState()
    const report = initialKhalaCodeQaModelReport()

    expect(decodeKhalaCodeQaModelState(model).delegateProgram.modules.map(step => step.module)).toEqual([
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ])
    expect(decodeKhalaCodeQaModelRunReport(report).schema).toBe("khala_code_qa_model_based_report.v1")
  })
})
