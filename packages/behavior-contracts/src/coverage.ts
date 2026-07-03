import { Context, Effect, Layer } from "effect"
import type { BehaviorContractRegistryDocument } from "./contract"

/**
 * Swappable evidence source for oracle refs. The default production layer
 * reads test files off disk; QA harness integrations can substitute a layer
 * that resolves scenario ids, and package tests use the in-memory layer.
 * Keeping this behind a service is what lets the same coverage program run
 * against any backing system (Effect here, but the contract data itself is
 * plain JSON and portable to non-Effect runners).
 */
export class BehaviorContractOracleSource extends Context.Service<
  BehaviorContractOracleSource,
  {
    readonly read: (
      ref: string,
    ) => Effect.Effect<string, BehaviorContractOracleSourceError>
  }
>()("@openagentsinc/behavior-contracts/BehaviorContractOracleSource") {}

export class BehaviorContractOracleSourceError {
  readonly _tag = "BehaviorContractOracleSourceError"

  constructor(
    readonly ref: string,
    readonly reason: string,
  ) {}
}

export const inMemoryOracleSourceLayer = (
  sources: Readonly<Record<string, string>>,
): Layer.Layer<BehaviorContractOracleSource> =>
  Layer.succeed(BehaviorContractOracleSource, {
    read: ref =>
      ref in sources
        ? Effect.succeed(sources[ref] ?? "")
        : Effect.fail(new BehaviorContractOracleSourceError(ref, "not found")),
  })

export const fileOracleSourceLayer = (
  readFile: (path: string) => Promise<string>,
  resolvePath: (ref: string) => string = ref => ref,
): Layer.Layer<BehaviorContractOracleSource> =>
  Layer.succeed(BehaviorContractOracleSource, {
    read: ref =>
      Effect.tryPromise({
        try: () => readFile(resolvePath(ref)),
        catch: error =>
          new BehaviorContractOracleSourceError(
            ref,
            error instanceof Error ? error.message : String(error),
          ),
      }),
  })

export type BehaviorContractCoverageStatus =
  | "covered"
  | "missing_source"
  | "missing_contract_reference"
  | "skipped_kind"
  | "skipped_state"

export type BehaviorContractCoverageResult = {
  readonly contractId: string
  readonly oracleId: string
  readonly ref: string
  readonly status: BehaviorContractCoverageStatus
}

export type BehaviorContractCoverageReport = {
  readonly ok: boolean
  readonly results: ReadonlyArray<BehaviorContractCoverageResult>
}

/**
 * Prove the registry and the test sweep agree: every enforced contract's
 * `bun-test` oracle file must exist and must reference the owning contractId,
 * so a contract cannot silently drift away from the tests that claim to
 * enforce it. Oracle kinds other than `bun-test` are reported as skipped
 * here; they are covered by their own runners (qa-harness scenarios, visual
 * smokes).
 */
export const checkBehaviorContractCoverage = (
  document: BehaviorContractRegistryDocument,
): Effect.Effect<BehaviorContractCoverageReport, never, BehaviorContractOracleSource> =>
  Effect.gen(function* () {
    const oracleSource = yield* BehaviorContractOracleSource
    const results: BehaviorContractCoverageResult[] = []

    for (const contract of document.contracts) {
      for (const oracle of contract.oracles) {
        if (contract.state !== "enforced") {
          results.push({
            contractId: contract.contractId,
            oracleId: oracle.id,
            ref: oracle.ref,
            status: "skipped_state",
          })
          continue
        }
        if (oracle.kind !== "bun-test") {
          results.push({
            contractId: contract.contractId,
            oracleId: oracle.id,
            ref: oracle.ref,
            status: "skipped_kind",
          })
          continue
        }
        const source = yield* oracleSource.read(oracle.ref).pipe(
          Effect.catch(() => Effect.succeed(null)),
        )
        results.push({
          contractId: contract.contractId,
          oracleId: oracle.id,
          ref: oracle.ref,
          status:
            source === null
              ? "missing_source"
              : source.includes(contract.contractId)
                ? "covered"
                : "missing_contract_reference",
        })
      }
    }

    const ok = results.every(
      result =>
        result.status === "covered" ||
        result.status === "skipped_kind" ||
        result.status === "skipped_state",
    )
    return { ok, results }
  })
