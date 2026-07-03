import {
  BehaviorContractOracleSource,
  BehaviorContractOracleSourceError,
} from "@openagentsinc/behavior-contracts"
import { Effect, Layer } from "effect"
import { KHALA_CODE_QA_SEED_SCENARIOS } from "./seed-corpus.js"

const seedScenariosById = new Map(
  KHALA_CODE_QA_SEED_SCENARIOS.map((scenario) => [scenario.id, scenario] as const),
)

export const khalaCodeQaSeedScenarioOracleSourceLayer = (options: {
  readonly readFile?: (path: string) => Promise<string>
  readonly resolvePath?: (ref: string) => string
} = {}): Layer.Layer<BehaviorContractOracleSource> =>
  Layer.succeed(BehaviorContractOracleSource, {
    read: ref => {
      const scenario = seedScenariosById.get(ref)
      if (scenario !== undefined) {
        return Effect.succeed(JSON.stringify(scenario))
      }
      if (options.readFile !== undefined) {
        return Effect.tryPromise({
          try: () => options.readFile?.(options.resolvePath?.(ref) ?? ref) ?? Promise.resolve(""),
          catch: error =>
            new BehaviorContractOracleSourceError(
              ref,
              error instanceof Error ? error.message : String(error),
            ),
        })
      }
      return Effect.fail(new BehaviorContractOracleSourceError(ref, "qa scenario not found"))
    },
  })
