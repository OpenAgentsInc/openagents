import { mutation, query } from "./_generated/server";
import { v } from "convex/values";
import type { Id } from "./_generated/dataModel";

// Migration script to assign existing sessions and messages to the user
// User GitHub ID: 14167547

export const migrateExistingDataToUser = mutation({
  args: {
    githubId: v.string(),
    dryRun: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const { githubId, dryRun = false } = args;
    
    console.log(`🔄 [MIGRATION] Starting data migration for GitHub ID: ${githubId} (dryRun: ${dryRun})`);
    
    // Find the user by GitHub ID
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
      .first();
    
    if (!user) {
      throw new Error(`User with GitHub ID ${githubId} not found. Please ensure the user has logged in at least once.`);
    }
    
    console.log(`✅ [MIGRATION] Found user: ${user.githubUsername} (${user.email})`);
    
    // Find all sessions without a userId (orphaned sessions)
    const orphanedSessions = await ctx.db
      .query("claudeSessions")
      .filter(q => q.eq(q.field("userId"), undefined))
      .collect();
    
    console.log(`📋 [MIGRATION] Found ${orphanedSessions.length} orphaned sessions`);
    
    // Find all messages without a userId (orphaned messages)
    const orphanedMessages = await ctx.db
      .query("claudeMessages")
      .filter(q => q.eq(q.field("userId"), undefined))
      .collect();
    
    console.log(`💬 [MIGRATION] Found ${orphanedMessages.length} orphaned messages`);
    
    if (dryRun) {
      console.log(`🔍 [MIGRATION] DRY RUN - Would migrate:`);
      console.log(`  - ${orphanedSessions.length} sessions`);
      console.log(`  - ${orphanedMessages.length} messages`);
      
      if (orphanedSessions.length > 0) {
        console.log(`📋 [MIGRATION] Sample sessions to migrate:`);
        orphanedSessions.slice(0, 3).forEach(session => {
          console.log(`  - ${session.sessionId} (${session.projectPath})`);
        });
      }
      
      return {
        success: true,
        dryRun: true,
        sessionsToMigrate: orphanedSessions.length,
        messagesToMigrate: orphanedMessages.length,
        userId: user._id,
        userName: user.githubUsername,
      };
    }
    
    let migratedSessions = 0;
    let migratedMessages = 0;
    
    // Migrate orphaned sessions
    for (const session of orphanedSessions) {
      try {
        await ctx.db.patch(session._id, {
          userId: user._id,
        });
        migratedSessions++;
        console.log(`✅ [MIGRATION] Migrated session: ${session.sessionId}`);
      } catch (error) {
        console.error(`❌ [MIGRATION] Failed to migrate session ${session.sessionId}:`, error);
      }
    }
    
    // Migrate orphaned messages
    for (const message of orphanedMessages) {
      try {
        await ctx.db.patch(message._id, {
          userId: user._id,
        });
        migratedMessages++;
        
        if (migratedMessages % 50 === 0) {
          console.log(`🔄 [MIGRATION] Migrated ${migratedMessages}/${orphanedMessages.length} messages`);
        }
      } catch (error) {
        console.error(`❌ [MIGRATION] Failed to migrate message ${message.messageId}:`, error);
      }
    }
    
    console.log(`🎉 [MIGRATION] Migration completed!`);
    console.log(`  - Migrated ${migratedSessions} sessions`);
    console.log(`  - Migrated ${migratedMessages} messages`);
    console.log(`  - All data now owned by: ${user.githubUsername} (${githubId})`);
    
    return {
      success: true,
      dryRun: false,
      migratedSessions,
      migratedMessages,
      userId: user._id,
      userName: user.githubUsername,
    };
  },
});

// Helper query to check migration status
export const getMigrationStatus = query({
  args: {
    githubId: v.string(),
  },
  handler: async (ctx, args) => {
    const { githubId } = args;
    
    // Find the user
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
      .first();
    
    if (!user) {
      return {
        userFound: false,
        message: `User with GitHub ID ${githubId} not found`,
      };
    }
    
    // Count sessions by ownership
    const userSessions = await ctx.db
      .query("claudeSessions")
      .withIndex("by_user_id", (q) => q.eq("userId", user._id))
      .collect();
    
    const orphanedSessions = await ctx.db
      .query("claudeSessions")
      .filter(q => q.eq(q.field("userId"), undefined))
      .collect();
    
    // Count messages by ownership
    const userMessages = await ctx.db
      .query("claudeMessages")
      .withIndex("by_user_id", (q) => q.eq("userId", user._id))
      .collect();
    
    const orphanedMessages = await ctx.db
      .query("claudeMessages")
      .filter(q => q.eq(q.field("userId"), undefined))
      .collect();
    
    return {
      userFound: true,
      user: {
        id: user._id,
        githubUsername: user.githubUsername,
        email: user.email,
      },
      sessions: {
        owned: userSessions.length,
        orphaned: orphanedSessions.length,
        total: userSessions.length + orphanedSessions.length,
      },
      messages: {
        owned: userMessages.length,
        orphaned: orphanedMessages.length,
        total: userMessages.length + orphanedMessages.length,
      },
      migrationNeeded: orphanedSessions.length > 0 || orphanedMessages.length > 0,
    };
  },
});

// Helper query to list recent sessions (for verification)
export const getRecentSessions = query({
  args: {
    githubId: v.string(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const { githubId, limit = 10 } = args;
    
    // Find the user
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
      .first();
    
    if (!user) {
      throw new Error(`User with GitHub ID ${githubId} not found`);
    }
    
    // Get user's sessions
    const sessions = await ctx.db
      .query("claudeSessions")
      .withIndex("by_user_id", (q) => q.eq("userId", user._id))
      .order("desc")
      .take(limit);
    
    return sessions.map(session => ({
      sessionId: session.sessionId,
      title: session.title,
      projectPath: session.projectPath,
      status: session.status,
      createdBy: session.createdBy,
      lastActivity: new Date(session.lastActivity).toLocaleString(),
    }));
  },
});