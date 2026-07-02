import { describe, expect, test } from "bun:test"
import fc, { type AsyncCommand } from "fast-check"

import {
  appServerRestartModelCommand,
  appServerStartModelCommand,
  appServerStopModelCommand,
  archiveThreadModelCommand,
  answerApprovalModelCommand,
  completeTurnModelCommand,
  decodeKhalaCodeQaModelRunReport,
  decodeKhalaCodeQaModelState,
  delegateProgramModelCommand,
  deleteThreadModelCommand,
  forkThreadModelCommand,
  initialKhalaCodeQaModelReport,
  initialKhalaCodeQaModelState,
  interruptApprovalModelCommand,
  interruptTurnModelCommand,
  makeKhalaCodeQaSeedCorpusFixtureFetch,
  makeKhalaCodeRpcQaDriver,
  startThreadModelCommand,
  supersedeApprovalModelCommand,
  unarchiveThreadModelCommand,
  type KhalaCodeQaModelRuntime,
  type KhalaCodeQaModelState,
} from "./index.js"

type ModelCommand = AsyncCommand<KhalaCodeQaModelState, KhalaCodeQaModelRuntime>

const command = (
  label: string,
  run: (model: KhalaCodeQaModelState, runtime: KhalaCodeQaModelRuntime) => Promise<void> | void,
): ModelCommand => ({
  check: () => true,
  run: async (model, runtime) => {
    await run(model, runtime)
  },
  toString: () => label,
})

const modelCommands: fc.Arbitrary<ModelCommand>[] = [
  fc.constant(command("thread.start", startThreadModelCommand)),
  fc.constant(command("thread.turn.complete", completeTurnModelCommand)),
  fc.constant(command("thread.turn.interrupt", interruptTurnModelCommand)),
  fc.constant(command("thread.archive", archiveThreadModelCommand)),
  fc.constant(command("thread.unarchive", unarchiveThreadModelCommand)),
  fc.constant(command("thread.fork", forkThreadModelCommand)),
  fc.constant(command("thread.delete", deleteThreadModelCommand)),
  fc.constant(command("approval.accept", (model, runtime) => answerApproval("accept", model, runtime))),
  fc.constant(command("approval.reject", (model, runtime) => answerApproval("reject", model, runtime))),
  fc.constant(command("approval.supersede", supersedeApprovalModelCommand)),
  fc.constant(command("approval.turn_interrupted", interruptApprovalModelCommand)),
  fc.constant(command("fleet.delegate", delegateProgramModelCommand)),
  fc.constant(command("app_server.start", appServerStartModelCommand)),
  fc.constant(command("app_server.restart", appServerRestartModelCommand)),
  fc.constant(command("app_server.stop", appServerStopModelCommand)),
]
const answerApproval = answerApprovalModelCommand

describe("Khala Code QA model-based tier", () => {
  test("exports Effect Schema-decodable models and reports", () => {
    const model = initialKhalaCodeQaModelState()
    const report = initialKhalaCodeQaModelReport()

    expect(decodeKhalaCodeQaModelState(model).delegateProgram.modules.map((step) => step.module)).toEqual([
      "intake",
      "preflight",
      "capacity",
      "dispatch",
      "closeout",
      "report",
    ])
    expect(decodeKhalaCodeQaModelRunReport(report).schema).toBe("khala_code_qa_model_based_report.v1")
  })

  test("generates fast-check model command sequences against the fixture Mode P RPC driver", async () => {
    const commandSequence = fc.commands(modelCommands, { maxCommands: 32 })

    await fc.assert(
      fc.asyncProperty(commandSequence, async (commands) => {
        const report = initialKhalaCodeQaModelReport()
        const driver = makeKhalaCodeRpcQaDriver({
          baseUrl: "http://fixture.local",
          fetch: makeKhalaCodeQaSeedCorpusFixtureFetch(),
          now: () => "2026-07-01T00:00:00.000Z",
        })

        await fc.asyncModelRun(
          () => ({
            model: initialKhalaCodeQaModelState(),
            real: { driver, report },
          }),
          commands,
        )

        expect(report.divergences).toEqual([])
      }),
      { numRuns: 24, seed: 7856 },
    )
  })
})
