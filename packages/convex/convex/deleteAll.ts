/**
 * Convex mutations to delete all data
 */

import { mutation } from "./_generated/server"

/**
 * Delete all messages
 */
export const deleteAllMessages = mutation({
  args: {},
  handler: async (ctx) => {
    const messages = await ctx.db.query("messages").collect()
    console.log(`Deleting ${messages.length} messages...`)
    
    for (const message of messages) {
      await ctx.db.delete(message._id)
    }
    
    return { deleted: messages.length }
  }
})

/**
 * Delete all sessions
 */
export const deleteAllSessions = mutation({
  args: {},
  handler: async (ctx) => {
    const sessions = await ctx.db.query("sessions").collect()
    console.log(`Deleting ${sessions.length} sessions...`)
    
    for (const session of sessions) {
      await ctx.db.delete(session._id)
    }
    
    return { deleted: sessions.length }
  }
})

/**
 * Delete everything (messages first, then sessions) with pagination
 */
export const deleteEverything = mutation({
  args: {},
  handler: async (ctx) => {
    let messagesDeleted = 0
    let sessionsDeleted = 0
    
    // Delete messages in batches of 100
    while (true) {
      const messages = await ctx.db.query("messages").take(100)
      if (messages.length === 0) break
      
      for (const message of messages) {
        await ctx.db.delete(message._id)
        messagesDeleted++
      }
    }
    
    // Delete sessions in batches of 100
    while (true) {
      const sessions = await ctx.db.query("sessions").take(100)
      if (sessions.length === 0) break
      
      for (const session of sessions) {
        await ctx.db.delete(session._id)
        sessionsDeleted++
      }
    }
    
    return {
      messagesDeleted,
      sessionsDeleted
    }
  }
})