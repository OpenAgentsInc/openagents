import { Effect, Option, Either } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  TrackDeviceSessionArgs,
  TrackDeviceSessionResult,
  CalculateUserAPMArgs,
  CalculateUserAPMResult,
  GetUserAPMStatsArgs,
  GetUserAPMStatsResult,
  GetConvexAPMStatsArgs,
  GetConvexAPMStatsResult,
} from "./apm.schemas";

// Helper function to get authenticated user with Effect-TS patterns (for mutations)
const getAuthenticatedUserEffectMutation = Effect.gen(function* () {
  const { db, auth } = yield* ConfectMutationCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [APM] Looking for user with OpenAuth subject: ${authSubject}`);
  
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () =>
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});

// Helper function to get authenticated user with Effect-TS patterns (for queries)
const getAuthenticatedUserEffectQuery = Effect.gen(function* () {
  const { db, auth } = yield* ConfectQueryCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [APM] Looking for user with OpenAuth subject: ${authSubject}`);
  
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () =>
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});

// Helper function to get time cutoff for different windows
function getTimeCutoff(now: number, window: string): number | null {
  switch (window) {
    case "1h": return now - (1 * 60 * 60 * 1000);
    case "6h": return now - (6 * 60 * 60 * 1000);
    case "1d": return now - (24 * 60 * 60 * 1000);
    case "1w": return now - (7 * 24 * 60 * 60 * 1000);
    case "1m": return now - (30 * 24 * 60 * 60 * 1000);
    case "lifetime": return null; // No cutoff for lifetime
    default: return null;
  }
}

// Helper function to calculate device APM
function calculateDeviceAPM(session: any, cutoff: number | null, now: number): number {
  let totalActions = session.actionsCount.messages + session.actionsCount.toolUses;
  if (session.actionsCount.githubEvents) {
    totalActions += session.actionsCount.githubEvents;
  }

  let totalMinutes = 0;
  
  for (const period of session.sessionPeriods) {
    const periodStart = period.start;
    const periodEnd = period.end || now;
    
    // Skip periods outside time window
    if (cutoff && periodEnd < cutoff) continue;
    
    const intervalStart = cutoff ? Math.max(periodStart, cutoff) : periodStart;
    const intervalEnd = periodEnd;
    
    if (intervalStart < intervalEnd) {
      totalMinutes += (intervalEnd - intervalStart) / (1000 * 60);
    }
  }
  
  return totalMinutes > 0 ? totalActions / totalMinutes : 0;
}

// Helper function to merge overlapping intervals
function mergeOverlappingIntervals(intervals: Array<{ start: number; end: number; deviceType: string; actions: number }>): Array<{ start: number; end: number }> {
  if (intervals.length === 0) return [];
  
  // Sort intervals by start time
  const sorted = intervals.slice().sort((a, b) => a.start - b.start);
  const merged = [{ start: sorted[0].start, end: sorted[0].end }];
  
  for (let i = 1; i < sorted.length; i++) {
    const current = sorted[i];
    const lastMerged = merged[merged.length - 1];
    
    if (current.start <= lastMerged.end) {
      // Overlapping intervals, merge them
      lastMerged.end = Math.max(lastMerged.end, current.end);
    } else {
      // Non-overlapping interval, add it
      merged.push({ start: current.start, end: current.end });
    }
  }
  
  return merged;
}

// Helper function to calculate overlapping minutes
function calculateOverlappingMinutes(intervals: Array<{ start: number; end: number; deviceType: string; actions: number }>): number {
  // TODO: Implement sophisticated overlap calculation
  // For now, return 0 as a placeholder
  return 0;
}

// Helper function to calculate peak concurrency
function calculatePeakConcurrency(intervals: Array<{ start: number; end: number; deviceType: string; actions: number }>): number {
  // TODO: Implement peak concurrency calculation
  // For now, return intervals length as a simple estimate
  return intervals.length;
}

