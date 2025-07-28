import { query, mutation } from "./_generated/server";
import { v } from "convex/values";
import type { Id } from "./_generated/dataModel";

// Session Management

export const createClaudeSession = mutation({
  args: {
    sessionId: v.string(),
    projectPath: v.string(), 
    createdBy: v.union(v.literal("desktop"), v.literal("mobile")),
    title: v.optional(v.string()),
    metadata: v.optional(v.object({
      workingDirectory: v.optional(v.string()),
      model: v.optional(v.string()),
      systemPrompt: v.optional(v.string()),
      originalMobileSessionId: v.optional(v.string()),
    })),
  },
  handler: async (ctx, args) => {
    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to create session");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // Check if session already exists
    const existingSession = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (existingSession) {
      // Verify existing session belongs to the authenticated user
      if (existingSession.userId !== user._id) {
        throw new Error("Access denied to update session");
      }
      
      // Update existing session
      await ctx.db.patch(existingSession._id, {
        title: args.title,
        status: "active",
        lastActivity: Date.now(),
        metadata: args.metadata,
        userId: user._id, // Link to authenticated user
      });
      return existingSession._id;
    }
    
    // Create new session
    const sessionDoc = await ctx.db.insert("claudeSessions", {
      sessionId: args.sessionId,
      projectPath: args.projectPath,
      title: args.title ?? `${args.createdBy} Session - ${new Date().toLocaleString()}`,
      status: "active",
      createdBy: args.createdBy,
      lastActivity: Date.now(),
      userId: user._id, // Link to authenticated user
      metadata: args.metadata,
    });
    
    // Initialize sync status
    await ctx.db.insert("syncStatus", {
      sessionId: args.sessionId,
    });
    
    return sessionDoc;
  },
});

export const updateSessionStatus = mutation({
  args: {
    sessionId: v.string(),
    status: v.union(v.literal("active"), v.literal("inactive"), v.literal("error"), v.literal("processed")),
  },
  handler: async (ctx, args) => {
    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (session) {
      await ctx.db.patch(session._id, {
        status: args.status,
        lastActivity: Date.now(),
      });
    }
  },
});

export const getSessions = query({
  args: { 
    limit: v.optional(v.number()),
    status: v.optional(v.union(v.literal("active"), v.literal("inactive"), v.literal("error"))),
  },
  handler: async (ctx, args) => {
    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to view sessions");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // Filter by user sessions only
    let query = ctx.db.query("claudeSessions").withIndex("by_user_id")
      .filter(q => q.eq(q.field("userId"), user._id));
    
    if (args.status) {
      query = query.filter(q => q.eq(q.field("status"), args.status));
    }
    
    return await query.order("desc").take(args.limit ?? 50);
  },
});

export const getSession = query({
  args: { sessionId: v.string() },
  handler: async (ctx, args) => {
    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to view session");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
    
    // Ensure session belongs to the authenticated user
    if (session && session.userId !== user._id) {
      throw new Error("Session not found or access denied");
    }
    
    return session;
  },
});

// Message Management

export const getSessionMessages = query({
  args: { 
    sessionId: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to view messages");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // First verify the session belongs to the authenticated user
    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
    
    if (!session) {
      throw new Error("Session not found");
    }
    
    if (session.userId !== user._id) {
      throw new Error("Access denied to session messages");
    }

    const query = ctx.db
      .query("claudeMessages")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .order("asc");
      
    if (args.limit) {
      return await query.take(args.limit);
    }
    
    return await query.collect();
  },
});

