import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

// Channel list component
const channelList = (
  channels: Array<{
    id: string
    name: string
    about: string | null
    picture: string | null
    creator_pubkey: string
    message_count: number
    last_message_at: Date | null
  }>
) =>
  html`
  <div class="channels-container">
    <div class="channels-header">
      <h2>Public Channels</h2>
      <a href="/channels/create" is-="button" variant-="foreground1">Create Channel</a>
    </div>
    
    ${
    channels.length === 0 ?
      html`
      <div is-="card" box-="double" class="empty-state">
        <p>No channels yet. Be the first to create one!</p>
      </div>
    ` :
      html`
      <div class="channels-grid">
        ${
        channels.map((channel) =>
          html`
          <a href="/channels/${channel.id}" is-="card" box-="double" class="channel-card">
            <div class="channel-header">
              ${
            channel.picture ?
              html`
                <img src="${channel.picture}" alt="${channel.name}" class="channel-avatar">
              ` :
              html`
                <div class="channel-avatar-placeholder">
                  ${channel.name.charAt(0).toUpperCase()}
                </div>
              `
          }
              <div>
                <h3>${channel.name}</h3>
                ${channel.about ? html`<p>${channel.about}</p>` : ""}
              </div>
            </div>
            <div class="channel-stats">
              <span>${channel.message_count} messages</span>
              ${
            channel.last_message_at ?
              html`<span>Last: ${formatDate(channel.last_message_at)}</span>` :
              ""
          }
            </div>
          </a>
        `
        ).join("")
      }
      </div>
    `
  }
  </div>
`

// Channel view component
const channelView = (
  channel: {
    id: string
    name: string
    about: string | null
    picture: string | null
  },
  messages: Array<{
    id: string
    pubkey: string
    content: string
    created_at: number
    tags: Array<Array<string>>
  }>
) =>
  html`
  <div class="channel-view">
    <div class="channel-header">
      <a href="/channels" is-="button" variant-="foreground2" box-="square">← Back</a>
      <div class="channel-info">
        <h2>${channel.name}</h2>
        ${channel.about ? html`<p>${channel.about}</p>` : ""}
      </div>
    </div>
    
    <div class="messages-container" id="messages-container">
      ${
    messages.map((msg) => {
      const isReply = msg.tags.some((tag) => tag[0] === "e" && tag[3] === "reply")
      return html`
        <div class="message ${isReply ? "reply" : ""}" data-id="${msg.id}">
          <div class="message-header">
            <span class="author">${msg.pubkey.slice(0, 8)}...</span>
            <span class="timestamp">${new Date(msg.created_at * 1000).toLocaleTimeString()}</span>
          </div>
          <p class="message-content">${msg.content}</p>
        </div>
      `
    }).join("")
  }
    </div>
    
    <form class="message-form" id="message-form">
      <input 
        type="text" 
        id="message-input"
        placeholder="Type a message..." 
        is-="input" 
        box-="square"
        autocomplete="off"
      />
      <button type="submit" is-="button" variant-="foreground1" box-="square">Send</button>
    </form>
  </div>
  
  <script>
    // WebSocket connection for real-time messages
    const ws = new WebSocket('ws://localhost:3003/relay');
    const channelId = '${channel.id}';
    
    ws.onopen = () => {
      // Subscribe to channel messages
      const subscriptionId = 'channel-' + Math.random().toString(36).substr(2, 9);
      ws.send(JSON.stringify([
        "REQ",
        subscriptionId,
        {
          kinds: [42],
          "#e": [channelId],
          limit: 100
        }
      ]));
    };
    
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg[0] === 'EVENT' && msg[2].kind === 42) {
          const nostrEvent = msg[2];
          const messagesContainer = document.getElementById('messages-container');
          
          // Check if message already exists
          if (!document.querySelector(\`[data-id="\${nostrEvent.id}"]\`)) {
            const isReply = nostrEvent.tags.some(tag => tag[0] === 'e' && tag[3] === 'reply');
            const messageDiv = document.createElement('div');
            messageDiv.className = \`message \${isReply ? 'reply' : ''} new\`;
            messageDiv.dataset.id = nostrEvent.id;
            messageDiv.innerHTML = \`
              <div class="message-header">
                <span class="author">\${nostrEvent.pubkey.slice(0, 8)}...</span>
                <span class="timestamp">\${new Date(nostrEvent.created_at * 1000).toLocaleTimeString()}</span>
              </div>
              <p class="message-content">\${nostrEvent.content}</p>
            \`;
            messagesContainer.appendChild(messageDiv);
            messagesContainer.scrollTop = messagesContainer.scrollHeight;
          }
        }
      } catch (e) {
        console.error('Failed to parse WebSocket message:', e);
      }
    };
    
    // Handle message sending
    document.getElementById('message-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const input = document.getElementById('message-input');
      const content = input.value.trim();
      
      if (!content) return;
      
      try {
        const response = await fetch('/api/channels/message', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            channelId: channelId,
            content: content
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
  </script>
`

