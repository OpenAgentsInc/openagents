import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { StatsPane } from "./StatsPane";

// Mock the invoke function directly  
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

// Mock the icons component
vi.mock("@/components/icons/React19Icons", () => ({
  BarChartIcon: () => <div data-testid="barchart-icon" />,
  ClockIcon: () => <div data-testid="clock-icon" />,
  TrendingUpIcon: () => <div data-testid="trending-up-icon" />,
  LoaderIcon: () => <div data-testid="loader2-icon" />,
  RefreshIcon: () => <div data-testid="refresh-cw-icon" />,
  EyeIcon: () => <div data-testid="eye-icon" />,
}));

// Mock the chart component
vi.mock("@/components/charts/HistoricalAPMChart", () => ({
  HistoricalAPMChart: () => <div data-testid="historical-apm-chart" />,
}));

// Create proper mock data matching the component's expected interfaces
const mockCombinedAPMData = {
  apm1h: 45.5,
  apm6h: 42.3,
  apm1d: 38.7,
  apm1w: 35.2,
  apm1m: 32.1,
  apmLifetime: 234.4,
  totalSessions: 150,
  totalMessages: 12500,
  totalToolUses: 8300,
  totalDuration: 3200,
  toolUsage: [
    { name: "Read", count: 2500, percentage: 30.1, category: "file" },
    { name: "Write", count: 2000, percentage: 24.1, category: "file" },
    { name: "Bash", count: 1500, percentage: 18.1, category: "system" },
  ],
  recentSessions: [
    {
      id: "session-1",
      project: "test-project",
      apm: 45.2,
      duration: 120,
      messageCount: 25,
      toolCount: 8,
      timestamp: "2024-01-01T10:00:00Z",
    },
  ],
  productivityByTime: {
    morning: 42.1,
    afternoon: 38.7,
    evening: 35.2,
    night: 28.9,
  },
  cliStats: {
    apm1h: 25.3,
    apm6h: 23.1,
    apm1d: 21.4,
    apm1w: 19.7,
    apm1m: 18.2,
    apmLifetime: 125.6,
    totalSessions: 75,
    totalMessages: 6000,
    totalToolUses: 4000,
    totalDuration: 1600,
    toolUsage: [],
    recentSessions: [],
    productivityByTime: { morning: 0, afternoon: 0, evening: 0, night: 0 },
  },
  sdkStats: {
    apm1h: 20.2,
    apm6h: 19.2,
    apm1d: 17.3,
    apm1w: 15.5,
    apm1m: 13.9,
    apmLifetime: 108.8,
    totalSessions: 75,
    totalMessages: 6500,
    totalToolUses: 4300,
    totalDuration: 1600,
    toolUsage: [],
    recentSessions: [],
    productivityByTime: { morning: 0, afternoon: 0, evening: 0, night: 0 },
  },
};

const mockAggregatedAPMData = {
  apm1h: 52.3,
  apm6h: 48.7,
  apm1d: 44.2,
  apm1w: 41.5,
  apm1m: 38.9,
  apmLifetime: 287.6,
  totalActions: 15000,
  activeMinutes: 3600,
  deviceBreakdown: {
    desktop: 35.2,
    mobile: 12.5,
    github: 4.6,
  },
  metadata: {
    overlappingMinutes: 120,
    peakConcurrency: 3,
  },
};