export const addClaudeMessage = mutation({
  args: {
    sessionId: v.string(),
    messageId: v.string(),
    messageType: v.union(v.literal("user"), v.literal("assistant"), v.literal("tool_use"), v.literal("tool_result"), v.literal("thinking")),
    content: v.string(),
    timestamp: v.string(),
    toolInfo: v.optional(v.object({
      toolName: v.string(),
      toolUseId: v.string(), 
      input: v.any(),
      output: v.optional(v.string()),
    })),
    metadata: v.optional(v.any()),
  },
  handler: async (ctx, args) => {
    console.log('💾 [CONVEX] addClaudeMessage called:', {
      sessionId: args.sessionId,
      messageId: args.messageId,
      messageType: args.messageType,
      contentPreview: args.content.substring(0, 50),
      timestamp: args.timestamp
    });

    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to add message");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // Verify the session belongs to the authenticated user
    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
    
    if (!session) {
      throw new Error("Session not found");
    }
    
    if (session.userId !== user._id) {
      throw new Error("Access denied to add message to this session");
    }
    
    // Check if message already exists to avoid duplicates
    const existingMessage = await ctx.db
      .query("claudeMessages")
      .withIndex("by_session_id")
      .filter(q => q.and(
        q.eq(q.field("sessionId"), args.sessionId),
        q.eq(q.field("messageId"), args.messageId)
      ))
      .first();
      
    if (existingMessage) {
      console.log('⚠️ [CONVEX] Message already exists, skipping duplicate:', args.messageId);
      return existingMessage._id;
    }
    
    // Add message with user ID
    const messageDoc = await ctx.db.insert("claudeMessages", {
      ...args,
      userId: user._id, // Link to authenticated user
    });
    console.log('✅ [CONVEX] Message added successfully:', args.messageId);
    
    // Update session last activity (reuse session from earlier validation)
    await ctx.db.patch(session._id, {
      lastActivity: Date.now(),
    });
    
    // Update sync status
    const syncStatus = await ctx.db
      .query("syncStatus")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (syncStatus) {
      await ctx.db.patch(syncStatus._id, {
        lastSyncedMessageId: args.messageId,
      });
    }
    
    return messageDoc;
  },
});

export const batchAddMessages = mutation({
  args: {
    sessionId: v.string(),
    messages: v.array(v.object({
      messageId: v.string(),
      messageType: v.union(v.literal("user"), v.literal("assistant"), v.literal("tool_use"), v.literal("tool_result"), v.literal("thinking")),
      content: v.string(),
      timestamp: v.string(),
      toolInfo: v.optional(v.object({
        toolName: v.string(),
        toolUseId: v.string(), 
        input: v.any(),
        output: v.optional(v.string()),
      })),
      metadata: v.optional(v.any()),
    })),
  },
  handler: async (ctx, args) => {
    const insertedIds = [];
    
    for (const message of args.messages) {
      // Check if message already exists
      const existingMessage = await ctx.db
        .query("claudeMessages")
        .withIndex("by_session_id")
        .filter(q => q.and(
          q.eq(q.field("sessionId"), args.sessionId),
          q.eq(q.field("messageId"), message.messageId)
        ))
        .first();
        
      if (!existingMessage) {
        const messageDoc = await ctx.db.insert("claudeMessages", {
          sessionId: args.sessionId,
          ...message,
        });
        insertedIds.push(messageDoc);
      }
    }
    
    // Update session last activity
    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (session) {
      await ctx.db.patch(session._id, {
        lastActivity: Date.now(),
      });
    }
    
    return insertedIds;
  },
});

// Mobile-initiated session requests

