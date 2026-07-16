import { describe, expect, test } from "vite-plus/test";

import { drainChildRuntimes } from "./update-runtime-drain.ts";

describe("bounded Desktop child-runtime drain", () => {
  test("drains every declared class and treats absent platform classes as empty", async () => {
    const calls: string[] = [];
    const receipt = await drainChildRuntimes({
      timeoutMs: 100,
      drainers: [
        {
          kind: "agent",
          drain: () => {
            calls.push("agent");
          },
        },
        {
          kind: "pty",
          drain: async () => {
            calls.push("pty");
          },
        },
      ],
    });
    expect(receipt.ok).toBe(true);
    expect(receipt.timedOut).toEqual([]);
    expect(receipt.drained).toEqual(["agent", "pty", "local_server", "helper", "window", "wsl"]);
    expect(calls.toSorted()).toEqual(["agent", "pty"]);
  });

  test("fails closed with the exact wedged classes without waiting serially", async () => {
    const receipt = await drainChildRuntimes({
      timeoutMs: 5,
      drainers: [
        { kind: "agent", drain: () => new Promise<void>(() => {}) },
        {
          kind: "pty",
          drain: () => {
            throw new Error("wedged");
          },
        },
        { kind: "helper", drain: () => undefined },
      ],
    });
    expect(receipt.ok).toBe(false);
    expect(receipt.timedOut).toEqual(["agent", "pty"]);
    expect(receipt.drained).toContain("helper");
  });
});
