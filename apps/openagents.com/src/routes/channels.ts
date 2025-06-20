import { createPsionicRoute } from "@openagentsinc/psionic"
import { Effect } from "effect"
import { sharedHeader } from "../components/shared-header"
import { navigation } from "../components/navigation"
import { html } from "@openagentsinc/psionic/browser"
import { styles } from "../styles"

// Channel list component
const channelList = (channels: Array<{
  id: string
  name: string
  about: string | null
  picture: string | null
  creator_pubkey: string
  message_count: number
  last_message_at: Date | null
}>) => html`
  <div class="channels-container">
    <div class="channels-header">
      <h2>Public Channels</h2>
      <a href="/channels/create" is-="button" variant-="foreground1">Create Channel</a>
    </div>
    
    ${channels.length === 0 ? html`
      <div is-="card" box-="double" class="empty-state">
        <p>No channels yet. Be the first to create one!</p>
      </div>
    ` : html`
      <div class="channels-grid">
        ${channels.map(channel => html`
          <a href="/channels/${channel.id}" is-="card" box-="double" class="channel-card">
            <div class="channel-header">
              ${channel.picture ? html`
                <img src="${channel.picture}" alt="${channel.name}" class="channel-avatar">
              ` : html`
                <div class="channel-avatar-placeholder">
                  ${channel.name.charAt(0).toUpperCase()}
                </div>
              `}
              <h3>${channel.name}</h3>
            </div>
            <p class="channel-about">${channel.about || 'No description'}</p>
            <div class="channel-stats">
              <span>${channel.message_count} messages</span>
              ${channel.last_message_at ? html`
                <span>Active ${formatRelativeTime(channel.last_message_at)}</span>
              ` : html`
                <span>No messages yet</span>
              `}
            </div>
          </a>
        `)}
      </div>
    `}
  </div>
`

// Channel chat component
const channelChat = (
  channel: any,
  messages: Array<{
    id: string
    pubkey: string
    content: string
    created_at: number
    tags: Array<Array<string>>
  }>
) => html`
  <div class="channel-view">
    <header is-="card" box-="square" class="channel-header">
      <a href="/channels" is-="button" variant-="foreground0">&larr; Back</a>
      <div class="channel-info">
        <h2>${channel.name}</h2>
        <p>${channel.about || 'No description'}</p>
      </div>
    </header>
    
    <div class="messages-container" id="message-list">
      ${messages.length === 0 ? html`
        <div class="empty-messages">
          <p>No messages yet. Start the conversation!</p>
        </div>
      ` : messages.map(msg => {
        const isReply = msg.tags.some(t => t[0] === 'e' && t[3] === 'reply')
        const replyTo = msg.tags.find(t => t[0] === 'e' && t[3] === 'reply')?.[1]
        
        return html`
          <div class="message ${isReply ? 'reply' : ''}">
            <div class="message-header">
              <strong class="author">${msg.pubkey.slice(0, 8)}...</strong>
              <time>${formatTime(msg.created_at)}</time>
            </div>
            ${isReply && replyTo ? html`
              <div class="reply-indicator">Replying to ${replyTo.slice(0, 8)}...</div>
            ` : ''}
            <p class="message-content">${msg.content}</p>
          </div>
        `
      })}
    </div>
    
    <form class="message-input-form" id="message-form">
      <input 
        type="text" 
        is-="input" 
        box-="square"
        placeholder="Type a message..."
        name="message"
        id="message-input"
        autocomplete="off"
      >
      <button type="submit" is-="button" variant-="foreground1">Send</button>
    </form>
  </div>

  <script>
    // WebSocket connection for real-time updates
    const channelId = '${channel.id}';
    let ws;
    let subscriptionId;
    
    function connectWebSocket() {
      ws = new WebSocket('ws://localhost:3003/relay');
      subscriptionId = crypto.randomUUID();
      
      ws.onopen = () => {
        console.log('Connected to relay');
        // Subscribe to channel messages
        const subscription = [
          'REQ',
          subscriptionId,
          {
            kinds: [42],
            '#e': [channelId],
            since: Math.floor(Date.now() / 1000)
          }
        ];
        ws.send(JSON.stringify(subscription));
      };
      
      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          if (msg[0] === 'EVENT' && msg[1] === subscriptionId) {
            const nostrEvent = msg[2];
            if (nostrEvent.kind === 42) {
              appendMessage(nostrEvent);
            }
          }
        } catch (e) {
          console.error('Failed to parse message:', e);
        }
      };
      
      ws.onclose = () => {
        console.log('Disconnected from relay, reconnecting...');
        setTimeout(connectWebSocket, 3000);
      };
    }
    
    function appendMessage(event) {
      const messageList = document.getElementById('message-list');
      const messageDiv = document.createElement('div');
      messageDiv.className = 'message new';
      
      const time = new Date(event.created_at * 1000).toLocaleTimeString();
      messageDiv.innerHTML = \`
        <div class="message-header">
          <strong class="author">\${event.pubkey.slice(0, 8)}...</strong>
          <time>\${time}</time>
        </div>
        <p class="message-content">\${event.content}</p>
      \`;
      
      messageList.appendChild(messageDiv);
      messageList.scrollTop = messageList.scrollHeight;
    }
    
    // Form submission
    const form = document.getElementById('message-form');
    const input = document.getElementById('message-input');
    
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      
      const message = input.value.trim();
      if (!message) return;
      
      // Send message via API
      try {
        const response = await fetch('/api/channels/message', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            channelId,
            content: message
          })
        });
        
        if (response.ok) {
          input.value = '';
        } else {
          console.error('Failed to send message');
        }
      } catch (error) {
        console.error('Error sending message:', error);
      }
    });
    
    // Connect on load
    connectWebSocket();
  </script>
`