export const requestDesktopSession = mutation({
  args: {
    projectPath: v.string(),
    initialMessage: v.optional(v.string()),
    title: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    // Get authenticated user (optional for backwards compatibility)
    const identity = await ctx.auth.getUserIdentity();
    let userId: Id<"users"> | undefined;
    
    if (identity) {
      const user = await ctx.db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();
      userId = user?._id;
    }

    const sessionId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;
    
    // Create session
    await ctx.db.insert("claudeSessions", {
      sessionId,
      projectPath: args.projectPath,
      title: args.title ?? `Mobile Session - ${new Date().toLocaleString()}`,
      status: "active",
      createdBy: "mobile",
      lastActivity: Date.now(),
      userId: userId, // Link to user if authenticated
    });
    
    // Initialize sync status
    await ctx.db.insert("syncStatus", {
      sessionId,
    });
    
    // Add initial message if provided
    if (args.initialMessage) {
      console.log('📱 [CONVEX] Mobile creating initial message for session:', sessionId);
      const messageId = `user-${Date.now()}`;
      await ctx.db.insert("claudeMessages", {
        sessionId,
        messageId,
        messageType: "user",
        content: args.initialMessage,
        timestamp: new Date().toISOString(),
        userId: userId, // Link to user if authenticated
      });
      console.log('✅ [CONVEX] Mobile initial message created with ID:', messageId);
    }
    
    return sessionId;
  },
});

// Sync status management

export const updateSyncStatus = mutation({
  args: {
    sessionId: v.string(),
    desktopLastSeen: v.optional(v.number()),
    mobileLastSeen: v.optional(v.number()),
    syncErrors: v.optional(v.array(v.string())),
  },
  handler: async (ctx, args) => {
    const syncStatus = await ctx.db
      .query("syncStatus")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (syncStatus) {
      const updates: any = {};
      if (args.desktopLastSeen !== undefined) updates.desktopLastSeen = args.desktopLastSeen;
      if (args.mobileLastSeen !== undefined) updates.mobileLastSeen = args.mobileLastSeen;
      if (args.syncErrors !== undefined) updates.syncErrors = args.syncErrors;
      
      await ctx.db.patch(syncStatus._id, updates);
    }
  },
});

export const getSyncStatus = query({
  args: { sessionId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("syncStatus")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
  },
});

// Helper functions for hook integration

export const syncSessionFromHook = mutation({
  args: {
    hookData: v.object({
      sessionId: v.string(),
      projectPath: v.string(),
      messages: v.optional(v.array(v.any())),
      event: v.string(),
      timestamp: v.string(),
    }),
  },
  handler: async (ctx, args) => {
    const { hookData } = args;
    
    // Ensure session exists
    let session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), hookData.sessionId))
      .first();
      
    if (!session) {
      await ctx.db.insert("claudeSessions", {
        sessionId: hookData.sessionId,
        projectPath: hookData.projectPath,
        title: `Desktop Session - ${hookData.projectPath}`,
        status: "active",
        createdBy: "desktop",
        lastActivity: Date.now(),
      });
      
      await ctx.db.insert("syncStatus", {
        sessionId: hookData.sessionId,
      });
    }
    
    // Sync messages if provided
    if (hookData.messages && hookData.messages.length > 0) {
      for (const message of hookData.messages) {
        await ctx.db.insert("claudeMessages", {
          sessionId: hookData.sessionId,
          messageId: message.id || `${message.message_type}-${Date.now()}-${Math.random()}`,
          messageType: message.message_type,
          content: message.content,
          timestamp: message.timestamp,
          toolInfo: message.tool_info ? {
            toolName: message.tool_info.tool_name,
            toolUseId: message.tool_info.tool_use_id,
            input: message.tool_info.input,
            output: message.tool_info.output,
          } : undefined,
          metadata: { hookEvent: hookData.event },
        });
      }
    }
    
    return { success: true };
  },
});

// Mark mobile session as processed by desktop
export const markMobileSessionProcessed = mutation({
  args: {
    mobileSessionId: v.string(),
  },
  handler: async (ctx, args) => {
    const session = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.mobileSessionId))
      .first();
      
    if (session) {
      await ctx.db.patch(session._id, {
        status: "processed",
        lastActivity: Date.now(),
      });
    }
  },
});