describe("StatsPane", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockReset();
    // Mock both invoke calls that the component makes
    mockInvoke
      .mockResolvedValueOnce({ success: true, data: mockCombinedAPMData })  // analyze_combined_conversations
      .mockResolvedValue({ success: true, data: mockAggregatedAPMData }); // get_user_apm_stats
  });

  describe("Rendering", () => {
    it("should render loading state initially", async () => {
      // Clear all existing mocks and set up delayed response
      mockInvoke.mockReset();
      mockInvoke.mockImplementation((command) => {
        if (command === 'analyze_combined_conversations') {
          return new Promise(resolve => 
            setTimeout(() => resolve({ success: true, data: mockCombinedAPMData }), 200)
          );
        }
        // Return aggregated data immediately for second call
        return Promise.resolve({ success: true, data: mockAggregatedAPMData });
      });
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      // Should show loading state immediately
      expect(screen.getByText("Analyzing conversations...")).toBeInTheDocument();
      
      // Wait for loading to complete
      await waitFor(() => {
        expect(screen.getByText("45.5")).toBeInTheDocument();
      }, { timeout: 1000 });
    });

    it("should render APM stats when data is loaded", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("45.5")).toBeInTheDocument(); // 1 Hour APM
        expect(screen.getByText("42.30")).toBeInTheDocument(); // 6 Hour APM  
        expect(screen.getByText("38.700")).toBeInTheDocument(); // 1 Day APM
      });
    });

    it("should show view mode toggle buttons", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Combined" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "CLI Only" })).toBeInTheDocument(); 
        expect(screen.getByRole("button", { name: "SDK Only" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "All Devices" })).toBeInTheDocument();
      });
    });

    it("should highlight active view mode", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        const combinedButton = screen.getByRole("button", { name: "Combined" });
        expect(combinedButton).toHaveClass("bg-primary");
      });
    });
  });

  describe("View Mode Switching", () => {
    it("should switch to All Devices view when clicked", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Combined" })).toBeInTheDocument();
      });
      
      await act(async () => {
        const allDevicesButton = screen.getByRole("button", { name: "All Devices" });
        fireEvent.click(allDevicesButton);
      });
      
      await waitFor(() => {
        expect(screen.getByText("52.3")).toBeInTheDocument(); // Aggregated 1h APM
        expect(screen.getByText("48.70")).toBeInTheDocument(); // Aggregated 6h APM
      });
    });

    it("should switch to CLI Only view", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByRole("button", { name: "Combined" })).toBeInTheDocument();
      });
      
      await act(async () => {
        const cliButton = screen.getByRole("button", { name: "CLI Only" });
        fireEvent.click(cliButton);
      });
      
      await waitFor(() => {
        expect(screen.getByText("25.3")).toBeInTheDocument(); // CLI 1h APM
        expect(screen.getByText("23.10")).toBeInTheDocument(); // CLI 6h APM
      });
    });

    it("should maintain view mode when stats refresh", async () => {
      vi.useFakeTimers();
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      // Switch to All Devices
      await waitFor(() => screen.getByRole("button", { name: "All Devices" }));
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "All Devices" }));
      });
      
      // Fast forward past the auto-refresh interval (10 seconds)  
      await act(async () => {
        vi.advanceTimersByTime(11000);
      });
      
      // Should maintain All Devices view after refresh
      await waitFor(() => {
        expect(screen.getByText("52.3")).toBeInTheDocument(); // Still showing aggregated data
      });
      
      vi.useRealTimers();
    });
  });

  describe("Auto-refresh", () => {
    it("should refresh stats every 10 seconds when data exists", async () => {
      vi.useFakeTimers();
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      // Wait for initial load
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledTimes(1);
      });
      
      // Fast forward past the auto-refresh interval (10 seconds)
      await act(async () => {
        vi.advanceTimersByTime(11000);
      });
      
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledTimes(2);
      });
      
      vi.useRealTimers();
    });

    it("should not auto-refresh when no initial data", async () => {
      vi.useFakeTimers();
      
      // Mock to return error on first call, preventing stats from being set
      mockInvoke.mockRejectedValueOnce(new Error("No data"));
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledTimes(1);
      });
      
      // Fast forward past refresh interval
      await act(async () => {
        vi.advanceTimersByTime(11000);
      });
      
      // Should still only be called once since no stats were loaded
      expect(mockInvoke).toHaveBeenCalledTimes(1);
      
      vi.useRealTimers();
    });
  });

  describe("Error Handling", () => {
    it("should handle API errors gracefully", async () => {
      mockInvoke.mockRejectedValue(new Error("API Error"));
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(consoleSpy).toHaveBeenCalledWith(
          "Error loading APM stats:",
          expect.any(Error)
        );
      });
      
      // Should show error UI with retry button
      expect(screen.getByText("API Error")).toBeInTheDocument();
      expect(screen.getByText("Retry")).toBeInTheDocument();
      
      consoleSpy.mockRestore();
    });

    it("should handle invalid response format", async () => {
      mockInvoke.mockResolvedValue({ invalid: "response" });
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("Invalid response format")).toBeInTheDocument();
        expect(screen.getByText("Retry")).toBeInTheDocument();
      });
    });

    it("should handle missing aggregated data gracefully", async () => {
      mockInvoke
        .mockResolvedValueOnce({ success: true, data: mockCombinedAPMData })
        .mockRejectedValue(new Error("Not authenticated"));
      
      const consoleSpy = vi.spyOn(console, "log").mockImplementation(() => {});
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        // Should render successfully with just combined data
        expect(screen.getByText("45.5")).toBeInTheDocument();
        // All Devices button should be disabled
        expect(screen.getByText("All Devices").parentElement).toBeDisabled();
      });
      
      consoleSpy.mockRestore();
    });
  });

  describe("All Time Stats", () => {
    it("should display lifetime APM statistics", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("234.400")).toBeInTheDocument(); // Lifetime APM
        expect(screen.getByText("150")).toBeInTheDocument(); // Total Sessions
        expect(screen.getByText("12,500")).toBeInTheDocument(); // Total Messages
      });
    });

    it("should format large numbers correctly", async () => {
      const largeNumberData = {
        ...mockCombinedAPMData,
        totalSessions: 1500,
        totalMessages: 1234567,
        totalDuration: 98765,
        apmLifetime: 751.234,
      };
      mockInvoke.mockResolvedValue({ success: true, data: largeNumberData });
      
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("1,500")).toBeInTheDocument(); // Sessions
        expect(screen.getByText("1,234,567")).toBeInTheDocument(); // Messages
        expect(screen.getByText("751.234")).toBeInTheDocument(); // Lifetime APM
      });
    });
  });

  describe("Summary Stats", () => {
    it("should display summary statistics", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("Summary")).toBeInTheDocument();
        expect(screen.getByText("Total Sessions")).toBeInTheDocument();
        expect(screen.getByText("Total Messages")).toBeInTheDocument();
        expect(screen.getByText("Total Tools")).toBeInTheDocument();
        expect(screen.getByText("Total Time")).toBeInTheDocument();
      });
    });
  });

  describe("Productivity by Time", () => {
    it("should show productivity breakdown by time of day", async () => {
      await act(async () => {
        render(<StatsPane />);
      });
      
      await waitFor(() => {
        expect(screen.getByText("Productivity by Time")).toBeInTheDocument();
        expect(screen.getByText("Morning (6-12)")).toBeInTheDocument();
        expect(screen.getByText("Afternoon (12-18)")).toBeInTheDocument();
        expect(screen.getByText("Evening (18-24)")).toBeInTheDocument();
        expect(screen.getByText("Night (0-6)")).toBeInTheDocument();
      });
    });
  });
});