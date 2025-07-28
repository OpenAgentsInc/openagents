import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // Users table for authenticated users
  users: defineTable({
    email: v.string(),
    name: v.optional(v.string()),
    avatar: v.optional(v.string()),
    githubId: v.string(),
    githubUsername: v.string(),
    openAuthSubject: v.optional(v.string()), // OpenAuth JWT subject for lookup
    createdAt: v.number(),
    lastLogin: v.number(),
  }).index("by_email", ["email"])
    .index("by_github_id", ["githubId"])
    .index("by_openauth_subject", ["openAuthSubject"]),

  // Existing basic messages (keep for demo compatibility)
  messages: defineTable({
    body: v.string(),
    user: v.string(),
    timestamp: v.number(),
    userId: v.optional(v.id("users")), // Link to authenticated user
  }),
  
  // Claude Code sessions
  claudeSessions: defineTable({
    sessionId: v.string(),          // From desktop Claude Code
    projectPath: v.string(),        // Project being worked on
    title: v.optional(v.string()),  // Session title/description  
    status: v.union(                // Session state
      v.literal("active"),
      v.literal("inactive"), 
      v.literal("error"),
      v.literal("processed")
    ),
    createdBy: v.union(             // Which platform created it
      v.literal("desktop"),
      v.literal("mobile")
    ),
    lastActivity: v.number(),       // Timestamp of last message
    userId: v.optional(v.id("users")), // Link to authenticated user
    metadata: v.optional(v.object({ // Additional session data
      workingDirectory: v.optional(v.string()),
      model: v.optional(v.string()),
      systemPrompt: v.optional(v.string()),
      originalMobileSessionId: v.optional(v.string()), // Track original mobile session
    })),
  }).index("by_session_id", ["sessionId"])
    .index("by_status", ["status"])
    .index("by_last_activity", ["lastActivity"])
    .index("by_user_id", ["userId"]),
    
  // Claude Code messages within sessions  
  claudeMessages: defineTable({
    sessionId: v.string(),         // References claudeSessions
    messageId: v.string(),         // Unique message ID
    messageType: v.union(          // Message type from Claude Code
      v.literal("user"),
      v.literal("assistant"), 
      v.literal("tool_use"),
      v.literal("tool_result"),
      v.literal("thinking")
    ),
    content: v.string(),           // Message content
    timestamp: v.string(),         // ISO timestamp
    userId: v.optional(v.id("users")), // Link to authenticated user
    toolInfo: v.optional(v.object({ // Tool information if applicable
      toolName: v.string(),
      toolUseId: v.string(), 
      input: v.any(),             // Tool input parameters
      output: v.optional(v.string()), // Tool output if available
    })),
    metadata: v.optional(v.any()), // Additional message metadata
  }).index("by_session_id", ["sessionId"])
    .index("by_timestamp", ["timestamp"])
    .index("by_user_id", ["userId"]),
    
  // Sync status tracking
  syncStatus: defineTable({
    sessionId: v.string(),
    lastSyncedMessageId: v.optional(v.string()),
    desktopLastSeen: v.optional(v.number()),
    mobileLastSeen: v.optional(v.number()),
    syncErrors: v.optional(v.array(v.string())),
  }).index("by_session_id", ["sessionId"]),

  // Multi-client APM tracking
  userDeviceSessions: defineTable({
    userId: v.id("users"),                 // Link to authenticated user
    deviceId: v.string(),                  // Unique device identifier
    deviceType: v.union(                   // Device type
      v.literal("desktop"),
      v.literal("mobile"),
      v.literal("github")
    ),
    sessionPeriods: v.array(v.object({     // Active time periods for this device
      start: v.number(),                   // Timestamp when session started
      end: v.optional(v.number()),         // Timestamp when session ended (optional for active sessions)
    })),
    actionsCount: v.object({               // Actions performed on this device
      messages: v.number(),
      toolUses: v.number(),
      githubEvents: v.optional(v.number()), // For GitHub integration
    }),
    lastActivity: v.number(),              // Last activity timestamp
    metadata: v.optional(v.object({        // Device-specific metadata
      platform: v.optional(v.string()),   // OS platform for desktop/mobile
      version: v.optional(v.string()),     // App version
      location: v.optional(v.string()),    // Geographic location (optional)
    })),
  }).index("by_user_id", ["userId"])
    .index("by_device_id", ["deviceId"])
    .index("by_user_device", ["userId", "deviceType"])
    .index("by_last_activity", ["lastActivity"]),

  // Aggregated user APM statistics (cached for performance)
  userAPMStats: defineTable({
    userId: v.id("users"),                 // Link to authenticated user
    timeWindow: v.union(                   // Time window for these stats
      v.literal("1h"),
      v.literal("6h"), 
      v.literal("1d"),
      v.literal("1w"),
      v.literal("1m"),
      v.literal("lifetime")
    ),
    aggregatedAPM: v.number(),             // Combined APM across all devices
    deviceBreakdown: v.object({            // APM breakdown by device type
      desktop: v.optional(v.number()),
      mobile: v.optional(v.number()),
      github: v.optional(v.number()),
    }),
    totalActions: v.number(),              // Total actions in this time window
    activeMinutes: v.number(),             // Total active minutes (with overlap handling)
    calculatedAt: v.number(),              // When this was calculated
    metadata: v.optional(v.object({        // Additional aggregation metadata
      overlappingMinutes: v.optional(v.number()), // Minutes of concurrent device usage
      peakConcurrency: v.optional(v.number()),     // Max devices active simultaneously
    })),
  }).index("by_user_id", ["userId"])
    .index("by_user_window", ["userId", "timeWindow"])
    .index("by_calculated_at", ["calculatedAt"]),

  // GitHub integration events tracking
  githubEvents: defineTable({
    userId: v.id("users"),                 // Link to authenticated user
    eventType: v.string(),                 // GitHub event type (e.g., "issues.opened")
    action: v.string(),                    // GitHub action (e.g., "opened", "closed")
    timestamp: v.number(),                 // Event timestamp
    repository: v.optional(v.string()),    // Repository full name
    payload: v.optional(v.any()),          // GitHub webhook payload (for debugging)
    deviceId: v.string(),                  // GitHub device ID for APM tracking
  }).index("by_user_id", ["userId"])
    .index("by_timestamp", ["timestamp"])
    .index("by_event_type", ["eventType"])
    .index("by_repository", ["repository"]),
});