// Get pending mobile sessions that need desktop attention
export const getPendingMobileSessions = query({
  args: {},
  handler: async (ctx) => {
    const results = await ctx.db
      .query("claudeSessions")
      .withIndex("by_status")
      .filter(q => q.and(
        q.eq(q.field("status"), "active"),
        q.eq(q.field("createdBy"), "mobile")
      ))
      .order("desc")
      .take(10);
    
    console.log('🔍 [CONVEX] getPendingMobileSessions query returned:', results.length, 'sessions');
    if (results.length > 0) {
      console.log('📋 [CONVEX] Mobile sessions:', results.map(s => ({
        sessionId: s.sessionId,
        status: s.status,
        createdBy: s.createdBy,
        metadata: s.metadata
      })));
    }
    
    return results;
  },
});

// APM (Actions Per Minute) Analysis for SDK/Convex conversations (User-scoped)

export const getConvexAPMStats = query({
  args: {},
  handler: async (ctx, args) => {
    // Get authenticated user - authentication is required
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Authentication required to view APM stats");
    }
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // Get user-specific sessions only
    const sessions = await ctx.db
      .query("claudeSessions")
      .withIndex("by_user_id", (q) => q.eq("userId", user._id))
      .collect();
    
    if (sessions.length === 0) {
      return {
        apm1h: 0,
        apm6h: 0,
        apm1d: 0,
        apm1w: 0,
        apm1m: 0,
        apmLifetime: 0,
        totalSessions: 0,
        totalMessages: 0,
        totalToolUses: 0,
        totalDuration: 0,
        toolUsage: [],
        recentSessions: [],
        productivityByTime: {
          morning: 0,
          afternoon: 0,
          evening: 0,
          night: 0,
        },
      };
    }
    
    // Get messages for user's sessions only
    const userSessionIds = sessions.map(s => s.sessionId);
    const allMessages = await ctx.db.query("claudeMessages")
      .filter(q => userSessionIds.some(sessionId => q.eq(q.field("sessionId"), sessionId)))
      .collect();
    
    const now = Date.now();
    const timeWindows = {
      hour1: now - (1 * 60 * 60 * 1000),     // 1 hour ago
      hours6: now - (6 * 60 * 60 * 1000),    // 6 hours ago
      day1: now - (24 * 60 * 60 * 1000),     // 1 day ago
      week1: now - (7 * 24 * 60 * 60 * 1000), // 1 week ago
      month1: now - (30 * 24 * 60 * 60 * 1000), // 30 days ago
    };
    
    // Initialize counters
    let windowCounts = {
      hour1: { messages: 0, tools: 0 },
      hours6: { messages: 0, tools: 0 },
      day1: { messages: 0, tools: 0 },
      week1: { messages: 0, tools: 0 },
      month1: { messages: 0, tools: 0 },
    };
    
    let totalMessages = 0;
    let totalToolUses = 0;
    let toolCounts: Record<string, number> = {};
    let productivityByHour = [[], [], [], []] as number[][]; // morning, afternoon, evening, night
    let earliestTimestamp: number | null = null;
    let latestTimestamp: number | null = null;
    
    // Process sessions for APM calculation
    const sessionStats = [];
    
    for (const session of sessions) {
      const sessionMessages = allMessages.filter(m => m.sessionId === session.sessionId);
      
      if (sessionMessages.length === 0) continue;
      
      // Sort messages by timestamp
      sessionMessages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
      
      const sessionStart = new Date(sessionMessages[0].timestamp).getTime();
      const sessionEnd = new Date(sessionMessages[sessionMessages.length - 1].timestamp).getTime();
      const sessionDuration = (sessionEnd - sessionStart) / (1000 * 60); // minutes
      
      if (sessionDuration <= 0) continue;
      
      // Track lifetime bounds
      if (earliestTimestamp === null || sessionStart < earliestTimestamp) {
        earliestTimestamp = sessionStart;
      }
      if (latestTimestamp === null || sessionEnd > latestTimestamp) {
        latestTimestamp = sessionEnd;
      }
      
      let sessionMessageCount = 0;
      let sessionToolCount = 0;
      
      for (const message of sessionMessages) {
        const messageTime = new Date(message.timestamp).getTime();
        
        // Count message types
        if (message.messageType === "user" || message.messageType === "assistant") {
          sessionMessageCount++;
          totalMessages++;
          
          // Count for all applicable time windows in one pass
          for (const [windowName, cutoff] of Object.entries(timeWindows)) {
            if (messageTime >= cutoff) {
              windowCounts[windowName as keyof typeof windowCounts].messages++;
            }
          }
        }
        
        // Count tool uses
        if (message.toolInfo) {
          sessionToolCount++;
          totalToolUses++;
          
          const toolName = message.toolInfo.toolName;
          toolCounts[toolName] = (toolCounts[toolName] || 0) + 1;
          
          // Count for all applicable time windows in one pass
          for (const [windowName, cutoff] of Object.entries(timeWindows)) {
            if (messageTime >= cutoff) {
              windowCounts[windowName as keyof typeof windowCounts].tools++;
            }
          }
        }
      }
      
      const sessionAPM = (sessionMessageCount + sessionToolCount) / sessionDuration;
      
      // Track productivity by time of day
      const startHour = new Date(sessionStart).getHours();
      const timeSlot = startHour >= 6 && startHour < 12 ? 0 : // morning
                     startHour >= 12 && startHour < 18 ? 1 : // afternoon  
                     startHour >= 18 && startHour < 24 ? 2 : // evening
                     3; // night
      
      productivityByHour[timeSlot].push(sessionAPM);
      
      sessionStats.push({
        id: session.sessionId,
        project: session.projectPath,
        apm: sessionAPM,
        duration: sessionDuration,
        messageCount: sessionMessageCount,
        toolCount: sessionToolCount,
        timestamp: new Date(sessionStart).toISOString(),
      });
    }
    
    // Calculate APM for different time windows
    const calculateAPM = (windowData: {messages: number, tools: number}, minutes: number) => {
      return (windowData.messages + windowData.tools) / minutes;
    };
    
    const apm1h = calculateAPM(windowCounts.hour1, 60);
    const apm6h = calculateAPM(windowCounts.hours6, 360);
    const apm1d = calculateAPM(windowCounts.day1, 1440);
    const apm1w = calculateAPM(windowCounts.week1, 10080);
    const apm1m = calculateAPM(windowCounts.month1, 43200);
    
    // Lifetime APM calculation
    const apmLifetime = (earliestTimestamp && latestTimestamp) ? 
      (totalMessages + totalToolUses) / ((latestTimestamp - earliestTimestamp) / (1000 * 60)) : 0;
    
    // Process tool usage statistics
    const toolUsage = Object.entries(toolCounts)
      .map(([name, count]) => ({
        name,
        count,
        percentage: totalToolUses > 0 ? (count / totalToolUses) * 100 : 0,
        category: getToolCategory(name),
      }))
      .sort((a, b) => b.count - a.count);
    
    // Calculate productivity by time of day averages
    const productivityByTime = {
      morning: productivityByHour[0].length > 0 ? 
        productivityByHour[0].reduce((a, b) => a + b, 0) / productivityByHour[0].length : 0,
      afternoon: productivityByHour[1].length > 0 ? 
        productivityByHour[1].reduce((a, b) => a + b, 0) / productivityByHour[1].length : 0,
      evening: productivityByHour[2].length > 0 ? 
        productivityByHour[2].reduce((a, b) => a + b, 0) / productivityByHour[2].length : 0,
      night: productivityByHour[3].length > 0 ? 
        productivityByHour[3].reduce((a, b) => a + b, 0) / productivityByHour[3].length : 0,
    };
    
    // Sort sessions by APM and take recent ones
    sessionStats.sort((a, b) => b.apm - a.apm);
    const recentSessions = sessionStats.slice(0, 20);
    
    // Calculate total duration from sessions
    const totalDuration = sessionStats.reduce((sum, session) => sum + session.duration, 0);
    
    return {
      apm1h,
      apm6h,
      apm1d,
      apm1w,
      apm1m,
      apmLifetime,
      totalSessions: sessions.length,
      totalMessages,
      totalToolUses,
      totalDuration,
      toolUsage,
      recentSessions,
      productivityByTime,
    };
  },
});

