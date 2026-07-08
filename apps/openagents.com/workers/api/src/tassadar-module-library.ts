export type TassadarDemandPriceSignal = Readonly<
  Record<string, unknown> & {
    baselineBudgetSats: number
    recommendedBudgetSats: number
  }
>
export type TassadarDemandRankedModuleLibraryEntry = Readonly<
  Record<string, unknown> & {
    recommendedBudgetSats?: number | undefined
  }
>
export type TassadarDemandRankedModuleLibraryProjection = Readonly<{
  collapsedDuplicateCount: number
  duplicateGroupCount: number
  entries: ReadonlyArray<TassadarDemandRankedModuleLibraryEntry>
  generatedBy: string
  modules: ReadonlyArray<TassadarDemandRankedModuleLibraryEntry>
}>

export const buildTassadarDemandPriceSignal = (
  input: Readonly<Record<string, unknown>> = {},
): TassadarDemandPriceSignal => ({
  ...input,
  archived: true,
  baselineBudgetSats:
    typeof input.baselineBudgetSats === 'number'
      ? input.baselineBudgetSats
      : 0,
  backroomPath: 'openagents-prune-20260708-tassadar-psionic',
  recommendedBudgetSats:
    typeof input.baselineBudgetSats === 'number'
      ? input.baselineBudgetSats + 1
      : 1,
})

export const rankTassadarCompiledModuleLibrary = (
  input: Readonly<{
    entries?: ReadonlyArray<TassadarDemandRankedModuleLibraryEntry>
  }> = {},
): TassadarDemandRankedModuleLibraryProjection => ({
  collapsedDuplicateCount: 0,
  duplicateGroupCount: 0,
  entries: input.entries ?? [],
  generatedBy: 'tassadar_module_library_ranker.v1',
  modules: input.entries ?? [],
})

export const listTassadarCompiledModules = async (
  ..._args: unknown[]
): Promise<ReadonlyArray<unknown>> => []
