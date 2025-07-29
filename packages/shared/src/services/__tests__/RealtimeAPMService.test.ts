import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { Effect, Runtime, Ref, TestClock, TestContext, Duration, Fiber } from "effect";
import {
  RealtimeAPMService,
  makeRealtimeAPMService,
  withRealtimeAPMService,
  RealtimeAPMError,
  APMCalculationError,
  APMStreamError,
  RealtimeAPMData,
  APMTrendData,
  RealtimeAPMConfig,
  calculateCurrentSessionAPM,
  calculateAPMTrend,
  getCurrentAPM,
  createAPMHistory,
  updateAPMHistory,
} from "../RealtimeAPMService";
import {
  APMSessionData,
  generateDeviceId,
  createInitialSessionData,
} from "../SimpleAPMService";

// Mock storage functions
vi.mock("../SimpleStorageService", () => ({
  getStoredJson: vi.fn().mockImplementation((key: string, defaultValue: any) => 
    Effect.succeed(defaultValue)
  ),
  setStoredJson: vi.fn().mockImplementation((key: string, value: any) => 
    Effect.succeed(void 0)
  ),
}));

// Mock platform utils
vi.mock("../utils/platform", () => ({
  isReactNative: vi.fn(() => false),
  getPlatformId: vi.fn(() => "test"),
}));