// Helper function to aggregate device metrics with time overlap handling
function aggregateDeviceMetrics(
  deviceSessions: any[],
  cutoff: number | null,
  now: number
): {
  totalActions: number;
  activeMinutes: number;
  deviceBreakdown: { desktop?: number; mobile?: number; github?: number };
  metadata: { overlappingMinutes?: number; peakConcurrency?: number };
} {
  let totalActions = 0;
  const deviceBreakdown: { desktop?: number; mobile?: number; github?: number } = {};
  
  // Collect all time intervals from all devices
  const allIntervals: Array<{ start: number; end: number; deviceType: string; actions: number }> = [];
  
  for (const session of deviceSessions) {
    let deviceActions = session.actionsCount.messages + session.actionsCount.toolUses;
    if (session.actionsCount.githubEvents) {
      deviceActions += session.actionsCount.githubEvents;
    }
    
    totalActions += deviceActions;
    
    // Track device breakdown
    const deviceAPM = calculateDeviceAPM(session, cutoff, now);
    if (deviceAPM > 0) {
      const deviceType = session.deviceType as keyof typeof deviceBreakdown;
      if (deviceType === "desktop" || deviceType === "mobile" || deviceType === "github") {
        deviceBreakdown[deviceType] = (deviceBreakdown[deviceType] || 0) + deviceAPM;
      }
    }
    
    // Collect intervals for overlap calculation
    for (const period of session.sessionPeriods) {
      const periodStart = period.start;
      const periodEnd = period.end || now;
      
      // Skip periods outside time window
      if (cutoff && periodEnd < cutoff) continue;
      
      const intervalStart = cutoff ? Math.max(periodStart, cutoff) : periodStart;
      const intervalEnd = periodEnd;
      
      if (intervalStart < intervalEnd) {
        allIntervals.push({
          start: intervalStart,
          end: intervalEnd,
          deviceType: session.deviceType,
          actions: deviceActions,
        });
      }
    }
  }
  
  // Merge overlapping intervals to get total active time
  const mergedIntervals = mergeOverlappingIntervals(allIntervals);
  const activeMinutes = mergedIntervals.reduce((total, interval) => {
    return total + (interval.end - interval.start) / (1000 * 60);
  }, 0);
  
  // Calculate overlap metadata
  const overlappingMinutes = calculateOverlappingMinutes(allIntervals);
  const peakConcurrency = calculatePeakConcurrency(allIntervals);
  
  return {
    totalActions,
    activeMinutes,
    deviceBreakdown,
    metadata: {
      overlappingMinutes,
      peakConcurrency,
    },
  };
}

// Track device session
export const trackDeviceSession = mutation({
  args: TrackDeviceSessionArgs,
  returns: TrackDeviceSessionResult,
  handler: ({ deviceId, deviceType, sessionStart, sessionEnd, actions, metadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`📊 [APM] trackDeviceSession called:`, {
        deviceId,
        deviceType,
        sessionStart,
        sessionEnd,
        actions,
      });

      // Find existing device session
      const existingSession = yield* db
        .query("userDeviceSessions")
        .withIndex("by_device_id", (q) => q.eq("deviceId", deviceId))
        .first();

      const sessionPeriod = {
        start: sessionStart,
        end: sessionEnd,
      };

      return yield* Option.match(existingSession, {
        onSome: (session) => {
          // Update existing session
          const updatedSessionPeriods = [...session.sessionPeriods, sessionPeriod];
          const updatedActionsCount = {
            messages: session.actionsCount.messages + actions.messages,
            toolUses: session.actionsCount.toolUses + actions.toolUses,
            githubEvents: (session.actionsCount.githubEvents || 0) + (actions.githubEvents || 0),
          };

          return db.patch(session._id, {
            sessionPeriods: updatedSessionPeriods,
            actionsCount: updatedActionsCount,
            lastActivity: sessionEnd || sessionStart,
            metadata,
          }).pipe(Effect.as(session._id));
        },
        onNone: () =>
          // Create new device session
          db.insert("userDeviceSessions", {
            userId: user._id,
            deviceId,
            deviceType,
            sessionPeriods: [sessionPeriod],
            actionsCount: actions,
            lastActivity: sessionEnd || sessionStart,
            metadata,
          })
      });
    }),
});

