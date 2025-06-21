/**
 * Agent Chat Component with Effect.js WebSocket
 * Pure Effect implementation - no React
 */

import { 
  BrowserServicesLive,
  ChannelService,
  type Channel, 
  type ChannelMessage 
} from "@openagentsinc/sdk/browser"
import { Effect, Ref, Stream } from "effect"
import { dateUtils } from "../lib/date.js"
import { html } from "../lib/html.js"

// Component state
interface ChatState {
  currentChannelId: string | null
  channels: Map<string, Channel>
  messages: Map<string, Array<ChannelMessage>>
  loading: boolean
  error: string | null
  selectedAgent: { name: string; pubkey: string } | null
}

// Create initial state
const createInitialState = (): ChatState => ({
  currentChannelId: null,
  channels: new Map(),
  messages: new Map(),
  loading: true,
  error: null,
  selectedAgent: null
})

// Render functions
const renderError = (error: string) =>
  html`
  <div class="error-message" style="color: var(--danger); padding: 1rem;">
    Error: ${error}
  </div>
`

const renderLoading = () =>
  html`
  <div class="loading" style="padding: 2rem; text-align: center;">
    <div class="spinner"></div>
    Connecting to relay...
  </div>
`

const renderChannelList = (channels: Map<string, Channel>, currentChannelId: string | null) => {
  if (channels.size === 0) {
    return html`<div class="no-channels">No channels yet</div>`
  }

  return Array.from(channels.values())
    .map((channel) =>
      html`
      <button
        class="channel-item ${currentChannelId === channel.id ? "active" : ""}"
        data-channel-id="${channel.id}"
        onclick="window.selectChannel('${channel.id}')"
      >
        <div class="channel-name">${channel.name}</div>
        <div class="channel-about">${channel.about}</div>
        <div class="channel-meta">
          ${channel.message_count || 0} messages
        </div>
      </button>
    `
    )
    .join("")
}

const renderMessages = (messages: Array<ChannelMessage>) => {
  if (!messages || messages.length === 0) {
    return html`<div class="no-messages">No messages in this channel yet</div>`
  }

  return messages
    .sort((a, b) => a.created_at - b.created_at)
    .map((msg) =>
      html`
      <div class="message">
        <div class="message-header">
          <span class="message-author">${msg.pubkey.slice(0, 8)}...</span>
          <span class="message-time">${dateUtils.formatRelativeTime(msg.created_at)}</span>
        </div>
        <div class="message-content">${msg.content}</div>
      </div>
    `
    )
    .join("")
}

const renderChat = (state: ChatState) => {
  if (state.loading) return renderLoading()
  if (state.error) return renderError(state.error)

  const currentChannel = state.currentChannelId ? state.channels.get(state.currentChannelId) : null
  const currentMessages = state.currentChannelId ? (state.messages.get(state.currentChannelId) || []) : []

  return html`
    <div class="agent-chat-container">
      <div class="channel-sidebar">
        <div class="channel-header">
          <h3>Channels</h3>
          <button class="create-channel-btn" onclick="window.showCreateChannel()">+</button>
        </div>
        <div class="channel-list">
          ${renderChannelList(state.channels, state.currentChannelId)}
        </div>
      </div>
      
      <div class="chat-main">
        ${
    currentChannel ?
      html`
          <div class="chat-header">
            <h2>${currentChannel.name}</h2>
            <span class="channel-about">${currentChannel.about}</span>
          </div>
          
          <div class="messages-container" id="messages-container">
            ${renderMessages(currentMessages)}
          </div>
          
          <div class="message-input-container">
            <textarea
              id="message-input"
              placeholder="Type a message..."
              onkeypress="if(event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); window.sendMessage(); }"
            ></textarea>
            <button onclick="window.sendMessage()" class="send-button">Send</button>
          </div>
        ` :
      html`
          <div class="no-channel-selected">
            Select a channel to start chatting
          </div>
        `
  }
      </div>
    </div>
  `
}

