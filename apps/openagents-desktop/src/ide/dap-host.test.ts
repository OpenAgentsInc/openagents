import { createHash } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  IdeDebugBreakpointRefSchema,
  IdeDebugBreakpointSchema,
  IdeDebugOperationRefSchema,
  type IdeDebugConfiguration,
} from "./debug-contract.ts";
import { ideDebugFixtureConfiguration } from "./debug-fixture.ts";
import {
  IdeDapAdapterResolutionSchema,
  IdeDapDiscoveredConfigurationSchema,
  ideDebugBindingFor,
  openIdeDapHost,
  type IdeDapDiscoveredConfiguration,
  type IdeDapHost,
} from "./dap-host.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

const owner = { _tag: "Human" as const, actorRef: "owner.desktop" };
let operationSequence = 0;
const operationRef = () =>
  IdeDebugOperationRefSchema.make(`ide.debug-operation.host-${++operationSequence}`);
const configurationDigest = (configuration: IdeDebugConfiguration): string =>
  createHash("sha256").update(JSON.stringify(configuration)).digest("hex");
const fixtureAdapter = fileURLToPath(
  new URL("../../scripts/fixtures/ide-dap-fixture.cjs", import.meta.url),
);

const localPermit = (grantRef: string): IdePortableMutationPermit => ({
  _tag: "LocalOnly",
  key: `local:${grantRef}:session.fixture:work.fixture`,
  grantRef,
  sessionRef: "session.fixture",
  workContextRef: "work.fixture",
  attachmentRef: null,
  generation: null,
  targetRef: null,
});
const immutableMutationAuthority: IdePortableMutationAuthority = {
  authorize: (grantRef) => ({ _tag: "Permitted", permit: localPermit(grantRef) }),
  reauthorize: () => true,
};
const revocableMutationAuthority = () => {
  let live = true;
  return {
    authority: {
      authorize: (grantRef: string) =>
        live
          ? ({ _tag: "Permitted", permit: localPermit(grantRef) } as const)
          : ({ _tag: "Refused", reason: "sync_unavailable" } as const),
      reauthorize: (permit: IdePortableMutationPermit) =>
        live && permit.key === localPermit(permit.grantRef).key,
    } satisfies IdePortableMutationAuthority,
    revoke: () => {
      live = false;
    },
  };
};
const processIsAlive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const temporaryRoots: Array<string> = [];
const openedHosts: Array<IdeDapHost> = [];

afterEach(async () => {
  await Promise.all(openedHosts.splice(0).map((host) => host.dispose()));
  await Promise.all(
    temporaryRoots.splice(0).map((root) => rm(root, { recursive: true, force: true })),
  );
});

const fixtureEntry = (
  root: string,
  grantRef: string,
  mode: "launch" | "attach" = "attach",
  adapterArguments: ReadonlyArray<string> = [],
): IdeDapDiscoveredConfiguration => {
  const source = ideDebugFixtureConfiguration(mode);
  const configuration = {
    ...source,
    binding: ideDebugBindingFor({ root, grantRef }),
  };
  return IdeDapDiscoveredConfigurationSchema.make({
    configuration,
    resolution: IdeDapAdapterResolutionSchema.make({
      configurationRef: configuration.configurationRef,
      configurationDigest: configurationDigest(configuration),
      executable: process.execPath,
      argv: [fixtureAdapter, ...adapterArguments],
      cwd: root,
      environment: { PATH: process.env.PATH ?? "" },
      adapterId: "fixture",
      startCommand: mode,
      startArguments: { target: "fixture" },
    }),
  });
};

const openFixtureHost = async (): Promise<
  Readonly<{
    host: IdeDapHost;
    root: string;
    persistenceRoot: string;
    entry: IdeDapDiscoveredConfiguration;
  }>
> => {
  const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-host-"));
  const persistenceRoot = path.join(root, "persistence");
  temporaryRoots.push(root);
  const grantRef = "grant.fixture";
  const entry = fixtureEntry(root, grantRef);
  const host = await openIdeDapHost({
    workspace: () => ({ root, grantRef }),
    mutationAuthority: immutableMutationAuthority,
    discoverConfigurations: () => [entry],
    persistenceRoot,
  });
  openedHosts.push(host);
  return { host, root, persistenceRoot, entry };
};

