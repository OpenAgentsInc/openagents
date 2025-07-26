import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

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
    // Get authenticated user (optional for backwards compatibility during migration)
    const identity = await ctx.auth.getUserIdentity();
    let userId: string | undefined;
    
    if (identity) {
      // Find user by GitHub ID from identity subject
      const user = await ctx.db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();
      userId = user?._id;
    }

    // Check if session already exists
    const existingSession = await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
      
    if (existingSession) {
      // Update existing session
      await ctx.db.patch(existingSession._id, {
        title: args.title,
        status: "active",
        lastActivity: Date.now(),
        metadata: args.metadata,
        userId: userId, // Link to user if authenticated
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
      userId: userId, // Link to user if authenticated
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
    // Get authenticated user (optional for backwards compatibility)
    const identity = await ctx.auth.getUserIdentity();
    let userId: string | undefined;
    
    if (identity) {
      const user = await ctx.db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();
      userId = user?._id;
    }

    let query;
    
    if (userId) {
      // Filter by user sessions when authenticated
      query = ctx.db.query("claudeSessions").withIndex("by_user_id")
        .filter(q => q.eq(q.field("userId"), userId));
      
      if (args.status) {
        query = query.filter(q => q.eq(q.field("status"), args.status));
      }
    } else {
      // Legacy mode: show all sessions (for backwards compatibility)
      query = ctx.db.query("claudeSessions").withIndex("by_last_activity");
      
      if (args.status) {
        query = ctx.db.query("claudeSessions").withIndex("by_status")
          .filter(q => q.eq(q.field("status"), args.status));
      }
    }
    
    return await query.order("desc").take(args.limit ?? 50);
  },
});

export const getSession = query({
  args: { sessionId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("claudeSessions")
      .withIndex("by_session_id")
      .filter(q => q.eq(q.field("sessionId"), args.sessionId))
      .first();
  },
});

// Message Management

export const getSessionMessages = query({
  args: { 
    sessionId: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
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

    // Get authenticated user (optional for backwards compatibility)
    const identity = await ctx.auth.getUserIdentity();
    let userId: string | undefined;
    
    if (identity) {
      const user = await ctx.db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();
      userId = user?._id;
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
      userId: userId, // Link to user if authenticated
    });
    console.log('✅ [CONVEX] Message added successfully:', args.messageId);
    
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
    let userId: string | undefined;
    
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

// APM (Actions Per Minute) Analysis for SDK/Convex conversations

export const getConvexAPMStats = query({
  args: {},
  handler: async (ctx) => {
    // Get all sessions
    const sessions = await ctx.db.query("claudeSessions").collect();
    
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
    
    // Get all messages
    const allMessages = await ctx.db.query("claudeMessages").collect();
    
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