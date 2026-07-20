import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

import { openDapClient, type DapClient, type DapClientOptions } from "./dap-client.ts";
import type { DapEvent, DapTimeoutScheduler } from "./dap-transport.ts";

const fixture = fileURLToPath(
  new URL("../../scripts/fixtures/ide-dap-fixture.cjs", import.meta.url),
);

const ignoreDeferredValue = <A>(_value: A): void => undefined;

const makeDeferred = <A>(): Readonly<{
  promise: Promise<A>;
  resolve: (value: A) => void;
}> => {
  let resolveValue = ignoreDeferredValue<A>;
  const promise = new Promise<A>((resolve) => {
    resolveValue = resolve;
  });
  return { promise, resolve: resolveValue };
};

const waitForEvent = (
  events: ReadonlyArray<DapEvent>,
  listeners: Set<(event: DapEvent) => void>,
  name: string,
): Promise<DapEvent> => {
  const existing = events.findLast((event) => event.event === name);
  if (existing !== undefined) return Promise.resolve(existing);
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`Timed out waiting for DAP ${name}.`)), 5_000);
    const listener = (event: DapEvent): void => {
      if (event.event !== name) return;
      clearTimeout(timer);
      listeners.delete(listener);
      resolve(event);
    };
    listeners.add(listener);
  });
};

interface FixtureClient {
  readonly client: DapClient;
  readonly events: ReadonlyArray<DapEvent>;
  readonly listeners: Set<(event: DapEvent) => void>;
  readonly exit: Promise<
    Readonly<{
      code: number | null;
      signal: NodeJS.Signals | null;
      stderr: string;
    }>
  >;
}

const openFixtureClient = (
  flags: ReadonlyArray<string> = [],
  overrides: Partial<Omit<DapClientOptions, "launch" | "onEvent" | "onExit">> = {},
): FixtureClient => {
  const events: DapEvent[] = [];
  const listeners = new Set<(event: DapEvent) => void>();
  const exit = makeDeferred<
    Readonly<{
      code: number | null;
      signal: NodeJS.Signals | null;
      stderr: string;
    }>
  >();
  const client = openDapClient({
    launch: {
      executable: process.execPath,
      argv: [fixture, ...flags],
      cwd: process.cwd(),
      environment: { PATH: process.env.PATH ?? "" },
      timeoutMs: 5_000,
    },
    onEvent: (event) => {
      events.push(event);
      for (const listener of listeners) listener(event);
    },
    onExit: exit.resolve,
    ...overrides,
  });
  return { client, events, listeners, exit: exit.promise };
};

const initialize = async (client: DapClient): Promise<void> => {
  await client.request("initialize", {
    clientID: "openagents",
    clientName: "OpenAgents",
    adapterID: "fixture",
    pathFormat: "path",
    linesStartAt1: true,
    columnsStartAt1: true,
  });
};

