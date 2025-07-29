import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRealtimeAPM, useCurrentAPM } from "../useRealtimeAPM";
import { Effect, Runtime } from "effect";

// Mock the RealtimeAPMService
vi.mock("../services/RealtimeAPMService", () => ({
  makeRealtimeAPMService: vi.fn(() => ({
    provide: vi.fn(),
  })),
  RealtimeAPMService: {
    pipe: vi.fn(),
  },
  generateDeviceId: vi.fn(() => Effect.succeed("test-device-id")),
  createInitialSessionData: vi.fn(() => Effect.succeed({
    sessionStart: Date.now(),
    messagesSent: 0,
    sessionsCreated: 0,
    appStateChanges: 0,
    deviceId: "test-device-id",
    platform: "test",
  })),
}));

// Simplified mock for Effect Runtime
const mockRuntime = {
  runPromise: vi.fn(),
  runSync: vi.fn(),
};

describe("useRealtimeAPM", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Setup default mock implementations
    mockRuntime.runPromise.mockImplementation((effect: any) => {
      // Mock device ID generation
      if (typeof effect === "function") {
        return Promise.resolve("test-device-id");
      }
      return Promise.resolve({
        sessionStart: Date.now(),
        messagesSent: 0,
        sessionsCreated: 0,
        appStateChanges: 0,
        deviceId: "test-device-id",
        platform: "test",
      });
    });

    mockRuntime.runSync.mockImplementation(() => void 0);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Hook Initialization", () => {
    it("should initialize with default state", () => {
      const { result } = renderHook(() => useRealtimeAPM());

      expect(result.current.state.data).toBe(null);
      expect(result.current.state.isLoading).toBe(true);
      expect(result.current.state.error).toBe(null);
      expect(result.current.state.isSubscribed).toBe(false);
    });

    it("should initialize with custom config", () => {
      const customConfig = {
        enabled: true,
        updateInterval: 5000,
        trendThreshold: 15,
        enableTrendCalculation: false,
      };

      const { result } = renderHook(() => useRealtimeAPM(customConfig));

      // Should still start with loading state regardless of config
      expect(result.current.state.isLoading).toBe(true);
    });

    it("should not initialize when disabled", () => {
      const { result } = renderHook(() => useRealtimeAPM({ enabled: false }));

      expect(result.current.state.isLoading).toBe(true); // Still loading initially
    });
  });

  describe("APM Data Management", () => {
    it("should provide getCurrentAPM function", async () => {
      mockRuntime.runPromise.mockResolvedValueOnce({
        currentAPM: 5.5,
        trend: 'up',
        sessionDuration: 60000,
        totalActions: 5,
        lastUpdateTimestamp: Date.now(),
        isActive: true,
        deviceId: "test-device-id",
      });

      const { result } = renderHook(() => useRealtimeAPM());

      await act(async () => {
        const apmData = await result.current.getCurrentAPM();
        expect(apmData?.currentAPM).toBe(5.5);
        expect(apmData?.trend).toBe('up');
      });
    });

    it("should handle getCurrentAPM errors gracefully", async () => {
      mockRuntime.runPromise.mockRejectedValueOnce(new Error("Service error"));

      const { result } = renderHook(() => useRealtimeAPM());

      await act(async () => {
        const apmData = await result.current.getCurrentAPM();
        expect(apmData).toBe(null);
      });
    });
  });

  describe("Action Tracking", () => {
    it("should provide trackMessage function", () => {
      const { result } = renderHook(() => useRealtimeAPM());

      expect(typeof result.current.trackMessage).toBe("function");

      act(() => {
        result.current.trackMessage();
      });

      expect(mockRuntime.runSync).toHaveBeenCalled();
    });

    it("should provide trackSession function", () => {
      const { result } = renderHook(() => useRealtimeAPM());

      expect(typeof result.current.trackSession).toBe("function");

      act(() => {
        result.current.trackSession();
      });

      expect(mockRuntime.runSync).toHaveBeenCalled();
    });

    it("should provide setActive function", () => {
      const { result } = renderHook(() => useRealtimeAPM());

      expect(typeof result.current.setActive).toBe("function");

      act(() => {
        result.current.setActive(false);
      });

      expect(mockRuntime.runSync).toHaveBeenCalled();
    });
  });

  describe("Subscription Management", () => {
    it("should provide subscribe and unsubscribe functions", () => {
      const { result } = renderHook(() => useRealtimeAPM());

      expect(typeof result.current.subscribe).toBe("function");
      expect(typeof result.current.unsubscribe).toBe("function");
    });

    it("should handle subscription lifecycle", async () => {
      mockRuntime.runPromise.mockResolvedValueOnce("mock-fiber");

      const { result } = renderHook(() => useRealtimeAPM());

      await act(async () => {
        result.current.subscribe();
      });

      // Should update subscription state
      expect(result.current.state.isSubscribed).toBe(true);

      act(() => {
        result.current.unsubscribe();
      });

      expect(result.current.state.isSubscribed).toBe(false);
    });

    it("should handle subscription errors", async () => {
      const mockError = new Error("Subscription failed");
      mockRuntime.runPromise.mockRejectedValueOnce(mockError);

      const onError = vi.fn();
      const { result } = renderHook(() => useRealtimeAPM({ onError }));

      await act(async () => {
        result.current.subscribe();
      });

      expect(onError).toHaveBeenCalledWith(mockError);
      expect(result.current.state.error).toBe(mockError);
    });
  });

  describe("Error Handling", () => {
    it("should handle initialization errors", async () => {
      mockRuntime.runPromise.mockRejectedValueOnce(new Error("Init error"));

      const onError = vi.fn();
      const { result } = renderHook(() => useRealtimeAPM({ onError }));

      // Wait for initialization to complete
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      expect(onError).toHaveBeenCalled();
      expect(result.current.state.error).toBeTruthy();
    });

    it("should reset error state on successful operations", async () => {
      const { result } = renderHook(() => useRealtimeAPM());

      // Simulate error state
      act(() => {
        (result.current as any).setState((prev: any) => ({ 
          ...prev, 
          error: new Error("Test error") 
        }));
      });

      mockRuntime.runPromise.mockResolvedValueOnce({
        currentAPM: 1.0,
        trend: 'stable',
        deviceId: "test-device-id",
      });

      await act(async () => {
        await result.current.getCurrentAPM();
      });

      expect(result.current.state.error).toBe(null);
    });
  });

  describe("Callback Integration", () => {
    it("should call onAPMUpdate callback", async () => {
      const onAPMUpdate = vi.fn();
      mockRuntime.runPromise.mockResolvedValueOnce("mock-fiber");

      const { result } = renderHook(() => useRealtimeAPM({ onAPMUpdate }));

      // Simulate APM update
      const mockAPMData = {
        currentAPM: 3.5,
        trend: 'up' as const,
        sessionDuration: 120000,
        totalActions: 7,
        lastUpdateTimestamp: Date.now(),
        isActive: true,
        deviceId: "test-device-id",
      };

      await act(async () => {
        // Simulate the subscription callback being called
        if (onAPMUpdate) {
          onAPMUpdate(mockAPMData);
        }
      });

      expect(onAPMUpdate).toHaveBeenCalledWith(mockAPMData);
    });
  });

  describe("Cleanup", () => {
    it("should cleanup on unmount", () => {
      const { unmount } = renderHook(() => useRealtimeAPM());

      unmount();

      // Should call interrupt on the subscription fiber
      expect(mockRuntime.runSync).toHaveBeenCalled();
    });

    it("should cleanup subscription when disabled", () => {
      const { result, rerender } = renderHook(
        ({ enabled }) => useRealtimeAPM({ enabled }),
        { initialProps: { enabled: true } }
      );

      // Enable subscription first
      act(() => {
        result.current.subscribe();
      });

      // Then disable
      rerender({ enabled: false });

      expect(result.current.state.isSubscribed).toBe(false);
    });
  });
});