// Helper function for tool categorization (matches Rust implementation)
function getToolCategory(toolName: string): string {
  switch (toolName) {
    case "Edit":
    case "MultiEdit":
    case "Write":
      return "Code Generation";
    case "Read":
    case "LS":
    case "Glob":
      return "File Operations";
    case "Bash":
      return "System Operations";
    case "Grep":
    case "WebSearch":
    case "WebFetch":
      return "Search";
    case "TodoWrite":
    case "TodoRead":
      return "Planning";
    default:
      return "Other";
  }
}

// Multi-Client APM Functions

// Track device session activity
export const trackDeviceSession = mutation({
  args: {
    deviceId: v.string(),
    deviceType: v.union(v.literal("desktop"), v.literal("mobile"), v.literal("github")),
    sessionStart: v.number(),
    sessionEnd: v.optional(v.number()),
    actions: v.object({
      messages: v.number(),
      toolUses: v.number(),
      githubEvents: v.optional(v.number()),
    }),
    metadata: v.optional(v.object({
      platform: v.optional(v.string()),
      version: v.optional(v.string()),
      location: v.optional(v.string()),
    })),
  },
  handler: async (ctx, args) => {
    // Get authenticated user
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("User must be authenticated to track device sessions");
    }

    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    // Find existing device session
    const existingSession = await ctx.db
      .query("userDeviceSessions")
      .withIndex("by_device_id", (q) => q.eq("deviceId", args.deviceId))
      .first();

    const sessionPeriod = {
      start: args.sessionStart,
      end: args.sessionEnd,
    };

    if (existingSession) {
      // Update existing session
      await ctx.db.patch(existingSession._id, {
        sessionPeriods: [...existingSession.sessionPeriods, sessionPeriod],
        actionsCount: {
          messages: existingSession.actionsCount.messages + args.actions.messages,
          toolUses: existingSession.actionsCount.toolUses + args.actions.toolUses,
          githubEvents: (existingSession.actionsCount.githubEvents || 0) + (args.actions.githubEvents || 0),
        },
        lastActivity: args.sessionEnd || args.sessionStart,
        metadata: args.metadata,
      });
      return existingSession._id;
    } else {
      // Create new device session
      return await ctx.db.insert("userDeviceSessions", {
        userId: user._id,
        deviceId: args.deviceId,
        deviceType: args.deviceType,
        sessionPeriods: [sessionPeriod],
        actionsCount: args.actions,
        lastActivity: args.sessionEnd || args.sessionStart,
        metadata: args.metadata,
      });
    }
  },
});

