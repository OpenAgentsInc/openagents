import { describe, expect, test } from "bun:test"
import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"
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
  KhalaCodeQaModelDivergenceError,
  makeKhalaCodeRpcQaDriver,
  makeKhalaCodeRealAppRpcFetch,
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
  check: (model: Readonly<KhalaCodeQaModelState>) => boolean = () => true,
): ModelCommand => ({
  check,
  run: async (model, runtime) => {
    await run(model, runtime)
  },
  toString: () => label,
})

const modelCommands: fc.Arbitrary<ModelCommand>[] = [
  fc.constant(command("thread.start", startThreadModelCommand)),
  fc.constant(command(
    "thread.turn.complete",
    completeTurnModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted,
  )),
  fc.constant(command(
    "thread.turn.interrupt",
    interruptTurnModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted,
  )),
  fc.constant(command(
    "thread.archive",
    archiveThreadModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted && !model.thread.archived,
  )),
  fc.constant(command(
    "thread.unarchive",
    unarchiveThreadModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted && model.thread.archived,
  )),
  fc.constant(command(
    "thread.fork",
    forkThreadModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted,
  )),
  fc.constant(command(
    "thread.delete",
    deleteThreadModelCommand,
    model => model.thread.threadId !== null && !model.thread.deleted,
  )),
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
const artifactPath = join(process.cwd(), "artifacts", "model-divergences.jsonl")

const writeDivergenceArtifact = async (
  reports: readonly ReturnType<typeof initialKhalaCodeQaModelReport>[],
): Promise<void> => {
  await mkdir(join(process.cwd(), "artifacts"), { recursive: true })
  await writeFile(
    artifactPath,
    reports.map(report => JSON.stringify(report)).join("\n") + "\n",
  )
}

describe("Khala Code QA model-based tier", () => {
  test("exports Effect Schema-decodable models and reports", () => {
    const model = initialKhalaCodeQaModelState()
    const report = initialKhalaCodeQaModelReport()

    expect(decodeKhalaCodeQaModelState(model).delegateProgram.modules.map((step) => step.module)).toEqual([
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ])
    expect(decodeKhalaCodeQaModelRunReport(report).schema).toBe("khala_code_qa_model_based_report.v1")
  })

  test("generates fast-check model command sequences against the real desktop RPC handlers", async () => {
    const commandSequence = fc.commands(modelCommands, { maxCommands: 32 })
    const reports: ReturnType<typeof initialKhalaCodeQaModelReport>[] = []

    try {
      await fc.assert(
        fc.asyncProperty(commandSequence, async (commands) => {
          const report = initialKhalaCodeQaModelReport()
          reports.push(report)
          const realApp = await makeKhalaCodeRealAppRpcFetch()
          const driver = makeKhalaCodeRpcQaDriver({
            baseUrl: "http://real-app.local",
            fetch: realApp.fetch,
            now: () => "2026-07-01T00:00:00.000Z",
          })

          try {
            await fc.asyncModelRun(
              () => ({
                model: initialKhalaCodeQaModelState(),
                real: { driver, report },
              }),
              commands,
            )

            expect(report.divergences).toEqual([])
          } finally {
            realApp.dispose()
          }
        }),
        { numRuns: 24, seed: 7856 },
      )
    } catch (error) {
      if (error instanceof KhalaCodeQaModelDivergenceError) {
        reports.push(error.report)
      }
      throw error
    } finally {
      await writeDivergenceArtifact(reports)
    }
  }, 20_000)
})
