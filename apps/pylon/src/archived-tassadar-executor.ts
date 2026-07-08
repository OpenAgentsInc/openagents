export const TASSADAR_EXECUTOR_CAPABILITY_REF =
  "capability.tassadar_poc.numeric_model_executor.archived"

export const TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND =
  "tassadar_executor_trace_homework"

export const TASSADAR_EXECUTOR_TRACE_JOB_KIND =
  "tassadar_executor_trace"

export type ArchivedTassadarNumericModel = Record<string, unknown>

export type ArchivedTassadarTrace = Readonly<{
  stepOutputs: ReadonlyArray<unknown>
  traceDigest: string
  stepCount: number
}>

export const collectInterpreterOutputs = (
  _stepOutputs: ReadonlyArray<unknown>,
): { outputs: ReadonlyArray<unknown>; halted: boolean } => ({
  outputs: [],
  halted: false,
})

export const executeTassadarNumericModel = async (
  _model: ArchivedTassadarNumericModel,
  _steps: ReadonlyArray<ReadonlyArray<number>>,
): Promise<ArchivedTassadarTrace> => {
  throw new Error("Tassadar executor assignments are archived in backroom.")
}
