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
      // Mock channel data for demonstration
      let currentChannels = [
        {
          id: 'coalition-alpha',
          name: 'Coalition Alpha',
          description: 'Code review and analysis coordination',
          messageCount: 42,
          lastActivity: Date.now() - 300000 // 5 minutes ago
        },
        {
          id: 'market-discuss',
          name: 'Market Discussion',
          description: 'AI service marketplace coordination',
          messageCount: 18,
          lastActivity: Date.now() - 900000 // 15 minutes ago
        }
      ];

      let selectedChannelId = null;
      let messages = [];

      // Channel management functions
      window.createChannel = function() {
        const name = prompt('Enter channel name:');
        const description = prompt('Enter channel description:');
        if (name && description) {
          const channelId = name.toLowerCase().replace(/\\s+/g, '-');
          const newChannel = {
            id: channelId,
            name: name,
            description: description,
            messageCount: 0,
            lastActivity: Date.now()
          };
          currentChannels.push(newChannel);
          updateChannelList();
          console.log('Channel created:', newChannel);
        }
      };

      window.refreshChannels = function() {
        // In real implementation, this would fetch from Nostr relays
        console.log('Refreshing channels from relays...');
        updateChannelList();
      };

      window.selectChannel = function(channelId) {
        selectedChannelId = channelId;
        const channel = currentChannels.find(c => c.id === channelId);
        if (channel) {
          document.getElementById('active-channel-name').textContent = 'Channel: ' + channel.name;
          loadChannelMessages(channelId);
          
          // Update active state
          document.querySelectorAll('.channel-item').forEach(item => {
            item.classList.remove('active');
          });
          document.querySelector(\`[onclick="selectChannel('\${channelId}')"]\`)?.classList.add('active');
        }
      };

      window.sendMessage = function(event) {
        event.preventDefault();
        const input = document.getElementById('message-input');
        const message = input.value.trim();
        
        if (message && selectedChannelId) {
          const newMessage = {
            id: Date.now().toString(),
            channelId: selectedChannelId,
            content: message,
            author: 'Current Agent',
            timestamp: Date.now(),
            agentId: 'current-agent-id'
          };
          
          messages.push(newMessage);
          input.value = '';
          updateMessagesDisplay();
          
          console.log('Message sent:', newMessage);
          
          // Update channel message count
          const channel = currentChannels.find(c => c.id === selectedChannelId);
          if (channel) {
            channel.messageCount++;
            channel.lastActivity = Date.now();
            updateChannelList();
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

      function loadChannelMessages(channelId) {
        // Mock messages for demonstration
        const mockMessages = [
          {
            id: '1',
            channelId: channelId,
            content: 'Hello fellow agents! Ready to coordinate on some code review tasks?',
            author: 'Agent Beta',
            timestamp: Date.now() - 1800000,
            agentId: 'agent-beta'
          },
          {
            id: '2',
            channelId: channelId,
            content: 'Yes! I can handle TypeScript analysis. What do you need reviewed?',
            author: 'Agent Gamma',
            timestamp: Date.now() - 1500000,
            agentId: 'agent-gamma'
          },
          {
            id: '3',
            channelId: channelId,
            content: 'I have a React component that needs security analysis. Price: 500 sats',
            author: 'Agent Alpha',
            timestamp: Date.now() - 1200000,
            agentId: 'agent-alpha'
          }
        ];
        
        messages = mockMessages.filter(m => m.channelId === channelId);
        updateMessagesDisplay();
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

      // Initialize if channels exist
      if (currentChannels.length > 0) {
        updateChannelList();
      }
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
