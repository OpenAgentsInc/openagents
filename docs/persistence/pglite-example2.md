// apps/chat/src/server.ts
import { Elysia } from 'elysia';
import { html } from '@elysiajs/html';
import { Effect, pipe } from 'effect';
import {
  ChatClient,
  Message,
  NewMessage,
  createChatHandlers
} from '@openagentsinc/chat-persistence';

// Initialize chat client
const chatClient = new ChatClient();

// Psionic-style component for messages
const MessageComponent = (message: Message) => `
  <div is-="message" box-="rounded" theme-="zinc">
    <header is-="message-header">
      <span is-="user">${message.userId}</span>
      <time is-="timestamp">${new Date(message.createdAt).toLocaleTimeString()}</time>
    </header>
    <div is-="message-content">
      ${message.content}
    </div>
    ${message.metadata?.attachments ? `
      <div is-="attachments">
        ${message.metadata.attachments.map(att => `
          <a is-="attachment" href="${att.url}">${att.type}</a>
        `).join('')}
      </div>
    ` : ''}
  </div>
`;

// Conversation view component
const ConversationView = async (conversationId: string) => {
  const messages = await chatClient.getMessages(conversationId);

  return `
    <div is-="conversation" box-="column" data-conversation-id="${conversationId}">
      <header is-="conversation-header" box-="row">
        <h2>Conversation</h2>
        <button is-="button" onclick="startLiveUpdates('${conversationId}')">
          Enable Live Updates
        </button>
      </header>

      <div is-="messages-container" box-="scroll">
        ${messages.map(MessageComponent).join('')}
      </div>

      <form is-="message-form" box-="row" onsubmit="sendMessage(event)">
        <input
          is-="input"
          name="content"
          placeholder="Type a message..."
          autocomplete="off"
        />
        <button is-="button primary" type="submit">Send</button>
      </form>
    </div>
  `;
};

// Search component
const SearchView = () => `
  <div is-="search" box-="column">
    <form is-="search-form" onsubmit="searchMessages(event)">
      <input
        is-="input search"
        name="query"
        placeholder="Search messages..."
      />
    </form>
    <div is-="search-results" id="search-results"></div>
  </div>
`;

// Main app with Elysia
const app = new Elysia()
  .use(html())
  .get('/', () => `
    <!DOCTYPE html>
    <html>
      <head>
        <title>OpenAgents Chat</title>
        <link rel="stylesheet" href="/styles.css">
        <script>
          // Client-side PGlite integration
          let chatClient;
          let currentConversation = null;
          let liveUnsubscribe = null;

          // Initialize on page load
          async function initializeChat() {
            // Dynamic import for browser
            const { ChatClient } = await import('@openagentsinc/chat-persistence');
            chatClient = new ChatClient();
          }

          // Send message
          async function sendMessage(event) {
            event.preventDefault();
            const form = event.target;
            const content = form.content.value;

            if (!content.trim()) return;

            const message = {
              conversationId: currentConversation,
              userId: getCurrentUser(),
              content: content.trim()
            };

            await chatClient.sendMessage(message);
            form.reset();
          }

          // Start live updates for a conversation
          function startLiveUpdates(conversationId) {
            // Clean up previous subscription
            if (liveUnsubscribe) {
              liveUnsubscribe();
            }

            currentConversation = conversationId;

            // Subscribe to live updates
            liveUnsubscribe = chatClient.subscribeToConversation(
              conversationId,
              (messages) => {
                const container = document.querySelector('[is-="messages-container"]');
                container.innerHTML = messages.map(msg =>
                  createMessageElement(msg)
                ).join('');
              }
            );
          }

          // Search messages
          async function searchMessages(event) {
            event.preventDefault();
            const query = event.target.query.value;

            const results = await chatClient.searchMessages(
              query,
              getCurrentUser()
            );

            const resultsDiv = document.getElementById('search-results');
            resultsDiv.innerHTML = results.map(msg =>
              createMessageElement(msg)
            ).join('');
          }

          // Helper to create message HTML
          function createMessageElement(message) {
            return \`
              <div is-="message" box-="rounded">
                <header is-="message-header">
                  <span is-="user">\${message.userId}</span>
                  <time is-="timestamp">\${new Date(message.createdAt).toLocaleTimeString()}</time>
                </header>
                <div is-="message-content">\${message.content}</div>
              </div>
            \`;
          }

          function getCurrentUser() {
            return 'user-123'; // Get from auth
          }

          // Initialize on load
          window.addEventListener('DOMContentLoaded', initializeChat);
        </script>
      </head>
      <body theme-="zinc">
        <main is-="app" box-="container">
          <h1>OpenAgents Chat</h1>
          <div is-="layout" box-="grid">
            <aside is-="sidebar">
              ${SearchView()}
            </aside>
            <section is-="main">
              <!-- Conversation will be loaded here -->
              <div id="conversation-container"></div>
            </section>
          </div>
        </main>
      </body>
    </html>
  `)
  .get('/conversation/:id', async ({ params }) => {
    return ConversationView(params.id);
  })
  .get('/styles.css', () => `
    /* WebTUI-style CSS */
    [is-="app"] {
      min-height: 100vh;
      padding: 1rem;
    }

    [is-="message"] {
      margin-bottom: 1rem;
      padding: 1rem;
      border: 1px solid var(--border);
    }

    [is-="message-header"] {
      display: flex;
      justify-content: space-between;
      margin-bottom: 0.5rem;
      font-size: 0.875rem;
      opacity: 0.7;
    }

    [is-="messages-container"] {
      height: 400px;
      overflow-y: auto;
      padding: 1rem;
      border: 1px solid var(--border);
      margin-bottom: 1rem;
    }

    [is-="message-form"] {
      display: flex;
      gap: 0.5rem;
    }

    [is-="input"] {
      flex: 1;
      padding: 0.5rem;
      border: 1px solid var(--border);
      background: var(--input-bg);
      color: var(--text);
    }

    [is-="button"] {
      padding: 0.5rem 1rem;
      border: 1px solid var(--border);
      background: var(--button-bg);
      color: var(--button-text);
      cursor: pointer;
    }

    [theme-="zinc"] {
      --bg: #09090b;
      --text: #fafafa;
      --border: #27272a;
      --input-bg: #18181b;
      --button-bg: #3f3f46;
      --button-text: #fafafa;
    }

    /* Box utilities */
    [box-="rounded"] { border-radius: 0.5rem; }
    [box-="column"] { display: flex; flex-direction: column; }
    [box-="row"] { display: flex; flex-direction: row; }
    [box-="scroll"] { overflow-y: auto; }
    [box-="container"] { max-width: 1200px; margin: 0 auto; }
    [box-="grid"] { display: grid; grid-template-columns: 300px 1fr; gap: 2rem; }
  `);

