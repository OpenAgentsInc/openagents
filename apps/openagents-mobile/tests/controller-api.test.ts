import { buildQueuedCommand } from "@openagentsinc/client-command-outbox";
import { describe, expect, it } from "vite-plus/test";

import type {
  NativeSessionCredential,
  NativeSessionSecureStore,
} from "../src/auth/native-session-vault.ts";
import {
  fetchControllerBootstrap,
  fetchScreenshotHarnessBootstrap,
  makeControllerTransport,
  registerMobilePushDevice,
  screenshotGrantFromUrl,
  sendImmediateInterrupt,
} from "../src/controller/api.ts";

const original: NativeSessionCredential = {
  ownerUserId: "github:controller",
  accessToken: "access-original",
  refreshToken: "refresh-original",
};

const memoryStore = (): NativeSessionSecureStore & { value: string | null } => ({
  value: null,
  getItemAsync: async function () {
    return this.value;
  },
  setItemAsync: async function (_key, value) {
    this.value = value;
  },
  deleteItemAsync: async function () {
    this.value = null;
  },
});

describe("Pro mobile controller API", () => {
  it("exchanges only the dedicated deep-link grant for a real read-only bootstrap", async () => {
    expect(screenshotGrantFromUrl("openagents://harness?grant=signed.grant")).toBe("signed.grant");
    expect(screenshotGrantFromUrl("https://example.com/?grant=signed.grant")).toBeNull();
    const bootstrap = await fetchScreenshotHarnessBootstrap({
      grant: "signed.grant",
      fetch: (async (url: URL | RequestInfo, init?: RequestInit) => {
        expect(String(url)).toBe("https://pro.openagents.com/api/mobile/controller/harness");
        expect(JSON.parse(String(init?.body))).toEqual({ grant: "signed.grant" });
        return Response.json({
          version: "openagents.mobile_controller.v1",
          token: "read-only-convex-jwt",
          convexUrl: "https://convex.openagents.com",
          actor: { userId: "screenshot:fixture", name: "OpenAgents QA", avatarUrl: "" },
          workspace: { workspaceId: "screenshot:fixture", label: "Disposable QA workspace" },
        });
      }) as typeof fetch,
    });
    expect(bootstrap.workspace.workspaceId).toBe("screenshot:fixture");
  });

  it("accepts only the server-derived owner and persists token rotation", async () => {
    const store = memoryStore();
    const result = await fetchControllerBootstrap({
      credential: original,
      secureStore: store,
      fetch: (async (_url: URL | RequestInfo, init?: RequestInit) => {
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer access-original");
        return Response.json({
          version: "openagents.mobile_controller.v1",
          token: "convex-jwt",
          convexUrl: "https://convex.openagents.com",
          actor: { userId: original.ownerUserId, name: "Controller", avatarUrl: "" },
          workspace: { workspaceId: original.ownerUserId, label: "Personal workspace" },
          rotatedTokens: { access: "access-next", refresh: "refresh-next", expiresIn: 300 },
        });
      }) as typeof fetch,
    });
    expect(result.credential).toEqual({
      ownerUserId: original.ownerUserId,
      accessToken: "access-next",
      refreshToken: "refresh-next",
    });
    expect(store.value).not.toContain("convex-jwt");
  });

  it("delivers the exact durable outbox row and returns the authoritative receipt", async () => {
    const store = memoryStore();
    let credential = original;
    const queued = buildQueuedCommand({
      commandId: "cmd-mobile-api-1",
      operation: "thread.message.send",
      orderingKey: "thread:one",
      payload: { aggregateType: "thread", aggregateId: "one", text: "Continue" },
      createdAtMs: 1,
    });
    const transport = makeControllerTransport({
      credential: () => credential,
      updateCredential: (next) => {
        credential = next;
      },
      secureStore: store,
      fetch: (async (_url: URL | RequestInfo, init?: RequestInit) => {
        expect(JSON.parse(String(init?.body))).toEqual(queued);
        return Response.json({ status: "accepted", receiptRef: "evt_cmd-mobile-api-1" });
      }) as typeof fetch,
    });
    await expect(transport.send(queued)).resolves.toEqual({
      status: "accepted",
      receiptRef: "evt_cmd-mobile-api-1",
    });
  });

  it("uses the immediate lane for live interrupt without persisting it", async () => {
    const store = memoryStore();
    let observed: unknown;
    const receipt = await sendImmediateInterrupt({
      commandId: "cmd-interrupt-api-1",
      target: { aggregateType: "thread", aggregateId: "one", expectedGeneration: 2 },
      credential: original,
      updateCredential: () => undefined,
      secureStore: store,
      fetch: (async (_url: URL | RequestInfo, init?: RequestInit) => {
        observed = JSON.parse(String(init?.body));
        return Response.json({ status: "accepted", receiptRef: "evt_interrupt" });
      }) as typeof fetch,
    });
    expect(observed).toMatchObject({
      version: "openagents.mobile_controller.v1",
      operation: "runtime.interrupt",
    });
    expect(receipt).toEqual({ status: "accepted", receiptRef: "evt_interrupt" });
    expect(store.value).toBeNull();
  });

  it("registers an Expo address through the authenticated controller and persists rotation", async () => {
    const store = memoryStore();
    let observedUrl = "";
    const result = await registerMobilePushDevice({
      deviceId: "device.mobile-api",
      pushToken: "ExponentPushToken[mobile_api]",
      platform: "ios",
      credential: original,
      secureStore: store,
      fetch: (async (url: URL | RequestInfo, init?: RequestInit) => {
        observedUrl = String(url);
        expect(new Headers(init?.headers).get("authorization")).toBe("Bearer access-original");
        expect(JSON.parse(String(init?.body))).toEqual({
          deviceId: "device.mobile-api",
          provider: "expo",
          pushToken: "ExponentPushToken[mobile_api]",
          platform: "ios",
        });
        return Response.json({
          deviceId: "device.mobile-api",
          registered: true,
          rotatedTokens: { access: "access-next", refresh: "refresh-next", expiresIn: 300 },
        });
      }) as typeof fetch,
    });
    expect(observedUrl).toBe("https://pro.openagents.com/api/mobile/controller/device");
    expect(result.credential).toEqual({
      ownerUserId: original.ownerUserId,
      accessToken: "access-next",
      refreshToken: "refresh-next",
    });
  });
});
