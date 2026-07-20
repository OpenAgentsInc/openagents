import type {
  PortableCommittedCheckpointArtifactResolver,
  PortableSessionCommandDispatchConfig,
  PortableSessionCommandDispatchReport,
  SyncSql,
} from "@openagentsinc/khala-sync-server";
import { Effect } from "effect";
import { describe, expect, test, vi } from "vitest";

import {
  makePortableSessionCommandDispatchScheduled,
  type PortableSessionCommandRuntimeAdapters,
} from "./portable-session-command-dispatch-scheduled";

const report = {
  workerInstanceRef: "worker.portable-command.test",
  discovered: 0,
  skippedCommandRefs: [],
  items: [],
} satisfies PortableSessionCommandDispatchReport;

const sql = Object.assign(
  async () => [],
  { begin: async <A>(run: (tx: SyncSql) => Promise<A>) => run(sql) },
) as unknown as SyncSql;

const checkpointArtifacts: PortableCommittedCheckpointArtifactResolver = Object.create(null);
const adapters = {
  brokerFactory: { create: async () => ({ targets: [], adapters: [], vault: {} as never }) },
  checkpointArtifacts: Object.assign(checkpointArtifacts, {
    commandResolver: () => ({ resolve: async () => ({} as never) }),
  }),
} satisfies PortableSessionCommandRuntimeAdapters;

const base = () => ({
  connectionString: () => "postgres://portable-dispatch",
  openSqlClient: async () => ({ sql, end: async () => undefined }),
  capabilityAuthority: () => ({ resolve: async () => [] }),
  runtimeAdapters: async () => adapters,
});

describe("portable session command scheduled composition", () => {
  test("is a no-op unless the exact flag is 1", async () => {
    const openSqlClient = vi.fn(base().openSqlClient);
    const run = makePortableSessionCommandDispatchScheduled({
      ...base(),
      openSqlClient,
    });
    await expect(Effect.runPromise(run({
      env: {},
      enabled: "true",
      scheduledTimeMs: Date.parse("2026-07-20T13:00:00.000Z"),
    }))).resolves.toEqual({ state: "disabled" });
    expect(openSqlClient).not.toHaveBeenCalled();
  });

  test("fails closed before opening storage when authority is absent", async () => {
    const openSqlClient = vi.fn(base().openSqlClient);
    const run = makePortableSessionCommandDispatchScheduled({
      ...base(),
      openSqlClient,
      capabilityAuthority: () => undefined,
    });
    await expect(Effect.runPromise(run({
      env: {},
      enabled: "1",
      scheduledTimeMs: Date.parse("2026-07-20T13:00:00.000Z"),
    }))).rejects.toMatchObject({
      blockerRef: "blocker.portable-command-dispatch.capability_authority_unavailable",
    });
    expect(openSqlClient).not.toHaveBeenCalled();
  });

  test("fails closed and closes storage when runtime adapters are absent", async () => {
    const end = vi.fn(async () => undefined);
    const { runtimeAdapters: _runtimeAdapters, ...withoutRuntimeAdapters } = base();
    const run = makePortableSessionCommandDispatchScheduled({
      ...withoutRuntimeAdapters,
      openSqlClient: async () => ({ sql, end }),
    });
    await expect(Effect.runPromise(run({
      env: {},
      enabled: "1",
      scheduledTimeMs: Date.parse("2026-07-20T13:00:00.000Z"),
    }))).rejects.toMatchObject({
      blockerRef: "blocker.portable-command-dispatch.runtime_adapters_unavailable",
    });
    expect(end).toHaveBeenCalledOnce();
  });

  test("constructs and runs the canonical dispatcher with exact resolvers", async () => {
    let config: PortableSessionCommandDispatchConfig | undefined;
    const runTick = vi.fn(() => Effect.succeed(report));
    const run = makePortableSessionCommandDispatchScheduled({
      ...base(),
      createDispatch: (input) => {
        config = input;
        return { runTick };
      },
    });
    await expect(Effect.runPromise(run({
      env: {},
      enabled: "1",
      scheduledTimeMs: Date.parse("2026-07-20T13:00:00.000Z"),
    }))).resolves.toEqual({ state: "completed", report });
    expect(runTick).toHaveBeenCalledOnce();
    expect(config).toMatchObject({
      dispatcherRef: "dispatcher.api.portable-session-command",
      brokerFactory: adapters.brokerFactory,
    });
    expect(config?.pylonBindings.constructor.name).toBe(
      "PostgresPortableCommandPylonBindingResolver",
    );
    expect(config?.capabilityGrantFacts.constructor.name).toBe(
      "PostgresPortableCommandCapabilityGrantFactResolver",
    );
  });
});
