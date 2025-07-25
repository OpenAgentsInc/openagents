import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// Query to get all messages
export const getMessages = query({
  args: {},
  handler: async (ctx) => {
    return await ctx.db.query("messages").order("desc").take(100);
  },
});

// Mutation to add a new message
export const addMessage = mutation({
  args: { 
    body: v.string(), 
    user: v.string() 
  },
  handler: async (ctx, args) => {
    const message = {
      body: args.body,
      user: args.user,
      timestamp: Date.now(),
    };
    return await ctx.db.insert("messages", message);
  },
});

// Query to get message count
export const getMessageCount = query({
  args: {},
  handler: async (ctx) => {
    const messages = await ctx.db.query("messages").collect();
    return messages.length;
  },
});