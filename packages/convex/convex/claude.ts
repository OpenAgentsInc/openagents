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
    let query = ctx.db.query("claudeSessions").withIndex("by_last_activity");
    
    if (args.status) {
      query = ctx.db.query("claudeSessions").withIndex("by_status")
        .filter(q => q.eq(q.field("status"), args.status));
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
    
    // Add message
    const messageDoc = await ctx.db.insert("claudeMessages", args);
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
    const sessionId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;
    
    // Create session
    await ctx.db.insert("claudeSessions", {
      sessionId,
      projectPath: args.projectPath,
      title: args.title ?? `Mobile Session - ${new Date().toLocaleString()}`,
      status: "active",
      createdBy: "mobile",
      lastActivity: Date.now(),
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