// Channel creation form
const channelCreateForm = () => html`
  <div class="channel-create">
    <h2>Create New Channel</h2>
    
    <form id="create-channel-form" is-="card" box-="double">
      <div class="form-group">
        <label for="name">Channel Name</label>
        <input 
          type="text" 
          is-="input" 
          box-="square"
          name="name"
          id="name"
          required
          placeholder="My Awesome Channel"
        >
      </div>
      
      <div class="form-group">
        <label for="about">Description</label>
        <textarea 
          is-="textarea" 
          box-="square"
          name="about"
          id="about"
          rows="4"
          placeholder="What's this channel about?"
        ></textarea>
      </div>
      
      <div class="form-group">
        <label for="picture">Picture URL (optional)</label>
        <input 
          type="url" 
          is-="input" 
          box-="square"
          name="picture"
          id="picture"
          placeholder="https://example.com/image.jpg"
        >
      </div>
      
      <div class="form-actions">
        <a href="/channels" is-="button" variant-="foreground0">Cancel</a>
        <button type="submit" is-="button" variant-="foreground1">Create Channel</button>
      </div>
    </form>
  </div>

  <script>
    const form = document.getElementById('create-channel-form');
    
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      
      const formData = new FormData(form);
      const data = {
        name: formData.get('name'),
        about: formData.get('about'),
        picture: formData.get('picture')
      };
      
      try {
        const response = await fetch('/api/channels/create', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(data)
        });
        
        if (response.ok) {
          const { channelId } = await response.json();
          window.location.href = \`/channels/\${channelId}\`;
        } else {
          alert('Failed to create channel');
        }
      } catch (error) {
        console.error('Error creating channel:', error);
        alert('Error creating channel');
      }
    });
  </script>
`

// Helper functions
function formatRelativeTime(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000)
  
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  return `${Math.floor(seconds / 86400)}d ago`
}

function formatTime(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleTimeString([], { 
    hour: '2-digit', 
    minute: '2-digit' 
  })
}

// Routes
export const channelsRoute = createPsionicRoute({
  path: "/channels",
  handler: () => Effect.gen(function*() {
    // Fetch channels from API
    let channels: any[] = []
    try {
      const response = yield* Effect.tryPromise(() => 
        fetch('http://localhost:3003/api/channels/list')
      )
      const data = yield* Effect.tryPromise(() => response.json())
      channels = data.channels || []
    } catch (error) {
      console.error("Failed to fetch channels:", error)
    }
    
    return {
      html: html`
        <!DOCTYPE html>
        <html lang="en">
          <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Channels - OpenAgents</title>
            ${styles()}
            <style>
              .channels-container {
                max-width: 1200px;
                margin: 0 auto;
                padding: 2rem;
              }
              
              .channels-header {
                display: flex;
                justify-content: space-between;
                align-items: center;
                margin-bottom: 2rem;
              }
              
              .channels-grid {
                display: grid;
                grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                gap: 1.5rem;
              }
              
              .channel-card {
                display: block;
                text-decoration: none;
                color: inherit;
                transition: transform 0.2s;
                padding: 1.5rem;
              }
              
              .channel-card:hover {
                transform: translateY(-2px);
              }
              
              .channel-header {
                display: flex;
                align-items: center;
                gap: 1rem;
                margin-bottom: 1rem;
              }
              
              .channel-avatar, .channel-avatar-placeholder {
                width: 48px;
                height: 48px;
                border-radius: 8px;
                object-fit: cover;
              }
              
              .channel-avatar-placeholder {
                background: var(--foreground1);
                color: var(--background0);
                display: flex;
                align-items: center;
                justify-content: center;
                font-weight: bold;
                font-size: 1.5rem;
              }
              
              .channel-about {
                margin-bottom: 1rem;
                opacity: 0.8;
              }
              
              .channel-stats {
                display: flex;
                gap: 1rem;
                font-size: 0.875rem;
                opacity: 0.6;
              }
              
              .empty-state {
                text-align: center;
                padding: 3rem;
              }
            </style>
          </head>
          <body>
            ${sharedHeader()}
            <main>
              ${navigation({ current: "channels" })}
              ${channelList(channels)}
            </main>
          </body>
        </html>
      `
    }
  })
})

