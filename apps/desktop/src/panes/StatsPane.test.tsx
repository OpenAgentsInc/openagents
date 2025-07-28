import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { StatsPane } from "./StatsPane";
import { invoke } from "@tauri-apps/api/core";

// Mock Tauri API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Get properly typed mock
const mockInvoke = vi.mocked(invoke);

// Mock lucide-react icons
vi.mock("lucide-react", () => ({
  Monitor: () => <div data-testid="monitor-icon" />,
  Smartphone: () => <div data-testid="smartphone-icon" />,
  Github: () => <div data-testid="github-icon" />,
  Globe: () => <div data-testid="globe-icon" />,
  BarChart: () => <div data-testid="barchart-icon" />,
  Clock: () => <div data-testid="clock-icon" />,
  TrendingUp: () => <div data-testid="trending-up-icon" />,
  Loader2: () => <div data-testid="loader2-icon" />,
  RefreshCw: () => <div data-testid="refresh-cw-icon" />,
  Eye: () => <div data-testid="eye-icon" />,
  Calendar: () => <div data-testid="calendar-icon" />,
}));

const mockAPMData = {
  currentAPM: 45.5,
  fiveMinuteAPM: 42.3,
  thirtyMinuteAPM: 38.7,
  allTimeStats: {
    totalSessions: 150,
    totalActions: 12500,
    totalMinutes: 3200,
    apm: 234.4,
  },
};

const mockUserAPMData = {
  currentAPM: 52.3,
  fiveMinuteAPM: 48.7,
  thirtyMinuteAPM: 44.2,
  deviceBreakdown: {
    desktop: { apm: 35.2, actions: 8500, minutes: 2400 },
    mobile: { apm: 12.5, actions: 2500, minutes: 600 },
    github: { apm: 4.6, actions: 1500, minutes: 200 },
  },
  metadata: {
    overlappingMinutes: 120,
    peakConcurrentDevices: 3,
    totalUniqueMinutes: 3000,
  },
};