// Calculate aggregated APM with time overlap handling
export const calculateUserAPM = mutation({
  args: CalculateUserAPMArgs,
  returns: CalculateUserAPMResult,
  handler: ({ timeWindow }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      const now = Date.now();
      const timeWindows = timeWindow ? [timeWindow] : ["1h", "6h", "1d", "1w", "1m", "lifetime"] as const;

      yield* Effect.logInfo(`🧮 [APM] calculateUserAPM for windows:`, timeWindows);

      for (const window of timeWindows) {
        const cutoff = getTimeCutoff(now, window);
        
        // Get all device sessions for this user
        const deviceSessions = yield* db
          .query("userDeviceSessions")
          .withIndex("by_user_id", (q) => q.eq("userId", user._id))
          .collect();

        // Calculate aggregated metrics
        const { totalActions, activeMinutes, deviceBreakdown, metadata } = 
          aggregateDeviceMetrics(deviceSessions, cutoff, now);

        const aggregatedAPM = activeMinutes > 0 ? totalActions / activeMinutes : 0;

        yield* Effect.logInfo(`📈 [APM] Window ${window}:`, {
          aggregatedAPM,
          totalActions,
          activeMinutes,
          deviceBreakdown
        });

        // Store/update cached stats
        const existingStats = yield* db
          .query("userAPMStats")
          .withIndex("by_user_window", (q) => q.eq("userId", user._id).eq("timeWindow", window))
          .first();

        yield* Option.match(existingStats, {
          onSome: (stats) =>
            db.patch(stats._id, {
              aggregatedAPM,
              deviceBreakdown,
              totalActions,
              activeMinutes,
              calculatedAt: now,
              metadata,
            }),
          onNone: () =>
            db.insert("userAPMStats", {
              userId: user._id,
              timeWindow: window,
              aggregatedAPM,
              deviceBreakdown,
              totalActions,
              activeMinutes,
              calculatedAt: now,
              metadata,
            }).pipe(Effect.asVoid)
        });
      }

      return { success: true };
    }),
});

// Get user's aggregated APM stats
export const getUserAPMStats = query({
  args: GetUserAPMStatsArgs,
  returns: GetUserAPMStatsResult,
  handler: ({ includeDeviceBreakdown }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      
      // Try to get authenticated user - return null if not authenticated
      const userResult = yield* Effect.either(getAuthenticatedUserEffectQuery);
      
      if (Either.isLeft(userResult)) {
        yield* Effect.logInfo("❌ [APM] User not authenticated for getUserAPMStats");
        return null;
      }
      
      const user = userResult.right;
      
      // Get cached APM stats for all time windows
      const allStats = yield* db
        .query("userAPMStats")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .collect();

      const statsByWindow = allStats.reduce((acc, stat) => {
        acc[stat.timeWindow] = stat;
        return acc;
      }, {} as Record<string, any>);

      // Build response in same format as existing getConvexAPMStats
      const result = {
        apm1h: statsByWindow["1h"]?.aggregatedAPM || 0,
        apm6h: statsByWindow["6h"]?.aggregatedAPM || 0,
        apm1d: statsByWindow["1d"]?.aggregatedAPM || 0,
        apm1w: statsByWindow["1w"]?.aggregatedAPM || 0,
        apm1m: statsByWindow["1m"]?.aggregatedAPM || 0,
        apmLifetime: statsByWindow["lifetime"]?.aggregatedAPM || 0,
        totalActions: statsByWindow["lifetime"]?.totalActions || 0,
        activeMinutes: statsByWindow["lifetime"]?.activeMinutes || 0,
        ...(includeDeviceBreakdown ? {
          deviceBreakdown: {
            desktop: statsByWindow["lifetime"]?.deviceBreakdown?.desktop || 0,
            mobile: statsByWindow["lifetime"]?.deviceBreakdown?.mobile || 0,
            github: statsByWindow["lifetime"]?.deviceBreakdown?.github || 0,
          },
          metadata: statsByWindow["lifetime"]?.metadata,
        } : {})
      };

      yield* Effect.logInfo(`📊 [APM] getUserAPMStats result:`, result);

      return result;
    }),
});

// Get Convex APM stats (backwards compatibility)
export const getConvexAPMStats = query({
  args: GetConvexAPMStatsArgs,
  returns: GetConvexAPMStatsResult,
  handler: () =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      const user = yield* getAuthenticatedUserEffectQuery;

      yield* Effect.logInfo(`🔍 [APM] getConvexAPMStats for user:`, user._id);

      // Get user-specific sessions only
      const sessions = yield* db
        .query("claudeSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .collect();

      const sessionCount = sessions.length;
      
      // Get message count for user's sessions
      let totalMessages = 0;
      const recentActivity = [];
      
      for (const session of sessions.slice(-10)) { // Last 10 sessions
        const messages = yield* db
          .query("claudeMessages")
          .withIndex("by_session_id", (q) => q.eq("sessionId", session.sessionId))
          .collect();

        const messageCount = messages.length;
        totalMessages += messageCount;
        
        recentActivity.push({
          sessionId: session.sessionId,
          messageCount,
          lastActivity: session.lastActivity,
          title: session.title,
        });
      }

      const avgMessagesPerSession = sessionCount > 0 ? totalMessages / sessionCount : 0;

      // For uniqueUsers, just return 1 since this is user-specific
      const uniqueUsers = 1;

      return {
        sessionCount,
        messageCount: totalMessages,
        avgMessagesPerSession,
        uniqueUsers,
        recentActivity,
      };
    }),
});