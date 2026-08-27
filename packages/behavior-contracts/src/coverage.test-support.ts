import { Context, Effect, Layer } from "effect";
import type { BehaviorContractRegistryDocument } from "./contract";

export class BehaviorContractOracleSource extends Context.Service<
  BehaviorContractOracleSource,
  {
    readonly read: (ref: string) => Effect.Effect<string, BehaviorContractOracleSourceError>;
  }
>()("@openagentsinc/behavior-contracts/BehaviorContractOracleSource") {}

export class BehaviorContractOracleSourceError {
  readonly _tag = "BehaviorContractOracleSourceError";

  constructor(
    readonly ref: string,
    readonly reason: string,
  ) {}
}

export const inMemoryOracleSourceLayer = (
  sources: Readonly<Record<string, string>>,
): Layer.Layer<BehaviorContractOracleSource> =>
  Layer.succeed(BehaviorContractOracleSource, {
    read: (ref) =>
      ref in sources
        ? Effect.succeed(sources[ref] ?? "")
        : Effect.fail(new BehaviorContractOracleSourceError(ref, "not found")),
  });

export const fileOracleSourceLayer = (
  readFile: (path: string) => Promise<string>,
  resolvePath: (ref: string) => string = (ref) => ref,
): Layer.Layer<BehaviorContractOracleSource> =>
  Layer.succeed(BehaviorContractOracleSource, {
    read: (ref) =>
      Effect.tryPromise({
        try: () => readFile(resolvePath(ref)),
        catch: (error) =>
          new BehaviorContractOracleSourceError(
            ref,
            error instanceof Error ? error.message : String(error),
          ),
      }),
  });

export type BehaviorContractCoverageStatus =
  | "covered"
  | "missing_source"
  | "missing_contract_reference"
  | "seam_oracle_not_e2e"
  | "skipped_kind"
  | "skipped_state";

const seamE2eOracleRefPattern = /\.e2e\./u;

export type BehaviorContractCoverageResult = {
  readonly contractId: string;
  readonly oracleId: string;
  readonly ref: string;
  readonly status: BehaviorContractCoverageStatus;
};

export type BehaviorContractCoverageReport = {
  readonly ok: boolean;
  readonly results: ReadonlyArray<BehaviorContractCoverageResult>;
};

export const checkBehaviorContractCoverage = (
  document: BehaviorContractRegistryDocument,
): Effect.Effect<BehaviorContractCoverageReport, never, BehaviorContractOracleSource> =>
  Effect.gen(function* () {
    const oracleSource = yield* BehaviorContractOracleSource;
    const results: BehaviorContractCoverageResult[] = [];

    for (const contract of document.contracts) {
      for (const oracle of contract.oracles) {
        if (contract.state !== "enforced") {
          results.push({
            contractId: contract.contractId,
            oracleId: oracle.id,
            ref: oracle.ref,
            status: "skipped_state",
          });
          continue;
        }
        if (oracle.kind !== "bun-test" && oracle.kind !== "qa-scenario") {
          results.push({
            contractId: contract.contractId,
            oracleId: oracle.id,
            ref: oracle.ref,
            status: "skipped_kind",
          });
          continue;
        }
        if (
          contract.seam !== undefined &&
          oracle.kind === "bun-test" &&
          !seamE2eOracleRefPattern.test(oracle.ref)
        ) {
          results.push({
            contractId: contract.contractId,
            oracleId: oracle.id,
            ref: oracle.ref,
            status: "seam_oracle_not_e2e",
          });
          continue;
        }
        const source = yield* oracleSource
          .read(oracle.ref)
          .pipe(Effect.catch(() => Effect.succeed(null)));
        results.push({
          contractId: contract.contractId,
          oracleId: oracle.id,
          ref: oracle.ref,
          status:
            source === null
              ? "missing_source"
              : oracle.kind === "bun-test" && !source.includes(contract.contractId)
                ? "missing_contract_reference"
                : "covered",
        });
      }
    }

    const ok = results.every(
      (result) =>
        result.status === "covered" ||
        result.status === "skipped_kind" ||
        result.status === "skipped_state",
    );
    return { ok, results };
  });