describe("IDE-11 DAP child client", () => {
  test("runs a fragmented real exchange, rejects reverse requests, and drains pending work", async () => {
    const fixtureClient = openFixtureClient(
      ["--fragment-writes", "--reverse-request", "--stderr-burst"],
      { maxStderrBytes: 1_024 },
    );
    const { client, events, listeners } = fixtureClient;
    try {
      const initialized = waitForEvent(events, listeners, "initialized");
      const initializeResponse = await client.request("initialize", {
        clientID: "openagents",
        clientName: "OpenAgents",
        adapterID: "fixture",
        pathFormat: "path",
        linesStartAt1: true,
        columnsStartAt1: true,
      });
      expect(initializeResponse.body).toMatchObject({
        supportsConfigurationDoneRequest: true,
        supportsSetVariable: true,
        supportsRestartRequest: true,
      });
      await initialized;
      await expect(waitForEvent(events, listeners, "output")).resolves.toMatchObject({
        body: { output: "reverse-request-rejected\n" },
      });

      await client.request("attach", { target: "fixture" });
      const stopped = waitForEvent(events, listeners, "stopped");
      await client.request("configurationDone");
      await expect(stopped).resolves.toMatchObject({ event: "stopped" });
      await expect(client.request("threads")).resolves.toMatchObject({
        body: { threads: [{ id: 1, name: "Fixture Main Thread" }] },
      });

      const terminated = waitForEvent(events, listeners, "terminated");
      await client.request("disconnect", { restart: false, terminateDebuggee: true });
      await terminated;
      const exit = await fixtureClient.exit;
      expect(exit.code).toBe(0);
      expect(Buffer.byteLength(exit.stderr, "utf8")).toBeLessThanOrEqual(1_024);
      expect(exit.stderr).toContain("«redacted»");
      expect(exit.stderr).not.toContain("fixture-secret-token");
      expect(client.pendingRequestCount()).toBe(0);
      expect(client.isExited()).toBe(true);
    } finally {
      await client.dispose("test complete");
    }
  });

  test("cancels and times out one ignored request without disturbing later work", async () => {
    const scheduled = new Set<() => void>();
    const scheduler: DapTimeoutScheduler = (_milliseconds, callback) => {
      scheduled.add(callback);
      return () => {
        scheduled.delete(callback);
      };
    };
    const fixtureClient = openFixtureClient(["--ignore-threads"], {
      scheduleRequestTimeout: scheduler,
    });
    const { client } = fixtureClient;
    try {
      await initialize(client);
      await client.request("attach", { target: "fixture" });

      const controller = new AbortController();
      const cancelled = client.request("threads", undefined, { signal: controller.signal });
      controller.abort();
      await expect(cancelled).rejects.toMatchObject({
        _tag: "DapTransportFailure",
        phase: "cancel",
      });
      expect(client.pendingRequestCount()).toBe(0);
      expect(scheduled.size).toBe(0);

      const timedOut = client.request("threads");
      expect(client.pendingRequestCount()).toBe(1);
      const timeout = [...scheduled][0];
      expect(timeout).toBeDefined();
      timeout?.();
      await expect(timedOut).rejects.toMatchObject({
        _tag: "DapTransportFailure",
        phase: "response",
        retryable: true,
      });
      expect(client.pendingRequestCount()).toBe(0);
      expect(scheduled.size).toBe(0);

      await expect(client.request("evaluate", { expression: "counter" })).resolves.toMatchObject({
        body: { result: "7" },
      });
      await client.request("disconnect");
      await fixtureClient.exit;
    } finally {
      await client.dispose("timeout and cancellation test complete");
    }
  });

  test("terminates an adapter after malformed output", async () => {
    const fixtureClient = openFixtureClient(["--malformed-after-initialize"], {
      terminateGraceMs: 100,
      killGraceMs: 500,
    });
    try {
      await initialize(fixtureClient.client);
      const exit = await fixtureClient.exit;
      expect(exit.signal === "SIGTERM" || exit.code !== 0).toBe(true);
      expect(fixtureClient.client.isExited()).toBe(true);
      expect(fixtureClient.client.pendingRequestCount()).toBe(0);
    } finally {
      await fixtureClient.client.dispose("malformed output test complete");
    }
  });

  test("escalates process-group teardown from TERM to KILL", async () => {
    const fixtureClient = openFixtureClient(["--ignore-sigterm"], {
      terminateGraceMs: 25,
      killGraceMs: 1_000,
    });
    await initialize(fixtureClient.client);
    const firstDispose = fixtureClient.client.dispose("force teardown test");
    const secondDispose = fixtureClient.client.dispose("duplicate teardown request");
    expect(secondDispose).toBe(firstDispose);
    await firstDispose;
    const exit = await fixtureClient.exit;
    expect(exit.signal).toBe("SIGKILL");
    expect(exit.stderr).toContain("ignored SIGTERM");
    expect(fixtureClient.client.isExited()).toBe(true);
    expect(fixtureClient.client.pendingRequestCount()).toBe(0);
  });
});