// Calculate aggregated APM with time overlap handling
export const calculateUserAPM = mutation({
  args: {
    timeWindow: v.optional(v.union(v.literal("1h"), v.literal("6h"), v.literal("1d"), v.literal("1w"), v.literal("1m"), v.literal("lifetime"))),
  },
  handler: async (ctx, args) => {
    // Get authenticated user
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("User must be authenticated to calculate APM");
    }

    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      throw new Error("User not found");
    }

    const now = Date.now();
    const timeWindows = args.timeWindow ? [args.timeWindow] : ["1h", "6h", "1d", "1w", "1m", "lifetime"] as const;

    for (const window of timeWindows) {
      const cutoff = getTimeCutoff(now, window);
      
      // Get all device sessions for this user
      const deviceSessions = await ctx.db
        .query("userDeviceSessions")
        .withIndex("by_user_id", (q) => q.eq("userId", user._id))
        .collect();

      // Calculate aggregated metrics
      const { totalActions, activeMinutes, deviceBreakdown, metadata } = 
        aggregateDeviceMetrics(deviceSessions, cutoff, now);

      const aggregatedAPM = activeMinutes > 0 ? totalActions / activeMinutes : 0;

      // Store/update cached stats
      const existingStats = await ctx.db
        .query("userAPMStats")
        .withIndex("by_user_window", (q) => q.eq("userId", user._id).eq("timeWindow", window))
        .first();

      if (existingStats) {
        await ctx.db.patch(existingStats._id, {
          aggregatedAPM,
          deviceBreakdown,
          totalActions,
          activeMinutes,
          calculatedAt: now,
          metadata,
        });
      } else {
        await ctx.db.insert("userAPMStats", {
          userId: user._id,
          timeWindow: window,
          aggregatedAPM,
          deviceBreakdown,
          totalActions,
          activeMinutes,
          calculatedAt: now,
          metadata,
        });
      }
    }

    return { success: true };
  },
});

