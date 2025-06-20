/**
 * Agent Chat Component - Real-time NIP-28 channel conversations
 */

import { html } from "@openagentsinc/psionic"

export interface AgentChatProps {
  agentId?: string
  channelId?: string
  channels?: Array<{
    id: string
    name: string
    description: string
    messageCount: number
    lastActivity: number
  }>
}

export function agentChat({ agentId: _agentId, channelId, channels = [] }: AgentChatProps = {}) {
  return html`
    <div class="agent-chat" box-="square">
      <div class="chat-header">
        <h3>Agent Communications</h3>
        <div class="chat-actions">
          <button is-="button" size-="small" variant-="foreground1" onclick="createChannel()">
            + Channel
          </button>
          <button is-="button" size-="small" variant-="background1" onclick="refreshChannels()">
            Refresh
          </button>
        </div>
      </div>

      <!-- Channel List -->
      <div class="channel-list">
        <h4>Active Channels</h4>
        <div id="channel-list-container">
          ${
    channels.length === 0 ?
      html`<div class="empty-state">
              <p>No channels yet. Create a coordination channel to start collaborating with other agents.</p>
            </div>` :
      html`<div class="channels">
              ${
        channels.map((channel) =>
          html`
                <div class="channel-item ${channelId === channel.id ? "active" : ""}" 
                     onclick="selectChannel('${channel.id}')">
                  <div class="channel-info">
                    <span class="channel-name">${channel.name}</span>
                    <span class="channel-description">${channel.description}</span>
                  </div>
                  <div class="channel-meta">
                    <span is-="badge" variant-="background2" size-="small">
                      ${channel.messageCount}
                    </span>
                    <span class="last-activity">
                      ${new Date(channel.lastActivity).toLocaleString()}
                    </span>
                  </div>
                </div>
              `
        ).join("")
      }
            </div>`
  }
        </div>
      </div>

      <!-- Active Channel Chat -->
      ${
    channelId ?
      html`
        <div class="active-chat">
          <div class="chat-title">
            <h4 id="active-channel-name">Channel: ${channelId}</h4>
            <span is-="badge" variant-="foreground0" size-="small" id="agent-count">
              3 agents online
            </span>
          </div>

          <!-- Messages Container -->
          <div class="messages-container" id="messages-container">
            <div class="message-placeholder">
              <p>Loading channel messages...</p>
            </div>
          </div>

          <!-- Message Input -->
          <div class="message-input-container">
            <form id="message-form" onsubmit="return sendMessage(event)">
              <input 
                is-="input" 
                type="text" 
                id="message-input" 
                placeholder="Type your message..." 
                box-="square"
                style="flex: 1;"
                required
              >
              <button is-="button" type="submit" variant-="foreground1" box-="square">
                Send
              </button>
            </form>
          </div>
        </div>
      ` :
      html`
        <div class="no-channel-selected">
          <p>Select a channel to start communicating</p>
        </div>
      `
  }
    </div>

    <script>
      // Real channel data from relay
      let currentChannels = [];
      let selectedChannelId = null;
      let messages = [];
      let ws = null;
      let subscriptionId = null;

      // Initialize WebSocket connection
      function initWebSocket() {
        if (ws && ws.readyState === WebSocket.OPEN) return;
        
        ws = new WebSocket('ws://localhost:3003/relay');
        
        ws.onopen = () => {
          console.log('Connected to Nostr relay');
          // Subscribe to all channels when connected
          if (selectedChannelId) {
            subscribeToChannel(selectedChannelId);
          }
        };
        
        ws.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            if (msg[0] === 'EVENT' && msg[1] === subscriptionId) {
              const nostrEvent = msg[2];
              if (nostrEvent.kind === 42) {
                // New channel message
                const message = {
                  id: nostrEvent.id,
                  channelId: nostrEvent.tags.find(t => t[0] === 'e' && t[3] === 'root')?.[1],
                  content: nostrEvent.content,
                  author: nostrEvent.pubkey.slice(0, 8) + '...',
                  timestamp: nostrEvent.created_at * 1000,
                  agentId: nostrEvent.pubkey
                };
                
                if (message.channelId === selectedChannelId) {
                  messages.push(message);
                  updateMessagesDisplay();
                }
                
                // Update channel stats
                const channel = currentChannels.find(c => c.id === message.channelId);
                if (channel) {
                  channel.messageCount++;
                  channel.lastActivity = Date.now();
                  updateChannelList();
                }
              }
            }
          } catch (e) {
            console.error('Failed to parse WebSocket message:', e);
          }
        };
        
        ws.onclose = () => {
          console.log('Disconnected from relay, reconnecting...');
          setTimeout(initWebSocket, 3000);
        };
      }

      // Subscribe to channel messages
      function subscribeToChannel(channelId) {
        if (!ws || ws.readyState !== WebSocket.OPEN) return;
        
        // Close previous subscription
        if (subscriptionId) {
          ws.send(JSON.stringify(['CLOSE', subscriptionId]));
        }
        
        subscriptionId = crypto.randomUUID();
        const subscription = [
          'REQ',
          subscriptionId,
          {
            kinds: [42],
            '#e': [channelId],
            limit: 50
          }
        ];
        ws.send(JSON.stringify(subscription));
      }

      // Load channels from API
      async function loadChannels() {
        try {
          const response = await fetch('/api/channels/list');
          const data = await response.json();
          currentChannels = data.channels || [];
          updateChannelList();
        } catch (error) {
          console.error('Failed to load channels:', error);
        }
      }

      // Channel management functions
      window.createChannel = async function() {
        const name = prompt('Enter channel name:');
        const description = prompt('Enter channel description:');
        if (name && description) {
          try {
            const response = await fetch('/api/channels/create', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                name: name,
                about: description
              })
            });
            
            if (response.ok) {
              const { channelId } = await response.json();
              console.log('Channel created:', channelId);
              // Reload channels
              await loadChannels();
            }
          } catch (error) {
            console.error('Failed to create channel:', error);
          }
        }
      };

      window.refreshChannels = function() {
        loadChannels();
      };

      window.selectChannel = async function(channelId) {
        selectedChannelId = channelId;
        const channel = currentChannels.find(c => c.id === channelId);
        if (channel) {
          document.getElementById('active-channel-name').textContent = 'Channel: ' + channel.name;
          await loadChannelMessages(channelId);
          
          // Subscribe to channel via WebSocket
          subscribeToChannel(channelId);
          
          // Update active state
          document.querySelectorAll('.channel-item').forEach(item => {
            item.classList.remove('active');
          });
          document.querySelector(\`[onclick="selectChannel('\${channelId}')"]\`)?.classList.add('active');
        }
      };

      window.sendMessage = async function(event) {
        event.preventDefault();
        const input = document.getElementById('message-input');
        const message = input.value.trim();
        
        if (message && selectedChannelId) {
          try {
            const response = await fetch('/api/channels/message', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                channelId: selectedChannelId,
                content: message
              })
            });
            
            if (response.ok) {
              input.value = '';
              // Message will appear via WebSocket subscription
            } else {
              console.error('Failed to send message');
            }
          } catch (error) {
            console.error('Error sending message:', error);
          }
        }
        
        return false;
      };

      function updateChannelList() {
        const container = document.getElementById('channel-list-container');
        if (container && currentChannels.length > 0) {
          container.innerHTML = \`
            <div class="channels">
              \${currentChannels.map(channel => \`
                <div class="channel-item \${selectedChannelId === channel.id ? 'active' : ''}" 
                     onclick="selectChannel('\${channel.id}')">
                  <div class="channel-info">
                    <span class="channel-name">\${channel.name}</span>
                    <span class="channel-description">\${channel.description}</span>
                  </div>
                  <div class="channel-meta">
                    <span is-="badge" variant-="background2" size-="small">
                      \${channel.messageCount}
                    </span>
                    <span class="last-activity">
                      \${new Date(channel.lastActivity).toLocaleString()}
                    </span>
                  </div>
                </div>
              \`).join('')}
            </div>
          \`;
        }
      }

      async function loadChannelMessages(channelId) {
        try {
          const response = await fetch(\`/api/channels/\${channelId}\`);
          const data = await response.json();
          
          if (data.messages) {
            messages = data.messages.map(msg => ({
              id: msg.id,
              channelId: msg.tags.find(t => t[0] === 'e' && t[3] === 'root')?.[1] || channelId,
              content: msg.content,
              author: msg.pubkey.slice(0, 8) + '...',
              timestamp: msg.created_at * 1000,
              agentId: msg.pubkey
            }));
            updateMessagesDisplay();
          }
        } catch (error) {
          console.error('Failed to load channel messages:', error);
          messages = [];
          updateMessagesDisplay();
        }
      }

      function updateMessagesDisplay() {
        const container = document.getElementById('messages-container');
        if (container && messages.length > 0) {
          container.innerHTML = messages.map(message => \`
            <div class="message">
              <div class="message-header">
                <span class="message-author">\${message.author}</span>
                <span class="message-timestamp">\${new Date(message.timestamp).toLocaleTimeString()}</span>
              </div>
              <div class="message-content">\${message.content}</div>
            </div>
          \`).join('');
          
          // Scroll to bottom
          container.scrollTop = container.scrollHeight;
        }
      }

      // Initialize on load
      (async function init() {
        await loadChannels();
        initWebSocket();
        
        // Select first channel if any exist
        if (currentChannels.length > 0) {
          selectChannel(currentChannels[0].id);
        }
      })();
    </script>

    <style>
      .agent-chat {
        background: var(--background1);
        padding: 1.5rem;
        height: 600px;
        display: flex;
        flex-direction: column;
      }

      .chat-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
        padding-bottom: 1rem;
        border-bottom: 1px solid var(--background3);
      }

      .chat-header h3 {
        margin: 0;
        color: var(--foreground0);
      }

      .chat-actions {
        display: flex;
        gap: 0.5rem;
      }

      /* Channel List */
      .channel-list {
        margin-bottom: 1rem;
      }

      .channel-list h4 {
        margin: 0 0 0.5rem 0;
        color: var(--foreground1);
        font-size: 0.9rem;
      }

      .channels {
        max-height: 120px;
        overflow-y: auto;
        border: 1px solid var(--background3);
      }

      .channel-item {
        padding: 0.75rem;
        border-bottom: 1px solid var(--background2);
        cursor: pointer;
        transition: background-color 0.2s ease;
      }

      .channel-item:hover {
        background: var(--background2);
      }

      .channel-item.active {
        background: var(--background3);
        border-left: 3px solid var(--foreground0);
      }

      .channel-info {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
      }

      .channel-name {
        font-weight: 600;
        color: var(--foreground0);
        font-size: 0.9rem;
      }

      .channel-description {
        color: var(--foreground2);
        font-size: 0.8rem;
      }

      .channel-meta {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-top: 0.5rem;
      }

      .last-activity {
        font-size: 0.7rem;
        color: var(--foreground2);
      }

      /* Active Chat */
      .active-chat {
        flex: 1;
        display: flex;
        flex-direction: column;
        min-height: 0;
      }

      .chat-title {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
        padding-bottom: 0.5rem;
        border-bottom: 1px solid var(--background3);
      }

      .chat-title h4 {
        margin: 0;
        color: var(--foreground0);
      }

      .messages-container {
        flex: 1;
        overflow-y: auto;
        border: 1px solid var(--background3);
        padding: 1rem;
        margin-bottom: 1rem;
        background: var(--background0);
      }

      .message {
        margin-bottom: 1rem;
        padding: 0.75rem;
        background: var(--background1);
        border-left: 3px solid var(--background3);
      }

      .message-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.5rem;
      }

      .message-author {
        font-weight: 600;
        color: var(--foreground0);
        font-size: 0.9rem;
      }

      .message-timestamp {
        font-size: 0.8rem;
        color: var(--foreground2);
      }

      .message-content {
        color: var(--foreground1);
        line-height: 1.4;
      }

      .message-input-container {
        display: flex;
        gap: 0.5rem;
        align-items: flex-end;
      }

      .message-input-container form {
        display: flex;
        gap: 0.5rem;
        width: 100%;
        align-items: flex-end;
      }

      /* No channel selected / Empty states */
      .no-channel-selected, .empty-state {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--foreground2);
        text-align: center;
      }

      .message-placeholder {
        text-align: center;
        color: var(--foreground2);
        padding: 2rem;
      }

      /* Responsive */
      @media (max-width: 768px) {
        .agent-chat {
          height: 400px;
        }

        .channels {
          max-height: 80px;
        }

        .message-input-container form {
          flex-direction: column;
        }
      }
    </style>
  `
}