describe("IDE-11 DAP host", () => {
  test("refuses a portable-authority start before creating an adapter process", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-refused-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-refused";
    const entry = fixtureEntry(root, grantRef);
    let openCount = 0;
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: {
        authorize: () => ({ _tag: "Refused", reason: "sync_unavailable" }),
        reauthorize: () => false,
      },
      discoverConfigurations: () => [entry],
      openClient: () => {
        openCount += 1;
        throw new Error("the adapter must not open");
      },
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });

    const result = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });

    expect(result).toMatchObject({ _tag: "Refused", reason: "stale_generation" });
    expect(openCount).toBe(0);
  });

  test("kills an adapter exactly once when authority changes inside spawn", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-spawn-revoke-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-spawn-revoke";
    const entry = fixtureEntry(root, grantRef);
    const portable = revocableMutationAuthority();
    let disposeCount = 0;
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: portable.authority,
      discoverConfigurations: () => [entry],
      openClient: () => {
        portable.revoke();
        return {
          pid: 4242,
          request: async () => {
            throw new Error("a revoked adapter must not receive requests");
          },
          dispose: async () => {
            disposeCount += 1;
          },
          drainEvents: async () => undefined,
          pendingRequestCount: () => 0,
          isExited: () => false,
        };
      },
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });

    const result = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });

    expect(result).toMatchObject({ _tag: "Refused", reason: "stale_generation" });
    expect(disposeCount).toBe(1);
    await host.dispose();
    expect(disposeCount).toBe(1);
  });

  test("drops a stale active generation and tears down its adapter process", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-active-revoke-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-active-revoke";
    const entry = fixtureEntry(root, grantRef);
    const portable = revocableMutationAuthority();
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: portable.authority,
      discoverConfigurations: () => [entry],
      persistenceRoot: path.join(root, "persistence"),
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    if (started?._tag !== "Succeeded") throw new Error("The fixture DAP session did not start.");
    const session = started.snapshot.sessions.at(-1);
    const pid =
      started.payload !== null &&
      typeof started.payload === "object" &&
      "pid" in started.payload &&
      typeof started.payload.pid === "number"
        ? started.payload.pid
        : null;
    if (session === undefined || pid === null)
      throw new Error("The active DAP identity is absent.");

    portable.revoke();
    const stale = await host.command({
      _tag: "Control",
      operationRef: operationRef(),
      sessionRef: session.sessionRef,
      sessionGeneration: session.sessionGeneration,
      adapterGeneration: session.adapterGeneration,
      targetGeneration: session.targetGeneration,
      operation: "continue",
      actor: owner,
    });

    expect(stale).toMatchObject({ _tag: "Refused", reason: "stale_generation" });
    const deadline = Date.now() + 2_000;
    while (processIsAlive(pid) && Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(processIsAlive(pid)).toBe(false);
    expect((await host.snapshot())?.sessions).toEqual([]);
    expect(host.pendingRequestCount()).toBe(0);
  });

  test("resets adapter-scoped source identities between sessions", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-session-sources-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-session-sources";
    const first = fixtureEntry(root, grantRef, "attach");
    const second = fixtureEntry(root, grantRef, "launch", [
      "--alternate-source",
      "--reject-empty-breakpoint-clear",
    ]);
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: immutableMutationAuthority,
      discoverConfigurations: () => [first, second],
      runTask: async () => true,
      persistenceRoot: path.join(root, "persistence"),
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const firstStarted = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: first.configuration.configurationRef,
      actor: owner,
    });
    if (firstStarted?._tag !== "Succeeded") throw new Error("The first session did not start.");
    const firstSession = firstStarted.snapshot.sessions.at(-1);
    const firstLocation = firstSession?.frames[0]?.location;
    if (firstSession === undefined || firstLocation === null || firstLocation === undefined)
      throw new Error("The first fixture location is absent.");
    const firstBreakpoint = IdeDebugBreakpointSchema.cases.Source.make({
      breakpointRef: IdeDebugBreakpointRefSchema.make("ide.debug-breakpoint.first-session"),
      enabled: true,
      condition: null,
      hitCondition: null,
      logMessage: null,
      verified: false,
      message: null,
      location: firstLocation,
      requestedLine: firstLocation.line,
      sourceVersion: null,
    });
    const firstReplaced = await host.command({
      _tag: "ReplaceBreakpoints",
      operationRef: operationRef(),
      sessionRef: firstSession.sessionRef,
      sessionGeneration: firstSession.sessionGeneration,
      adapterGeneration: firstSession.adapterGeneration,
      targetGeneration: firstSession.targetGeneration,
      breakpoints: [firstBreakpoint],
      actor: owner,
    });
    if (firstReplaced?._tag !== "Succeeded") throw new Error("The first breakpoint was refused.");
    const firstChanged = firstReplaced.snapshot.sessions.at(-1);
    if (firstChanged === undefined) throw new Error("The first changed session is absent.");
    await host.command({
      _tag: "Control",
      operationRef: operationRef(),
      sessionRef: firstChanged.sessionRef,
      sessionGeneration: firstChanged.sessionGeneration,
      adapterGeneration: firstChanged.adapterGeneration,
      targetGeneration: firstChanged.targetGeneration,
      operation: "disconnect",
      actor: owner,
    });
    const secondStarted = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: second.configuration.configurationRef,
      actor: owner,
    });
    if (secondStarted?._tag !== "Succeeded") throw new Error("The second session did not start.");
    const secondSession = secondStarted.snapshot.sessions.at(-1);
    const secondLocation = secondSession?.frames[0]?.location;
    if (secondSession === undefined || secondLocation === null || secondLocation === undefined)
      throw new Error("The second fixture location is absent.");
    const replaced = await host.command({
      _tag: "ReplaceBreakpoints",
      operationRef: operationRef(),
      sessionRef: secondSession.sessionRef,
      sessionGeneration: secondSession.sessionGeneration,
      adapterGeneration: secondSession.adapterGeneration,
      targetGeneration: secondSession.targetGeneration,
      breakpoints: [
        IdeDebugBreakpointSchema.cases.Source.make({
          breakpointRef: IdeDebugBreakpointRefSchema.make("ide.debug-breakpoint.second-session"),
          enabled: true,
          condition: null,
          hitCondition: null,
          logMessage: null,
          verified: false,
          message: null,
          location: secondLocation,
          requestedLine: secondLocation.line,
          sourceVersion: null,
        }),
      ],
      actor: owner,
    });
    expect(replaced?._tag).toBe("Succeeded");
  });

  test("projects scope variables without eager recursive expansion", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-scope-variables-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-scope-variables";
    const entry = fixtureEntry(root, grantRef, "attach", ["--nested-variables"]);
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: immutableMutationAuthority,
      discoverConfigurations: () => [entry],
      persistenceRoot: path.join(root, "persistence"),
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    if (started?._tag !== "Succeeded") throw new Error("The fixture session did not start.");
    const variables = started.snapshot.sessions[0]?.variables ?? [];
    expect(variables.some((variable) => variable.name === "nested")).toBe(true);
    expect(variables.some((variable) => variable.name === "nested-value")).toBe(false);
    expect(variables.every((variable) => variable.parentRef === null)).toBe(true);
  });

  test("completes owned teardown when an adapter refuses disconnect", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-refused-disconnect-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-refused-disconnect";
    const entry = fixtureEntry(root, grantRef, "attach", ["--reject-disconnect"]);
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: immutableMutationAuthority,
      discoverConfigurations: () => [entry],
      persistenceRoot: path.join(root, "persistence"),
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    if (started?._tag !== "Succeeded") throw new Error("The fixture session did not start.");
    const session = started.snapshot.sessions[0];
    if (session === undefined) throw new Error("The fixture session is absent.");
    const disconnected = await host.command({
      _tag: "Control",
      operationRef: operationRef(),
      sessionRef: session.sessionRef,
      sessionGeneration: session.sessionGeneration,
      adapterGeneration: session.adapterGeneration,
      targetGeneration: session.targetGeneration,
      operation: "disconnect",
      actor: owner,
    });
    expect(disconnected?._tag).toBe("Succeeded");
    expect(
      disconnected?._tag === "Succeeded" ? disconnected.snapshot.sessions[0]?.lifecycle._tag : null,
    ).toBe("Disconnected");
    expect(host.pendingRequestCount()).toBe(0);
  });

  test("completes configuration before awaiting a delayed launch response", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "openagents-ide-dap-delayed-launch-"));
    temporaryRoots.push(root);
    const grantRef = "grant.fixture-delayed-launch";
    const entry = fixtureEntry(root, grantRef, "launch", ["--delay-start-response"]);
    const host = await openIdeDapHost({
      workspace: () => ({ root, grantRef }),
      mutationAuthority: immutableMutationAuthority,
      discoverConfigurations: () => [entry],
      runTask: async () => true,
      persistenceRoot: path.join(root, "persistence"),
    });
    openedHosts.push(host);
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    expect(started?._tag).toBe("Succeeded");
    expect(
      started?._tag === "Succeeded" ? started.snapshot.sessions[0]?.lifecycle._tag : null,
    ).toBe("Stopped");
  });

  test("discovers main-owned configurations and runs a real attach projection", async () => {
    const { host, entry, root } = await openFixtureHost();
    const discovered = await host.command({
      _tag: "Discover",
      operationRef: operationRef(),
      actor: owner,
    });
    expect(discovered?._tag).toBe("Succeeded");
    expect(discovered?._tag === "Succeeded" ? discovered.snapshot.configurations : []).toHaveLength(
      1,
    );

    const unknown = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: "ide.debug-config.renderer-invented",
      actor: owner,
    });
    expect(unknown).toMatchObject({ _tag: "Refused", reason: "not_admitted" });

    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    expect(started?._tag).toBe("Succeeded");
    if (started?._tag !== "Succeeded") throw new Error("The fixture session did not start.");
    const session = started.snapshot.sessions[0];
    expect(session?.configuration.configurationRef).toBe(entry.configuration.configurationRef);
    expect(session?.threads[0]?.name).toBe("Fixture Main Thread");
    expect(session?.frames.length).toBeGreaterThan(0);
    expect(session?.configuration.binding.projectRef).toBe(
      ideDebugBindingFor({
        root,
        grantRef: "grant.fixture",
      }).projectRef,
    );
    if (session === undefined) throw new Error("The fixture session is absent.");
    const restarted = await host.command({
      _tag: "Control",
      operationRef: operationRef(),
      sessionRef: session.sessionRef,
      sessionGeneration: session.sessionGeneration,
      adapterGeneration: session.adapterGeneration,
      targetGeneration: session.targetGeneration,
      operation: "restart_session",
      actor: owner,
    });
    expect(restarted?._tag).toBe("Succeeded");
    expect(
      restarted?._tag === "Succeeded" ? restarted.snapshot.sessions[0]?.adapterGeneration : null,
    ).toBe(2);
    expect(host.pendingRequestCount()).toBe(0);
  });

  test("persists breakpoint identities and deletes retained project data", async () => {
    const { host, root, persistenceRoot, entry } = await openFixtureHost();
    await host.command({ _tag: "Discover", operationRef: operationRef(), actor: owner });
    const started = await host.command({
      _tag: "Start",
      operationRef: operationRef(),
      configurationRef: entry.configuration.configurationRef,
      actor: owner,
    });
    if (started?._tag !== "Succeeded") throw new Error("The fixture session did not start.");
    const session = started.snapshot.sessions[0];
    const location = session?.frames[0]?.location;
    if (session === undefined || location === null || location === undefined)
      throw new Error("The fixture did not return a source location.");
    const breakpoint = IdeDebugBreakpointSchema.cases.Source.make({
      breakpointRef: IdeDebugBreakpointRefSchema.make("ide.debug-breakpoint.host-fixture"),
      enabled: true,
      condition: null,
      hitCondition: null,
      logMessage: null,
      verified: false,
      message: null,
      location,
      requestedLine: location.line,
      sourceVersion: null,
    });
    const replaced = await host.command({
      _tag: "ReplaceBreakpoints",
      operationRef: operationRef(),
      sessionRef: session.sessionRef,
      sessionGeneration: session.sessionGeneration,
      adapterGeneration: session.adapterGeneration,
      targetGeneration: session.targetGeneration,
      breakpoints: [breakpoint],
      actor: owner,
    });
    expect(replaced?._tag).toBe("Succeeded");
    expect(
      replaced?._tag === "Succeeded"
        ? replaced.snapshot.breakpointSets[0]?.breakpoints[0]?.breakpointRef
        : null,
    ).toBe(breakpoint.breakpointRef);

    await host.dispose();
    openedHosts.splice(openedHosts.indexOf(host), 1);
    const restored = await openIdeDapHost({
      workspace: () => ({ root, grantRef: "grant.fixture" }),
      mutationAuthority: immutableMutationAuthority,
      discoverConfigurations: () => [entry],
      persistenceRoot,
    });
    openedHosts.push(restored);
    expect((await restored.snapshot())?.breakpointSets[0]?.breakpoints[0]?.breakpointRef).toBe(
      breakpoint.breakpointRef,
    );
    const deleted = await restored.command({
      _tag: "DeleteRetainedData",
      operationRef: operationRef(),
      reason: "Owner requested deletion.",
      actor: owner,
    });
    expect(deleted?._tag === "Succeeded" ? deleted.snapshot.breakpointSets : []).toHaveLength(0);
  });
});
