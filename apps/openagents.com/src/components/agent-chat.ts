/**
 * Agent Chat Component - Real-time NIP-28 channel conversations with Effect.js
 */

import { html } from "@openagentsinc/psionic"

export interface AgentChatProps {
  agentId?: string
  channelId?: string
}

// Component that returns HTML and initializes Effect runtime
export function agentChat({ agentId: _agentId, channelId: initialChannelId }: AgentChatProps = {}) {
  // Return the HTML structure with a container for Effect to manage
  return html`
    <div class="agent-chat" box-="square" id="agent-chat-container">
      <div class="chat-loading">
        <div class="spinner"></div>
        Initializing WebSocket connection...
      </div>
    </div>
    
    <script type="module">
      // Initialize Effect runtime when component mounts
      import { Effect, Stream, Ref } from "https://esm.sh/effect@3.10.3"
      
      const container = document.getElementById('agent-chat-container')
      const initialChannelId = ${JSON.stringify(initialChannelId || null)}
      
      // State management
      const state = {
        channels: new Map(),
        messages: new Map(),
        currentChannelId: initialChannelId,
        loading: true,
        error: null
      }
      
      // UI update function
      function updateUI() {
        if (state.loading) {
          container.innerHTML = \`
            <div class="chat-loading">
              <div class="spinner"></div>
              Connecting to relay...
            </div>
          \`
          return
        }
        
        if (state.error) {
          container.innerHTML = \`
            <div class="chat-error">
              <p>Error: \${state.error}</p>
              <button onclick="location.reload()">Retry</button>
            </div>
          \`
          return
        }
        
        const channelList = Array.from(state.channels.values())
        const currentChannel = state.currentChannelId ? state.channels.get(state.currentChannelId) : null
        const messages = state.currentChannelId ? (state.messages.get(state.currentChannelId) || []) : []
        
        container.innerHTML = \`
          <div class="chat-header">
            <h3>Agent Communications</h3>
            <div class="chat-actions">
              <button is-="button" size-="small" variant-="foreground1" onclick="window.createChannel()">
                + Channel
              </button>
            </div>
          </div>
          
          <div class="chat-content">
            <!-- Channel List -->
            <div class="channel-sidebar">
              <h4>Channels</h4>
              <div class="channel-list">
                \${channelList.length === 0 ? 
                  '<div class="empty-state">No channels yet</div>' :
                  channelList.map(channel => \`
                    <div class="channel-item \${state.currentChannelId === channel.id ? 'active' : ''}" 
                         onclick="window.selectChannel('\${channel.id}')">
                      <div class="channel-name">\${channel.name}</div>
                      <div class="channel-meta">\${channel.message_count || 0} messages</div>
                    </div>
                  \`).join('')
                }
              </div>
            </div>
            
            <!-- Messages -->
            <div class="message-area">
              \${currentChannel ? \`
                <div class="message-header">
                  <h4>\${currentChannel.name}</h4>
                  <span class="channel-about">\${currentChannel.about}</span>
                </div>
                
                <div class="messages" id="messages-container">
                  \${messages.length === 0 ?
                    '<div class="empty-messages">No messages yet</div>' :
                    messages.map(msg => \`
                      <div class="message">
                        <div class="message-author">\${msg.pubkey.slice(0, 8)}...</div>
                        <div class="message-content">\${msg.content}</div>
                        <div class="message-time">\${new Date(msg.created_at * 1000).toLocaleTimeString()}</div>
                      </div>
                    \`).join('')
                  }
                </div>
                
                <div class="message-input">
                  <textarea 
                    id="message-text" 
                    placeholder="Type a message..."
                    onkeypress="if(event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); window.sendMessage(); }"
                  ></textarea>
                  <button onclick="window.sendMessage()">Send</button>
                </div>
              \` : \`
                <div class="no-channel">Select a channel to start chatting</div>
              \`}
            </div>
          </div>
        \`
      }
      
      // Window functions
      window.selectChannel = (channelId) => {
        state.currentChannelId = channelId
        updateUI()
        
        // Subscribe to messages if not already
        if (!state.messages.has(channelId)) {
          state.messages.set(channelId, [])
          // Message subscription would be set up here
        }
      }
      
      window.sendMessage = () => {
        const textarea = document.getElementById('message-text')
        const message = textarea.value.trim()
        if (!message) return
        
        // Clear input
        textarea.value = ''
        
        // Note: Real implementation needs key management
        console.log('Sending message:', message)
        alert('Message sending requires key management implementation')
      }
      
      window.createChannel = () => {
        const name = prompt('Channel name:')
        if (!name) return
        
        const about = prompt('Channel description:')
        if (!about) return
        
        // Note: Real implementation needs key management
        console.log('Creating channel:', { name, about })
        alert('Channel creation requires key management implementation')
      }
      
      // Initialize WebSocket connection
      async function initialize() {
        try {
          // Direct WebSocket connection without Effect for now
          const ws = new WebSocket('ws://localhost:3003/relay')
          
          ws.onopen = () => {
            console.log('WebSocket connected')
            state.loading = false
            
            // Subscribe to channels
            const channelSub = {
              id: 'channels-' + Date.now(),
              filters: [{ kinds: [40], limit: 100 }]
            }
            ws.send(JSON.stringify(['REQ', channelSub.id, ...channelSub.filters]))
            
            updateUI()
          }
          
          ws.onmessage = (event) => {
            try {
              const msg = JSON.parse(event.data)
              
              if (msg[0] === 'EVENT') {
                const [, subId, nostrEvent] = msg
                
                // Handle channel events
                if (nostrEvent.kind === 40) {
                  try {
                    const content = JSON.parse(nostrEvent.content)
                    const channel = {
                      id: nostrEvent.id,
                      name: content.name,
                      about: content.about || '',
                      picture: content.picture,
                      created_at: nostrEvent.created_at,
                      pubkey: nostrEvent.pubkey,
                      message_count: 0
                    }
                    state.channels.set(channel.id, channel)
                    updateUI()
                  } catch (e) {
                    console.error('Failed to parse channel:', e)
                  }
                }
                
                // Handle message events
                if (nostrEvent.kind === 42) {
                  const channelTag = nostrEvent.tags.find(t => t[0] === 'e')
                  if (channelTag) {
                    const channelId = channelTag[1]
                    const messages = state.messages.get(channelId) || []
                    messages.push({
                      id: nostrEvent.id,
                      channel_id: channelId,
                      pubkey: nostrEvent.pubkey,
                      content: nostrEvent.content,
                      created_at: nostrEvent.created_at,
                      tags: nostrEvent.tags
                    })
                    state.messages.set(channelId, messages)
                    updateUI()
                  }
                }
              }
            } catch (e) {
              console.error('Failed to parse message:', e)
            }
          }
          
          ws.onerror = (error) => {
            console.error('WebSocket error:', error)
            state.error = 'Connection failed'
            state.loading = false
            updateUI()
          }
          
          ws.onclose = () => {
            console.log('WebSocket closed')
            state.error = 'Connection lost'
            updateUI()
          }
          
        } catch (error) {
          console.error('Failed to initialize:', error)
          state.error = error.message
          state.loading = false
          updateUI()
        }
      }
      
      // Start initialization
      initialize()
    </script>
    
    <style>
      .agent-chat {
        display: flex;
        flex-direction: column;
        height: 600px;
        max-height: 80vh;
      }
      
      .chat-header {
        padding: 1rem;
        border-bottom: 1px solid var(--border);
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      
      .chat-content {
        flex: 1;
        display: flex;
        overflow: hidden;
      }
      
      .channel-sidebar {
        width: 250px;
        border-right: 1px solid var(--border);
        padding: 1rem;
        overflow-y: auto;
      }
      
      .channel-list {
        margin-top: 1rem;
      }
      
      .channel-item {
        padding: 0.75rem;
        margin-bottom: 0.5rem;
        cursor: pointer;
        border-radius: 0.25rem;
        transition: background 0.2s;
      }
      
      .channel-item:hover {
        background: var(--muted);
      }
      
      .channel-item.active {
        background: var(--accent);
        color: var(--accent-foreground);
      }
      
      .channel-name {
        font-weight: 500;
      }
      
      .channel-meta {
        font-size: 0.875rem;
        opacity: 0.7;
      }
      
      .message-area {
        flex: 1;
        display: flex;
        flex-direction: column;
      }
      
      .message-header {
        padding: 1rem;
        border-bottom: 1px solid var(--border);
      }
      
      .messages {
        flex: 1;
        overflow-y: auto;
        padding: 1rem;
      }
      
      .message {
        margin-bottom: 1rem;
        padding: 0.75rem;
        background: var(--muted);
        border-radius: 0.25rem;
      }
      
      .message-author {
        font-weight: 500;
        margin-bottom: 0.25rem;
      }
      
      .message-time {
        font-size: 0.75rem;
        opacity: 0.6;
        margin-top: 0.25rem;
      }
      
      .message-input {
        padding: 1rem;
        border-top: 1px solid var(--border);
        display: flex;
        gap: 0.5rem;
      }
      
      .message-input textarea {
        flex: 1;
        padding: 0.5rem;
        border: 1px solid var(--border);
        border-radius: 0.25rem;
        background: var(--background);
        color: var(--foreground);
        resize: none;
        min-height: 3rem;
      }
      
      .message-input button {
        padding: 0.5rem 1rem;
        background: var(--primary);
        color: var(--primary-foreground);
        border: none;
        border-radius: 0.25rem;
        cursor: pointer;
      }
      
      .empty-state, .empty-messages, .no-channel {
        text-align: center;
        padding: 2rem;
        opacity: 0.6;
      }
      
      .chat-loading, .chat-error {
        text-align: center;
        padding: 2rem;
      }
      
      .spinner {
        width: 2rem;
        height: 2rem;
        margin: 0 auto 1rem;
        border: 2px solid var(--border);
        border-top-color: var(--primary);
        border-radius: 50%;
        animation: spin 1s linear infinite;
      }
      
      @keyframes spin {
        to { transform: rotate(360deg); }
      }
    </style>
  `
}
