/**
 * Convex functions for message embeddings and vector search
 * @since 1.0.0
 */

import { v } from "convex/values"
import { query, mutation, action } from "./_generated/server"
import { api } from "./_generated/api"

/**
 * Store an embedding for a message
 */
export const storeEmbedding = mutation({
  args: {
    message_id: v.id("messages"),
    embedding: v.array(v.float64()),
    model: v.string(),
    dimensions: v.number()
  },
  handler: async (ctx, args) => {
    // Check if embedding already exists for this message
    const existing = await ctx.db
      .query("message_embeddings")
      .withIndex("by_message_id", q => q.eq("message_id", args.message_id))
      .first()
    
    if (existing) {
      // Update existing embedding
      return await ctx.db.patch(existing._id, {
        embedding: args.embedding,
        model: args.model,
        dimensions: args.dimensions,
        created_at: Date.now()
      })
    }
    
    // Create new embedding
    const embeddingId = await ctx.db.insert("message_embeddings", {
      message_id: args.message_id,
      embedding: args.embedding,
      model: args.model,
      dimensions: args.dimensions,
      created_at: Date.now()
    })
    
    // Update message with embedding reference
    await ctx.db.patch(args.message_id, {
      embedding_id: embeddingId
    })
    
    return embeddingId
  }
})

/**
 * Get embedding for a message
 */
export const getEmbedding = query({
  args: { message_id: v.id("messages") },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("message_embeddings")
      .withIndex("by_message_id", q => q.eq("message_id", args.message_id))
      .first()
  }
})

/**
 * Get messages that have embeddings
 */
export const getMessagesWithEmbeddings = query({
  args: {
    sessionId: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
      .filter(q => q.neq(q.field("embedding_id"), undefined))
      .take(args.limit ?? 100)
    
    // Fetch embeddings for each message
    const messagesWithEmbeddings = await Promise.all(
      messages.map(async (message) => {
        const embedding = message.embedding_id 
          ? await ctx.db.get(message.embedding_id)
          : null
        return { ...message, embedding }
      })
    )
    
    return messagesWithEmbeddings
  }
})

/**
 * Batch store embeddings for multiple messages
 */
export const batchStoreEmbeddings = mutation({
  args: {
    embeddings: v.array(v.object({
      message_id: v.id("messages"),
      embedding: v.array(v.float64()),
      model: v.string(),
      dimensions: v.number()
    }))
  },
  handler: async (ctx, args) => {
    const results = []
    
    for (const embeddingData of args.embeddings) {
      // Check if embedding already exists
      const existing = await ctx.db
        .query("message_embeddings")
        .withIndex("by_message_id", q => q.eq("message_id", embeddingData.message_id))
        .first()
      
      let embeddingId
      
      if (existing) {
        // Update existing
        await ctx.db.patch(existing._id, {
          embedding: embeddingData.embedding,
          model: embeddingData.model,
          dimensions: embeddingData.dimensions,
          created_at: Date.now()
        })
        embeddingId = existing._id
      } else {
        // Create new
        embeddingId = await ctx.db.insert("message_embeddings", {
          message_id: embeddingData.message_id,
          embedding: embeddingData.embedding,
          model: embeddingData.model,
          dimensions: embeddingData.dimensions,
          created_at: Date.now()
        })
        
        // Update message reference
        await ctx.db.patch(embeddingData.message_id, {
          embedding_id: embeddingId
        })
      }
      
      results.push({ message_id: embeddingData.message_id, embedding_id: embeddingId })
    }
    
    return results
  }
})

/**
 * Vector similarity search for messages
 * Note: This is a Convex action because vector search is only available in actions
 */
