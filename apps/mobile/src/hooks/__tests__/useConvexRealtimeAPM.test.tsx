import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react-native";
import { AppState } from "react-native";
import { useConvexRealtimeAPM, useAPMActionTracker } from "../useConvexRealtimeAPM";

// Mock Convex hooks
const mockUseQuery = vi.fn();
const mockUseMutation = vi.fn();

vi.mock("convex/react", () => ({
  useQuery: mockUseQuery,
  useMutation: mockUseMutation,
}));

// Mock React Native AppState
const mockAppState = {
  currentState: "active",
  addEventListener: vi.fn(),
};

vi.mock("react-native", () => ({
  AppState: mockAppState,
}));

// Mock API
vi.mock("../../convex/_generated/api", () => ({
  api: {
    confect: {
      apm: {
        getRealtimeAPM: "mock.getRealtimeAPM",
        trackRealtimeAction: "mock.trackRealtimeAction",
        updateRealtimeAPM: "mock.updateRealtimeAPM",
      },
    },
  },
}));

describe("useConvexRealtimeAPM", () => {
  const mockTrackAction = vi.fn();
  const mockUpdateAPM = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseMutation.mockImplementation((mutationName) => {
      if (mutationName === "mock.trackRealtimeAction") {
        return mockTrackAction;
      }
      if (mutationName === "mock.updateRealtimeAPM") {
        return mockUpdateAPM;  
      }
      return vi.fn();
    });

    // Reset AppState mock
    mockAppState.currentState = "active";
    mockAppState.addEventListener.mockReturnValue({
      remove: vi.fn(),
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Hook Initialization", () => {
    it("should initialize with default config", () => {
      const mockAPMData = {
        currentAPM: 2.5,
        trend: "stable" as const,
        sessionDuration: 60000,
        totalActions: 5,
        lastUpdateTimestamp: Date.now(),
        isActive: true,
        deviceId: "test-device",
      };

      mockUseQuery.mockReturnValue(mockAPMData);

      const { result } = renderHook(() => useConvexRealtimeAPM());

      expect(result.current.state.data).toEqual(mockAPMData);
      expect(result.current.state.isLoading).toBe(false);
      expect(result.current.state.error).toBe(null);
      expect(result.current.state.isActive).toBe(true);
    });

    it("should initialize with custom config", () => {
      mockUseQuery.mockReturnValue(undefined); // Loading state

      const config = {
        enabled: true,
        deviceId: "custom-device",
        includeHistory: true,
      };

      const { result } = renderHook(() => useConvexRealtimeAPM(config));

      expect(result.current.state.isLoading).toBe(true);
      expect(mockUseQuery).toHaveBeenCalledWith(
        "mock.getRealtimeAPM",
        { deviceId: "custom-device", includeHistory: true }
      );
    });

    it("should skip query when disabled", () => {
      const { result } = renderHook(() => 
        useConvexRealtimeAPM({ enabled: false })
      );

      expect(mockUseQuery).toHaveBeenCalledWith("mock.getRealtimeAPM", "skip");
      expect(result.current.state.isLoading).toBe(true);
    });
  });

  describe("APM Data Updates", () => {
    it("should call onAPMUpdate when data changes", () => {
      const onAPMUpdate = vi.fn();
      const mockAPMData = {
        currentAPM: 3.0,
        trend: "up" as const,
        deviceId: "test-device",
      };

      mockUseQuery.mockReturnValue(mockAPMData);

      renderHook(() => useConvexRealtimeAPM({ onAPMUpdate }));

      expect(onAPMUpdate).toHaveBeenCalledWith(mockAPMData);
    });

    it("should not call onAPMUpdate when data is null", () => {
      const onAPMUpdate = vi.fn();
      mockUseQuery.mockReturnValue(null);

      renderHook(() => useConvexRealtimeAPM({ onAPMUpdate }));

      expect(onAPMUpdate).not.toHaveBeenCalled();
    });

    it("should handle loading state", () => {
      mockUseQuery.mockReturnValue(undefined);

      const { result } = renderHook(() => useConvexRealtimeAPM());

      expect(result.current.state.isLoading).toBe(true);
      expect(result.current.state.data).toBe(null);
    });
  });

  describe("App State Management", () => {
    it("should handle app state changes", () => {
      const mockAPMData = {
        currentAPM: 2.0,
        deviceId: "test-device",
        totalActions: 4,
        sessionDuration: 120000,
      };

      mockUseQuery.mockReturnValue(mockAPMData);
      mockUpdateAPM.mockResolvedValue({});

      let appStateCallback: (state: string) => void;
      mockAppState.addEventListener.mockImplementation((event, callback) => {
        if (event === "change") {
          appStateCallback = callback;
        }
        return { remove: vi.fn() };
      });

      renderHook(() => useConvexRealtimeAPM());

      // Simulate app going to background
      act(() => {
        appStateCallback!("background");
      });

      expect(mockUpdateAPM).toHaveBeenCalledWith({
        deviceId: "test-device",
        currentAPM: 2.0,
        totalActions: 4,
        sessionDuration: 120000,
        isActive: false,
      });
    });

    it("should handle app state change errors", async () => {
      const onError = vi.fn();
      const mockAPMData = { deviceId: "test-device", currentAPM: 1.0 };
      
      mockUseQuery.mockReturnValue(mockAPMData);
      mockUpdateAPM.mockRejectedValue(new Error("Update failed"));

      let appStateCallback: (state: string) => void;
      mockAppState.addEventListener.mockImplementation((event, callback) => {
        if (event === "change") {
          appStateCallback = callback;
        }
        return { remove: vi.fn() };
      });

      const { result } = renderHook(() => 
        useConvexRealtimeAPM({ onError })
      );

      await act(async () => {
        appStateCallback!("inactive");
        // Wait for async error handling
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      expect(onError).toHaveBeenCalledWith(expect.any(Error));
      expect(result.current.state.error).toBeInstanceOf(Error);
    });

    it("should cleanup app state listener on unmount", () => {
      const mockRemove = vi.fn();
      mockAppState.addEventListener.mockReturnValue({ remove: mockRemove });

      const { unmount } = renderHook(() => useConvexRealtimeAPM());

      unmount();

      expect(mockRemove).toHaveBeenCalled();
    });
  });

  describe("Action Tracking", () => {
    const setupTrackingTest = () => {
      const mockAPMData = {
        deviceId: "test-device",
        currentAPM: 5.0,
        totalActions: 10,
      };
      
      mockUseQuery.mockReturnValue(mockAPMData);
      mockTrackAction.mockResolvedValue({
        success: true,
        newAPM: 5.5,
        totalActions: 11,
      });

      return mockAPMData;
    };

    it("should track message actions", async () => {
      setupTrackingTest();

      const { result } = renderHook(() => useConvexRealtimeAPM());

      await act(async () => {
        const actionResult = await result.current.actions.trackMessage();
        expect(actionResult).toEqual({
          success: true,
          newAPM: 5.5,
          totalActions: 11,
        });
      });

      expect(mockTrackAction).toHaveBeenCalledWith({
        deviceId: "test-device",
        actionType: "message",
        timestamp: expect.any(Number),
      });
    });

    it("should track session actions", async () => {
      setupTrackingTest();

      const { result } = renderHook(() => useConvexRealtimeAPM());

      await act(async () => {
        await result.current.actions.trackSession();
      });

      expect(mockTrackAction).toHaveBeenCalledWith({
        deviceId: "test-device",
        actionType: "session",
        timestamp: expect.any(Number),
      });
    });

    it("should track tool actions with metadata", async () => {
      setupTrackingTest();

      const { result } = renderHook(() => useConvexRealtimeAPM());
      const metadata = { toolName: "bash", command: "ls" };

      await act(async () => {
        await result.current.actions.trackTool(metadata);
      });

      expect(mockTrackAction).toHaveBeenCalledWith({
        deviceId: "test-device",
        actionType: "tool",
        timestamp: expect.any(Number),
        metadata,
      });
    });

    it("should handle tracking errors", async () => {
      const onError = vi.fn();
      setupTrackingTest();
      mockTrackAction.mockRejectedValue(new Error("Track failed"));

      const { result } = renderHook(() => 
        useConvexRealtimeAPM({ onError })
      );

      await act(async () => {
        const actionResult = await result.current.actions.trackMessage();
        expect(actionResult).toBe(null);
      });

      expect(onError).toHaveBeenCalledWith(expect.any(Error));
    });

    it("should not track when disabled", async () => {
      mockUseQuery.mockReturnValue(null);

      const { result } = renderHook(() => 
        useConvexRealtimeAPM({ enabled: false })
      );

      await act(async () => {
        const actionResult = await result.current.actions.trackMessage();
        expect(actionResult).toBe(null);
      });

      expect(mockTrackAction).not.toHaveBeenCalled();
    });

    it("should not track when device ID unavailable", async () => {
      mockUseQuery.mockReturnValue({ currentAPM: 1.0 }); // No deviceId

      const { result } = renderHook(() => useConvexRealtimeAPM());

      await act(async () => {
        const actionResult = await result.current.actions.trackMessage();
        expect(actionResult).toBe(null);
      });

      expect(mockTrackAction).not.toHaveBeenCalled();
    });
  });

  describe("Data Access Methods", () => {
    const mockAPMData = {
      currentAPM: 7.5,
      trend: "up" as const,
      sessionDuration: 180000,
      totalActions: 15,
      isActive: true,
      deviceId: "test-device",
    };

    beforeEach(() => {
      mockUseQuery.mockReturnValue(mockAPMData);
    });

    it("should provide getCurrentAPM", () => {
      const { result } = renderHook(() => useConvexRealtimeAPM());

      const currentAPM = result.current.data.getCurrentAPM();
      expect(currentAPM).toBe(7.5);
    });

    it("should provide isTrendingUp", () => {
      const { result } = renderHook(() => useConvexRealtimeAPM());

      expect(result.current.data.isTrendingUp()).toBe(true);
      expect(result.current.data.isTrendingDown()).toBe(false);
    });

    it("should provide getSessionInfo", () => {
      const { result } = renderHook(() => useConvexRealtimeAPM());

      const sessionInfo = result.current.data.getSessionInfo();
      expect(sessionInfo).toEqual({
        duration: 180000,
        totalActions: 15,
        apm: 7.5,
        isActive: true,
      });
    });

    it("should handle null data gracefully", () => {
      mockUseQuery.mockReturnValue(null);

      const { result } = renderHook(() => useConvexRealtimeAPM());

      expect(result.current.data.getCurrentAPM()).toBe(0);
      expect(result.current.data.isTrendingUp()).toBe(false);
      expect(result.current.data.getSessionInfo()).toEqual({
        duration: 0,
        totalActions: 0,
        apm: 0,
        isActive: false,
      });
    });
  });

  describe("Refresh Functionality", () => {
    it("should provide refresh function", () => {
      mockUseQuery.mockReturnValue(null);

      const { result } = renderHook(() => useConvexRealtimeAPM());

      act(() => {
        result.current.actions.refresh();
      });

      // Should reset error state
      expect(result.current.state.error).toBe(null);
    });
  });
});

describe("useAPMActionTracker", () => {
  const mockTrackAction = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseMutation.mockReturnValue(mockTrackAction);
    mockTrackAction.mockResolvedValue({
      success: true,
      newAPM: 3.0,
      totalActions: 6,
    });
  });

  it("should initialize with default device ID", () => {
    const { result } = renderHook(() => useAPMActionTracker());

    expect(result.current.deviceId).toMatch(/^mobile-\d+-[a-z0-9]+$/);
  });

  it("should use provided device ID", () => {
    const { result } = renderHook(() => 
      useAPMActionTracker({ deviceId: "custom-device" })
    );

    expect(result.current.deviceId).toBe("custom-device");
  });

  it("should track message actions", async () => {
    const { result } = renderHook(() => useAPMActionTracker());

    await act(async () => {
      const actionResult = await result.current.trackMessage();
      expect(actionResult).toEqual({
        success: true,
        newAPM: 3.0,
        totalActions: 6,
      });
    });

    expect(mockTrackAction).toHaveBeenCalledWith({
      deviceId: result.current.deviceId,
      actionType: "message",
      timestamp: expect.any(Number),
    });
  });

  it("should track session actions", async () => {
    const { result } = renderHook(() => useAPMActionTracker());

    await act(async () => {
      await result.current.trackSession();
    });

    expect(mockTrackAction).toHaveBeenCalledWith({
      deviceId: result.current.deviceId,
      actionType: "session", 
      timestamp: expect.any(Number),
    });
  });

  it("should handle tracking errors", async () => {
    mockTrackAction.mockRejectedValue(new Error("Track failed"));

    const { result } = renderHook(() => useAPMActionTracker());

    await act(async () => {
      const actionResult = await result.current.trackMessage();
      expect(actionResult).toBe(null);
    });
  });

  it("should not track when disabled", async () => {
    const { result } = renderHook(() => 
      useAPMActionTracker({ enabled: false })
    );

    await act(async () => {
      const actionResult = await result.current.trackMessage();
      expect(actionResult).toBe(null);
    });

    expect(mockTrackAction).not.toHaveBeenCalled();
  });

  it("should generate stable device ID across renders", () => {
    const { result, rerender } = renderHook(() => useAPMActionTracker());

    const initialDeviceId = result.current.deviceId;
    
    rerender();
    
    expect(result.current.deviceId).toBe(initialDeviceId);
  });
});