describe("StatsPane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(mockAPMData);
  });

  describe("Rendering", () => {
    it("should render loading state initially", () => {
      render(<StatsPane />);
      expect(screen.getByText("Loading stats...")).toBeInTheDocument();
    });

    it("should render APM stats when data is loaded", async () => {
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(screen.getByText("45.5")).toBeInTheDocument(); // Current APM
        expect(screen.getByText("42.3")).toBeInTheDocument(); // 5 min APM
        expect(screen.getByText("38.7")).toBeInTheDocument(); // 30 min APM
      });
    });

    it("should show view mode toggle buttons", async () => {
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(screen.getByText("My Device")).toBeInTheDocument();
        expect(screen.getByText("All Devices")).toBeInTheDocument();
      });
    });

    it("should highlight active view mode", async () => {
      render(<StatsPane />);
      
      await waitFor(() => {
        const myDeviceButton = screen.getByText("My Device").parentElement;
        expect(myDeviceButton).toHaveClass("bg-purple-600");
      });
    });
  });

  describe("View Mode Switching", () => {
    it("should switch to All Devices view when clicked", async () => {
      mockInvoke
        .mockResolvedValueOnce(mockAPMData) // Initial load
        .mockResolvedValueOnce(mockUserAPMData); // After switch
      
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(screen.getByText("My Device")).toBeInTheDocument();
      });
      
      const allDevicesButton = screen.getByText("All Devices");
      fireEvent.click(allDevicesButton);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("get_user_apm_stats");
        expect(screen.getByText("52.3")).toBeInTheDocument(); // User APM
      });
    });

    it("should show device breakdown in All Devices view", async () => {
      mockInvoke.mockResolvedValue(mockUserAPMData);
      
      render(<StatsPane />);
      
      // Switch to All Devices view
      const allDevicesButton = screen.getByText("All Devices");
      fireEvent.click(allDevicesButton);
      
      await waitFor(() => {
        expect(screen.getByTestId("monitor-icon")).toBeInTheDocument();
        expect(screen.getByTestId("smartphone-icon")).toBeInTheDocument();
        expect(screen.getByTestId("github-icon")).toBeInTheDocument();
        expect(screen.getByText("35.2 APM")).toBeInTheDocument(); // Desktop APM
        expect(screen.getByText("12.5 APM")).toBeInTheDocument(); // Mobile APM
        expect(screen.getByText("4.6 APM")).toBeInTheDocument(); // GitHub APM
      });
    });

    it("should maintain view mode when stats refresh", async () => {
      vi.useFakeTimers();
      
      render(<StatsPane />);
      
      // Switch to All Devices
      await waitFor(() => screen.getByText("All Devices"));
      fireEvent.click(screen.getByText("All Devices"));
      
      // Fast forward to trigger refresh
      vi.advanceTimersByTime(5000);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenLastCalledWith("get_user_apm_stats");
      });
      
      vi.useRealTimers();
    });
  });

  describe("Auto-refresh", () => {
    it("should refresh stats every 5 seconds when running", async () => {
      vi.useFakeTimers();
      
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledTimes(1);
      });
      
      vi.advanceTimersByTime(5000);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledTimes(2);
      });
      
      vi.advanceTimersByTime(5000);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledTimes(3);
      });
      
      vi.useRealTimers();
    });

    it("should not refresh when not running", async () => {
      vi.useFakeTimers();
      
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledTimes(1);
      });
      
      vi.advanceTimersByTime(10000);
      
      // Should still only be called once
      expect(invoke).toHaveBeenCalledTimes(1);
      
      vi.useRealTimers();
    });
  });

  describe("Error Handling", () => {
    it("should handle API errors gracefully", async () => {
      mockInvoke.mockRejectedValue(new Error("API Error"));
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(consoleSpy).toHaveBeenCalledWith(
          "Failed to fetch APM stats:",
          expect.any(Error)
        );
      });
      
      // Should still show UI, possibly with default values
      expect(screen.getByText("Loading stats...")).toBeInTheDocument();
      
      consoleSpy.mockRestore();
    });

    it("should handle missing device breakdown data", async () => {
      const incompleteData = {
        ...mockUserAPMData,
        deviceBreakdown: undefined,
      };
      mockInvoke.mockResolvedValue(incompleteData);
      
      render(<StatsPane />);
      
      fireEvent.click(screen.getByText("All Devices"));
      
      await waitFor(() => {
        // Should render without crashing
        expect(screen.getByText("52.3")).toBeInTheDocument();
      });
    });
  });

  describe("All Time Stats", () => {
    it("should display all time statistics", async () => {
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(screen.getByText("All Time")).toBeInTheDocument();
        expect(screen.getByText("150")).toBeInTheDocument(); // Sessions
        expect(screen.getByText("12,500")).toBeInTheDocument(); // Actions
        expect(screen.getByText("234.4")).toBeInTheDocument(); // APM
      });
    });

    it("should format large numbers correctly", async () => {
      const largeNumberData = {
        ...mockAPMData,
        allTimeStats: {
          totalSessions: 1500,
          totalActions: 1234567,
          totalMinutes: 98765,
          apm: 751.2,
        },
      };
      mockInvoke.mockResolvedValue(largeNumberData);
      
      render(<StatsPane />);
      
      await waitFor(() => {
        expect(screen.getByText("1,500")).toBeInTheDocument();
        expect(screen.getByText("1,234,567")).toBeInTheDocument();
      });
    });
  });

  describe("Device Icons", () => {
    it("should show appropriate icons for each device type", async () => {
      mockInvoke.mockResolvedValue(mockUserAPMData);
      
      render(<StatsPane />);
      
      fireEvent.click(screen.getByText("All Devices"));
      
      await waitFor(() => {
        expect(screen.getByTestId("monitor-icon")).toBeInTheDocument();
        expect(screen.getByTestId("smartphone-icon")).toBeInTheDocument(); 
        expect(screen.getByTestId("github-icon")).toBeInTheDocument();
      });
    });
  });

  describe("Metadata Display", () => {
    it("should show overlap information in All Devices view", async () => {
      mockInvoke.mockResolvedValue(mockUserAPMData);
      
      render(<StatsPane />);
      
      fireEvent.click(screen.getByText("All Devices"));
      
      await waitFor(() => {
        // Should display metadata about overlapping usage
        expect(screen.getByText(/120/)).toBeInTheDocument(); // Overlapping minutes
        expect(screen.getByText(/3/)).toBeInTheDocument(); // Peak concurrent devices
      });
    });
  });
});