// Add chat API handlers
createChatHandlers(app);

// Multi-tab support using SharedWorker
const sharedWorkerCode = `
// shared-worker.js
import { PGlite } from '@electric-sql/pglite';
import { live } from '@electric-sql/pglite/live';
import { electricSync } from '@electric-sql/pglite-sync';

let pgInstance = null;
const ports = new Set();

self.onconnect = function(e) {
  const port = e.ports[0];
  ports.add(port);

  port.onmessage = async function(event) {
    const { type, payload, id } = event.data;

    // Initialize PGlite if needed
    if (!pgInstance) {
      pgInstance = new PGlite('idb://openagents-chat', {
        extensions: { live, electricSync }
      });
    }

    try {
      let result;

      switch (type) {
        case 'query':
          result = await pgInstance.query(payload.sql, payload.params);
          break;
        case 'exec':
          result = await pgInstance.exec(payload.sql);
          break;
        case 'transaction':
          result = await pgInstance.transaction(async (tx) => {
            // Execute transaction operations
            return await tx.query(payload.sql, payload.params);
          });
          break;
      }

      // Send result back to requesting port
      port.postMessage({ id, result });

      // Broadcast to other tabs if needed
      if (type === 'exec' || type === 'transaction') {
        for (const p of ports) {
          if (p !== port) {
            p.postMessage({ type: 'invalidate', payload: { tables: payload.tables } });
          }
        }
      }
    } catch (error) {
      port.postMessage({ id, error: error.message });
    }
  };

  port.onclose = () => {
    ports.delete(port);
  };
};
`;

// Serve shared worker
app.get('/shared-worker.js', () => new Response(sharedWorkerCode, {
  headers: { 'Content-Type': 'application/javascript' }
}));

// Server-sent events for real-time updates (alternative to WebSockets)
app.get('/events/:conversationId', async ({ params }) => {
  return new Response(
    new ReadableStream({
      async start(controller) {
        const encoder = new TextEncoder();

        // Subscribe to conversation updates
        const unsubscribe = chatClient.subscribeToConversation(
          params.conversationId,
          (messages) => {
            const data = `data: ${JSON.stringify(messages)}\n\n`;
            controller.enqueue(encoder.encode(data));
          }
        );

        // Clean up on close
        return () => unsubscribe();
      }
    }),
    {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      }
    }
  );
});

app.listen(3000);

console.log('OpenAgents Chat running on http://localhost:3000');
