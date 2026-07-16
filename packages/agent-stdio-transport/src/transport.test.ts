import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  AgentStdioTransport,
  AgentStdioTransportError,
  DEFAULT_AGENT_STDIO_LIMITS,
} from "./index.ts";

const fixture = resolve(import.meta.dirname, "fixture-agent.mjs");
const start = (
  mode = "normal",
  options: Partial<Parameters<typeof AgentStdioTransport.start>[0]> = {},
) =>
  AgentStdioTransport.start({
    executable: process.execPath,
    args: [fixture, mode],
    ...options,
  });

const waitFor = async (predicate: () => boolean, timeoutMs = 1_000): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() >= deadline) throw new Error("condition timed out");
    // eslint-disable-next-line no-await-in-loop -- bounded polling removes fixed-delay flakes.
    await new Promise((resolveWait) => setTimeout(resolveWait, 10));
  }
};

describe("bounded bidirectional agent stdio transport", () => {
  it("correlates concurrent requests while reverse permission, fs, and terminal handlers run", async () => {
    const transport = await start();
    for (const method of ["session/request_permission", "fs/read_text_file", "terminal/create"]) {
      transport.registerReverseHandler(method, async (params) => {
        await Promise.resolve();
        return { accepted: true, params };
      });
    }
    const concurrent = Array.from({ length: 20 }, (_, index) =>
      transport.request("test/echo", { index }),
    );
    const reverse = transport.request("test/reverse", {});
    await expect(Promise.all(concurrent)).resolves.toEqual(
      Array.from({ length: 20 }, (_, index) => ({ index })),
    );
    await expect(reverse).resolves.toMatchObject({
      permission: { accepted: true },
      filesystem: { accepted: true },
      terminal: { accepted: true },
    });
    expect(transport.getReceipt().counters.reverseRequests).toBe(3);
    await transport.dispose();
  });

  it.each(["fragmented", "coalesced"])(
    "handles %s frames, CRLF, blank lines, and multiple messages per chunk",
    async (mode) => {
      const transport = await start(mode);
      let updates = 0;
      transport.onNotification("session/update", () => {
        updates += 1;
      });
      await expect(transport.request("test/echo", { ok: true })).resolves.toEqual({ ok: true });
      if (mode === "coalesced") {
        await new Promise((resolveWait) => setImmediate(resolveWait));
        expect(updates).toBe(1);
      }
      await transport.dispose();
    },
  );

  it.each(["malformed", "binary"])("fails closed on %s protocol output", async (mode) => {
    const transport = await start(mode);
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(transport.state).toBe("failed");
    expect(transport.getReceipt()).toMatchObject({
      terminalOutcome: "protocol_violation",
      counters: { parseFailures: 1, protocolViolations: 1 },
    });
    await transport.dispose();
  });

  it("stops queued authority work after the first protocol violation", async () => {
    const transport = await start("invalid-then-reverse");
    let invoked = false;
    transport.registerReverseHandler("fs/read_text_file", () => {
      invoked = true;
      return { content: "must not run" };
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(transport.state).toBe("failed");
    expect(invoked).toBe(false);
    await transport.dispose();
  });

  it("reaps a failed child that ignores SIGTERM before disposal returns", async () => {
    const transport = await start("forced-malformed", {
      limits: { shutdownGraceMs: 10, terminateGraceMs: 10 },
    });
    const pid = transport.pid;
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(transport.state).toBe("failed");
    await transport.dispose();
    expect(transport.state).toBe("disposed");
    if (pid !== undefined) expect(() => process.kill(pid, 0)).toThrow();
  });

  it("enforces line, partial-buffer, and inbound-queue limits at exact boundaries", async () => {
    const exactLine = await start("sized", {
      args: [fixture, "sized", "128"],
      limits: { maxLineBytes: 128, maxBufferedBytes: 129 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(exactLine.state).toBe("running");
    await exactLine.dispose();

    const overLine = await start("sized", {
      args: [fixture, "sized", "128"],
      limits: { maxLineBytes: 127, maxBufferedBytes: 129 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(overLine.state).toBe("failed");
    await overLine.dispose();

    const exactBuffer = await start("partial", {
      args: [fixture, "partial", "128"],
      limits: { maxLineBytes: 256, maxBufferedBytes: 128 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(exactBuffer.state).toBe("running");
    await exactBuffer.dispose();

    const overBuffer = await start("partial", {
      args: [fixture, "partial", "128"],
      limits: { maxLineBytes: 256, maxBufferedBytes: 127 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(overBuffer.state).toBe("failed");
    await overBuffer.dispose();

    const exactQueue = await start("burst", {
      args: [fixture, "burst", "3"],
      limits: { maxInboundQueue: 3 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(exactQueue.state).toBe("running");
    await exactQueue.dispose();

    const overQueue = await start("burst", {
      args: [fixture, "burst", "4"],
      limits: { maxInboundQueue: 3 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(overQueue.state).toBe("failed");
    await overQueue.dispose();

    const coalescedBlank = await start("blank-burst", {
      args: [fixture, "blank-burst", "20"],
      limits: { maxBufferedBytes: 10 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(coalescedBlank.state).toBe("running");
    await coalescedBlank.dispose();
  });

  it("enforces in-flight and timeout limits and makes late replies harmless", async () => {
    const transport = await start("normal", {
      executable: process.execPath,
      args: [fixture, "normal"],
      limits: { maxInFlightRequests: 2, requestTimeoutMs: 15 },
    });
    const first = transport.request("test/never");
    const second = transport.request("test/late", { late: true });
    await expect(transport.request("test/echo")).rejects.toMatchObject({ kind: "overload" });
    await expect(first).rejects.toMatchObject({ kind: "timeout" });
    await expect(second).rejects.toMatchObject({ kind: "timeout" });
    await waitFor(() => transport.getReceipt().counters.unknownOrLateResponses === 1);
    expect(transport.getReceipt().counters).toMatchObject({
      requestsTimedOut: 2,
      unknownOrLateResponses: 1,
      overloads: 1,
    });
    await transport.dispose();
  });

  it("covers outbound queue, notification, stderr, evidence, and deadline boundaries", async () => {
    const blocked = await start("no-read", {
      args: [fixture, "no-read"],
      limits: {
        maxLineBytes: 300_000,
        maxOutboundQueue: 2,
        maxInFlightRequests: 10,
        requestTimeoutMs: 1_000,
        shutdownGraceMs: 10,
        terminateGraceMs: 10,
      },
    });
    const blockedRequests = Array.from({ length: 3 }, (_, index) =>
      blocked.request("test/never", { index, pad: "x".repeat(200_000) }).catch(() => undefined),
    );
    await expect(blocked.request("test/never", { pad: "x".repeat(200_000) })).rejects.toMatchObject(
      {
        kind: "overload",
      },
    );
    expect(blocked.getReceipt().counters.peakOutboundQueue).toBe(2);
    await blocked.dispose();
    await Promise.all(blockedRequests);

    const exactRate = await start("burst", {
      args: [fixture, "burst", "3"],
      limits: { maxNotificationsPerSecond: 3 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(exactRate.state).toBe("running");
    await exactRate.dispose();
    const overRate = await start("burst", {
      args: [fixture, "burst", "4"],
      limits: { maxNotificationsPerSecond: 3 },
    });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(overRate.state).toBe("failed");
    await overRate.dispose();

    const stderrText =
      "XAI_API_KEY=xai-secret-value token=cursor-login-token prompt=private-file-content\n";
    const stderrBytes = Buffer.byteLength(stderrText);
    const exactStderr = await start("stderr", { limits: { maxStderrBytes: stderrBytes } });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(exactStderr.getReceipt().counters).toMatchObject({
      stderrBytes,
      stderrDroppedBytes: 0,
    });
    await exactStderr.dispose();
    const overStderr = await start("stderr", { limits: { maxStderrBytes: stderrBytes - 1 } });
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
    expect(overStderr.getReceipt().counters.stderrDroppedBytes).toBe(1);
    await overStderr.dispose();

    const outbound = { jsonrpc: "2.0", id: 1, method: "test/echo", params: { x: "y" } };
    const outboundBytes = Buffer.byteLength(JSON.stringify(outbound));
    const exactEvidence = await start("normal", { limits: { maxEvidenceBytes: outboundBytes } });
    await exactEvidence.request("test/echo", { x: "y" });
    const exactRows = exactEvidence.readNativeEvidence(exactEvidence.authorizeNativeEvidence());
    expect(exactRows[0]?.raw).toEqual(outbound);
    expect(exactRows[1]?.raw).toBeUndefined();
    await exactEvidence.dispose();
    const overEvidence = await start("normal", { limits: { maxEvidenceBytes: outboundBytes - 1 } });
    await overEvidence.request("test/echo", { x: "y" });
    expect(
      overEvidence.readNativeEvidence(overEvidence.authorizeNativeEvidence())[0]?.raw,
    ).toBeUndefined();
    await overEvidence.dispose();

    const deadline = await start("normal", { limits: { requestTimeoutMs: 1 } });
    await expect(deadline.request("test/never")).rejects.toMatchObject({ kind: "timeout" });
    expect(deadline.getReceipt().counters).toMatchObject({
      requestsTimedOut: 1,
      currentInFlight: 0,
      peakInFlight: 1,
    });
    expect(deadline.getTraces()).toEqual(
      expect.arrayContaining([expect.objectContaining({ kind: "request_timeout" })]),
    );
    await deadline.dispose();
  });

  it("returns bounded reverse errors for missing handlers, timeout, and overload", async () => {
    const transport = await start("normal", {
      executable: process.execPath,
      args: [fixture, "normal"],
      limits: { maxReverseConcurrency: 1, reverseRequestTimeoutMs: 10 },
    });
    transport.registerReverseHandler("session/request_permission", () => new Promise(() => {}));
    await expect(transport.request("test/reverse", {}, { timeoutMs: 200 })).resolves.toMatchObject({
      permission: { code: -32_001 },
      filesystem: { code: -32_005 },
      terminal: { code: -32_005 },
    });
    expect(transport.getReceipt().counters).toMatchObject({ reverseTimeouts: 1, overloads: 2 });
    expect(transport.getReceipt().counters).toMatchObject({
      peakReverse: 1,
      currentReverse: 0,
    });
    await transport.dispose();
  });

  it("rejects invalid stable ACP params before an authority handler runs", async () => {
    const transport = await start();
    let invoked = false;
    transport.registerReverseHandler("fs/read_text_file", () => {
      invoked = true;
      return { content: "must not run" };
    });
    await expect(transport.request("test/invalid-reverse")).resolves.toMatchObject({
      code: -32_602,
      message: "invalid params",
    });
    expect(invoked).toBe(false);
    await transport.dispose();
  });

  it("separates and redacts stderr, argv, and environment while gating native evidence", async () => {
    const transport = await start("stderr", {
      executable: process.execPath,
      args: [fixture, "stderr", "--api-key=xai-secret-value", "--token", "cursor-secret"],
      env: { PATH: process.env.PATH, XAI_API_KEY: "must-not-appear" },
      versionProbeArgs: ["--version"],
      limits: { maxEvidenceEntries: 2, maxEvidenceBytes: 256, maxStderrBytes: 128 },
    });
    await transport.request("test/echo", { prompt: "private prompt" });
    const receiptText = JSON.stringify(transport.getReceipt());
    expect(receiptText).not.toContain("xai-secret-value");
    expect(receiptText).not.toContain("cursor-secret");
    expect(receiptText).not.toContain("must-not-appear");
    expect(receiptText).not.toContain("cursor-login-token");
    expect(receiptText).toContain("[REDACTED]");
    expect(() => transport.readNativeEvidence({})).toThrow("access denied");
    const evidence = transport.readNativeEvidence(transport.authorizeNativeEvidence());
    expect(evidence).toHaveLength(2);
    expect(evidence.some((entry) => entry.raw !== undefined)).toBe(true);
    await transport.dispose();

    const split = await start("split-stderr");
    await new Promise((resolveWait) => setTimeout(resolveWait, 50));
    expect(split.getReceipt().stderrExcerpt).not.toContain("secret-value");
    expect(split.getReceipt().stderrExcerpt).not.toContain("private-content");
    await split.dispose();
  });

  it("distinguishes client request cancellation from ACP session cancellation", async () => {
    const transport = await start();
    const controller = new AbortController();
    const request = transport.request("test/never", {}, { signal: controller.signal });
    controller.abort();
    await expect(request).rejects.toMatchObject({ kind: "cancelled" });
    transport.notify("session/cancel", { sessionId: "s-1" });
    expect(transport.getReceipt().counters.requestsCancelled).toBe(1);
    await transport.dispose();
  });

  it("records clean exit and crash as distinct terminal receipts", async () => {
    const clean = await start();
    await clean.shutdown();
    expect(clean.getReceipt()).toMatchObject({
      state: "exited",
      exitCode: 0,
      terminalOutcome: "clean_exit",
    });
    await clean.dispose();

    const crash = await start();
    const pending = crash.request("test/crash");
    await expect(pending).rejects.toMatchObject({ kind: "process_exit" });
    expect(crash.getReceipt()).toMatchObject({
      state: "failed",
      exitCode: 7,
      terminalOutcome: "crash",
    });
    await crash.dispose();
  });

  it("escalates graceful shutdown to forced termination exactly once", async () => {
    const transport = await start("forced", {
      executable: process.execPath,
      args: [fixture, "forced"],
      limits: { shutdownGraceMs: 10, terminateGraceMs: 10 },
    });
    await transport.request("test/echo", { ready: true });
    await transport.shutdown(["s-1"]);
    expect(transport.getReceipt().terminalOutcome).toBe("forced_termination");
    await transport.dispose();
    expect(transport.state).toBe("disposed");
  });

  it("cleans resources over repeated start, exit, crash, and dispose cycles", async () => {
    const heapBefore = process.memoryUsage().heapUsed;
    for (let index = 0; index < 20; index += 1) {
      // eslint-disable-next-line no-await-in-loop -- cycles must stay sequential to expose leaks.
      const transport = await start();
      // eslint-disable-next-line no-await-in-loop -- cycles must stay sequential to expose leaks.
      await expect(transport.request("test/echo", { index })).resolves.toEqual({ index });
      // eslint-disable-next-line no-await-in-loop -- cycles must stay sequential to expose leaks.
      await transport.dispose();
      expect(transport.state).toBe("disposed");
      expect(transport.getResourceDiagnostics()).toEqual({
        pending: 0,
        reverse: 0,
        inboundQueue: 0,
        outboundQueue: 0,
        stdoutBufferBytes: 0,
        nativeEvidenceEntries: 0,
        processListeners: 0,
        streamListeners: 0,
      });
    }
    expect(process.memoryUsage().heapUsed - heapBefore).toBeLessThan(64 * 1_024 * 1_024);
  });

  it("publishes exact trusted default limits", () => {
    expect(DEFAULT_AGENT_STDIO_LIMITS).toMatchObject({
      maxLineBytes: 1_048_576,
      maxBufferedBytes: 2_097_152,
      maxInboundQueue: 256,
      maxOutboundQueue: 256,
      maxInFlightRequests: 64,
      maxReverseConcurrency: 16,
      maxStderrBytes: 65_536,
      maxEvidenceEntries: 128,
      maxEvidenceBytes: 1_048_576,
    });
    expect(() => new AgentStdioTransportError("overload", "bounded")).not.toThrow();
  });
});