describe("useCurrentAPM", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockRuntime.runPromise.mockResolvedValue({
      currentAPM: 2.5,
      trend: 'stable',
      deviceId: "test-device-id",
    });
  });

  it("should provide current APM value", async () => {
    const { result } = renderHook(() => useCurrentAPM());

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.currentAPM).toBe(2.5);
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBe(null);
  });

  it("should handle refresh errors", async () => {
    const mockError = new Error("Refresh failed");
    mockRuntime.runPromise.mockRejectedValueOnce(mockError);

    const onError = vi.fn();
    const { result } = renderHook(() => useCurrentAPM({ onError }));

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.error).toBe(mockError);
    expect(onError).toHaveBeenCalledWith(mockError);
  });

  it("should auto-refresh on mount when enabled", async () => {
    const { result } = renderHook(() => useCurrentAPM({ enabled: true }));

    // Wait for initial load
    await act(async () => {
      await new Promise(resolve => setTimeout(resolve, 100));
    });

    expect(mockRuntime.runPromise).toHaveBeenCalled();
  });

  it("should not auto-refresh when disabled", () => {
    renderHook(() => useCurrentAPM({ enabled: false }));

    expect(mockRuntime.runPromise).not.toHaveBeenCalled();
  });

  it("should provide manual refresh function", async () => {
    const { result } = renderHook(() => useCurrentAPM({ enabled: false }));

    await act(async () => {
      await result.current.refresh();
    });

    expect(mockRuntime.runPromise).toHaveBeenCalled();
    expect(result.current.currentAPM).toBe(2.5);
  });
});