// Get user's aggregated APM stats
export const getUserAPMStats = query({
  args: {
    includeDeviceBreakdown: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    // Get authenticated user
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      return null; // Return null for unauthenticated users
    }

    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
      .first();
    
    if (!user) {
      return null;
    }

    // Get cached APM stats for all time windows
    const allStats = await ctx.db
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
    };

    if (args.includeDeviceBreakdown) {
      return {
        ...result,
        deviceBreakdown: {
          desktop: statsByWindow["lifetime"]?.deviceBreakdown?.desktop || 0,
          mobile: statsByWindow["lifetime"]?.deviceBreakdown?.mobile || 0,
          github: statsByWindow["lifetime"]?.deviceBreakdown?.github || 0,
        },
        metadata: statsByWindow["lifetime"]?.metadata,
      };
    }

    return result;
  },
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

// Helper function to calculate APM for a specific device
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

// Helper function to merge overlapping time intervals
export function mergeOverlappingIntervals(intervals: Array<{ start: number; end: number }>): Array<{ start: number; end: number }> {
  if (intervals.length === 0) return [];
  
  // Sort intervals by start time
  const sorted = intervals.slice().sort((a, b) => a.start - b.start);
  const merged = [sorted[0]];
  
  for (let i = 1; i < sorted.length; i++) {
    const current = sorted[i];
    const lastMerged = merged[merged.length - 1];
    
    if (current.start <= lastMerged.end) {
      // Overlapping intervals - merge them
      lastMerged.end = Math.max(lastMerged.end, current.end);
    } else {
      // Non-overlapping interval - add it
      merged.push(current);
    }
  }
  
  return merged;
}

// Helper function to calculate total overlapping minutes
function calculateOverlappingMinutes(intervals: Array<{ start: number; end: number }>): number {
  const totalMinutes = intervals.reduce((total, interval) => {
    return total + (interval.end - interval.start) / (1000 * 60);
  }, 0);
  
  const mergedMinutes = mergeOverlappingIntervals(intervals).reduce((total, interval) => {
    return total + (interval.end - interval.start) / (1000 * 60);
  }, 0);
  
  return Math.max(0, totalMinutes - mergedMinutes);
}

// Helper function to calculate peak concurrency
function calculatePeakConcurrency(intervals: Array<{ start: number; end: number }>): number {
  if (intervals.length === 0) return 0;
  
  // Create events for interval starts and ends
  const events: Array<{ time: number; type: 'start' | 'end' }> = [];
  
  for (const interval of intervals) {
    events.push({ time: interval.start, type: 'start' });
    events.push({ time: interval.end, type: 'end' });
  }
  
  // Sort events by time (starts before ends for same time)
  events.sort((a, b) => {
    if (a.time === b.time) {
      return a.type === 'start' ? -1 : 1;
    }
    return a.time - b.time;
  });
  
  let currentCount = 0;
  let maxCount = 0;
  
  for (const event of events) {
    if (event.type === 'start') {
      currentCount++;
      maxCount = Math.max(maxCount, currentCount);
    } else {
      currentCount--;
    }
  }
  
  return maxCount;
}