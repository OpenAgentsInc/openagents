import { renderHook, act } from "@testing-library/react-hooks";
import { AppState, AppStateStatus } from "react-native";
import { describe, it, expect, jest, beforeEach, afterEach } from "jest";
import { useAPMTracking } from "./useAPMTracking";

// Mock Convex hooks
jest.mock("convex/react", () => ({
  useMutation: jest.fn(() => jest.fn()),
  useQuery: jest.fn(() => ({ _id: "user123", name: "Test User" })),
}));

// Mock React Native modules
jest.mock("react-native", () => ({
  AppState: {
    addEventListener: jest.fn(),
    currentState: "active",
  },
}));

// Mock AsyncStorage
jest.mock("@react-native-async-storage/async-storage", () => ({
  getItem: jest.fn(() => Promise.resolve(null)),
  setItem: jest.fn(() => Promise.resolve()),
}));

describe("useAPMTracking", () => {
  let appStateListeners: { [key: string]: (state: AppStateStatus) => void } = {};
  
  beforeEach(() => {
    jest.clearAllMocks();
    appStateListeners = {};
    
    // Mock AppState.addEventListener
    (AppState.addEventListener as jest.Mock).mockImplementation((event, handler) => {
      appStateListeners[event] = handler;
      return { remove: jest.fn() };
    });
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  describe("Initialization", () => {
    it("should initialize with default state when enabled", () => {
      const { result } = renderHook(() => useAPMTracking({ enabled: true }));
      
      expect(result.current.isTracking).toBe(true);
      expect(result.current.activityCount).toBe(0);
      expect(result.current.sessionId).toMatch(/^mobile-/);
    });

    it("should not track when disabled", () => {
      const { result } = renderHook(() => useAPMTracking({ enabled: false }));
      
      expect(result.current.isTracking).toBe(false);
      expect(result.current.activityCount).toBe(0);
    });

    it("should set up app state listener when trackAppState is true", () => {
      renderHook(() => useAPMTracking({ 
        enabled: true, 
        trackAppState: true 
      }));
      
      expect(AppState.addEventListener).toHaveBeenCalledWith(
        "change",
        expect.any(Function)
      );
    });
  });

  describe("Activity Tracking", () => {
    it("should increment activity count when tracking messages", () => {
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackMessages: true 
      }));
      
      act(() => {
        result.current.trackMessageSent();
      });
      
      expect(result.current.activityCount).toBe(1);
      
      act(() => {
        result.current.trackMessageSent();
        result.current.trackMessageSent();
      });
      
      expect(result.current.activityCount).toBe(3);
    });

    it("should track session creation", () => {
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackSessions: true 
      }));
      
      const initialCount = result.current.activityCount;
      
      act(() => {
        result.current.trackSessionCreated();
      });
      
      expect(result.current.activityCount).toBe(initialCount + 1);
    });

    it("should not track when feature is disabled", () => {
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackMessages: false 
      }));
      
      act(() => {
        result.current.trackMessageSent();
      });
      
      expect(result.current.activityCount).toBe(0);
    });
  });

  describe("App State Transitions", () => {
    it("should handle app going to background", () => {
      jest.useFakeTimers();
      
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackAppState: true 
      }));
      
      // Start with active state
      expect(result.current.isTracking).toBe(true);
      
      // Simulate app going to background
      act(() => {
        appStateListeners.change?.("background");
      });
      
      expect(result.current.isTracking).toBe(false);
    });

    it("should handle app returning to foreground", () => {
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackAppState: true 
      }));
      
      // Simulate background -> active transition
      act(() => {
        appStateListeners.change?.("background");
      });
      
      expect(result.current.isTracking).toBe(false);
      
      act(() => {
        appStateListeners.change?.("active");
      });
      
      expect(result.current.isTracking).toBe(true);
    });

    it("should handle inactive state", () => {
      const { result } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackAppState: true 
      }));
      
      act(() => {
        appStateListeners.change?.("inactive");
      });
      
      // Inactive should pause tracking
      expect(result.current.isTracking).toBe(false);
    });
  });

  describe("Periodic Sync", () => {
    it("should sync data every 5 minutes", () => {
      jest.useFakeTimers();
      
      const mockTrackSession = jest.fn();
      const { useMutation } = require("convex/react");
      useMutation.mockReturnValue(mockTrackSession);
      
      renderHook(() => useAPMTracking({ 
        enabled: true,
        syncInterval: 300000 // 5 minutes
      }));
      
      // Fast forward 5 minutes
      act(() => {
        jest.advanceTimersByTime(300000);
      });
      
      expect(mockTrackSession).toHaveBeenCalled();
    });

    it("should not sync when tracking is disabled", () => {
      jest.useFakeTimers();
      
      const mockTrackSession = jest.fn();
      const { useMutation } = require("convex/react");
      useMutation.mockReturnValue(mockTrackSession);
      
      renderHook(() => useAPMTracking({ 
        enabled: false
      }));
      
      act(() => {
        jest.advanceTimersByTime(300000);
      });
      
      expect(mockTrackSession).not.toHaveBeenCalled();
    });
  });

  describe("Error Handling", () => {
    it("should handle sync errors gracefully", async () => {
      const mockTrackSession = jest.fn().mockRejectedValue(new Error("Sync failed"));
      const { useMutation } = require("convex/react");
      useMutation.mockReturnValue(mockTrackSession);
      
      const consoleSpy = jest.spyOn(console, "error").mockImplementation();
      
      const { result } = renderHook(() => useAPMTracking({ enabled: true }));
      
      // Trigger sync
      await act(async () => {
        await result.current.syncNow?.();
      });
      
      expect(consoleSpy).toHaveBeenCalledWith(
        expect.stringContaining("Failed to track mobile session"),
        expect.any(Error)
      );
      
      consoleSpy.mockRestore();
    });

    it("should handle missing user gracefully", () => {
      const { useQuery } = require("convex/react");
      useQuery.mockReturnValue(null); // No user
      
      const { result } = renderHook(() => useAPMTracking({ enabled: true }));
      
      // Should still initialize but may not sync
      expect(result.current.sessionId).toMatch(/^mobile-/);
    });
  });

  describe("Cleanup", () => {
    it("should clean up listeners on unmount", () => {
      const removeListener = jest.fn();
      (AppState.addEventListener as jest.Mock).mockReturnValue({ 
        remove: removeListener 
      });
      
      const { unmount } = renderHook(() => useAPMTracking({ 
        enabled: true,
        trackAppState: true 
      }));
      
      unmount();
      
      expect(removeListener).toHaveBeenCalled();
    });

    it("should clear sync interval on unmount", () => {
      jest.useFakeTimers();
      const clearIntervalSpy = jest.spyOn(global, "clearInterval");
      
      const { unmount } = renderHook(() => useAPMTracking({ 
        enabled: true 
      }));
      
      unmount();
      
      expect(clearIntervalSpy).toHaveBeenCalled();
    });
  });

  describe("Session Management", () => {
    it("should generate unique session IDs", () => {
      const { result: result1 } = renderHook(() => useAPMTracking({ enabled: true }));
      const { result: result2 } = renderHook(() => useAPMTracking({ enabled: true }));
      
      expect(result1.current.sessionId).not.toBe(result2.current.sessionId);
      expect(result1.current.sessionId).toMatch(/^mobile-[\w-]+$/);
      expect(result2.current.sessionId).toMatch(/^mobile-[\w-]+$/);
    });

    it("should persist device ID across sessions", async () => {
      const AsyncStorage = require("@react-native-async-storage/async-storage");
      AsyncStorage.getItem.mockResolvedValue("existing-device-id");
      
      const { result } = renderHook(() => useAPMTracking({ enabled: true }));
      
      // Wait for async storage to be read
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 0));
      });
      
      expect(AsyncStorage.getItem).toHaveBeenCalledWith("@openagents/deviceId");
    });
  });
});