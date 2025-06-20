/**
 * Client-side utilities for PGlite persistence
 * Provides a simple API for browser usage without Effect
 */
import { Effect, Layer, ManagedRuntime, Runtime } from "effect"
import type { Conversation, Message, NewConversation, NewMessage } from "./schema"
import {
  ConversationRepository,
  ConversationRepositoryLive,
  MessageRepository,
  MessageRepositoryLive,
  type PGliteConfig,
  PGliteServiceLive
} from "./services"

export class ChatClient {
  private managedRuntime: any
  private runtime: Runtime.Runtime<ConversationRepository | MessageRepository>
  private listeners: Map<string, Set<(messages: Array<Message>) => void>> = new Map()
  private unsubscribers: Map<string, () => void> = new Map()

  constructor(config?: PGliteConfig) {
    // Create runtime with persistence services
    const layer = Layer.provide(
      Layer.mergeAll(ConversationRepositoryLive, MessageRepositoryLive),
      PGliteServiceLive(config)
    )
    this.managedRuntime = ManagedRuntime.make(layer)
    this.runtime = (this.managedRuntime as any).runtime
  }

  // Conversation methods
  async createConversation(conversation: Partial<NewConversation> = {}): Promise<Conversation> {
    const newConversation: NewConversation = {
      userId: conversation.userId || "local",
      title: conversation.title,
      model: conversation.model,
      metadata: conversation.metadata || {},
      ...conversation
    }

    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.create(newConversation)
      })
    )
  }

  async getConversation(id: string): Promise<Conversation | undefined> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.get(id)
      })
    )
  }

  async listConversations(userId = "local", includeArchived = false): Promise<Array<Conversation>> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.list(userId, includeArchived)
      })
    )
  }

  async updateConversation(id: string, updates: Partial<Conversation>): Promise<Conversation | undefined> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.update(id, updates)
      })
    )
  }

  async deleteConversation(id: string): Promise<boolean> {
    // Clean up any listeners for this conversation
    this.unsubscribeFromConversation(id)

    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.delete(id)
      })
    )
  }

  async archiveConversation(id: string): Promise<boolean> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* ConversationRepository
        return yield* repo.archive(id)
      })
    )
  }

  // Message methods
  async sendMessage(message: NewMessage): Promise<Message> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* MessageRepository
        const sent = yield* repo.send(message)

        // Auto-generate title if this is the first user message
        if (message.role === "user") {
          const conversationRepo = yield* ConversationRepository
          const conversation = yield* conversationRepo.get(message.conversationId)
          if (conversation && !conversation.title) {
            yield* conversationRepo.generateTitle(message.conversationId)
          }
        }

        return sent
      })
    )
  }

  async getMessages(conversationId: string, limit?: number): Promise<Array<Message>> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* MessageRepository
        const messages = yield* repo.getConversation(conversationId, limit)
        // Reverse to get chronological order (query returns desc)
        return messages.reverse()
      })
    )
  }

  async searchMessages(query: string, userId = "local"): Promise<Array<Message>> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* MessageRepository
        return yield* repo.search(query, userId)
      })
    )
  }

  async deleteMessage(id: string): Promise<boolean> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* MessageRepository
        return yield* repo.delete(id)
      })
    )
  }

  // Live updates
  subscribeToConversation(
    conversationId: string,
    callback: (messages: Array<Message>) => void
  ): () => void {
    // Store callback
    if (!this.listeners.has(conversationId)) {
      this.listeners.set(conversationId, new Set())
    }
    this.listeners.get(conversationId)!.add(callback)

    // Set up live query if not already active
    if (!this.unsubscribers.has(conversationId)) {
      // eslint-disable-next-line @typescript-eslint/no-this-alias
      const self = this
      Runtime.runPromise(this.runtime)(
        Effect.gen(function*() {
          const repo = yield* MessageRepository
          const unsubscribe = yield* repo.watchConversation(conversationId, (messages) => {
            // Notify all listeners for this conversation
            const listeners = self.listeners.get(conversationId)
            if (listeners) {
              // Ensure chronological order
              const orderedMessages = [...messages].sort((a, b) =>
                new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
              )
              listeners.forEach((cb) => cb(orderedMessages))
            }
          })
          self.unsubscribers.set(conversationId, unsubscribe)
        })
      )
    }

    // Return unsubscribe function
    return () => {
      const listeners = this.listeners.get(conversationId)
      if (listeners) {
        listeners.delete(callback)
        // If no more listeners, clean up the live query
        if (listeners.size === 0) {
          this.unsubscribeFromConversation(conversationId)
        }
      }
    }
  }

  private unsubscribeFromConversation(conversationId: string) {
    const unsubscribe = this.unsubscribers.get(conversationId)
    if (unsubscribe) {
      unsubscribe()
      this.unsubscribers.delete(conversationId)
    }
    this.listeners.delete(conversationId)
  }

  // Helper method to create a new conversation with an initial message
  async startConversation(
    initialMessage: string,
    model?: string,
    metadata?: any
  ): Promise<{ conversation: Conversation; message: Message }> {
    // Create conversation
    const conversation = await this.createConversation({
      model,
      metadata
    })

    // Send initial message
    const message = await this.sendMessage({
      conversationId: conversation.id,
      role: "user",
      content: initialMessage,
      metadata: {}
    })

    return { conversation, message }
  }
}

// Export types for convenience
export type { Conversation, Message, NewConversation, NewMessage } from "./schema"
