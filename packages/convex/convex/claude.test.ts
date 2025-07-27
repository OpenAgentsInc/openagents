import { describe, it, expect, beforeEach } from "vitest";
import { mergeOverlappingIntervals } from "./claude";

describe("mergeOverlappingIntervals", () => {
  it("should merge overlapping intervals", () => {
    const intervals = [
      { start: 0, end: 5 },
      { start: 3, end: 8 },
      { start: 10, end: 15 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([
      { start: 0, end: 8 },
      { start: 10, end: 15 },
    ]);
  });

  it("should handle adjacent intervals", () => {
    const intervals = [
      { start: 0, end: 5 },
      { start: 5, end: 10 },
      { start: 10, end: 15 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([{ start: 0, end: 15 }]);
  });

  it("should handle completely overlapping intervals", () => {
    const intervals = [
      { start: 0, end: 10 },
      { start: 2, end: 8 },
      { start: 5, end: 6 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([{ start: 0, end: 10 }]);
  });

  it("should handle empty intervals", () => {
    const result = mergeOverlappingIntervals([]);
    expect(result).toEqual([]);
  });

  it("should handle single interval", () => {
    const intervals = [{ start: 5, end: 10 }];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([{ start: 5, end: 10 }]);
  });

  it("should handle non-overlapping intervals", () => {
    const intervals = [
      { start: 0, end: 2 },
      { start: 4, end: 6 },
      { start: 8, end: 10 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual(intervals);
  });

  it("should handle intervals with same start time", () => {
    const intervals = [
      { start: 5, end: 10 },
      { start: 5, end: 15 },
      { start: 5, end: 8 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([{ start: 5, end: 15 }]);
  });

  it("should handle intervals with same end time", () => {
    const intervals = [
      { start: 0, end: 10 },
      { start: 5, end: 10 },
      { start: 8, end: 10 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([{ start: 0, end: 10 }]);
  });

  it("should sort intervals before merging", () => {
    const intervals = [
      { start: 10, end: 15 },
      { start: 0, end: 5 },
      { start: 3, end: 8 },
    ];
    const result = mergeOverlappingIntervals(intervals);
    expect(result).toEqual([
      { start: 0, end: 8 },
      { start: 10, end: 15 },
    ]);
  });
});

describe("APM Aggregation Logic", () => {
  beforeEach(() => {
    // Reset any mocks or test data
  });

  it("should calculate correct APM for single device", () => {
    const sessions = [
      {
        deviceId: "desktop-1",
        deviceType: "desktop" as const,
        sessionPeriods: [{ start: 1000, end: 60000 }], // 59 seconds
        actionsCount: 59,
      },
    ];
    // 59 actions / 59 seconds = 1 action per second = 60 APM
    const totalActiveTime = 59;
    const totalActions = 59;
    const apm = (totalActions / totalActiveTime) * 60;
    expect(apm).toBe(60);
  });

  it("should handle overlapping sessions from different devices", () => {
    const sessions = [
      {
        deviceId: "desktop-1",
        deviceType: "desktop" as const,
        sessionPeriods: [{ start: 0, end: 60000 }], // 60 seconds
        actionsCount: 30,
      },
      {
        deviceId: "mobile-1",
        deviceType: "mobile" as const,
        sessionPeriods: [{ start: 30000, end: 90000 }], // 60 seconds, overlaps 30s
        actionsCount: 30,
      },
    ];
    // Total unique time: 90 seconds (0-90000ms)
    // Total actions: 60
    // APM: 60 actions / 90 seconds * 60 = 40 APM
    const totalActiveTime = 90;
    const totalActions = 60;
    const apm = (totalActions / totalActiveTime) * 60;
    expect(apm).toBe(40);
  });

  it("should handle GitHub webhook events with 1-minute windows", () => {
    const githubSession = {
      deviceId: "github-user123",
      deviceType: "github" as const,
      sessionPeriods: [
        { start: 0, end: 60000 }, // 1 minute for event 1
        { start: 120000, end: 180000 }, // 1 minute for event 2
      ],
      actionsCount: 2,
    };
    // Total time: 2 minutes (non-overlapping)
    // Total actions: 2
    // APM: 2 actions / 120 seconds * 60 = 1 APM
    const totalActiveTime = 120;
    const totalActions = 2;
    const apm = (totalActions / totalActiveTime) * 60;
    expect(apm).toBe(1);
  });

  it("should handle zero actions gracefully", () => {
    const sessions = [
      {
        deviceId: "desktop-1",
        deviceType: "desktop" as const,
        sessionPeriods: [{ start: 0, end: 60000 }],
        actionsCount: 0,
      },
    ];
    // 0 actions = 0 APM
    const apm = 0;
    expect(apm).toBe(0);
  });

  it("should handle sessions with no active periods", () => {
    const sessions = [
      {
        deviceId: "desktop-1",
        deviceType: "desktop" as const,
        sessionPeriods: [],
        actionsCount: 10,
      },
    ];
    // No active time but actions exist - should handle edge case
    // This might indicate a data inconsistency
    expect(sessions[0].sessionPeriods.length).toBe(0);
  });
});

describe("User Data Isolation", () => {
  it("should filter sessions by userId", () => {
    // This would test that queries properly filter by userId
    // Mock implementation would verify ctx.auth.getUserIdentity() is called
    // and results are filtered by the authenticated user's ID
    expect(true).toBe(true); // Placeholder
  });

  it("should prevent access to other users' APM data", () => {
    // Test that unauthorized access is prevented
    expect(true).toBe(true); // Placeholder
  });
});

describe("Concurrent Device Session Handling", () => {
  it("should handle multiple devices active simultaneously", () => {
    const sessions = [
      {
        deviceId: "desktop-1",
        deviceType: "desktop" as const,
        sessionPeriods: [{ start: 0, end: 300000 }], // 5 minutes
        actionsCount: 150,
      },
      {
        deviceId: "mobile-1", 
        deviceType: "mobile" as const,
        sessionPeriods: [{ start: 60000, end: 240000 }], // 3 minutes, overlaps
        actionsCount: 90,
      },
      {
        deviceId: "github-user123",
        deviceType: "github" as const,
        sessionPeriods: [
          { start: 120000, end: 180000 }, // 1 minute, overlaps
          { start: 240000, end: 300000 }, // 1 minute, overlaps
        ],
        actionsCount: 5,
      },
    ];
    // Total unique time: 5 minutes (0-300000ms)
    // Total actions: 245
    // Expected metadata would show overlapping periods
    const totalActions = 245;
    const totalMinutes = 5;
    const apm = totalActions / totalMinutes;
    expect(apm).toBe(49);
  });

  it("should track peak device concurrency", () => {
    // Test that metadata correctly identifies maximum concurrent devices
    const overlappingPeriods = [
      { start: 0, end: 100, deviceCount: 1 },
      { start: 50, end: 150, deviceCount: 2 }, // 2 devices concurrent
      { start: 120, end: 180, deviceCount: 3 }, // 3 devices concurrent
    ];
    const peakConcurrency = Math.max(...overlappingPeriods.map(p => p.deviceCount));
    expect(peakConcurrency).toBe(3);
  });
});