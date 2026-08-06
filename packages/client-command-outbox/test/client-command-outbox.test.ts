import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  CLIENT_OPERATION_CATALOG,
  ClientCommandTransport,
  OfflineCommandRefusedError,
  ObservationCacheEntry,
  OutboxTransportError,
  authorizeImmediateCommand,
  buildQueuedCommand,
  drainClientOutbox,
  enqueueClientCommand,
  projectObservation,
} from "../src/index.ts";
import { makeMemoryOutbox } from "./memory.ts";

const online = {
  convexConnected: true,
  shellLive: true,
  decisionRevisions: {},
} as const;

describe("client command policy", () => {
  it("classifies the complete controller catalog", () => {
    expect(Object.keys(CLIENT_OPERATION_CATALOG)).toHaveLength(15);
    expect(new Set(Object.values(CLIENT_OPERATION_CATALOG))).toEqual(
      new Set(["durable_intent", "expiring_decision", "live_control", "destructive_git", "observation"]),
    );
  });

  it("matches the Pro web fingerprint fixture", () => {
    expect(
      buildQueuedCommand({
        commandId: "parity-1",
        operation: "thread.message.send",
        orderingKey: "thread-1",
        payload: { text: "hello" },
        createdAtMs: 1,
      }).fingerprint,
    ).toBe("sha256:8422a87a0591a4557c726c2fd5cb5f784731ab8dba833c8daf601cc1233a9e93");
  });

  it("never queues credentials, live controls, or destructive Git", () => {
    expect(() =>
      buildQueuedCommand({
        commandId: "cmd-secret",
        operation: "thread.message.send",
        orderingKey: "thread-1",
        payload: { access_token: "secret" },
        createdAtMs: 1,
      }),
    ).toThrow(/credential-shaped field/u);
    expect(() =>
      buildQueuedCommand({
        commandId: "cmd-interrupt",
        operation: "runtime.interrupt",
        orderingKey: "thread-1",
        payload: {},
        createdAtMs: 1,
      }),
    ).toThrow(/Only durable intents/u);
    expect(() => authorizeImmediateCommand({ operation: "runtime.interrupt", online: false })).toThrow(
      OfflineCommandRefusedError,
    );
    expect(() => authorizeImmediateCommand({ operation: "git.destructive.execute", online: true })).toThrow(
      /live preflight/u,
    );
  });
});

describe("client command outbox", () => {
  it("uses the same path online and offline, then delivers exactly once after reconnect", async () => {
    const outbox = makeMemoryOutbox();
    let sends = 0;
    const transport = Layer.succeed(ClientCommandTransport, {
      send: (command) =>
        Effect.sync(() => {
          sends += 1;
          return { status: "accepted" as const, receiptRef: `receipt.server.${command.commandId}` };
        }),
    });
    const layer = Layer.merge(outbox.layer, transport);
    const input = {
      commandId: "cmd-airplane-1",
      operation: "thread.message.send",
      orderingKey: "thread-1",
      payload: { text: "Ship it" },
      createdAtMs: 10,
    } as const;

    await Effect.runPromise(enqueueClientCommand(input).pipe(Effect.provide(layer)));
    await Effect.runPromise(
      drainClientOutbox({ ...online, convexConnected: false }, 20).pipe(Effect.provide(layer)),
    );
    expect(outbox.state.commands.size).toBe(1);
    expect(sends).toBe(0);

    const reloaded = buildQueuedCommand(input);
    expect(reloaded.fingerprint).toBe([...outbox.state.commands.values()][0]?.fingerprint);
    const summary = await Effect.runPromise(drainClientOutbox(online, 30).pipe(Effect.provide(layer)));
    expect(summary).toEqual({ delivered: 1, terminal: 0, pending: 0 });
    expect(sends).toBe(1);
    expect(outbox.state.receipts[0]?.status).toBe("accepted");

    await Effect.runPromise(drainClientOutbox(online, 40).pipe(Effect.provide(layer)));
    expect(sends).toBe(1);
  });

  it("preserves per-thread order while allowing another thread through", async () => {
    const outbox = makeMemoryOutbox();
    const seen: Array<string> = [];
    const transport = Layer.succeed(ClientCommandTransport, {
      send: (command) => {
        seen.push(command.commandId);
        return command.commandId === "a1"
          ? Effect.fail(new OutboxTransportError({ detail: "offline", retryable: true }))
          : Effect.succeed({ status: "accepted" as const, receiptRef: `receipt.${command.commandId}` });
      },
    });
    const layer = Layer.merge(outbox.layer, transport);
    for (const [commandId, orderingKey, createdAtMs] of [
      ["a1", "thread-a", 1],
      ["a2", "thread-a", 2],
      ["b1", "thread-b", 3],
    ] as const) {
      await Effect.runPromise(
        enqueueClientCommand({
          commandId,
          operation: "thread.message.send",
          orderingKey,
          payload: { text: commandId },
          createdAtMs,
        }).pipe(Effect.provide(layer)),
      );
    }
    await Effect.runPromise(drainClientOutbox(online, 5).pipe(Effect.provide(layer)));
    expect(seen).toEqual(["a1", "b1"]);
    expect([...outbox.state.commands.keys()]).toEqual(["a1", "a2"]);
  });

  it("fails expired decisions closed with a fresh-prompt receipt", async () => {
    const outbox = makeMemoryOutbox();
    const transport = Layer.succeed(ClientCommandTransport, {
      send: () => Effect.die("expired decisions must not reach transport"),
    });
    const layer = Layer.merge(outbox.layer, transport);
    await Effect.runPromise(
      enqueueClientCommand({
        commandId: "decision-1",
        operation: "approval.respond",
        orderingKey: "thread-1",
        payload: { decision: "approve" },
        createdAtMs: 1,
        decisionRevision: "rev-1",
        expiresAtMs: 10,
      }).pipe(Effect.provide(layer)),
    );
    await Effect.runPromise(
      drainClientOutbox({ ...online, decisionRevisions: { "approval.respond": "rev-1" } }, 10).pipe(
        Effect.provide(layer),
      ),
    );
    expect(outbox.state.commands.size).toBe(0);
    expect(outbox.state.receipts[0]).toMatchObject({ status: "expired", code: "fresh_decision_required" });
  });
});

describe("observation projection", () => {
  it("reports cache phase and age explicitly", () => {
    expect(
      projectObservation({
        entry: new ObservationCacheEntry({ key: "threads", valueJson: "[]", observedAtMs: 10 }),
        connected: false,
        synchronizing: false,
        nowMs: 25,
      }),
    ).toEqual({ phase: "cached", ageMs: 15, value: [] });
  });
});
