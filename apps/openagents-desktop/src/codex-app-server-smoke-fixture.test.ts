import { describe, expect, test } from "vite-plus/test";

import { openCodexAppServerClient } from "./codex-app-server-client.ts";
import { makeCodexAppServerSmokeHarness } from "./codex-app-server-smoke-fixture.ts";

const waitFor = async (predicate: () => boolean): Promise<void> => {
  await new Promise<void>((resolve, reject) => {
    let attempts = 0;
    const poll = (): void => {
      if (predicate()) {
        resolve();
        return;
      }
      attempts += 1;
      if (attempts >= 100) {
        reject(new Error("fixture condition did not settle"));
        return;
      }
      setTimeout(poll, 1);
    };
    poll();
  });
};

describe("Codex app-server installed-smoke peer", () => {
  test("blocks completion until the exact provider-originated approval is accepted", async () => {
    const harness = makeCodexAppServerSmokeHarness();
    const answer: { current?: (value: unknown) => void } = {};
    const notifications: string[] = [];
    const client = openCodexAppServerClient({
      binary: "/packaged/codex",
      cwd: "/fixture/workspace",
      env: {},
      spawnImpl: harness.spawn,
      onServerRequest: (request) => {
        expect(request).toMatchObject({
          id: 91,
          method: "item/commandExecution/requestApproval",
          params: { command: "echo fixture" },
        });
        return new Promise((resolve) => {
          answer.current = resolve;
        });
      },
    });
    const unsubscribe = client.onNotification((message) => {
      if (typeof message.method === "string") notifications.push(message.method);
    });

    try {
      await client.initialize();
      await client.request("thread/start", {});
      await client.request("turn/start", {});
      await waitFor(() => harness.receipt().requestId === 91 && answer.current !== undefined);
      expect(harness.receipt()).toEqual({
        requestId: 91,
        decision: null,
        completionEmitted: false,
      });
      expect(notifications).not.toContain("turn/completed");

      if (answer.current === undefined) throw new Error("approval answer hook was not installed");
      answer.current({ decision: "accept" });
      await waitFor(() => harness.receipt().completionEmitted);
      expect(harness.receipt()).toEqual({
        requestId: 91,
        decision: "accept",
        completionEmitted: true,
      });
      expect(notifications).toContain("item/agentMessage/delta");
      expect(notifications).toContain("turn/completed");
    } finally {
      unsubscribe();
      client.close();
    }
  });
});