// Channel creation form
const channelCreateForm = () =>
  html`
  <div class="channel-create-container">
    <div class="channel-create-header">
      <a href="/channels" is-="button" variant-="foreground2" box-="square">← Back</a>
      <h2>Create New Channel</h2>
    </div>
    
    <form class="channel-create-form" id="channel-create-form">
      <div class="form-group">
        <label for="channel-name">Channel Name</label>
        <input 
          type="text" 
          id="channel-name"
          name="name"
          placeholder="General Discussion" 
          is-="input" 
          box-="square"
          required
        />
      </div>
      
      <div class="form-group">
        <label for="channel-about">About (optional)</label>
        <textarea 
          id="channel-about"
          name="about"
          placeholder="What's this channel about?" 
          is-="textarea" 
          box-="square"
          rows="3"
        ></textarea>
      </div>
      
      <div class="form-group">
        <label for="channel-picture">Picture URL (optional)</label>
        <input 
          type="url" 
          id="channel-picture"
          name="picture"
          placeholder="https://example.com/image.jpg" 
          is-="input" 
          box-="square"
        />
      </div>
      
      <button type="submit" is-="button" variant-="foreground1" box-="square">
        Create Channel
      </button>
    </form>
  </div>
  
  <script>
    document.getElementById('channel-create-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const formData = new FormData(e.target);
      
      try {
        const response = await fetch('/api/channels/create', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            name: formData.get('name'),
            about: formData.get('about') || undefined,
            picture: formData.get('picture') || undefined
          })
        });
        
        if (response.ok) {
          const data = await response.json();
          window.location.href = '/channels/' + data.channelId;
        } else {
          console.error('Failed to create channel');
        }
      } catch (error) {
        console.error('Error creating channel:', error);
      }
    });
  </script>
`

// Helper function
function formatDate(date: Date | string): string {
  const d = typeof date === "string" ? new Date(date) : date
  return d.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  })
}

// Routes
export async function channelsRoute() {
  // Fetch channels from API
  let channels: Array<any> = []
  try {
    const response = await fetch("http://localhost:3003/api/channels/list")
    const data = await response.json()
    channels = data.channels || []
  } catch (error) {
    console.error("Failed to fetch channels:", error)
  }

  return document({
    title: "Channels - OpenAgents",
    styles: baseStyles + `
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
          padding: 1.5rem;
          text-decoration: none;
          color: inherit;
          transition: all 0.2s;
        }
        
        .channel-card:hover {
          transform: translateY(-2px);
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
        }
        
        .channel-avatar {
          width: 48px;
          height: 48px;
          border-radius: 8px;
          object-fit: cover;
        }
        
        .channel-avatar-placeholder {
          width: 48px;
          height: 48px;
          border-radius: 8px;
          background: var(--foreground0);
          color: var(--background0);
          display: flex;
          align-items: center;
          justify-content: center;
          font-weight: bold;
        }
        
        .channel-header {
          display: flex;
          gap: 1rem;
          align-items: center;
          margin-bottom: 1rem;
        }
        
        .channel-stats {
          display: flex;
          justify-content: space-between;
          font-size: 0.875rem;
          opacity: 0.8;
        }
        
        .empty-state {
          text-align: center;
          padding: 3rem;
        }
      </style>
    `,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "channels" })}

        <!-- Main Content -->
        <main class="homepage-main">
          ${channelList(channels)}
        </main>
      </div>
    `
  })
}

export async function channelViewRoute({ params }: { params: { id: string } }) {
  const channelId = params.id

  // Fetch channel and messages from API
  let channel = { id: channelId, name: "Unknown Channel", about: null, picture: null }
  let messages: Array<any> = []

  try {
    const response = await fetch(`http://localhost:3003/api/channels/${channelId}`)
    const data = await response.json()

    if (data.channel) {
      channel = data.channel
      messages = data.messages || []
    }
  } catch (error) {
    console.error("Failed to fetch channel:", error)
  }

  return document({
    title: `${channel.name} - OpenAgents`,
    styles: baseStyles + `
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
        
        .message-form {
          display: flex;
          gap: 1rem;
          padding: 1rem 2rem;
          border-top: 1px solid var(--background2);
        }
        
        .message-form input {
          flex: 1;
        }
      </style>
    `,
    body: html`
      ${channelView(channel, messages)}
    `
  })
}

export async function channelCreateRoute() {
  return document({
    title: "Create Channel - OpenAgents",
    styles: baseStyles + `
      <style>
        .channel-create-container {
          max-width: 600px;
          margin: 0 auto;
          padding: 2rem;
        }
        
        .channel-create-header {
          display: flex;
          gap: 1rem;
          align-items: center;
          margin-bottom: 2rem;
        }
        
        .channel-create-form {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }
        
        .form-group {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }
        
        .form-group label {
          font-weight: 600;
        }
      </style>
    `,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed-layout">
        ${sharedHeader({ current: "channels" })}

        <!-- Main Content -->
        <main class="homepage-main">
          ${channelCreateForm()}
        </main>
      </div>
    `
  })
}
