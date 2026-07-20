import {
  PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  type PortableCommandExecutionClaim,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test, vi } from "vite-plus/test";

import {
  createProductionPortableCommandBrokerFactory,
  PortableCommandBrokerFactoryError,
  type PortableCommandDestinationRunnerSessionResolution,
  type PortableCommandDestinationRunnerSessionResolver,
  type PortableCommandTargetInstallationPortResolver,
  type PortableCommandTargetInstallationPortResolution,
} from "./portable-command-broker-factory.js";
import type { PortableCommandGrantAuthorityBinding } from "./portable-session-command-runner.js";
import type { PortableCapabilityTransfer } from "./portable-session-move.js";

const ownerRef = "owner.ide13.command-broker";
const sessionRef = "session.ide13.command-broker";
const localTargetRef = "target.ide13.command-broker.local";
const managedTargetRef = "target.ide13.command-broker.managed";

const descriptor = (
  targetRef: string,
  targetClass: PortableTargetDescriptor["targetClass"],
  adapterRef: string,
): PortableTargetDescriptor => ({
  targetRef,
  targetClass,
  adapterRef,
  ownerRef,
  compatibilityRef: "compatibility.ide13.command-broker",
  isolation: targetClass === "owner_local" ? "owner_host_process" : "dedicated_microvm",
  dataPosture: targetClass === "owner_local" ? "owner_device_only" : "openagents_managed_region",
  health: "ready",
});

const local = descriptor(localTargetRef, "owner_local", "adapter.ide13.command-broker.local");
const managed = descriptor(
  managedTargetRef,
  "openagents_managed",
  "adapter.ide13.command-broker.managed",
);

const claim = (
  source: PortableTargetDescriptor = local,
  destination: PortableTargetDescriptor = managed,
): PortableCommandExecutionClaim => ({
  schema: PORTABLE_COMMAND_EXECUTION_SCHEMA_VERSION,
  claimRef: `claim.ide13.command-broker.${source.targetClass}.${destination.targetClass}`,
  commandRef: "command.ide13.command-broker",
  ownerRef,
  sessionRef,
  commandKind: "move",
  commandFingerprint: `sha256:${"1".repeat(64)}`,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  sourceAttachmentRef: "attachment.ide13.command-broker.source",
  sourceGeneration: 1,
  destinationTargetRef: destination.targetRef,
  executorEnvironmentRef: source.targetRef,
  workerInstanceRef: "worker.ide13.command-broker",
  claimGeneration: 1,
  leaseRevision: 1,
  state: "claimed",
  claimedAt: "2026-07-20T12:00:00.000Z",
  leaseExpiresAt: "2026-07-20T12:10:00.000Z",
  updatedAt: "2026-07-20T12:00:00.000Z",
  terminalStatus: null,
  pendingReconcileRef: null,
  outcomeRef: null,
  evidenceRefs: [],
});

const binding: PortableCommandGrantAuthorityBinding = {
  sourceLeaseRef: "lease.ide13.command-broker.source",
  grantRef: "grant.ide13.command-broker.source",
  ownerUserId: ownerRef,
  kind: "provider",
  providerAccountRef: "account.ide13.command-broker",
  runnerSessionId: "runner.ide13.command-broker",
};

const transfer: PortableCapabilityTransfer = {
  sourceLeaseRef: "lease.ide13.command-broker.source",
  destinationLeaseRef: "lease.ide13.command-broker.destination",
  destinationSourceGrantRef: "grant.ide13.command-broker.destination",
  expiresAt: "2026-07-20T12:09:00.000Z",
};

const port = () => ({
  install: vi.fn(async (input) => ({
    installationRef: `installation.${input.lease.leaseRef}`,
    evidenceRef: `evidence.${input.lease.leaseRef}`,
    ...(input.managedMarkerPath === undefined
      ? {}
      : {
          marker: {
            leaseRef: input.lease.leaseRef,
            evidenceRef: `evidence.${input.lease.leaseRef}`,
          },
        }),
  })),
  wipe: vi.fn(async (input) => ({ wipeReceiptRef: `wipe.${input.leaseRef}` })),
});

