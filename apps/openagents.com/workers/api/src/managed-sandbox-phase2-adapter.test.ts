import {
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxPhase2Command,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import type { OpenAgentsWorkerEnv } from "./bindings";
import {
  isManagedSandboxPhase2Configured,
  isManagedSandboxPhase2Enabled,
  makeManagedSandboxPhase2Executor,
} from "./managed-sandbox-phase2-adapter";
import type {
  ManagedSandboxPhase2Operation,
  ManagedSandboxPhase2Store,
  ManagedSandboxPhase2Target,
} from "./managed-sandbox-phase2-service";

const digest = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`;

const command = {
  _tag: "CreateCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.adapter.create",
  idempotencyRef: "idempotency.sbx10.adapter.create",
  ownerRef: "owner.sbx10.adapter",
  tenantRef: "owner.sbx10.adapter",
  requestedAt: "2026-07-22T05:00:00.000Z",
  checkpointRef: "checkpoint.sbx10.adapter",
  sourceSandboxRef: "sandbox.sbx10.adapter",
  sourceResourceGeneration: 9,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.02905a7",
  repositoryPostImageDigest: digest("c"),
  formatRef: "format.sbx.content-tar.v1",
  retainedUntil: "2026-07-23T05:00:00.000Z",
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "CreateCheckpoint" }>;

const checkpoint = {
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: command.checkpointRef,
  ownerRef: command.ownerRef,
  tenantRef: command.tenantRef,
  sourceSandboxRef: command.sourceSandboxRef,
  sourceResourceGeneration: command.sourceResourceGeneration,
  sourceImageDigest: command.sourceImageDigest,
  sourceToolchainDigest: command.sourceToolchainDigest,
  repositoryRef: command.repositoryRef,
  repositoryRevisionRef: command.repositoryRevisionRef,
  repositoryPostImageDigest: command.repositoryPostImageDigest,
  contentDigest: digest("d"),
  contentBytes: 4_096,
  formatRef: command.formatRef,
  state: "completed",
  completedAt: "2026-07-22T05:00:01.000Z",
  verifiedAt: "2026-07-22T05:00:02.000Z",
  retainedUntil: command.retainedUntil,
  deleteOnExpiry: true,
  omissions: {
    credentials: "excluded",
    accountSecrets: "excluded",
    providerHiddenState: "excluded",
    processMemory: "excluded",
    processTable: "excluded",
    ptyState: "excluded",
    sockets: "excluded",
    ports: "excluded",
    networkIdentity: "excluded",
  },
  evidenceRefs: ["receipt.sbx10.adapter.verify"],
} satisfies ManagedSandboxContentCheckpoint;

const unused = () => Effect.die("unused Phase 2 target action");

const target: ManagedSandboxPhase2Target = {
  createCheckpoint: () => Effect.succeed(checkpoint),
  archiveWithCheckpoint: unused,
  verifyCheckpoint: () => Effect.succeed(true),
  observeResourceGeneration: unused,
  forkFromCheckpoint: unused,
  restoreCheckpoint: unused,
  deleteCheckpoint: unused,
  createPrivateIngress: unused,
  revokePrivateIngress: unused,
  expirePrivateIngress: unused,
};

const makeStore = () => {
  let settled = 0;
  const store: ManagedSandboxPhase2Store = {
    lookupOperation: () => Effect.sync((): ManagedSandboxPhase2Operation | undefined => undefined),
    readCheckpoint: () => Effect.sync((): ManagedSandboxContentCheckpoint | undefined => undefined),
    readPrivateIngress: () => Effect.succeed(undefined),
    settle: () =>
      Effect.sync(() => {
        settled += 1;
      }),
  };
  return { store, settled: () => settled };
};

// eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect-aware test API; this helper is the suite runtime boundary.
const runEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(effect);

describe("managed sandbox Phase 2 Worker adapter", () => {
  it("requires an explicit activation value", () => {
    expect(isManagedSandboxPhase2Enabled(undefined)).toBe(false);
    expect(isManagedSandboxPhase2Enabled("0")).toBe(false);
    expect(isManagedSandboxPhase2Enabled("false")).toBe(false);
    expect(isManagedSandboxPhase2Enabled("1")).toBe(true);
    expect(isManagedSandboxPhase2Enabled("TRUE")).toBe(true);
    expect(isManagedSandboxPhase2Enabled("on")).toBe(true);
  });

  it("requires Cloud SQL and an HTTPS authenticated private target", () => {
    const configured = {
      KHALA_SYNC_DB: { connectionString: "postgres://phase2.example/database" },
      OA_MANAGED_SANDBOX_CONTROL_URL: "https://phase2-control.example",
      OA_MANAGED_SANDBOX_CONTROL_TOKEN: "private-control-token",
    } as OpenAgentsWorkerEnv;

    expect(isManagedSandboxPhase2Configured(configured)).toBe(true);
    expect(
      isManagedSandboxPhase2Configured({
        ...configured,
        OA_MANAGED_SANDBOX_CONTROL_URL: "http://phase2-control.example",
      }),
    ).toBe(false);
    expect(
      isManagedSandboxPhase2Configured({
        ...configured,
        OA_MANAGED_SANDBOX_CONTROL_TOKEN: " ",
      }),
    ).toBe(false);
    expect(
      isManagedSandboxPhase2Configured({
        OA_MANAGED_SANDBOX_CONTROL_URL: configured.OA_MANAGED_SANDBOX_CONTROL_URL,
        OA_MANAGED_SANDBOX_CONTROL_TOKEN: configured.OA_MANAGED_SANDBOX_CONTROL_TOKEN,
      } as OpenAgentsWorkerEnv),
    ).toBe(false);
  });

  it("composes the durable store and target and always closes the client", async () => {
    const state = makeStore();
    const client = { clientRef: "client.phase2.adapter" };
    let opens = 0;
    let closes = 0;
    const execute = makeManagedSandboxPhase2Executor<Readonly<{ ref: string }>, typeof client>({
      open: async () => {
        opens += 1;
        return client;
      },
      close: async (value) => {
        expect(value).toBe(client);
        closes += 1;
      },
      store: (value) => {
        expect(value).toBe(client);
        return state.store;
      },
      target: (env) => {
        expect(env.ref).toBe("env.phase2.adapter");
        return target;
      },
    });

    const result = await runEffect(execute({ ref: "env.phase2.adapter" }, command));

    expect(result).toEqual(checkpoint);
    expect(opens).toBe(1);
    expect(closes).toBe(1);
    expect(state.settled()).toBe(1);
  });

  it("closes the client when adapter construction fails", async () => {
    const client = { clientRef: "client.phase2.adapter.failure" };
    let closes = 0;
    const execute = makeManagedSandboxPhase2Executor<
      Readonly<Record<string, never>>,
      typeof client
    >({
      open: async () => client,
      close: async () => {
        closes += 1;
      },
      store: () => {
        throw new Error("/Users/private/phase2-store-secret");
      },
      target: () => target,
    });

    const failure = await runEffect(Effect.flip(execute({}, command)));

    expect(failure).toEqual({
      _tag: "InvalidRequest",
      requestRef: command.commandRef,
      message: "managed-sandbox Phase 2 storage is unavailable",
      retryable: true,
      evidenceRefs: [],
    });
    expect(JSON.stringify(failure)).not.toContain("/Users/private");
    expect(closes).toBe(1);
  });

  it("returns a fixed public-safe error when the database cannot open", async () => {
    let closes = 0;
    const execute = makeManagedSandboxPhase2Executor<Readonly<Record<string, never>>, object>({
      open: async () => {
        throw new Error("postgres://operator:private@phase2.example/database");
      },
      close: async () => {
        closes += 1;
      },
      store: () => makeStore().store,
      target: () => target,
    });

    const failure = await runEffect(Effect.flip(execute({}, command)));

    expect(failure).toMatchObject({
      _tag: "InvalidRequest",
      requestRef: command.commandRef,
    });
    expect(JSON.stringify(failure)).not.toContain("operator:private");
    expect(closes).toBe(0);
  });
});
