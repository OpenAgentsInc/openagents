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
  GetRealtimeAPMArgs,
  GetRealtimeAPMResult,
  UpdateRealtimeAPMArgs,
  UpdateRealtimeAPMResult,
  TrackRealtimeActionArgs,
  TrackRealtimeActionResult,
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
  // For now, return 0 as a placeholder (using intervals.length to avoid unused warning)
  return intervals.length > 0 ? 0 : 0;
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

// Realtime APM functions
export const getRealtimeAPM = query({
  args: GetRealtimeAPMArgs,
  returns: GetRealtimeAPMResult,
  handler: ({ deviceId, includeHistory = false }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;
      const user = yield* getAuthenticatedUserEffectQuery;

      console.log(`📊 [RealtimeAPM] Getting realtime APM for user: ${user._id}, device: ${deviceId || 'current'}`);

      // Get the most recent device session for this user
      const recentSession = yield* db
        .query("userDeviceSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .filter((q) => deviceId ? q.eq(q.field("deviceId"), deviceId) : q.neq(q.field("deviceId"), ""))
        .order("desc")
        .first();

      if (Option.isNone(recentSession)) {
        console.log(`📊 [RealtimeAPM] No session found for user: ${user._id}`);
        return null;
      }

      const session = recentSession.value;
      const now = Date.now();
      
      // Calculate current session metrics
      const sessionStart = Math.min(...session.sessionPeriods.map(p => p.start));
      const sessionEnd = Math.max(...session.sessionPeriods.map(p => p.end || now));
      const sessionDuration = sessionEnd - sessionStart;
      
      // Calculate total actions (actionsCount is in session, not periods)
      const totalActions = session.actionsCount.messages + session.actionsCount.toolUses + (session.actionsCount.githubEvents || 0);
      
      // Calculate current APM
      const currentAPM = sessionDuration > 0 ? (totalActions / (sessionDuration / 60000)) : 0;
      
      // Determine if session is currently active (last activity within 5 minutes)
      const isActive = (now - sessionEnd) < (5 * 60 * 1000);

      // Get APM history for trend calculation if requested
      let history: number[] = [];
      let trend: "up" | "down" | "stable" = "stable";
      let trendPercentage = 0;

      if (includeHistory) {
        // Get recent APM calculations from user's recent sessions
        const recentSessions = yield* db
          .query("userDeviceSessions")
          .withIndex("by_user_id", (q) => q.eq("userId", user._id))
          .filter((q) => deviceId ? q.eq(q.field("deviceId"), deviceId) : q.neq(q.field("deviceId"), ""))
          .order("desc")
          .take(10);

        history = recentSessions.map(s => {
          const sDuration = Math.max(...s.sessionPeriods.map(p => p.end || now)) - 
                           Math.min(...s.sessionPeriods.map(p => p.start));
          const sActions = s.actionsCount.messages + s.actionsCount.toolUses + (s.actionsCount.githubEvents || 0);
          return sDuration > 0 ? (sActions / (sDuration / 60000)) : 0;
        });

        // Calculate trend
        if (history.length > 1) {
          const previousAPM = history[1]; // Second most recent (index 0 is current)
          if (previousAPM > 0) {
            trendPercentage = ((currentAPM - previousAPM) / previousAPM) * 100;
            if (Math.abs(trendPercentage) >= 10) { // 10% threshold
              trend = trendPercentage > 0 ? "up" : "down";
            }
          }
        }
      }

      console.log(`📊 [RealtimeAPM] Current APM: ${currentAPM.toFixed(2)}, Trend: ${trend}, Active: ${isActive}`);

      return {
        currentAPM: Number(currentAPM.toFixed(2)),
        trend,
        sessionDuration,
        totalActions,
        lastUpdateTimestamp: now,
        isActive,
        deviceId: session.deviceId,
        trendPercentage: includeHistory ? Number(trendPercentage.toFixed(1)) : undefined,
        history: includeHistory ? history : undefined,
      };
    }),
});

