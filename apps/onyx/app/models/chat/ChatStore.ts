import {
  flow, Instance, onSnapshot, SnapshotIn, SnapshotOut, types
} from "mobx-state-tree"
import { log } from "@/utils/log"
import { withSetPropAction } from "../_helpers/withSetPropAction"
import {
  getAllChats, initializeDatabase, loadChat, saveChat
} from "./ChatStorage"

// Message Types
export const MessageModel = types
  .model("Message", {
    id: types.string,
    role: types.enumeration(["system", "user", "assistant", "function"]),
    content: types.string,
    createdAt: types.number,
    metadata: types.optional(types.frozen(), {}),
  })
  .actions(self => ({
    updateContent(content: string) {
      self.content = content
    },
    updateMetadata(metadata: any) {
      self.metadata = metadata
    }
  }))

export interface IMessage extends Instance<typeof MessageModel> { }

// Chat Types
export const ChatModel = types
  .model("Chat", {
    id: types.string,
    messages: types.array(MessageModel),
  })

// Store Model
export const ChatStoreModel = types
  .model("ChatStore")
  .props({
    isInitialized: types.optional(types.boolean, false),
    error: types.maybeNull(types.string),
    messages: types.array(MessageModel),
    currentConversationId: types.optional(types.string, "default"),
    isGenerating: types.optional(types.boolean, false),
    activeModel: types.optional(types.enumeration(["groq", "gemini"]), "gemini"),
    enabledTools: types.optional(types.array(types.string), [
      "view_file",
      "view_folder",
      "create_file",
      "rewrite_file"
    ]),
    chats: types.optional(types.array(ChatModel), []),
  })
  .actions(withSetPropAction)
  .actions((self) => {
    // Helper action to replace messages
    const replaceMessages = (messages: any[]) => {
      self.messages.clear()
      messages.forEach(msg => {
        self.messages.push(MessageModel.create(msg))
      })
    }

    // Helper action to load messages
    const loadMessagesFromStorage = flow(function* () {
      try {
        const savedMessages = yield loadChat(self.currentConversationId)
        const parsedMessages = JSON.parse(savedMessages)
        replaceMessages(parsedMessages)
      } catch (e) {
        log.error("Error loading chat:", e)
        self.messages.clear()
      }
    })

    // Helper action to update chats list
    const updateChatsList = flow(function* () {
      try {
        const allChats = yield getAllChats()
        self.chats.replace(allChats)
      } catch (e) {
        log.error("Error updating chats list:", e)
      }
    })

    const loadAllChats = flow(function* () {
      try {
        const allChats = yield getAllChats()
        self.chats.replace(allChats)
      } catch (e) {
        log.error("Error loading all chats:", e)
      }
    })

    return {
      addMessage(message: {
        role: "system" | "user" | "assistant" | "function"
        content: string
        metadata?: any
      }) {
        const msg = MessageModel.create({
          id: Math.random().toString(36).substring(2, 9),
          createdAt: Date.now(),
          ...message,
          metadata: {
            ...message.metadata,
            conversationId: self.currentConversationId,
          }
        })
        self.messages.push(msg)

        // Update chats list after adding a message
        updateChatsList()

        return msg
      },

      updateMessage(messageId: string, updates: { content?: string, metadata?: any }) {
        const message = self.messages.find(msg => msg.id === messageId)
        if (message) {
          if (updates.content !== undefined) {
            message.updateContent(updates.content)
          }
          if (updates.metadata !== undefined) {
            message.updateMetadata({
              ...updates.metadata,
              conversationId: self.currentConversationId,
            })
          }
        }
      },

      clearMessages() {
        self.messages.clear()
        // Update chats list after clearing messages
        updateChatsList()
      },

      setCurrentConversationId: flow(function* (id: string) {
        self.currentConversationId = id
        yield loadMessagesFromStorage()

        // Ensure this chat exists in the chats list
        const chatExists = self.chats.some(chat => chat.id === id)
        if (!chatExists) {
          self.chats.push(ChatModel.create({ id, messages: [] }))
        }
      }),

      setIsGenerating(value: boolean) {
        self.isGenerating = value
      },

      setError(error: string | null) {
        self.error = error
      },

      setActiveModel(model: "groq" | "gemini") {
        self.activeModel = model
      },

      toggleTool(toolName: string) {
        const index = self.enabledTools.indexOf(toolName)
        if (index === -1) {
          self.enabledTools.push(toolName)
        } else {
          self.enabledTools.splice(index, 1)
        }
      },

      setEnabledTools(tools: string[]) {
        self.enabledTools.replace(tools)
      },

      loadAllChats,

      afterCreate: flow(function* () {
        try {
          // Initialize the database
          yield initializeDatabase()

          // Load initial conversation
          yield loadMessagesFromStorage()

          // Load all chats
          yield loadAllChats()

          // Set up persistence listener
          onSnapshot(self.messages, (snapshot) => {
            saveChat(self.currentConversationId, JSON.stringify(snapshot))
              .catch(e => log.error("Error saving chat:", e))
          })
        } catch (e) {
          log.error("Error in afterCreate:", e)
        }
      })
    }
  })
  .views((self) => {
    const filteredMessages = () => {
      return self.messages
        .filter(msg => !msg.metadata?.conversationId || msg.metadata.conversationId === self.currentConversationId)
        .slice()
    }

    return {
      get currentMessages() {
        return filteredMessages()
      },
      get conversationText() {
        return filteredMessages()
          .map((msg: IMessage) => msg.content)
          .join('\n\n')
      },
      isToolEnabled(toolName: string) {
        return self.enabledTools.includes(toolName)
      },
      get allChats() {
        return self.chats.slice()
      }
    }
  })

export interface ChatStore extends Instance<typeof ChatStoreModel> { }
export interface ChatStoreSnapshotOut extends SnapshotOut<typeof ChatStoreModel> { }
export interface ChatStoreSnapshotIn extends SnapshotIn<typeof ChatStoreModel> { }

export const createChatStoreDefaultModel = () =>
  ChatStoreModel.create({
    isInitialized: false,
    error: null,
    messages: [],
    currentConversationId: "default",
    isGenerating: false,
    activeModel: "gemini",
    enabledTools: ["view_file", "view_folder", "create_file", "rewrite_file"],
    chats: [],
  })