/**
 * Browser-safe chat client using Effect
 * This provides a simple Promise-based API while using Effect internally
 */
import { Effect, ManagedRuntime, Runtime } from "effect"
import { BrowserConversationRepository, BrowserMessageRepository, BrowserPersistenceLive } from "./client-services"
import type { Conversation, Message, NewConversation, NewMessage } from "./schema"

export class BrowserEffectChatClient {
  private managedRuntime: ManagedRuntime.ManagedRuntime<
    BrowserConversationRepository | BrowserMessageRepository,
    never
  >
  private runtime: Runtime.Runtime<BrowserConversationRepository | BrowserMessageRepository>
  private listeners: Map<string, Set<(messages: Array<Message>) => void>> = new Map()
  private unsubscribers: Map<string, () => void> = new Map()

  constructor(databaseName?: string) {
    // Create runtime using ManagedRuntime
    const layer = BrowserPersistenceLive(databaseName)
    this.managedRuntime = ManagedRuntime.make(layer) as any
    this.runtime = (this.managedRuntime as any).runtime
  }

  // Conversation methods
  async createConversation(conversation: Partial<NewConversation> = {}): Promise<Conversation> {
    const newConversation: NewConversation = {
      userId: conversation.userId || "local",
      title: conversation.title || null,
      model: conversation.model || null,
      metadata: conversation.metadata || {},
      archived: false,
      ...conversation
    }

    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.create(newConversation)
      })
    )
  }

  async getConversation(id: string): Promise<Conversation | undefined> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.get(id)
      })
    )
  }

  async listConversations(userId = "local", includeArchived = false): Promise<Array<Conversation>> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.list(userId, includeArchived)
      })
    )
  }

  async updateConversation(id: string, updates: Partial<Conversation>): Promise<Conversation | undefined> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.update(id, updates)
      })
    )
  }

  async deleteConversation(id: string): Promise<boolean> {
    // Clean up any listeners for this conversation
    this.unsubscribeFromConversation(id)

    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.delete(id)
      })
    )
  }

  async archiveConversation(id: string): Promise<boolean> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserConversationRepository
        return yield* repo.archive(id)
      })
    )
  }

  // Message methods
  async sendMessage(message: NewMessage): Promise<Message> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserMessageRepository
        const sent = yield* repo.send(message)

        // Auto-generate title if this is the first user message
        if (message.role === "user") {
          const conversationRepo = yield* BrowserConversationRepository
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
        const repo = yield* BrowserMessageRepository
        return yield* repo.getConversation(conversationId, limit)
      })
    )
  }

  async searchMessages(query: string, userId = "local"): Promise<Array<Message>> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserMessageRepository
        return yield* repo.search(query, userId)
      })
    )
  }

  async deleteMessage(id: string): Promise<boolean> {
    return Runtime.runPromise(this.runtime)(
      Effect.gen(function*() {
        const repo = yield* BrowserMessageRepository
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
      const setupWatch = Effect.gen(function*() {
        const repo = yield* BrowserMessageRepository
        const unsubscribe = yield* repo.watchConversation(conversationId, (messages) => {
          // Notify all listeners for this conversation
          const listeners = self.listeners.get(conversationId)
          if (listeners) {
            listeners.forEach((cb) => cb(messages))
          }
        })
        self.unsubscribers.set(conversationId, unsubscribe)
      })

      Runtime.runPromise(this.runtime)(setupWatch).catch(console.error)
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