export const updateRealtimeAPM = mutation({
  args: UpdateRealtimeAPMArgs,
  returns: UpdateRealtimeAPMResult,
  handler: ({ deviceId, currentAPM, totalActions, sessionDuration, isActive }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      console.log(`📊 [RealtimeAPM] Updating APM: ${currentAPM}, Device: ${deviceId}, Active: ${isActive}`);

      const now = Date.now();

      // Find or create device session
      const existingSession = yield* db
        .query("userDeviceSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .filter((q) => q.eq(q.field("deviceId"), deviceId))
        .order("desc")
        .first();

      if (Option.isSome(existingSession)) {
        // Update existing session
        yield* db.patch(existingSession.value._id, {
          sessionPeriods: [
            ...existingSession.value.sessionPeriods.slice(0, -1), // Keep all but last
            {
              ...existingSession.value.sessionPeriods[existingSession.value.sessionPeriods.length - 1],
              end: isActive ? undefined : now,
            }
          ],
          actionsCount: {
            messages: totalActions,
            toolUses: 0,
            githubEvents: 0,
          },
          lastActivity: now,
        });
      } else {
        // Create new session
        yield* db.insert("userDeviceSessions", {
          userId: user._id,
          deviceId,
          deviceType: deviceId.includes("mobile") ? "mobile" : "desktop",
          sessionPeriods: [{
            start: now - sessionDuration,
            end: isActive ? undefined : now,
          }],
          actionsCount: {
            messages: totalActions,
            toolUses: 0,
            githubEvents: 0,
          },
          lastActivity: now,
        });
      }

      return {
        success: true,
        timestamp: now,
      };
    }),
});

export const trackRealtimeAction = mutation({
  args: TrackRealtimeActionArgs,
  returns: TrackRealtimeActionResult,
  handler: ({ deviceId, actionType, timestamp = Date.now(), metadata: _metadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      console.log(`📊 [RealtimeAPM] Tracking action: ${actionType}, Device: ${deviceId}`);

      // Find current active session
      const currentSession = yield* db
        .query("userDeviceSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .filter((q) => q.eq(q.field("deviceId"), deviceId))
        .order("desc")
        .first();

      let newAPM = 0;
      let totalActions = 0;

      if (Option.isSome(currentSession)) {
        const session = currentSession.value;
        const now = Date.now();
        
        // Get the most recent session period
        const lastPeriod = session.sessionPeriods[session.sessionPeriods.length - 1];
        const currentActionsTotal = session.actionsCount.messages + session.actionsCount.toolUses + (session.actionsCount.githubEvents || 0);
        const updatedActionsCount = currentActionsTotal + 1;
        totalActions = updatedActionsCount;
        
        // Calculate new APM
        const sessionStart = lastPeriod.start;
        const duration = now - sessionStart;
        newAPM = duration > 0 ? (updatedActionsCount / (duration / 60000)) : 0;
        
        // Update the session
        const updatedPeriods = [
          ...session.sessionPeriods.slice(0, -1),
          {
            ...lastPeriod,
            end: undefined, // Keep session active
          }
        ];

        yield* db.patch(session._id, {
          sessionPeriods: updatedPeriods,
          actionsCount: {
            messages: updatedActionsCount,
            toolUses: 0,
            githubEvents: 0,
          },
          lastActivity: now,
        });
      } else {
        // Create new session with first action
        totalActions = 1;
        newAPM = 60; // 1 action in 1 second = 60 APM initially

        yield* db.insert("userDeviceSessions", {
          userId: user._id,
          deviceId,
          deviceType: deviceId.includes("mobile") ? "mobile" : "desktop",
          sessionPeriods: [{
            start: timestamp,
            end: undefined,
          }],
          actionsCount: {
            messages: 1,
            toolUses: 0,
            githubEvents: 0,
          },
          lastActivity: timestamp,
        });
      }

      console.log(`📊 [RealtimeAPM] Action tracked, new APM: ${newAPM.toFixed(2)}, total actions: ${totalActions}`);

      return {
        success: true,
        newAPM: Number(newAPM.toFixed(2)),
        totalActions,
      };
    }),
});