const exactResolver = () => {
  const ports = new Map([
    [local.targetRef, port()],
    [managed.targetRef, port()],
  ]);
  const resolve = vi.fn<PortableCommandTargetInstallationPortResolver["resolve"]>(
    async ({ target }): Promise<PortableCommandTargetInstallationPortResolution | null> => {
      const resolved = ports.get(target.targetRef);
      return resolved === undefined
        ? null
        : {
            targetRef: target.targetRef,
            targetClass: target.targetClass,
            adapterRef: target.adapterRef,
            port: resolved,
          };
    },
  );
  return { resolver: { resolve } satisfies PortableCommandTargetInstallationPortResolver, resolve };
};

const authorityFetch = (
  events: string[] = [],
  requests: Array<Readonly<{ path: string; body: Record<string, unknown> }>> = [],
) =>
  vi.fn(async (request: string | URL | Request, init?: RequestInit): Promise<Response> => {
    const path = new URL(request instanceof Request ? request.url : request.toString()).pathname;
    const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
    requests.push({ path, body });
    if (path.endsWith("/reissue")) {
      events.push("reissue");
      return Response.json({
        grant: { grantRef: body.destinationGrantRef, status: "issued" },
        material: "excluded",
      });
    }
    if (path.endsWith("/revoke")) {
      events.push("revoke");
      return Response.json({
        grant: { grantRef: body.grantRef, status: "revoked" },
        material: "excluded",
      });
    }
    throw new Error(`unexpected authority path: ${path}`);
  });

const destinationRunnerSessions = (): PortableCommandDestinationRunnerSessionResolver => ({
  resolve: async ({
    commandExecutionClaimRef,
    destination,
    sourceBinding,
    capabilityTransfers,
  }) => {
    const matched = capabilityTransfers[0];
    if (matched === undefined) return null;
    return {
      commandExecutionClaimRef,
      destinationTargetRef: destination.targetRef,
      sourceGrantRef: sourceBinding.grantRef,
      sourceLeaseRef: matched.sourceLeaseRef,
      destinationSourceGrantRef: matched.destinationSourceGrantRef,
      destinationRunnerSessionId: `runner.destination.${sourceBinding.grantRef}`,
    };
  },
});

const factory = (
  installationPorts: PortableCommandTargetInstallationPortResolver,
  fetch = authorityFetch(),
  runnerSessions = destinationRunnerSessions(),
) =>
  createProductionPortableCommandBrokerFactory({
    grantAuthority: {
      baseUrl: "https://openagents.example",
      serviceBearer: "service-fixture-command-broker",
      fetch,
    },
    installationPorts,
    destinationRunnerSessions: runnerSessions,
  });

