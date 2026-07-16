import { describe, expect, it } from "vite-plus/test";
import { initializeAndAuth, type GrokAcpClient } from "./acp-client.ts";

describe("Grok ACP capability truthfulness", () => {
  it("advertises filesystem and terminal false by default", async () => {
    const requests: Array<{ method: string; params?: Record<string, unknown> }> = [];
    const client: GrokAcpClient = {
      request: async (method, params) => {
        requests.push({ method, ...(params === undefined ? {} : { params }) });
        return method === "initialize" ? { authMethods: [{ id: "cached_token" }] } : {};
      },
      onSessionUpdate: () => {},
      kill: () => {},
      pid: undefined,
    };
    await initializeAndAuth(client);
    expect(requests[0]).toMatchObject({
      method: "initialize",
      params: {
        clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
      },
    });
  });
});
