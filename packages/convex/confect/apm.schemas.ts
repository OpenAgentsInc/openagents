import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// TrackDeviceSession schemas
export const TrackDeviceSessionArgs = Schema.Struct({
  deviceId: Schema.String,
  deviceType: Schema.Literal("desktop", "mobile", "github"),
  sessionStart: Schema.Number,
  sessionEnd: Schema.optional(Schema.Number),
  actions: Schema.Struct({
    messages: Schema.Number,
    toolUses: Schema.Number,
    githubEvents: Schema.optional(Schema.Number),
  }),
  metadata: Schema.optional(
    Schema.Struct({
      platform: Schema.optional(Schema.String),
      version: Schema.optional(Schema.String),
      location: Schema.optional(Schema.String),
    })
  ),
});

export const TrackDeviceSessionResult = Id.Id("userDeviceSessions");

// CalculateUserAPM schemas
export const CalculateUserAPMArgs = Schema.Struct({
  timeWindow: Schema.optional(Schema.Literal("1h", "6h", "1d", "1w", "1m", "lifetime")),
});

export const CalculateUserAPMResult = Schema.Struct({
  success: Schema.Boolean,
});

// GetUserAPMStats schemas
export const GetUserAPMStatsArgs = Schema.Struct({
  includeDeviceBreakdown: Schema.optional(Schema.Boolean),
});

export const GetUserAPMStatsResult = Schema.Union(
  Schema.Null,
  Schema.Struct({
    apm1h: Schema.Number,
    apm6h: Schema.Number,
    apm1d: Schema.Number,
    apm1w: Schema.Number,
    apm1m: Schema.Number,
    apmLifetime: Schema.Number,
    totalActions: Schema.Number,
    activeMinutes: Schema.Number,
    deviceBreakdown: Schema.optional(
      Schema.Struct({
        desktop: Schema.optional(Schema.Number),
        mobile: Schema.optional(Schema.Number),
        github: Schema.optional(Schema.Number),
      })
    ),
    metadata: Schema.optional(Schema.Any),
  })
);

// GetConvexAPMStats schemas (for backwards compatibility)
export const GetConvexAPMStatsArgs = Schema.Struct({});

export const GetConvexAPMStatsResult = Schema.Struct({
  sessionCount: Schema.Number,
  messageCount: Schema.Number,
  avgMessagesPerSession: Schema.Number,
  uniqueUsers: Schema.Number,
  recentActivity: Schema.Array(
    Schema.Struct({
      sessionId: Schema.String,
      messageCount: Schema.Number,
      lastActivity: Schema.Number,
      title: Schema.optional(Schema.String),
    })
  ),
});

// Realtime APM schemas
export const GetRealtimeAPMArgs = Schema.Struct({
  deviceId: Schema.optional(Schema.String),
  includeHistory: Schema.optional(Schema.Boolean),
});

export const GetRealtimeAPMResult = Schema.Union(
  Schema.Null,
  Schema.Struct({
    currentAPM: Schema.Number,
    trend: Schema.Literal("up", "down", "stable"),
    sessionDuration: Schema.Number,
    totalActions: Schema.Number,
    lastUpdateTimestamp: Schema.Number,
    isActive: Schema.Boolean,
    deviceId: Schema.String,
    trendPercentage: Schema.optional(Schema.Number),
    history: Schema.optional(Schema.Array(Schema.Number)),
  })
);

export const UpdateRealtimeAPMArgs = Schema.Struct({
  deviceId: Schema.String,
  currentAPM: Schema.Number,
  totalActions: Schema.Number,
  sessionDuration: Schema.Number,
  isActive: Schema.Boolean,
});

export const UpdateRealtimeAPMResult = Schema.Struct({
  success: Schema.Boolean,
  timestamp: Schema.Number,
});

export const SubscribeRealtimeAPMArgs = Schema.Struct({
  deviceId: Schema.optional(Schema.String),
  updateInterval: Schema.optional(Schema.Number),
});

export const TrackRealtimeActionArgs = Schema.Struct({
  deviceId: Schema.String,
  actionType: Schema.Literal("message", "session", "tool", "github"),
  timestamp: Schema.optional(Schema.Number),
  metadata: Schema.optional(Schema.Any),
});

export const TrackRealtimeActionResult = Schema.Struct({
  success: Schema.Boolean,
  newAPM: Schema.Number,
  totalActions: Schema.Number,
});