describe("production portable command broker factory", () => {
  test("creates only the exact local and managed adapters in both directions", async () => {
    const resolution = exactResolver();
    const brokerFactory = factory(resolution.resolver);

    await Promise.all(
      (
        [
          [local, managed],
          [managed, local],
        ] as const
      ).map(async ([source, destination]) => {
        const broker = await brokerFactory.create({
          claim: claim(source, destination),
          source,
          destination,
          grantBindings: [binding],
          capabilityTransfers: [transfer],
        });
        expect(broker.targets).toEqual([
          {
            targetRef: source.targetRef,
            targetClass: source.targetClass,
            adapterRef: source.adapterRef,
            ready: true,
          },
          {
            targetRef: destination.targetRef,
            targetClass: destination.targetClass,
            adapterRef: destination.adapterRef,
            ready: true,
          },
        ]);
        expect(
          broker.adapters.map(({ adapterRef, targetClass }) => ({ adapterRef, targetClass })),
        ).toEqual([
          { adapterRef: source.adapterRef, targetClass: source.targetClass },
          { adapterRef: destination.adapterRef, targetClass: destination.targetClass },
        ]);
      }),
    );
    expect(resolution.resolve).toHaveBeenCalledTimes(4);
  });

  test.each([
    ["adapter", { adapterRef: "adapter.ide13.command-broker.wrong" }],
    ["class", { targetClass: "owner_managed" as const }],
    ["target", { targetRef: "target.ide13.command-broker.wrong" }],
  ])("rejects a resolver with the wrong %s", async (_name, mismatch) => {
    const resolve = vi.fn<PortableCommandTargetInstallationPortResolver["resolve"]>(
      async ({ target }) => ({
        targetRef: target.targetRef,
        targetClass: target.targetClass,
        adapterRef: target.adapterRef,
        port: port(),
        ...mismatch,
      }),
    );
    await expect(
      factory({ resolve }).create({
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toMatchObject({ code: "target_mismatch" });
  });

  test.each([
    ["missing grant", [], [transfer]],
    [
      "extra grant",
      [binding, { ...binding, grantRef: "grant.ide13.command-broker.extra" }],
      [transfer],
    ],
    ["missing transfer", [binding], []],
    [
      "extra transfer",
      [binding],
      [
        transfer,
        {
          ...transfer,
          sourceLeaseRef: "lease.ide13.command-broker.extra",
          destinationLeaseRef: "lease.ide13.command-broker.extra-destination",
          destinationSourceGrantRef: "grant.ide13.command-broker.extra-destination",
        },
      ],
    ],
  ])("rejects %s", async (_name, grantBindings, capabilityTransfers) => {
    const resolution = exactResolver();
    await expect(
      factory(resolution.resolver).create({
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings,
        capabilityTransfers,
      }),
    ).rejects.toMatchObject({ code: "capability_mismatch" });
    expect(resolution.resolve).not.toHaveBeenCalled();
  });

  test("reissues the matching destination grant before source revocation", async () => {
    const events: string[] = [];
    const requests: Array<Readonly<{ path: string; body: Record<string, unknown> }>> = [];
    const broker = await factory(exactResolver().resolver, authorityFetch(events, requests)).create(
      {
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      },
    );
    await broker.vault.revokeSourceGrant({
      sourceGrantRef: binding.grantRef,
      leaseRef: transfer.sourceLeaseRef,
    });
    expect(events).toEqual(["reissue", "revoke"]);
    expect(requests[0]?.body).toMatchObject({
      sourceGrantRef: binding.grantRef,
      destinationGrantRef: transfer.destinationSourceGrantRef,
      runnerSessionId: `runner.destination.${binding.grantRef}`,
    });
    expect(requests[0]?.body.runnerSessionId).not.toBe(binding.runnerSessionId);
    await expect(
      broker.vault.revokeSourceGrant({
        sourceGrantRef: "grant.ide13.command-broker.unregistered",
        leaseRef: transfer.sourceLeaseRef,
      }),
    ).rejects.toThrow();
    expect(events).toEqual(["reissue", "revoke"]);
  });

  test("does not revoke the source when destination reissue is lost", async () => {
    const events: string[] = [];
    const fetch = vi.fn(async (request: string | URL | Request): Promise<Response> => {
      const path = new URL(request instanceof Request ? request.url : request.toString()).pathname;
      events.push(path.endsWith("/reissue") ? "reissue" : "revoke");
      throw new Error("authority connection lost");
    });
    const broker = await factory(exactResolver().resolver, fetch).create({
      claim: claim(),
      source: local,
      destination: managed,
      grantBindings: [binding],
      capabilityTransfers: [transfer],
    });
    await expect(
      broker.vault.revokeSourceGrant({
        sourceGrantRef: binding.grantRef,
        leaseRef: transfer.sourceLeaseRef,
      }),
    ).rejects.toThrow();
    expect(events).toEqual(["reissue"]);
  });

  test("fails closed before authority use when a runner resolver is absent", async () => {
    const fetch = authorityFetch();
    const brokerFactory = createProductionPortableCommandBrokerFactory({
      grantAuthority: {
        baseUrl: "https://openagents.example",
        serviceBearer: "service-fixture-command-broker",
        fetch,
      },
      installationPorts: exactResolver().resolver,
    });
    await expect(
      brokerFactory.create({
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toMatchObject({ code: "capability_mismatch" });
    expect(fetch).not.toHaveBeenCalled();
  });

  test.each([
    ["mismatched source grant", { sourceGrantRef: "grant.ide13.command-broker.wrong" }],
    ["reused source runner", { destinationRunnerSessionId: binding.runnerSessionId }],
    ["unsafe runner ref", { destinationRunnerSessionId: "/private/runner" }],
  ])("rejects a %s from the destination runner resolver", async (_name, mismatch) => {
    const resolver: PortableCommandDestinationRunnerSessionResolver = {
      resolve: async ({
        commandExecutionClaimRef,
        destination,
        sourceBinding,
        capabilityTransfers,
      }) =>
        ({
          commandExecutionClaimRef,
          destinationTargetRef: destination.targetRef,
          sourceGrantRef: sourceBinding.grantRef,
          sourceLeaseRef: capabilityTransfers[0]!.sourceLeaseRef,
          destinationSourceGrantRef: capabilityTransfers[0]!.destinationSourceGrantRef,
          destinationRunnerSessionId: "runner.destination.valid",
          ...mismatch,
        }) as PortableCommandDestinationRunnerSessionResolution,
    };
    const fetch = authorityFetch();
    await expect(
      factory(exactResolver().resolver, fetch, resolver).create({
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toMatchObject({ code: "capability_mismatch" });
    expect(fetch).not.toHaveBeenCalled();
  });

  test("does not expose source revocation when destination runner resolution fails", async () => {
    const resolver: PortableCommandDestinationRunnerSessionResolver = {
      resolve: async () => {
        throw new Error("runner authority unavailable");
      },
    };
    const fetch = authorityFetch();
    await expect(
      factory(exactResolver().resolver, fetch, resolver).create({
        claim: claim(),
        source: local,
        destination: managed,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toMatchObject({ code: "runner_session_unavailable" });
    expect(fetch).not.toHaveBeenCalled();
  });

  test("maps different source grants to exact destination runner sessions", async () => {
    const secondBinding: PortableCommandGrantAuthorityBinding = {
      ...binding,
      sourceLeaseRef: "lease.ide13.command-broker.source-two",
      grantRef: "grant.ide13.command-broker.source-two",
      runnerSessionId: "runner.ide13.command-broker.source-two",
    };
    const secondTransfer: PortableCapabilityTransfer = {
      ...transfer,
      sourceLeaseRef: "lease.ide13.command-broker.source-two",
      destinationLeaseRef: "lease.ide13.command-broker.destination-two",
      destinationSourceGrantRef: "grant.ide13.command-broker.destination-two",
    };
    const byGrant = new Map([
      [binding.grantRef, { transfer, runnerSessionId: "runner.destination.one" }],
      [
        secondBinding.grantRef,
        { transfer: secondTransfer, runnerSessionId: "runner.destination.two" },
      ],
    ]);
    const destinationResolver: PortableCommandDestinationRunnerSessionResolver = {
      resolve: async ({ commandExecutionClaimRef, destination, sourceBinding }) => {
        const matched = byGrant.get(sourceBinding.grantRef);
        return matched === undefined
          ? null
          : {
              commandExecutionClaimRef,
              destinationTargetRef: destination.targetRef,
              sourceGrantRef: sourceBinding.grantRef,
              sourceLeaseRef: matched.transfer.sourceLeaseRef,
              destinationSourceGrantRef: matched.transfer.destinationSourceGrantRef,
              destinationRunnerSessionId: matched.runnerSessionId,
            };
      },
    };
    const requests: Array<Readonly<{ path: string; body: Record<string, unknown> }>> = [];
    const fetch = authorityFetch([], requests);
    const brokerFactory = createProductionPortableCommandBrokerFactory({
      grantAuthority: {
        baseUrl: "https://openagents.example",
        serviceBearer: "service-fixture-command-broker",
        fetch,
      },
      installationPorts: exactResolver().resolver,
      destinationRunnerSessions: destinationResolver,
    });
    const broker = await brokerFactory.create({
      claim: claim(),
      source: local,
      destination: managed,
      grantBindings: [binding, secondBinding],
      capabilityTransfers: [transfer, secondTransfer],
    });
    await broker.vault.revokeSourceGrant({
      sourceGrantRef: binding.grantRef,
      leaseRef: transfer.sourceLeaseRef,
    });
    await broker.vault.revokeSourceGrant({
      sourceGrantRef: secondBinding.grantRef,
      leaseRef: secondTransfer.sourceLeaseRef,
    });
    expect(
      requests
        .filter(({ path }) => path.endsWith("/reissue"))
        .map(({ body }) => ({
          sourceGrantRef: body.sourceGrantRef,
          destinationGrantRef: body.destinationGrantRef,
          runnerSessionId: body.runnerSessionId,
        })),
    ).toEqual([
      {
        sourceGrantRef: binding.grantRef,
        destinationGrantRef: transfer.destinationSourceGrantRef,
        runnerSessionId: "runner.destination.one",
      },
      {
        sourceGrantRef: secondBinding.grantRef,
        destinationGrantRef: secondTransfer.destinationSourceGrantRef,
        runnerSessionId: "runner.destination.two",
      },
    ]);
  });

  test("rejects a runnerless two-grant lease swap before any authority call", async () => {
    const secondTransfer: PortableCapabilityTransfer = {
      ...transfer,
      sourceLeaseRef: "lease.ide13.command-broker.github-two",
      destinationLeaseRef: "lease.ide13.command-broker.github-two-destination",
      destinationSourceGrantRef: "grant.ide13.command-broker.github-two-destination",
    };
    const firstGrantRef = "grant.ide13.command-broker.github-one";
    const secondGrantRef = "grant.ide13.command-broker.github-two";
    const swappedBindings: PortableCommandGrantAuthorityBinding[] = [
      {
        sourceLeaseRef: secondTransfer.sourceLeaseRef,
        grantRef: firstGrantRef,
        ownerUserId: ownerRef,
        kind: "github",
      },
      {
        sourceLeaseRef: transfer.sourceLeaseRef,
        grantRef: secondGrantRef,
        ownerUserId: ownerRef,
        kind: "github",
      },
    ];
    const fetch = authorityFetch();
    const broker = await factory(exactResolver().resolver, fetch).create({
      claim: claim(),
      source: local,
      destination: managed,
      grantBindings: swappedBindings,
      capabilityTransfers: [transfer, secondTransfer],
    });
    await expect(
      broker.vault.revokeSourceGrant({
        sourceGrantRef: firstGrantRef,
        leaseRef: transfer.sourceLeaseRef,
      }),
    ).rejects.toMatchObject({ code: "capability_mismatch" });
    expect(fetch).not.toHaveBeenCalled();
  });

  test("rejects an unsupported managed-provider target before adapter creation", async () => {
    const provider = descriptor(
      "target.ide13.command-broker.provider",
      "managed_provider",
      "adapter.ide13.command-broker.provider",
    );
    const resolution = exactResolver();
    await expect(
      factory(resolution.resolver).create({
        claim: claim(local, provider),
        source: local,
        destination: provider,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toBeInstanceOf(PortableCommandBrokerFactoryError);
    await expect(
      factory(resolution.resolver).create({
        claim: claim(local, provider),
        source: local,
        destination: provider,
        grantBindings: [binding],
        capabilityTransfers: [transfer],
      }),
    ).rejects.toMatchObject({ code: "target_unsupported" });
  });

  test("admits a managed-provider only with an exact resolved port identity", async () => {
    const provider = descriptor(
      "target.ide13.command-broker.provider-admitted",
      "managed_provider",
      "adapter.ide13.command-broker.provider-admitted",
    );
    const resolve = vi.fn<PortableCommandTargetInstallationPortResolver["resolve"]>(
      async ({ target }) => ({
        targetRef: target.targetRef,
        targetClass: target.targetClass,
        adapterRef: target.adapterRef,
        port: port(),
      }),
    );
    const broker = await factory({ resolve }).create({
      claim: claim(local, provider),
      source: local,
      destination: provider,
      grantBindings: [binding],
      capabilityTransfers: [transfer],
    });
    expect(broker.adapters).toHaveLength(2);
    expect(broker.adapters[1]).toMatchObject({
      adapterRef: provider.adapterRef,
      targetClass: "managed_provider",
    });
  });
});
