import { describe, expect, test } from "vite-plus/test";

import {
  DapEventSchema,
  DapResponseSchema,
  DapTransportFailure,
  encodeDapProtocolMessage,
  makeDapMessageDecoder,
  makeDapRequestBroker,
  type DapRequest,
} from "./dap-transport.ts";

describe("IDE-11 DAP transport", () => {
  test("decodes fragmented UTF-8 frames and counts bytes instead of characters", () => {
    const decoder = makeDapMessageDecoder();
    const frame = encodeDapProtocolMessage(
      DapEventSchema.make({
        seq: 1,
        type: "event",
        event: "output",
        body: { output: "red panda 🐼" },
      }),
    );
    const messages = [
      ...decoder.push(frame.subarray(0, 1)),
      ...decoder.push(frame.subarray(1, 19)),
      ...decoder.push(frame.subarray(19, frame.length - 3)),
      ...decoder.push(frame.subarray(frame.length - 3)),
    ];
    expect(messages).toEqual([
      {
        seq: 1,
        type: "event",
        event: "output",
        body: { output: "red panda 🐼" },
      },
    ]);
    expect(decoder.bufferedBytes()).toBe(0);
    expect(() => decoder.finish()).not.toThrow();
  });

  test("decodes coalesced frames and retains an incomplete successor", () => {
    const decoder = makeDapMessageDecoder();
    const first = encodeDapProtocolMessage(
      DapEventSchema.make({
        seq: 1,
        type: "event",
        event: "initialized",
      }),
    );
    const second = encodeDapProtocolMessage(
      DapEventSchema.make({
        seq: 2,
        type: "event",
        event: "terminated",
      }),
    );
    const joined = Buffer.concat([first, second]);
    expect(decoder.push(joined.subarray(0, joined.length - 4))).toEqual([
      { seq: 1, type: "event", event: "initialized" },
    ]);
    expect(decoder.push(joined.subarray(joined.length - 4))).toEqual([
      { seq: 2, type: "event", event: "terminated" },
    ]);
    expect(() => decoder.finish()).not.toThrow();
  });

  test("rejects ambiguous and hostile Content-Length headers", () => {
    expect(() =>
      makeDapMessageDecoder().push(Buffer.from("Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}")),
    ).toThrow(/repeated/u);
    expect(() => makeDapMessageDecoder().push(Buffer.from("Content-Length: +2\r\n\r\n{}"))).toThrow(
      /invalid/u,
    );
    expect(() =>
      makeDapMessageDecoder().push(Buffer.from("Content-Length: 9007199254740992\r\n\r\n{}")),
    ).toThrow(/limit/u);
    expect(() =>
      makeDapMessageDecoder({ maxBodyBytes: 10 }).push(
        Buffer.from("Content-Length: 11\r\n\r\n12345678901"),
      ),
    ).toThrow(/limit/u);
    expect(() =>
      makeDapMessageDecoder().push(
        Buffer.from([
          ...Buffer.from("Content-L", "ascii"),
          0xff,
          ...Buffer.from("ength: 2\r\n\r\n{}", "ascii"),
        ]),
      ),
    ).toThrow(/ASCII/u);
  });

  test("bounds aggregate buffering and detects truncated frames", () => {
    const bounded = makeDapMessageDecoder({
      maxHeaderBytes: 32,
      maxBodyBytes: 64,
      maxBufferedBytes: 40,
    });
    expect(() => bounded.push(Buffer.alloc(41, 0x61))).toThrow(/buffered input/u);

    const decoder = makeDapMessageDecoder();
    decoder.push(Buffer.from('Content-Length: 12\r\n\r\n{"seq":'));
    expect(() => decoder.finish()).toThrow(/incomplete/u);
  });

  test("rejects invalid UTF-8, JSON syntax, and envelope shapes", () => {
    const invalidUtf8Body = Buffer.concat([
      Buffer.from('{"seq":1,"type":"event","event":"x","body":"', "ascii"),
      Buffer.from([0xff]),
      Buffer.from('"}', "ascii"),
    ]);
    const invalidUtf8 = Buffer.concat([
      Buffer.from(`Content-Length: ${invalidUtf8Body.byteLength}\r\n\r\n`, "ascii"),
      invalidUtf8Body,
    ]);
    expect(() => makeDapMessageDecoder().push(invalidUtf8)).toThrow(/UTF-8/u);
    expect(() => makeDapMessageDecoder().push(Buffer.from("Content-Length: 2\r\n\r\n{]"))).toThrow(
      /JSON/u,
    );
    expect(() => makeDapMessageDecoder().push(Buffer.from("Content-Length: 2\r\n\r\n[]"))).toThrow(
      /envelope/u,
    );
  });

  test("correlates out-of-order responses and rejects command substitution", async () => {
    const sent: DapRequest[] = [];
    const broker = makeDapRequestBroker({ onSend: (request) => sent.push(request) });
    const initialize = broker.request("initialize", { clientID: "openagents" });
    const launch = broker.request("launch", { noDebug: false });
    expect(broker.pendingCount()).toBe(2);
    expect(
      broker.accept(
        DapResponseSchema.make({
          seq: 91,
          type: "response",
          request_seq: launch.request.seq,
          success: true,
          command: "launch",
        }),
      ),
    ).toBe(true);
    await expect(launch.response).resolves.toMatchObject({ command: "launch", success: true });
    expect(
      broker.accept(
        DapResponseSchema.make({
          seq: 92,
          type: "response",
          request_seq: initialize.request.seq,
          success: true,
          command: "attach",
        }),
      ),
    ).toBe(true);
    await expect(initialize.response).rejects.toThrow(/does not match/u);
    expect(broker.pendingCount()).toBe(0);
    expect(sent.map((request) => request.seq)).toEqual([1, 2]);
  });

  test("times out deterministically and removes the scheduled callback", async () => {
    const scheduled = new Set<() => void>();
    const broker = makeDapRequestBroker({
      timeoutMs: 17,
      onSend: () => undefined,
      scheduleTimeout: (_timeoutMs, callback) => {
        scheduled.add(callback);
        return () => {
          scheduled.delete(callback);
        };
      },
    });
    const pending = broker.request("threads");
    expect(scheduled.size).toBe(1);
    const timeout = [...scheduled][0];
    expect(timeout).toBeDefined();
    timeout?.();
    await expect(pending.response).rejects.toThrow(/17 ms/u);
    expect(scheduled.size).toBe(0);
    expect(broker.pendingCount()).toBe(0);
  });

  test("cancels before and after send without accepting a late response", async () => {
    const controller = new AbortController();
    const sent: DapRequest[] = [];
    const broker = makeDapRequestBroker({ onSend: (request) => sent.push(request) });
    const pending = broker.request(
      "variables",
      { variablesReference: 1 },
      {
        signal: controller.signal,
      },
    );
    controller.abort();
    await expect(pending.response).rejects.toThrow(/cancelled/u);
    expect(broker.pendingCount()).toBe(0);
    expect(
      broker.accept(
        DapResponseSchema.make({
          seq: 4,
          type: "response",
          request_seq: pending.request.seq,
          success: true,
          command: "variables",
        }),
      ),
    ).toBe(false);
    expect(sent).toHaveLength(1);

    const alreadyAborted = new AbortController();
    alreadyAborted.abort();
    expect(() =>
      broker.request("threads", undefined, {
        signal: alreadyAborted.signal,
      }),
    ).toThrow(/before send/u);
  });

  test("bounds pending work, ignores unknown responses, and rejects all work on teardown", async () => {
    const broker = makeDapRequestBroker({
      maxPendingRequests: 1,
      onSend: () => undefined,
    });
    const pending = broker.request("threads");
    expect(() => broker.request("stackTrace")).toThrow(/pending request limit/u);
    expect(
      broker.accept(
        DapResponseSchema.make({
          seq: 10,
          type: "response",
          request_seq: 999,
          success: true,
          command: "threads",
        }),
      ),
    ).toBe(false);
    broker.failAll("adapter exited");
    await expect(pending.response).rejects.toThrow(/adapter exited/u);
    expect(broker.pendingCount()).toBe(0);
  });

  test("turns synchronous send failures into typed request failures", async () => {
    const broker = makeDapRequestBroker({
      onSend: () => {
        throw new Error("closed pipe");
      },
    });
    const pending = broker.request("initialize");
    await expect(pending.response).rejects.toMatchObject({
      _tag: "DapTransportFailure",
      phase: "request",
      retryable: true,
    } satisfies Partial<DapTransportFailure>);
    expect(broker.pendingCount()).toBe(0);
  });
});
