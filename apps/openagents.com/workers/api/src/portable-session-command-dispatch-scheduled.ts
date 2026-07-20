import {
  PostgresPortableCommandCapabilityGrantFactResolver,
  PostgresPortableCommandPylonBindingResolver,
  PostgresPortableSessionCommandDispatch,
  PostgresPortableTargetPylonBindingStore,
  type PortableCapabilityGrantFactAuthority,
  type PortableCommandBrokerFactory,
  type PortableSessionCommandDispatchConfig,
  type PortableSessionCommandDispatchReport,
  type PortableCommittedCheckpointArtifactResolver,
  type SyncSql,
  withSyncTransaction,
} from "@openagentsinc/khala-sync-server";
import { Effect, Schema } from "effect";

export const PORTABLE_SESSION_COMMAND_DISPATCH_FLAG =
  "PORTABLE_SESSION_COMMAND_DISPATCH_ENABLED" as const;

export class PortableSessionCommandDispatchScheduledError extends Schema.TaggedErrorClass<PortableSessionCommandDispatchScheduledError>()(
  "PortableSessionCommandDispatchScheduledError",
  {
    blockerRef: Schema.Literals([
      "blocker.portable-command-dispatch.database_unavailable",
      "blocker.portable-command-dispatch.capability_authority_unavailable",
      "blocker.portable-command-dispatch.runtime_adapters_unavailable",
      "blocker.portable-command-dispatch.execution_failed",
    ]),
  },
) {}

export type PortableSessionCommandRuntimeAdapters = Readonly<{
  brokerFactory: PortableCommandBrokerFactory;
  checkpointArtifacts: PortableCommittedCheckpointArtifactResolver;
}>;

type SqlClient = Readonly<{
  sql: SyncSql;
  end: () => Promise<unknown>;
}>;

type Dispatch = Readonly<{
  runTick: () => Effect.Effect<PortableSessionCommandDispatchReport, unknown>;
}>;

export type PortableSessionCommandDispatchScheduledDependencies<Env> = Readonly<{
  connectionString: (env: Env) => string | undefined;
  openSqlClient: (connectionString: string) => Promise<SqlClient>;
  capabilityAuthority: (
    env: Env,
  ) => PortableCapabilityGrantFactAuthority | undefined;
  runtimeAdapters?: (
    env: Env,
    sql: SyncSql,
  ) => Promise<PortableSessionCommandRuntimeAdapters | undefined>;
  /** Test seam only. Production uses PostgresPortableSessionCommandDispatch. */
  createDispatch?: (config: PortableSessionCommandDispatchConfig) => Dispatch;
}>;

export type PortableSessionCommandDispatchScheduledInput<Env> = Readonly<{
  env: Env;
  enabled: string | undefined;
  scheduledTimeMs: number;
}>;

export type PortableSessionCommandDispatchScheduledResult =
  | Readonly<{ state: "disabled" }>
  | Readonly<{
      state: "completed";
      report: PortableSessionCommandDispatchReport;
    }>;

const blocked = (
  blockerRef: PortableSessionCommandDispatchScheduledError["blockerRef"],
) => new PortableSessionCommandDispatchScheduledError({ blockerRef });

/**
 * Runs one bounded portable-command dispatch pass from the API scheduler.
 * The exact feature flag, database, capability authority, runtime broker, and
 * committed artifact custody adapter must all exist. This function does not
 * infer an adapter from target or Pylon presence.
 */
export const makePortableSessionCommandDispatchScheduled = <Env>(
  dependencies: PortableSessionCommandDispatchScheduledDependencies<Env>,
) =>
  Effect.fn("PortableSessionCommandDispatchScheduled.run")(function* (
    input: PortableSessionCommandDispatchScheduledInput<Env>,
  ) {
    if (input.enabled !== "1") {
      return { state: "disabled" } as const;
    }
    const connectionString = dependencies.connectionString(input.env)?.trim();
    if (connectionString === undefined || connectionString === "") {
      return yield* blocked("blocker.portable-command-dispatch.database_unavailable");
    }
    const authority = dependencies.capabilityAuthority(input.env);
    if (authority === undefined) {
      return yield* blocked(
        "blocker.portable-command-dispatch.capability_authority_unavailable",
      );
    }
    const scheduledAt = new Date(input.scheduledTimeMs);
    if (!Number.isFinite(scheduledAt.getTime())) {
      return yield* blocked("blocker.portable-command-dispatch.execution_failed");
    }
    const now = () => scheduledAt.toISOString();
    return yield* Effect.acquireUseRelease(
      Effect.tryPromise({
        try: () => dependencies.openSqlClient(connectionString),
        catch: () =>
          blocked("blocker.portable-command-dispatch.database_unavailable"),
      }),
      (client) =>
        Effect.gen(function* () {
          const makeRuntimeAdapters = dependencies.runtimeAdapters;
          if (makeRuntimeAdapters === undefined) {
            return yield* blocked(
              "blocker.portable-command-dispatch.runtime_adapters_unavailable",
            );
          }
          const adapters = yield* Effect.tryPromise({
            try: () => makeRuntimeAdapters(input.env, client.sql),
            catch: () =>
              blocked(
                "blocker.portable-command-dispatch.runtime_adapters_unavailable",
              ),
          });
          if (adapters === undefined) {
            return yield* blocked(
              "blocker.portable-command-dispatch.runtime_adapters_unavailable",
            );
          }
          const dispatchConfig: PortableSessionCommandDispatchConfig = {
            sql: client.sql,
            transaction: (run) => withSyncTransaction(client.sql, run),
            dispatcherRef: "dispatcher.api.portable-session-command",
            brokerFactory: adapters.brokerFactory,
            checkpointArtifacts: adapters.checkpointArtifacts.commandResolver(),
            pylonBindings: new PostgresPortableCommandPylonBindingResolver(
              new PostgresPortableTargetPylonBindingStore(client.sql, now),
              now,
            ),
            capabilityGrantFacts:
              new PostgresPortableCommandCapabilityGrantFactResolver({
                sql: client.sql,
                authority,
                now,
              }),
            now,
          };
          const dispatch = dependencies.createDispatch?.(dispatchConfig) ??
            new PostgresPortableSessionCommandDispatch(dispatchConfig);
          const report = yield* dispatch.runTick().pipe(
            Effect.mapError(() =>
              blocked("blocker.portable-command-dispatch.execution_failed"),
            ),
          );
          return { state: "completed", report } as const;
        }),
      (client) => Effect.promise(() => client.end()).pipe(Effect.ignore),
    );
  });
