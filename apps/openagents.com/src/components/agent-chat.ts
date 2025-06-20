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
      let channelSubscriptionId = null;

      // Get current agent's keys from localStorage
      function getAgentKeys() {
        const keys = localStorage.getItem('agent-keys');
        if (!keys) return null;
        try {
          const parsed = JSON.parse(keys);
          const activeAgent = localStorage.getItem('active-agent');
          return parsed[activeAgent] || Object.values(parsed)[0] || null;
        } catch (e) {
          console.error('Failed to parse agent keys:', e);
          return null;
        }
      }

      // Initialize WebSocket connection
      function initWebSocket() {
        if (ws && ws.readyState === WebSocket.OPEN) return;
        
        ws = new WebSocket('ws://localhost:3003/relay');
        
        ws.onopen = () => {
          console.log('Connected to Nostr relay');
          // Subscribe to all channels when connected
          subscribeToChannels();
          if (selectedChannelId) {
            subscribeToChannel(selectedChannelId);
          }
        };
        
        ws.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            
            // Handle channel list events (kind 40)
            if (msg[0] === 'EVENT' && msg[1] === channelSubscriptionId) {
              const nostrEvent = msg[2];
              if (nostrEvent.kind === 40) {
                // Channel creation event
                try {
                  const channelData = JSON.parse(nostrEvent.content);
                  const channel = {
                    id: nostrEvent.id,
                    name: channelData.name || 'Unnamed Channel',
                    description: channelData.about || '',
                    messageCount: 0,
                    lastActivity: nostrEvent.created_at * 1000,
                    creatorPubkey: nostrEvent.pubkey
                  };
                  
                  // Add or update channel
                  const existingIndex = currentChannels.findIndex(c => c.id === channel.id);
                  if (existingIndex >= 0) {
                    currentChannels[existingIndex] = { ...currentChannels[existingIndex], ...channel };
                  } else {
                    currentChannels.push(channel);
                  }
                  
                  updateChannelList();
                } catch (e) {
                  console.error('Failed to parse channel content:', e);
                }
              }
            }
            
            // Handle message events (kind 42)
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
            
            // Handle end of stored events
            if (msg[0] === 'EOSE') {
              console.log('End of stored events for subscription:', msg[1]);
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

      // Subscribe to all channels (kind 40)
      function subscribeToChannels() {
        if (!ws || ws.readyState !== WebSocket.OPEN) return;
        
        // Close previous subscription if exists
        if (channelSubscriptionId) {
          ws.send(JSON.stringify(['CLOSE', channelSubscriptionId]));
        }
        
        channelSubscriptionId = 'channels-' + crypto.randomUUID();
        const subscription = [
          'REQ',
          channelSubscriptionId,
          {
            kinds: [40],
            limit: 100
          }
        ];
        ws.send(JSON.stringify(subscription));
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

      // Channel management functions
      window.createChannel = async function() {
        const name = prompt('Enter channel name:');
        const description = prompt('Enter channel description:');
        if (name && description) {
          try {
            const agentKeys = getAgentKeys();
            if (!agentKeys || !agentKeys.privateKey) {
              alert('No agent keys found. Please create an agent first.');
              return;
            }

            // Use direct WebSocket event creation
            if (ws && ws.readyState === WebSocket.OPEN) {
              // Create channel event manually
              const event = {
                kind: 40,
                created_at: Math.floor(Date.now() / 1000),
                tags: [],
                content: JSON.stringify({ name, about: description }),
                pubkey: agentKeys.publicKey
              };
              
              // Sign event (simplified - in production use proper signing)
              const eventId = generateEventId(event);
              event.id = eventId;
              event.sig = await signEvent(event, agentKeys.privateKey);
              
              // Send event to relay
              ws.send(JSON.stringify(['EVENT', event]));
              console.log('Channel creation event sent');
            } else {
              alert('Not connected to relay. Please wait and try again.');
            }
          } catch (error) {
            console.error('Failed to create channel:', error);
            alert('Failed to create channel: ' + error.message);
          }
        }
      };

      window.refreshChannels = function() {
        // Channels are loaded via WebSocket subscription
        if (ws && ws.readyState === WebSocket.OPEN) {
          subscribeToChannels();
        }
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
            const agentKeys = getAgentKeys();
            if (!agentKeys || !agentKeys.privateKey) {
              alert('No agent keys found. Please create an agent first.');
              return false;
            }

            // Use direct WebSocket event creation
            if (ws && ws.readyState === WebSocket.OPEN) {
              // Create message event manually
              const event = {
                kind: 42,
                created_at: Math.floor(Date.now() / 1000),
                tags: [['e', selectedChannelId, '', 'root']],
                content: message,
                pubkey: agentKeys.publicKey
              };
              
              // Sign event (simplified - in production use proper signing)
              const eventId = generateEventId(event);
              event.id = eventId;
              event.sig = await signEvent(event, agentKeys.privateKey);
              
              // Send event to relay
              ws.send(JSON.stringify(['EVENT', event]));
              console.log('Message event sent');
              input.value = '';
            } else {
              alert('Not connected to relay. Please wait and try again.');
            }
          } catch (error) {
            console.error('Error sending message:', error);
            alert('Failed to send message: ' + error.message);
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
        // Clear existing messages
        messages = [];
        updateMessagesDisplay();
        
        // Messages will be loaded via WebSocket subscription to kind 42 events
        // No REST API call needed - subscribeToChannel() handles this
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

      // Helper functions for event signing (simplified fallback)
      async function generateEventId(event) {
        try {
          // Serialize event for hashing
          const serialized = JSON.stringify([
            0,
            event.pubkey,
            event.created_at,
            event.kind,
            event.tags,
            event.content
          ]);
          
          // Hash using Web Crypto API
          const encoder = new TextEncoder();
          const data = encoder.encode(serialized);
          const hashBuffer = await crypto.subtle.digest('SHA-256', data);
          const hashArray = Array.from(new Uint8Array(hashBuffer));
          return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
        } catch (error) {
          console.error('Failed to generate event ID:', error);
          // Fallback to random ID
          return crypto.randomUUID().replace(/-/g, '');
        }
      }

      async function signEvent(event, privateKey) {
        try {
          // Import crypto functions (these would normally be imported at top)
          const { schnorr } = await import('@noble/curves/secp256k1');
          const { hexToBytes, bytesToHex } = await import('@noble/hashes/utils');
          
          // Sign the event ID (which is the hash)
          const messageHash = hexToBytes(event.id);
          const privKeyBytes = hexToBytes(privateKey);
          const signature = schnorr.sign(messageHash, privKeyBytes);
          
          return bytesToHex(signature);
        } catch (error) {
          console.error('Failed to sign event:', error);
          // Return proper length placeholder if real signing fails
          return Array(128).fill('0').join('');
        }
      }

      // Initialize on load
      (async function init() {
        // Initialize WebSocket connection
        initWebSocket();
        
        // Channels will be loaded via WebSocket subscription
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
