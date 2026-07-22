import { createHash } from "node:crypto";

import { Effect, Schema } from "effect";

import {
  PostgresPortableSessionCommandQueue,
  type PortableSessionAcceptedCommandClaimRequest,
} from "./portable-session-command-queue.js";
import {
  PostgresPortableSessionCommandRunner,
  type PostgresPortableSessionCommandRunnerConfig,
} from "./portable-session-command-runner.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

const deterministicRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`;

export class PortableSessionCommandDispatchError extends Schema.TaggedErrorClass<PortableSessionCommandDispatchError>()(
  "PortableSessionCommandDispatchError",
  {
    reason: Schema.String,
  },
) {}

export type PortableSessionCommandDispatchItem = Readonly<{
  commandRef: string;
  claimRef: string;
  workerInstanceRef: string;
  status: "completed" | "failed" | "rejected" | "pending_reconcile" | "dispatch_failed";
  failureRef?: string;
}>;

export type PortableSessionCommandDispatchReport = Readonly<{
  workerInstanceRef: string;
  discovered: number;
  skippedCommandRefs: ReadonlyArray<string>;
  items: ReadonlyArray<PortableSessionCommandDispatchItem>;
}>;

export type PortableSessionCommandDispatchConfig = PostgresPortableSessionCommandRunnerConfig &
  Readonly<{
    dispatcherRef: string;
    batchSize?: number;
    concurrency?: number;
    leaseDurationMs?: number;
  }>;

/**
 * Runs one bounded accepted-command discovery pass. This class does not own a
 * scheduler. The caller supplies the exact resolver dependencies that the
 * canonical command runner requires.
 */
export class PostgresPortableSessionCommandDispatch {
  private readonly queue: PostgresPortableSessionCommandQueue;
  private readonly runner: PostgresPortableSessionCommandRunner;
  private readonly workerInstanceRef: string;
  private readonly batchSize: number;
  private readonly concurrency: number;
  private readonly leaseDurationMs: number;

  constructor(config: PortableSessionCommandDispatchConfig) {
    if (!SAFE_REF.test(config.dispatcherRef)) {
      throw new PortableSessionCommandDispatchError({
        reason: "portable command dispatcher ref is invalid",
      });
    }
    this.batchSize = config.batchSize ?? 10;
    this.concurrency = config.concurrency ?? 1;
    this.leaseDurationMs = config.leaseDurationMs ?? 5 * 60 * 1_000;
    if (
      !Number.isSafeInteger(this.batchSize) ||
      this.batchSize < 1 ||
      this.batchSize > 100 ||
      !Number.isSafeInteger(this.concurrency) ||
      this.concurrency < 1 ||
      this.concurrency > this.batchSize
    ) {
      throw new PortableSessionCommandDispatchError({
        reason: "portable command dispatcher bounds are invalid",
      });
    }
    this.workerInstanceRef = deterministicRef("worker.portable-command", config.dispatcherRef);
    this.queue = new PostgresPortableSessionCommandQueue(config.sql, config.now);
    this.runner = new PostgresPortableSessionCommandRunner(config);
  }

  readonly runTick = Effect.fn("PortableSessionCommandDispatch.runTick")(() => {
    const queue = this.queue;
    const runner = this.runner;
    const workerInstanceRef = this.workerInstanceRef;
    const batchSize = this.batchSize;
    const concurrency = this.concurrency;
    const leaseDurationMs = this.leaseDurationMs;
    const dispatchOne = (accepted: PortableSessionAcceptedCommandClaimRequest) =>
      this.dispatchOne(runner, accepted);
    return Effect.gen(function* () {
      const batch = yield* Effect.tryPromise({
        try: () =>
          queue.claimAcceptedBatch({
            workerInstanceRef,
            limit: batchSize,
            leaseDurationMs,
          }),
        catch: () =>
          new PortableSessionCommandDispatchError({
            reason: "portable accepted-command discovery failed",
          }),
      });
      const items = yield* Effect.forEach(batch.claims, dispatchOne, { concurrency });
      return {
        workerInstanceRef,
        discovered: batch.claims.length,
        skippedCommandRefs: batch.skippedCommandRefs,
        items,
      } satisfies PortableSessionCommandDispatchReport;
    });
  });

  private dispatchOne(
    runner: Pick<PostgresPortableSessionCommandRunner, "execute">,
    accepted: PortableSessionAcceptedCommandClaimRequest,
  ): Effect.Effect<PortableSessionCommandDispatchItem> {
    return Effect.tryPromise(() => runner.execute(accepted.claimRequest)).pipe(
      Effect.map(
        (result): PortableSessionCommandDispatchItem => ({
          commandRef: accepted.commandRef,
          claimRef: result.claim.claimRef,
          workerInstanceRef: result.claim.workerInstanceRef,
          status: result.status,
        }),
      ),
      Effect.catch(() =>
        Effect.succeed<PortableSessionCommandDispatchItem>({
          commandRef: accepted.commandRef,
          claimRef: accepted.claimRequest.claimRef,
          workerInstanceRef: accepted.claimRequest.workerInstanceRef,
          status: "dispatch_failed",
          failureRef: deterministicRef(
            "failure.portable-command-dispatch",
            accepted.claimRequest.claimRef,
          ),
        }),
      ),
    );
  }
}