// Create Effect program
export const createAgentChatProgram = (container: HTMLElement) =>
  Effect.gen(function*() {
    // Initialize services
    const channelService = yield* ChannelService
    const stateRef = yield* Ref.make(createInitialState())

    // Helper to update UI
    const updateUI = (state: ChatState) => {
      container.innerHTML = renderChat(state)
    }

    // Subscribe to channels
    const channelSubscription = yield* channelService.channels.pipe(
      Stream.tap((channel) =>
        Ref.update(stateRef, (state) => {
          const newChannels = new Map(state.channels)
          newChannels.set(channel.id, channel)
          return { ...state, channels: newChannels, loading: false }
        })
      ),
      Stream.tap(() => Ref.get(stateRef).pipe(Effect.map(updateUI))),
      Stream.runDrain
    ).pipe(
      Effect.fork
    )

    // Window functions for UI interactions
    window.selectChannel = (channelId: string) =>
      Effect.gen(function*() {
        yield* Ref.update(stateRef, (state) => ({
          ...state,
          currentChannelId: channelId
        }))

        const state = yield* Ref.get(stateRef)
        updateUI(state)

        // Subscribe to messages for this channel
        if (!state.messages.has(channelId)) {
          yield* channelService.messages(channelId).pipe(
            Stream.tap((message) =>
              Ref.update(stateRef, (state) => {
                const messages = new Map(state.messages)
                const channelMessages = messages.get(channelId) || []
                messages.set(channelId, [...channelMessages, message])
                return { ...state, messages }
              })
            ),
            Stream.tap(() => Ref.get(stateRef).pipe(Effect.map(updateUI))),
            Stream.runDrain
          ).pipe(
            Effect.fork
          )
        }
      })

    window.sendMessage = () => {
      const input = document.getElementById("message-input") as HTMLTextAreaElement
      const message = input.value.trim()
      if (!message) return

      // Clear input
      input.value = ""

      // Note: Actual message sending requires key management
      console.log("Message sending requires key management implementation:", message)
    }

    window.showCreateChannel = () => {
      // Note: Channel creation requires key management
      console.log("Channel creation requires key management implementation")
    }

    // Initial render
    const initialState = yield* Ref.get(stateRef)
    updateUI(initialState)

    // Handle errors
    yield* Effect.catchAll(
      Effect.all([channelSubscription]),
      (error) =>
        Ref.update(stateRef, (state) => ({
          ...state,
          loading: false,
          error: String(error)
        })).pipe(
          Effect.tap(() => Ref.get(stateRef).pipe(Effect.map(updateUI)))
        )
    )
  })

// Initialize component
export function initAgentChat(container: HTMLElement) {
  // Add styles
  const style = document.createElement("style")
  style.textContent = `
    .agent-chat-container {
      display: flex;
      height: 100%;
      background: var(--background);
      color: var(--foreground);
      border: 1px solid var(--border);
    }
    
    .channel-sidebar {
      width: 250px;
      border-right: 1px solid var(--border);
      display: flex;
      flex-direction: column;
    }
    
    .channel-header {
      padding: 1rem;
      border-bottom: 1px solid var(--border);
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    
    .channel-list {
      flex: 1;
      overflow-y: auto;
    }
    
    .channel-item {
      width: 100%;
      padding: 0.75rem 1rem;
      text-align: left;
      background: none;
      border: none;
      border-bottom: 1px solid var(--border);
      cursor: pointer;
      transition: background 0.2s;
    }
    
    .channel-item:hover {
      background: var(--muted);
    }
    
    .channel-item.active {
      background: var(--accent);
      color: var(--accent-foreground);
    }
    
    .chat-main {
      flex: 1;
      display: flex;
      flex-direction: column;
    }
    
    .chat-header {
      padding: 1rem;
      border-bottom: 1px solid var(--border);
    }
    
    .messages-container {
      flex: 1;
      overflow-y: auto;
      padding: 1rem;
    }
    
    .message {
      margin-bottom: 1rem;
      padding: 0.75rem;
      background: var(--muted);
      border-radius: 0.5rem;
    }
    
    .message-header {
      display: flex;
      justify-content: space-between;
      margin-bottom: 0.5rem;
      font-size: 0.875rem;
      opacity: 0.7;
    }
    
    .message-input-container {
      padding: 1rem;
      border-top: 1px solid var(--border);
      display: flex;
      gap: 0.5rem;
    }
    
    #message-input {
      flex: 1;
      padding: 0.5rem;
      background: var(--background);
      color: var(--foreground);
      border: 1px solid var(--border);
      border-radius: 0.25rem;
      resize: none;
    }
    
    .send-button {
      padding: 0.5rem 1rem;
      background: var(--primary);
      color: var(--primary-foreground);
      border: none;
      border-radius: 0.25rem;
      cursor: pointer;
    }
    
    .spinner {
      width: 2rem;
      height: 2rem;
      border: 2px solid var(--border);
      border-top-color: var(--primary);
      border-radius: 50%;
      animation: spin 1s linear infinite;
    }
    
    @keyframes spin {
      to { transform: rotate(360deg); }
    }
  `
  document.head.appendChild(style)

  // Run the Effect program
  Effect.runPromise(
    createAgentChatProgram(container).pipe(
      Effect.provide(BrowserServicesLive)
    )
  ).catch((error: any) => {
    console.error("Failed to initialize agent chat:", error)
    container.innerHTML = renderError(String(error))
  })
}

// Type declarations for window functions
declare global {
  interface Window {
    selectChannel: (channelId: string) => void
    sendMessage: () => void
    showCreateChannel: () => void
  }
}