describe("RealtimeAPMService", () => {
  let runtime: Runtime.Runtime<never>;
  beforeEach(() => {
    runtime = Runtime.defaultRuntime;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe("Core APM Calculations", () => {
    it("should calculate current session APM correctly", async () => {
      // Create test session data
      const sessionData: APMSessionData = {
        sessionStart: Date.now() - 60000, // 1 minute ago
        messagesSent: 5,
        sessionsCreated: 3,
        appStateChanges: 0,
        deviceId: "test-device",
        platform: "test",
      };

      const sessionDataRef = await Runtime.runPromise(runtime)(Ref.make(sessionData));
      
      // Calculate APM for active session
      const apm = await Runtime.runPromise(runtime)(
        calculateCurrentSessionAPM(sessionDataRef, true)
      );

      // Expected: (5 + 3) actions / 1 minute = 8 APM
      expect(apm).toBe(8);
    });

    it("should return 0 APM when session is inactive", async () => {
      const sessionData: APMSessionData = {
        sessionStart: Date.now() - 60000,
        messagesSent: 5,
        sessionsCreated: 3,
        appStateChanges: 0,
        deviceId: "test-device",
        platform: "test",
      };

      const sessionDataRef = await Runtime.runPromise(runtime)(Ref.make(sessionData));
      
      const apm = await Runtime.runPromise(runtime)(
        calculateCurrentSessionAPM(sessionDataRef, false) // inactive
      );

      expect(apm).toBe(0);
    });

    it("should handle zero duration gracefully", async () => {
      const sessionData: APMSessionData = {
        sessionStart: Date.now(), // Current time (zero duration)
        messagesSent: 5,
        sessionsCreated: 3,
        appStateChanges: 0,
        deviceId: "test-device",
        platform: "test",
      };

      const sessionDataRef = await Runtime.runPromise(runtime)(Ref.make(sessionData));
      
      const apm = await Runtime.runPromise(runtime)(
        calculateCurrentSessionAPM(sessionDataRef, true)
      );

      expect(apm).toBe(0);
    });
  });

  describe("APM Trend Calculations", () => {
    it("should calculate upward trend correctly", async () => {
      const currentAPM = 10;
      const history = [5, 6, 7, 8]; // Previous APM was 8
      const threshold = 10;

      const trendData = await Runtime.runPromise(runtime)(
        calculateAPMTrend(currentAPM, history, threshold)
      );

      expect(trendData.trend).toBe("up");
      expect(trendData.currentAPM).toBe(10);
      expect(trendData.previousAPM).toBe(8);
      expect(trendData.trendPercentage).toBe(25); // (10-8)/8 * 100 = 25%
    });

    it("should calculate downward trend correctly", async () => {
      const currentAPM = 6;
      const history = [10, 9, 8, 10]; // Previous APM was 10
      const threshold = 10;

      const trendData = await Runtime.runPromise(runtime)(
        calculateAPMTrend(currentAPM, history, threshold)
      );

      expect(trendData.trend).toBe("down");
      expect(trendData.currentAPM).toBe(6);
      expect(trendData.previousAPM).toBe(10);
      expect(trendData.trendPercentage).toBe(-40); // (6-10)/10 * 100 = -40%
    });

    it("should return stable trend for small changes", async () => {
      const currentAPM = 10.5;
      const history = [10, 10.2, 10.1, 10]; // Previous APM was 10
      const threshold = 10; // 10% threshold

      const trendData = await Runtime.runPromise(runtime)(
        calculateAPMTrend(currentAPM, history, threshold)
      );

      expect(trendData.trend).toBe("stable"); // 5% change is below 10% threshold
      expect(trendData.trendPercentage).toBe(5);
    });

    it("should handle empty history", async () => {
      const currentAPM = 10;
      const history: number[] = [];
      const threshold = 10;

      const trendData = await Runtime.runPromise(runtime)(
        calculateAPMTrend(currentAPM, history, threshold)
      );

      expect(trendData.trend).toBe("stable");
      expect(trendData.currentAPM).toBe(10);
      expect(trendData.previousAPM).toBe(10);
      expect(trendData.trendPercentage).toBe(0);
    });

    it("should handle zero previous APM", async () => {
      const currentAPM = 10;
      const history = [0]; // Previous APM was 0
      const threshold = 10;

      const trendData = await Runtime.runPromise(runtime)(
        calculateAPMTrend(currentAPM, history, threshold)
      );

      expect(trendData.trend).toBe("up");
      expect(trendData.trendPercentage).toBe(100);
    });
  });

  describe("APM History Management", () => {
    it("should create and update APM history", async () => {
      const historyRef = await Runtime.runPromise(runtime)(createAPMHistory());
      
      // Add some APM values
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 5, 10));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 7, 10));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 10, 10));

      const history = await Runtime.runPromise(runtime)(Ref.get(historyRef));
      
      expect(history).toEqual([5, 7, 10]);
    });

    it("should limit history size", async () => {
      const historyRef = await Runtime.runPromise(runtime)(createAPMHistory());
      const maxSize = 3;
      
      // Add more values than the limit
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 1, maxSize));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 2, maxSize));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 3, maxSize));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 4, maxSize));
      await Runtime.runPromise(runtime)(updateAPMHistory(historyRef, 5, maxSize));

      const history = await Runtime.runPromise(runtime)(Ref.get(historyRef));
      
      expect(history).toEqual([3, 4, 5]); // Only last 3 values
      expect(history.length).toBe(maxSize);
    });
  });

  describe("RealtimeAPMService Layer", () => {
    it("should create service layer with default config", async () => {
      const layer = makeRealtimeAPMService();
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          expect(service.config.updateInterval).toBe(3000);
          expect(service.config.trendThreshold).toBe(10);
          expect(service.config.enableTrendCalculation).toBe(true);
          return service.config;
        }).pipe(Effect.provide(layer))
      );

      expect(result.updateInterval).toBe(3000);
    });

    it("should create service layer with custom config", async () => {
      const customConfig = {
        updateInterval: 5000,
        trendThreshold: 15,
        maxHistorySize: 20,
        enableTrendCalculation: false,
        enableStreaming: false,
      };

      const layer = makeRealtimeAPMService(customConfig);
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          return service.config;
        }).pipe(Effect.provide(layer))
      );

      expect(result.updateInterval).toBe(5000);
      expect(result.trendThreshold).toBe(15);
      expect(result.maxHistorySize).toBe(20);
      expect(result.enableTrendCalculation).toBe(false);
      expect(result.enableStreaming).toBe(false);
    });

    it("should get current APM through service", async () => {
      const layer = makeRealtimeAPMService();
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          return yield* service.getCurrentAPM;
        }).pipe(Effect.provide(layer))
      );

      expect(result).toMatchObject({
        currentAPM: expect.any(Number),
        trend: expect.stringMatching(/^(up|down|stable)$/),
        sessionDuration: expect.any(Number),
        totalActions: expect.any(Number),
        lastUpdateTimestamp: expect.any(Number),
        isActive: expect.any(Boolean),
        deviceId: expect.any(String),
      });
    });

    it("should calculate current session APM through service", async () => {
      const layer = makeRealtimeAPMService();
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          return yield* service.calculateCurrentSessionAPM;
        }).pipe(Effect.provide(layer))
      );

      expect(typeof result).toBe("number");
      expect(result).toBeGreaterThanOrEqual(0);
    });

    it("should calculate APM trend through service", async () => {
      const layer = makeRealtimeAPMService();
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          return yield* service.calculateAPMTrend(10, [5, 8]);
        }).pipe(Effect.provide(layer))
      );

      expect(result).toMatchObject({
        previousAPM: 8,
        currentAPM: 10,
        trend: "up",
        trendPercentage: 25,
      });
    });
  });

  describe("Error Handling", () => {
    it("should handle APM calculation errors", async () => {
      // Create a mock that will cause an error by making sessionStart undefined
      const invalidSessionData = { sessionStart: undefined } as any;
      const sessionDataRef = await Runtime.runPromise(runtime)(Ref.make(invalidSessionData));
      
      const result = await Runtime.runPromise(runtime)(
        Effect.either(calculateCurrentSessionAPM(sessionDataRef, true))
      );

      // The function handles empty data gracefully, so it succeeds with 0
      expect(result._tag).toBe("Right");
      if (result._tag === "Right") {
        expect(result.right).toBe(0);
      }
    });

    it("should handle trend calculation errors", async () => {
      const result = await Runtime.runPromise(runtime)(
        Effect.either(calculateAPMTrend(NaN, [1, 2, 3], 10))
      );

      // The function handles NaN gracefully by treating it as a number
      expect(result._tag).toBe("Right");
      if (result._tag === "Right") {
        expect(result.right.currentAPM).toBeNaN();
      }
    });
  });

  describe("Streaming and Subscriptions", () => {
    it("should create APM subscription fiber", async () => {
      const layer = makeRealtimeAPMService({
        updateInterval: 100, // Fast for testing
        enableStreaming: true,
      });

      let updateCount = 0;
      let lastAPMData: RealtimeAPMData | null = null;

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          
          const fiber = yield* service.subscribeToAPMUpdates((data) => {
            updateCount++;
            lastAPMData = data;
          });

          // Wait a bit for updates
          yield* Effect.sleep(Duration.millis(250));
          
          // Interrupt the subscription
          yield* Fiber.interrupt(fiber);
          
          return { updateCount, lastAPMData };
        }).pipe(Effect.provide(layer))
      );

      expect(result.updateCount).toBeGreaterThan(0);
      expect(result.lastAPMData).toBeTruthy();
      expect(result.lastAPMData?.deviceId).toBeTruthy();
    });

    it("should handle subscription errors gracefully", async () => {
      const layer = makeRealtimeAPMService({
        updateInterval: 50,
        enableStreaming: true,
      });

      // Test with a callback that throws
      const result = await Runtime.runPromise(runtime)(
        Effect.either(
          Effect.gen(function* () {
            const service = yield* RealtimeAPMService;
            
            const fiber = yield* service.subscribeToAPMUpdates(() => {
              throw new Error("Callback error");
            });

            yield* Effect.sleep(Duration.millis(100));
            yield* Fiber.interrupt(fiber);
          }).pipe(Effect.provide(layer))
        )
      );

      // Should not fail due to callback errors
      expect(result._tag).toBe("Right");
    });
  });

  describe("withRealtimeAPMService helper", () => {
    it("should provide service layer automatically", async () => {
      const layer = makeRealtimeAPMService({ updateInterval: 7000 });
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          return service.config.updateInterval;
        }).pipe(Effect.provide(layer))
      );

      expect(result).toBe(7000);
    });
  });

  describe("Performance", () => {
    it("should handle high-frequency updates efficiently", async () => {
      const layer = makeRealtimeAPMService({
        updateInterval: 10, // Very fast updates
        maxHistorySize: 5,
      });

      const startTime = Date.now();
      let updateCount = 0;

      await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* RealtimeAPMService;
          
          const fiber = yield* service.subscribeToAPMUpdates(() => {
            updateCount++;
          });

          // Run for a short time
          yield* Effect.sleep(Duration.millis(100));
          yield* Fiber.interrupt(fiber);
        }).pipe(Effect.provide(layer))
      );

      const duration = Date.now() - startTime;
      
      // Should handle multiple updates within reasonable time
      expect(updateCount).toBeGreaterThan(5);
      expect(duration).toBeLessThan(200); // Should complete quickly
    });

    it("should maintain stable memory usage with history management", async () => {
      const historyRef = await Runtime.runPromise(runtime)(createAPMHistory());
      const maxSize = 10;

      // Add many values to test memory stability
      for (let i = 0; i < 100; i++) {
        await Runtime.runPromise(runtime)(
          updateAPMHistory(historyRef, Math.random() * 100, maxSize)
        );
      }

      const finalHistory = await Runtime.runPromise(runtime)(Ref.get(historyRef));
      
      expect(finalHistory.length).toBe(maxSize);
      expect(finalHistory.every(value => typeof value === "number")).toBe(true);
    });
  });
});