import { PostgresManagedSandboxPhase2Store, type SyncSql } from "@openagentsinc/khala-sync-server";
import {
  type ManagedSandboxPhase2Command,
  type ManagedSandboxPhase2Error,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";

import type { OpenAgentsWorkerEnv } from "./bindings";
import { defaultMakeKhalaSyncSqlClient } from "./khala-sync-push-routes";
import { makeManagedSandboxPhase2ControlTarget } from "./managed-sandbox-phase2-control-target";
import { makeManagedSandboxPhase2PostgresStore } from "./managed-sandbox-phase2-postgres-store";
import {
  type ManagedSandboxPhase2ExecutionResult,
  type ManagedSandboxPhase2Store,
  type ManagedSandboxPhase2Target,
  makeManagedSandboxPhase2Service,
} from "./managed-sandbox-phase2-service";

type Phase2Client = Readonly<{
  sql: SyncSql;
  end: () => Promise<void>;
}>;

const unavailable = (requestRef: string): ManagedSandboxPhase2Error => ({
  _tag: "InvalidRequest",
  requestRef,
  message: "managed-sandbox Phase 2 storage is unavailable",
  retryable: true,
  evidenceRefs: [],
});

export const isManagedSandboxPhase2Enabled = (value: string | undefined): boolean =>
  value === "1" || value?.toLowerCase() === "true" || value?.toLowerCase() === "on";

export const isManagedSandboxPhase2Configured = (env: OpenAgentsWorkerEnv): boolean =>
  typeof env.KHALA_SYNC_DB?.connectionString === "string" &&
  env.KHALA_SYNC_DB.connectionString.length > 0 &&
  typeof env.OA_MANAGED_SANDBOX_CONTROL_URL === "string" &&
  env.OA_MANAGED_SANDBOX_CONTROL_URL.trim().startsWith("https://") &&
  typeof env.OA_MANAGED_SANDBOX_CONTROL_TOKEN === "string" &&
  env.OA_MANAGED_SANDBOX_CONTROL_TOKEN.trim().length > 0;

export const makeManagedSandboxPhase2Executor = <Bindings, Client>(deps: {
  open: (env: Bindings) => Promise<Client>;
  close: (client: Client) => Promise<void>;
  store: (client: Client) => ManagedSandboxPhase2Store;
  target: (env: Bindings) => ManagedSandboxPhase2Target;
}) =>
  Effect.fn("ManagedSandboxPhase2Adapter.execute")(
    (
      env: Bindings,
      command: ManagedSandboxPhase2Command,
    ): Effect.Effect<ManagedSandboxPhase2ExecutionResult, ManagedSandboxPhase2Error> =>
      Effect.gen(function* () {
        const client = yield* Effect.tryPromise({
          try: () => deps.open(env),
          catch: () => unavailable(command.commandRef),
        });
        const execution = Effect.try({
          try: () =>
            makeManagedSandboxPhase2Service({
              store: deps.store(client),
              target: deps.target(env),
            }),
          catch: () => unavailable(command.commandRef),
        }).pipe(Effect.flatMap((service) => service.execute(command)));
        return yield* execution.pipe(
          Effect.ensuring(
            Effect.tryPromise({
              try: () => deps.close(client),
              catch: () => undefined,
            }).pipe(Effect.ignore),
          ),
        );
      }),
  );

const executeWithProductionAdapters = makeManagedSandboxPhase2Executor<
  OpenAgentsWorkerEnv,
  Phase2Client
>({
  open: async (env) => {
    const connectionString = env.KHALA_SYNC_DB?.connectionString;
    if (typeof connectionString !== "string" || connectionString.length === 0) {
      throw new Error("phase2_storage_not_configured");
    }
    const client = await defaultMakeKhalaSyncSqlClient(connectionString);
    return {
      sql: client.sql as unknown as SyncSql,
      end: () => client.end(),
    };
  },
  close: (client) => client.end(),
  store: (client) =>
    makeManagedSandboxPhase2PostgresStore(new PostgresManagedSandboxPhase2Store(client.sql)),
  target: (env) =>
    makeManagedSandboxPhase2ControlTarget({
      baseUrl: env.OA_MANAGED_SANDBOX_CONTROL_URL?.trim() ?? "",
      bearerToken: env.OA_MANAGED_SANDBOX_CONTROL_TOKEN?.trim() ?? "",
    }),
});

export const executeManagedSandboxPhase2ForEnv = (
  env: OpenAgentsWorkerEnv,
  command: ManagedSandboxPhase2Command,
) => executeWithProductionAdapters(env, command);