export const channelViewRoute = createPsionicRoute({
  path: "/channels/:id",
  handler: ({ params }) => Effect.gen(function*() {
    const channelId = params.id
    
    // Fetch channel and messages from API
    let channel = { id: channelId, name: "Unknown Channel", about: "" }
    let messages: any[] = []
    
    try {
      const response = yield* Effect.tryPromise(() => 
        fetch(`http://localhost:3003/api/channels/${channelId}`)
      )
      const data = yield* Effect.tryPromise(() => response.json())
      
      if (data.channel) {
        channel = data.channel
        messages = data.messages || []
      }
    } catch (error) {
      console.error("Failed to fetch channel:", error)
    }
    
    return {
      html: html`
        <!DOCTYPE html>
        <html lang="en">
          <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>${channel.name} - OpenAgents</title>
            ${styles()}
            <style>
              .channel-view {
                height: 100vh;
                display: flex;
                flex-direction: column;
              }
              
              .channel-header {
                display: flex;
                gap: 1rem;
                align-items: center;
                padding: 1rem 2rem;
              }
              
              .channel-info {
                flex: 1;
              }
              
              .channel-info h2 {
                margin: 0;
              }
              
              .channel-info p {
                margin: 0.25rem 0 0;
                opacity: 0.8;
              }
              
              .messages-container {
                flex: 1;
                overflow-y: auto;
                padding: 1rem 2rem;
              }
              
              .message {
                margin-bottom: 1rem;
                padding: 1rem;
                background: var(--background1);
                border-radius: 8px;
              }
              
              .message.reply {
                margin-left: 2rem;
              }
              
              .message.new {
                animation: fadeIn 0.3s ease;
              }
              
              @keyframes fadeIn {
                from { opacity: 0; transform: translateY(10px); }
                to { opacity: 1; transform: translateY(0); }
              }
              
              .message-header {
                display: flex;
                justify-content: space-between;
                margin-bottom: 0.5rem;
              }
              
              .author {
                color: var(--foreground0);
              }
              
              .message-content {
                margin: 0;
              }
              
              .reply-indicator {
                font-size: 0.875rem;
                opacity: 0.6;
                margin-bottom: 0.5rem;
              }
              
              .empty-messages {
                text-align: center;
                padding: 3rem;
                opacity: 0.6;
              }
              
              .message-input-form {
                display: flex;
                gap: 1rem;
                padding: 1rem 2rem;
                background: var(--background1);
                border-top: 1px solid var(--foreground2);
              }
              
              .message-input-form input {
                flex: 1;
              }
            </style>
          </head>
          <body>
            ${channelChat(channel, messages)}
          </body>
        </html>
      `
    }
  })
})

export const channelCreateRoute = createPsionicRoute({
  path: "/channels/create",
  handler: () => Effect.gen(function*() {
    return {
      html: html`
        <!DOCTYPE html>
        <html lang="en">
          <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Create Channel - OpenAgents</title>
            ${styles()}
            <style>
              .channel-create {
                max-width: 600px;
                margin: 0 auto;
                padding: 2rem;
              }
              
              .channel-create h2 {
                margin-bottom: 2rem;
              }
              
              .form-group {
                margin-bottom: 1.5rem;
              }
              
              .form-group label {
                display: block;
                margin-bottom: 0.5rem;
                font-weight: 500;
              }
              
              .form-actions {
                display: flex;
                gap: 1rem;
                justify-content: flex-end;
                margin-top: 2rem;
              }
            </style>
          </head>
          <body>
            ${sharedHeader()}
            <main>
              ${navigation({ current: "channels" })}
              ${channelCreateForm()}
            </main>
          </body>
        </html>
      `
    }
  })
})