import { describe, test, expect } from "bun:test";
import {
  isMacOS,
  findBridgePath,
  checkFMHealth,
  createFMClient,
  DEFAULT_FM_PORT,
  type FMHealthResult,
} from "./foundation-models.js";
import { Effect } from "effect";

describe("Foundation Models", () => {
  describe("isMacOS", () => {
    test("returns boolean", () => {
      const result = isMacOS();
      expect(typeof result).toBe("boolean");
    });

    test("correctly detects platform", () => {
      const expected = process.platform === "darwin";
      expect(isMacOS()).toBe(expected);
    });
  });

  describe("findBridgePath", () => {
    test("returns string or null", () => {
      const result = findBridgePath();
      expect(result === null || typeof result === "string").toBe(true);
    });
  });

  describe("checkFMHealth", () => {
    test("returns health result without throwing", async () => {
      const result = await Effect.runPromise(
        checkFMHealth(DEFAULT_FM_PORT).pipe(
          Effect.catchAll((e) =>
            Effect.succeed({
              available: false,
              serverRunning: false,
              modelAvailable: false,
              error: e.message,
            } as FMHealthResult),
          ),
        ),
      );

      expect(result).toHaveProperty("available");
      expect(result).toHaveProperty("serverRunning");
      expect(result).toHaveProperty("modelAvailable");
      expect(typeof result.available).toBe("boolean");
    });

    test("fails gracefully on non-macOS", async () => {
      // This test only runs on non-macOS
      if (isMacOS()) {
        return; // Skip on macOS
      }

      const result = await Effect.runPromise(
        checkFMHealth().pipe(
          Effect.catchAll((e) =>
            Effect.succeed({
              available: false,
              serverRunning: false,
              modelAvailable: false,
              error: e.message,
            } as FMHealthResult),
          ),
        ),
      );

      expect(result.available).toBe(false);
      expect(result.error).toContain("macOS");
    });
  });

  describe("createFMClient", () => {
    test("creates client with default config", () => {
      const client = createFMClient();
      expect(client).toHaveProperty("config");
      expect(client).toHaveProperty("chat");
      expect(client.config.port).toBe(DEFAULT_FM_PORT);
    });

    test("creates client with custom port", () => {
      const client = createFMClient({ port: 12345 });
      expect(client.config.port).toBe(12345);
    });

    test("creates client with autoStart disabled", () => {
      const client = createFMClient({ autoStart: false });
      expect(client.config.autoStart).toBe(false);
    });

    test("chat returns error on non-macOS", async () => {
      // This test only runs on non-macOS
      if (isMacOS()) {
        return; // Skip on macOS
      }

      const client = createFMClient({ autoStart: false });

      const result = await Effect.runPromise(
        client.chat({ messages: [{ role: "user", content: "test" }] }).pipe(
          Effect.catchAll((e) => Effect.succeed({ error: e.reason })),
        ),
      );

      expect(result).toHaveProperty("error");
      expect((result as { error: string }).error).toBe("not_macos");
    });
  });
});

// Integration test - only runs if server is available
describe("Foundation Models Integration", () => {
  test.skipIf(!isMacOS())("can check health on macOS", async () => {
    const result = await Effect.runPromise(
      checkFMHealth().pipe(
        Effect.catchAll((e) =>
          Effect.succeed({
            available: false,
            serverRunning: false,
            modelAvailable: false,
            error: e.message,
          } as FMHealthResult),
        ),
      ),
    );

    // On macOS, we should get a valid result (server may or may not be running)
    expect(result).toHaveProperty("serverRunning");
    expect(typeof result.serverRunning).toBe("boolean");
  });
});
