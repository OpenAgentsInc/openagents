import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { Effect, Runtime, Duration } from "effect";
import {
  ConvexRealtimeAPMService,
  makeConvexRealtimeAPMService,
  withConvexRealtimeAPMService,
  ConvexAPMError,
  ConvexConnectionError,
  trackActionInConvex,
  getRealtimeAPMFromConvex,
  subscribeToConvexAPMUpdates,
} from "../ConvexRealtimeAPMService";

// Mock ConvexClient
const mockConvexClient = {
  query: vi.fn(),
  mutation: vi.fn(),
  subscribe: vi.fn(),
};

// Mock RealtimeAPMService dependencies
vi.mock("../RealtimeAPMService", () => ({
  RealtimeAPMService: {
    pipe: vi.fn(),
  },
  makeRealtimeAPMService: vi.fn(),
  RealtimeAPMError: class extends Error {
    constructor(data: any) {
      super(data.message);
      this.name = "RealtimeAPMError";
    }
  },
  APMCalculationError: class extends Error {
    constructor(data: any) {
      super(data.message);
      this.name = "APMCalculationError";
    }
  },
}));

vi.mock("../SimpleAPMService", () => ({
  generateDeviceId: vi.fn(() => Effect.succeed("test-device-123")),
}));

describe("ConvexRealtimeAPMService", () => {
  let runtime: Runtime.Runtime<never>;

  beforeEach(() => {
    runtime = Runtime.defaultRuntime;
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Convex Integration Functions", () => {
    describe("trackActionInConvex", () => {
      it("should successfully track action in Convex", async () => {
        const mockResult = {
          success: true,
          newAPM: 5.5,
          totalActions: 10,
        };

        mockConvexClient.mutation.mockResolvedValueOnce(mockResult);

        const result = await Runtime.runPromise(runtime)(
          trackActionInConvex(mockConvexClient, "test-device", "message", { test: true })
        );

        expect(mockConvexClient.mutation).toHaveBeenCalledWith(
          "confect.apm.trackRealtimeAction",
          {
            deviceId: "test-device",
            actionType: "message",
            timestamp: expect.any(Number),
            metadata: { test: true },
          }
        );

        expect(result.newAPM).toBe(5.5);
        expect(result.totalActions).toBe(10);
      });

      it("should handle Convex mutation failure", async () => {
        const mockResult = {
          success: false,
          newAPM: 0,
          totalActions: 0,
        };

        mockConvexClient.mutation.mockResolvedValueOnce(mockResult);

        const result = await Runtime.runPromise(runtime)(
          Effect.either(
            trackActionInConvex(mockConvexClient, "test-device", "session")
          )
        );

        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ConvexAPMError);
          expect(result.left.operation).toBe("trackAction");
        }
      });

      it("should handle network errors", async () => {
        mockConvexClient.mutation.mockRejectedValueOnce(new Error("Network error"));

        const result = await Runtime.runPromise(runtime)(
          Effect.either(
            trackActionInConvex(mockConvexClient, "test-device", "tool")
          )
        );

        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ConvexAPMError);
          expect(result.left.message).toContain("Network error");
        }
      });
    });

    describe("getRealtimeAPMFromConvex", () => {
      it("should successfully get APM data from Convex", async () => {
        const mockAPMData = {
          currentAPM: 3.2,
          trend: "up" as const,
          sessionDuration: 120000,
          totalActions: 8,
          lastUpdateTimestamp: Date.now(),
          isActive: true,
          deviceId: "test-device",
        };

        mockConvexClient.query.mockResolvedValueOnce(mockAPMData);

        const result = await Runtime.runPromise(runtime)(
          getRealtimeAPMFromConvex(mockConvexClient, "test-device", true)
        );

        expect(mockConvexClient.query).toHaveBeenCalledWith(
          "confect.apm.getRealtimeAPM",
          {
            deviceId: "test-device",
            includeHistory: true,
          }
        );

        expect(result).toEqual(mockAPMData);
      });

      it("should handle null response", async () => {
        mockConvexClient.query.mockResolvedValueOnce(null);

        const result = await Runtime.runPromise(runtime)(
          getRealtimeAPMFromConvex(mockConvexClient)
        );

        expect(result).toBe(null);
      });

      it("should handle query errors", async () => {
        mockConvexClient.query.mockRejectedValueOnce(new Error("Query failed"));

        const result = await Runtime.runPromise(runtime)(
          Effect.either(
            getRealtimeAPMFromConvex(mockConvexClient, "test-device")
          )
        );

        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ConvexAPMError);
          expect(result.left.operation).toBe("getRealtimeAPM");
        }
      });
    });

    describe("subscribeToConvexAPMUpdates", () => {
      it("should create subscription successfully", async () => {
        const mockUnsubscribe = vi.fn();
        mockConvexClient.subscribe.mockReturnValueOnce(mockUnsubscribe);

        const callback = vi.fn();
        const result = await Runtime.runPromise(runtime)(
          subscribeToConvexAPMUpdates(mockConvexClient, "test-device", callback)
        );

        expect(mockConvexClient.subscribe).toHaveBeenCalledWith(
          "confect.apm.getRealtimeAPM",
          { deviceId: "test-device", includeHistory: false },
          expect.any(Function)
        );

        expect(typeof result).toBe("function");
        expect(result).toBe(mockUnsubscribe);
      });

      it("should call callback with APM data", async () => {
        let subscriptionCallback: (data: any) => void;
        mockConvexClient.subscribe.mockImplementationOnce((query, args, callback) => {
          subscriptionCallback = callback;
          return vi.fn();
        });

        const userCallback = vi.fn();
        await Runtime.runPromise(runtime)(
          subscribeToConvexAPMUpdates(mockConvexClient, "test-device", userCallback)
        );

        const mockAPMData = {
          currentAPM: 2.0,
          trend: "stable" as const,
          deviceId: "test-device",
        };

        // Simulate Convex calling the subscription callback
        subscriptionCallback!(mockAPMData);

        expect(userCallback).toHaveBeenCalledWith(mockAPMData);
      });

      it("should handle null data gracefully", async () => {
        let subscriptionCallback: (data: any) => void;
        mockConvexClient.subscribe.mockImplementationOnce((query, args, callback) => {
          subscriptionCallback = callback;
          return vi.fn();
        });

        const userCallback = vi.fn();
        await Runtime.runPromise(runtime)(
          subscribeToConvexAPMUpdates(mockConvexClient, "test-device", userCallback)
        );

        // Simulate Convex calling the subscription callback with null
        subscriptionCallback!(null);

        expect(userCallback).not.toHaveBeenCalled();
      });
    });
  });

  describe("ConvexRealtimeAPMService Layer", () => {
    it("should create service layer with default config", async () => {
      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return service.config;
        }).pipe(Effect.provide(layer))
      );

      expect(result.updateInterval).toBe(3000);
      expect(result.trendThreshold).toBe(10);
      expect(result.enableTrendCalculation).toBe(true);
      expect(result.enableStreaming).toBe(true);
    });

    it("should create service layer with custom config", async () => {
      const customConfig = {
        updateInterval: 5000,
        trendThreshold: 15,
        enableStreaming: false,
      };

      const layer = makeConvexRealtimeAPMService(mockConvexClient, customConfig);

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return service.config;
        }).pipe(Effect.provide(layer))
      );

      expect(result.updateInterval).toBe(5000);
      expect(result.trendThreshold).toBe(15);
      expect(result.enableStreaming).toBe(false);
    });

    it("should provide getCurrentAPM through service", async () => {
      const mockAPMData = {
        currentAPM: 4.0,
        trend: "down" as const,
        sessionDuration: 180000,
        totalActions: 12,
        lastUpdateTimestamp: Date.now(),
        isActive: true,
        deviceId: "test-device-123",
      };

      mockConvexClient.query.mockResolvedValueOnce(mockAPMData);

      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return yield* service.getCurrentAPM;
        }).pipe(Effect.provide(layer))
      );

      expect(result).toEqual(mockAPMData);
    });

    it("should handle getCurrentAPM when no data available", async () => {
      mockConvexClient.query.mockResolvedValueOnce(null);

      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const result = await Runtime.runPromise(runtime)(
        Effect.either(
          Effect.gen(function* () {
            const service = yield* ConvexRealtimeAPMService;
            return yield* service.getCurrentAPM;
          }).pipe(Effect.provide(layer))
        )
      );

      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left.message).toContain("No APM data available");
      }
    });

    it("should provide calculateCurrentSessionAPM through service", async () => {
      const mockAPMData = {
        currentAPM: 6.5,
        deviceId: "test-device-123",
      };

      mockConvexClient.query.mockResolvedValueOnce(mockAPMData);

      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return yield* service.calculateCurrentSessionAPM;
        }).pipe(Effect.provide(layer))
      );

      expect(result).toBe(6.5);
    });

    it("should provide calculateAPMTrend through service", async () => {
      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
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

  describe("Extended Convex Methods", () => {
    describe("trackAction", () => {
      it("should track action successfully", async () => {
        const mockResult = {
          success: true,
          newAPM: 7.2,
          totalActions: 15,
        };

        mockConvexClient.mutation.mockResolvedValueOnce(mockResult);

        const layer = makeConvexRealtimeAPMService(mockConvexClient);

        const result = await Runtime.runPromise(runtime)(
          Effect.gen(function* () {
            const service = yield* ConvexRealtimeAPMService;
            return yield* service.trackAction("message", { type: "user" });
          }).pipe(Effect.provide(layer))
        );

        expect(result.newAPM).toBe(7.2);
        expect(result.totalActions).toBe(15);
        
        expect(mockConvexClient.mutation).toHaveBeenCalledWith(
          "confect.apm.trackRealtimeAction",
          {
            deviceId: "test-device-123",
            actionType: "message",
            timestamp: expect.any(Number),
            metadata: { type: "user" },
          }
        );
      });

      it("should handle track action errors", async () => {
        mockConvexClient.mutation.mockRejectedValueOnce(new Error("Track failed"));

        const layer = makeConvexRealtimeAPMService(mockConvexClient);

        const result = await Runtime.runPromise(runtime)(
          Effect.either(
            Effect.gen(function* () {
              const service = yield* ConvexRealtimeAPMService;
              return yield* service.trackAction("session");
            }).pipe(Effect.provide(layer))
          )
        );

        expect(result._tag).toBe("Left");
        if (result._tag === "Left") {
          expect(result.left).toBeInstanceOf(ConvexAPMError);
        }
      });
    });

    describe("syncWithBackend", () => {
      it("should complete sync successfully", async () => {
        const layer = makeConvexRealtimeAPMService(mockConvexClient);

        const result = await Runtime.runPromise(runtime)(
          Effect.gen(function* () {
            const service = yield* ConvexRealtimeAPMService;
            return yield* service.syncWithBackend;
          }).pipe(Effect.provide(layer))
        );

        // Should complete without error
        expect(result).toBeUndefined();
      });
    });

    describe("subscribeToConvexUpdates", () => {
      it("should create Convex subscription", async () => {
        const mockUnsubscribe = vi.fn();
        mockConvexClient.subscribe.mockReturnValueOnce(mockUnsubscribe);

        const layer = makeConvexRealtimeAPMService(mockConvexClient);
        const callback = vi.fn();

        const result = await Runtime.runPromise(runtime)(
          Effect.gen(function* () {
            const service = yield* ConvexRealtimeAPMService;
            return yield* service.subscribeToConvexUpdates(callback);
          }).pipe(Effect.provide(layer))
        );

        expect(typeof result).toBe("function");
        expect(mockConvexClient.subscribe).toHaveBeenCalledWith(
          "confect.apm.getRealtimeAPM",
          { deviceId: "test-device-123", includeHistory: false },
          expect.any(Function)
        );
      });
    });
  });

  describe("withConvexRealtimeAPMService helper", () => {
    it("should provide service layer automatically", async () => {
      const layer = makeConvexRealtimeAPMService(mockConvexClient, { updateInterval: 8000 });
      
      const result = await Runtime.runPromise(runtime)(
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return service.config.updateInterval;
        }).pipe(Effect.provide(layer))
      );

      expect(result).toBe(8000);
    });
  });

  describe("Error Handling", () => {
    it("should handle Convex client null/undefined", async () => {
      const nullClient = null as any;

      const result = await Runtime.runPromise(runtime)(
        Effect.either(
          trackActionInConvex(nullClient, "test-device", "message")
        )
      );

      expect(result._tag).toBe("Left");
      if (result._tag === "Left") {
        expect(result.left).toBeInstanceOf(ConvexAPMError);
      }
    });

    it("should handle malformed Convex responses", async () => {
      mockConvexClient.mutation.mockResolvedValueOnce({
        // Missing required fields
        success: true,
        // newAPM and totalActions missing
      });

      const result = await Runtime.runPromise(runtime)(
        Effect.either(
          trackActionInConvex(mockConvexClient, "test-device", "tool")
        )
      );

      // Should handle gracefully or fail appropriately
      expect(result._tag).toBe("Right");
      if (result._tag === "Right") {
        expect(result.right.newAPM).toBeUndefined();
        expect(result.right.totalActions).toBeUndefined();
      }
    });

    it("should handle subscription callback errors", async () => {
      let subscriptionCallback: (data: any) => void;
      mockConvexClient.subscribe.mockImplementationOnce((query, args, callback) => {
        subscriptionCallback = callback;
        return vi.fn();
      });

      const faultyCallback = vi.fn(() => {
        throw new Error("Callback error");
      });

      await Runtime.runPromise(runtime)(
        subscribeToConvexAPMUpdates(mockConvexClient, "test-device", faultyCallback)
      );

      const mockAPMData = { currentAPM: 1.0, deviceId: "test-device" };

      // The callback will throw, but should be handled gracefully by the subscription
      expect(() => {
        subscriptionCallback!(mockAPMData);
      }).toThrow("Callback error");

      expect(faultyCallback).toHaveBeenCalledWith(mockAPMData);
    });
  });

  describe("Performance", () => {
    it("should handle multiple concurrent track actions", async () => {
      mockConvexClient.mutation.mockResolvedValue({
        success: true,
        newAPM: 5.0,
        totalActions: 10,
      });

      const layer = makeConvexRealtimeAPMService(mockConvexClient);

      const actions = Array.from({ length: 10 }, (_, i) =>
        Effect.gen(function* () {
          const service = yield* ConvexRealtimeAPMService;
          return yield* service.trackAction("message", { index: i });
        }).pipe(Effect.provide(layer))
      );

      const results = await Runtime.runPromise(runtime)(
        Effect.all(actions, { concurrency: "unbounded" })
      );

      expect(results).toHaveLength(10);
      expect(results.every(r => typeof r.newAPM === 'number')).toBe(true); // trackAction returns { newAPM, totalActions }
      expect(mockConvexClient.mutation).toHaveBeenCalledTimes(10);
    });

    it("should handle rapid subscription updates", async () => {
      let subscriptionCallback: (data: any) => void;
      mockConvexClient.subscribe.mockImplementationOnce((query, args, callback) => {
        subscriptionCallback = callback;
        return vi.fn();
      });

      const updateCount = { value: 0 };
      const callback = vi.fn(() => {
        updateCount.value++;
      });

      await Runtime.runPromise(runtime)(
        subscribeToConvexAPMUpdates(mockConvexClient, "test-device", callback)
      );

      // Simulate rapid updates
      for (let i = 0; i < 50; i++) {
        subscriptionCallback!({
          currentAPM: Math.random() * 10,
          deviceId: "test-device",
        });
      }

      expect(callback).toHaveBeenCalledTimes(50);
      expect(updateCount.value).toBe(50);
    });
  });
});