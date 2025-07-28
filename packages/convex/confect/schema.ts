import { defineSchema, defineTable, Id } from "@rjdellecese/confect/server";
import { Schema } from "effect";

export const confectSchema = defineSchema({
  // Users table for authenticated users
  users: defineTable(
    Schema.Struct({
      email: Schema.String.pipe(Schema.nonEmpty()),
      name: Schema.optional(Schema.String),
      avatar: Schema.optional(Schema.String),
      githubId: Schema.String.pipe(Schema.nonEmpty()),
      githubUsername: Schema.String.pipe(Schema.nonEmpty()),
      createdAt: Schema.Number,
      lastLogin: Schema.Number,
    })
  ).index("by_email", ["email"])
   .index("by_github_id", ["githubId"]),

  // Existing basic messages (keep for demo compatibility)
  messages: defineTable(
    Schema.Struct({
      body: Schema.String,
      user: Schema.String,
      timestamp: Schema.Number,
      userId: Schema.optional(Id.Id("users")),
    })
  ),

  // Claude Code sessions
  claudeSessions: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      projectPath: Schema.String.pipe(Schema.nonEmpty()),
      title: Schema.optional(Schema.String),
      status: Schema.Literal("active", "inactive", "error", "processed"),
      createdBy: Schema.Literal("desktop", "mobile"),
      lastActivity: Schema.Number,
      userId: Schema.optional(Id.Id("users")),
      metadata: Schema.optional(
        Schema.Struct({
          workingDirectory: Schema.optional(Schema.String),
          model: Schema.optional(Schema.String),
          systemPrompt: Schema.optional(Schema.String),
          originalMobileSessionId: Schema.optional(Schema.String),
        })
      ),
    })
  ).index("by_session_id", ["sessionId"])
   .index("by_status", ["status"])
   .index("by_last_activity", ["lastActivity"])
   .index("by_user_id", ["userId"]),
   
  // Claude Code messages within sessions
  claudeMessages: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      messageId: Schema.String.pipe(Schema.nonEmpty()),
      messageType: Schema.Literal("user", "assistant", "tool_use", "tool_result", "thinking"),
      content: Schema.String,
      timestamp: Schema.String, // ISO timestamp
      userId: Schema.optional(Id.Id("users")),
      toolInfo: Schema.optional(
        Schema.Struct({
          toolName: Schema.String,
          toolUseId: Schema.String,
          input: Schema.Any, // Tool input parameters
          output: Schema.optional(Schema.String), // Tool output if available
        })
      ),
      metadata: Schema.optional(Schema.Any), // Additional message metadata
    })
  ).index("by_session_id", ["sessionId"])
   .index("by_timestamp", ["timestamp"])
   .index("by_user_id", ["userId"]),

  // Sync status tracking
  syncStatus: defineTable(
    Schema.Struct({
      sessionId: Schema.String.pipe(Schema.nonEmpty()),
      lastSyncedMessageId: Schema.optional(Schema.String),
      desktopLastSeen: Schema.optional(Schema.Number),
      mobileLastSeen: Schema.optional(Schema.Number),
      syncErrors: Schema.optional(Schema.Array(Schema.String)),
    })
  ).index("by_session_id", ["sessionId"]),

  // Multi-client APM tracking
  userDeviceSessions: defineTable(
    Schema.Struct({
      userId: Id.Id("users"),
      deviceId: Schema.String.pipe(Schema.nonEmpty()),
      deviceType: Schema.Literal("desktop", "mobile", "github"),
      sessionPeriods: Schema.Array(
        Schema.Struct({
          start: Schema.Number,
          end: Schema.optional(Schema.Number),
        })
      ),
      actionsCount: Schema.Struct({
        messages: Schema.Number,
        toolUses: Schema.Number,
        githubEvents: Schema.optional(Schema.Number),
      }),
      lastActivity: Schema.Number,
      metadata: Schema.optional(
        Schema.Struct({
          platform: Schema.optional(Schema.String),
          version: Schema.optional(Schema.String),
          location: Schema.optional(Schema.String),
        })
      ),
    })
  ).index("by_user_id", ["userId"])
   .index("by_device_id", ["deviceId"])
   .index("by_user_device", ["userId", "deviceType"])
   .index("by_last_activity", ["lastActivity"]),

  // Aggregated user APM statistics (cached for performance)
  userAPMStats: defineTable(
    Schema.Struct({
      userId: Id.Id("users"),
      timeWindow: Schema.Literal("1h", "6h", "1d", "1w", "1m", "lifetime"),
      aggregatedAPM: Schema.Number,
      deviceBreakdown: Schema.Struct({
        desktop: Schema.optional(Schema.Number),
        mobile: Schema.optional(Schema.Number),
        github: Schema.optional(Schema.Number),
      }),
      totalActions: Schema.Number,
      activeMinutes: Schema.Number,
      calculatedAt: Schema.Number,
      metadata: Schema.optional(
        Schema.Struct({
          overlappingMinutes: Schema.optional(Schema.Number),
          peakConcurrency: Schema.optional(Schema.Number),
        })
      ),
    })
  ).index("by_user_id", ["userId"])
   .index("by_user_window", ["userId", "timeWindow"])
   .index("by_calculated_at", ["calculatedAt"]),

  // GitHub integration events tracking
  githubEvents: defineTable(
    Schema.Struct({
      userId: Id.Id("users"),
      eventType: Schema.String.pipe(Schema.nonEmpty()),
      action: Schema.String.pipe(Schema.nonEmpty()),
      timestamp: Schema.Number,
      repository: Schema.optional(Schema.String),
      payload: Schema.optional(Schema.Any), // GitHub webhook payload
      deviceId: Schema.String.pipe(Schema.nonEmpty()), // GitHub device ID for APM tracking
    })
  ).index("by_user_id", ["userId"])
   .index("by_timestamp", ["timestamp"])
   .index("by_event_type", ["eventType"])
   .index("by_repository", ["repository"]),

  // User onboarding progress tracking
  onboardingProgress: defineTable(
    Schema.Struct({
      userId: Id.Id("users"),
      step: Schema.Literal(
        "welcome",
        "permissions_explained", 
        "github_connected",
        "repository_selected",
        "preferences_set",
        "completed"
      ),
      startedAt: Schema.Number,
      completedAt: Schema.optional(Schema.Number),
      completedSteps: Schema.Array(Schema.String),
      activeRepository: Schema.optional(
        Schema.Struct({
          url: Schema.String.pipe(Schema.nonEmpty()),
          name: Schema.String.pipe(Schema.nonEmpty()),
          owner: Schema.String.pipe(Schema.nonEmpty()),
          isPrivate: Schema.Boolean,
          defaultBranch: Schema.optional(Schema.String),
        })
      ),
      preferences: Schema.optional(
        Schema.Struct({
          theme: Schema.optional(Schema.Literal("light", "dark", "system")),
          notifications: Schema.optional(Schema.Boolean),
          autoSync: Schema.optional(Schema.Boolean),
          defaultModel: Schema.optional(Schema.String),
        })
      ),
      permissions: Schema.optional(
        Schema.Struct({
          notifications: Schema.Boolean,
          storage: Schema.Boolean,
          network: Schema.Boolean,
          camera: Schema.optional(Schema.Boolean),
          microphone: Schema.optional(Schema.Boolean),
        })
      ),
      metadata: Schema.optional(
        Schema.Struct({
          platform: Schema.optional(Schema.String),
          version: Schema.optional(Schema.String),
          deviceModel: Schema.optional(Schema.String),
          skipReason: Schema.optional(Schema.String), // If user skipped certain steps
        })
      ),
    })
  ).index("by_user_id", ["userId"])
   .index("by_step", ["step"])
   .index("by_started_at", ["startedAt"])
   .index("by_completed_at", ["completedAt"]),

  // User permissions state tracking
  userPermissions: defineTable(
    Schema.Struct({
      userId: Id.Id("users"),
      permissionType: Schema.Literal(
        "notifications",
        "storage", 
        "network",
        "camera",
        "microphone",
        "location"
      ),
      status: Schema.Literal("granted", "denied", "not_requested", "restricted"),
      requestedAt: Schema.optional(Schema.Number),
      grantedAt: Schema.optional(Schema.Number),
      deniedAt: Schema.optional(Schema.Number),
      platform: Schema.String.pipe(Schema.nonEmpty()), // "ios", "android", "desktop", "web"
      metadata: Schema.optional(
        Schema.Struct({
          reason: Schema.optional(Schema.String), // Why permission was needed
          fallbackEnabled: Schema.optional(Schema.Boolean), // If fallback is available
          retryCount: Schema.optional(Schema.Number),
          lastRetryAt: Schema.optional(Schema.Number),
        })
      ),
    })
  ).index("by_user_id", ["userId"])
   .index("by_permission_type", ["permissionType"])
   .index("by_status", ["status"])
   .index("by_user_permission", ["userId", "permissionType"]),
});

// Export the traditional Convex schema for compatibility
export default confectSchema.convexSchemaDefinition;