export const similarMessages = action({
  args: {
    query_embedding: v.array(v.float64()),
    session_id: v.optional(v.string()),
    limit: v.optional(v.number()),
    model_filter: v.optional(v.string())
  },
  handler: async (ctx, args) => {
    // Perform vector search
    const searchResults = await ctx.vectorSearch("message_embeddings", "by_embedding", {
      vector: args.query_embedding,
      limit: args.limit ?? 10,
      filter: args.model_filter 
        ? (q) => q.eq("model", args.model_filter)
        : undefined
    })
    
    // Fetch the full message data for each result
    const messagesWithScores = await Promise.all(
      searchResults.map(async ({ _id, _score }) => {
        const embedding = await ctx.runQuery(api.embeddings.getEmbeddingById, { id: _id })
        if (!embedding) return null
        
        const message = await ctx.runQuery(api.embeddings.getMessageById, { id: embedding.message_id })
        if (!message) return null
        
        // Filter by session if specified
        if (args.session_id && message.session_id !== args.session_id) {
          return null
        }
        
        return {
          message,
          embedding,
          similarity_score: _score
        }
      })
    )
    
    // Filter out nulls and return
    return messagesWithScores.filter(result => result !== null)
  }
})

/**
 * Search for similar messages using text query
 * This action generates an embedding from the text and searches for similar messages
 */
export const searchSimilarByText = action({
  args: {
    query_text: v.string(),
    session_id: v.optional(v.string()),
    limit: v.optional(v.number()),
    model_filter: v.optional(v.string())
  },
  handler: async (ctx, args) => {
    // This would need to call an external embedding API
    // For now, return a placeholder error
    throw new Error("Text-based similarity search requires external embedding API integration")
  }
})

/**
 * Get embedding by ID (internal helper)
 */
export const getEmbeddingById = query({
  args: { id: v.id("message_embeddings") },
  handler: async (ctx, args) => {
    return await ctx.db.get(args.id)
  }
})

/**
 * Get message by ID (internal helper)
 */
export const getMessageById = query({
  args: { id: v.id("messages") },
  handler: async (ctx, args) => {
    return await ctx.db.get(args.id)
  }
})

/**
 * Delete embeddings for a session
 */
export const deleteSessionEmbeddings = mutation({
  args: { session_id: v.string() },
  handler: async (ctx, args) => {
    // Get all messages for the session
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_session_id", q => q.eq("session_id", args.session_id))
      .collect()
    
    let deletedCount = 0
    
    // Delete embeddings for each message
    for (const message of messages) {
      if (message.embedding_id) {
        await ctx.db.delete(message.embedding_id)
        await ctx.db.patch(message._id, { embedding_id: undefined })
        deletedCount++
      }
    }
    
    return { deleted: deletedCount }
  }
})

/**
 * Get statistics about embeddings
 */
export const getEmbeddingStats = query({
  args: { session_id: v.optional(v.string()) },
  handler: async (ctx, args) => {
    let query = ctx.db.query("message_embeddings")
    
    if (args.session_id) {
      // Get messages for session first
      const messageIds = await ctx.db
        .query("messages")
        .withIndex("by_session_id", q => q.eq("session_id", args.session_id))
        .collect()
        .then(messages => messages.map(m => m._id))
      
      // Filter embeddings by message IDs
      const embeddings = await query.collect()
      const filteredEmbeddings = embeddings.filter(e => 
        messageIds.includes(e.message_id)
      )
      
      const modelCounts = filteredEmbeddings.reduce((acc, e) => {
        acc[e.model] = (acc[e.model] || 0) + 1
        return acc
      }, {} as Record<string, number>)
      
      return {
        total: filteredEmbeddings.length,
        by_model: modelCounts,
        oldest: filteredEmbeddings.reduce((min, e) => 
          e.created_at < min ? e.created_at : min, 
          Date.now()
        ),
        newest: filteredEmbeddings.reduce((max, e) => 
          e.created_at > max ? e.created_at : max, 
          0
        )
      }
    }
    
    // Global stats
    const embeddings = await query.collect()
    const modelCounts = embeddings.reduce((acc, e) => {
      acc[e.model] = (acc[e.model] || 0) + 1
      return acc
    }, {} as Record<string, number>)
    
    return {
      total: embeddings.length,
      by_model: modelCounts,
      oldest: embeddings.length > 0 
        ? embeddings.reduce((min, e) => e.created_at < min ? e.created_at : min, Date.now())
        : null,
      newest: embeddings.length > 0
        ? embeddings.reduce((max, e) => e.created_at > max ? e.created_at : max, 0)
        : null
    }
  }
})