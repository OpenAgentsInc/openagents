import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

// Mock data for demo
const mockThreads = [
  {
    id: "1",
    title: "Bitcoin Lightning Integration",
    preview: "How can I integrate Lightning payments into my agent?",
    timestamp: "2 hours ago",
    active: true
  },
  {
    id: "2",
    title: "Nostr Protocol Questions",
    preview: "I need help understanding NIP-06 key derivation...",
    timestamp: "Yesterday",
    active: false
  },
  {
    id: "3",
    title: "Agent Development",
    preview: "What's the best way to structure an autonomous agent?",
    timestamp: "3 days ago",
    active: false
  }
]

const mockMessages = [
  {
    id: "1",
    type: "user",
    name: "You",
    content: "How can I integrate Lightning payments into my agent?",
    timestamp: "2:34 PM"
  },
  {
    id: "2",
    type: "assistant",
    name: "Assistant",
    content:
      `To integrate Lightning payments into your OpenAgents agent, you'll want to use the Lightning namespace from the SDK. Here's a basic example:

\`\`\`typescript
import { Agent, Lightning } from '@openagentsinc/sdk'

// Create an agent with Lightning capabilities
const agent = await Agent.create({
  name: 'PaymentAgent',
  capabilities: ['lightning']
})

// Generate a Lightning invoice
const invoice = await Lightning.createInvoice({
  amount: 1000, // satoshis
  description: 'Agent service payment'
})

// Listen for payment
Lightning.onPayment(invoice.id, (payment) => {
  console.log('Payment received!', payment)
})
\`\`\`

The SDK handles all the complexity of Lightning node management and provides a simple API for your agent to receive and send payments.`,
    timestamp: "2:35 PM"
  },
  {
    id: "3",
    type: "user",
    name: "You",
    content: "That looks great! Can agents also make payments autonomously?",
    timestamp: "2:36 PM"
  },
  {
    id: "4",
    type: "assistant",
    name: "Assistant",
    content:
      `Yes! Agents can make autonomous payments using the Lightning.pay() method. You'll need to configure spending limits and approval rules:

\`\`\`typescript
// Configure spending limits
const spendingRules = {
  maxPerPayment: 10000, // max 10k sats per payment
  dailyLimit: 100000,   // max 100k sats per day
  requireApproval: (amount) => amount > 5000 // require approval for payments over 5k sats
}

// Make a payment
const result = await Lightning.pay({
  invoice: 'lnbc...',
  rules: spendingRules
})
\`\`\`

This ensures your agent can operate autonomously while maintaining security boundaries.`,
    timestamp: "2:37 PM"
  }
]

function renderThread(thread: typeof mockThreads[0]) {
  return html`
    <div class="thread-item ${thread.active ? "active" : ""}">
      <div class="thread-title">${thread.title}</div>
      <div class="thread-preview">${thread.preview}</div>
      <div class="thread-meta">${thread.timestamp}</div>
    </div>
  `
}

function renderMessage(message: typeof mockMessages[0]) {
  const isUser = message.type === "user"

  return html`
    <div class="message">
      <div class="message-avatar ${message.type}">
        ${isUser ? "U" : "A"}
      </div>
      <div class="message-content">
        <div class="message-header">
          <span class="message-name">${message.name}</span>
          <span class="message-time">${message.timestamp}</span>
        </div>
        <div class="message-body ${message.type}">
          ${message.content.includes("```") ? renderCodeMessage(message.content) : message.content}
        </div>
        ${
    !isUser ?
      html`
          <div class="message-actions">
            <button class="message-action-button" title="Copy">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
              </svg>
            </button>
            <button class="message-action-button" title="Good response">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 10h4.764a2 2 0 011.789 2.894l-3.5 7A2 2 0 0115.263 21h-4.017c-.163 0-.326-.02-.485-.06L7 20m7-10V5a2 2 0 00-2-2h-.095c-.5 0-.905.405-.905.905 0 .714-.211 1.412-.608 2.006L7 11v9m7-10h-2M7 20H5a2 2 0 01-2-2v-6a2 2 0 012-2h2.5"></path>
              </svg>
            </button>
          </div>
        ` :
      ""
  }
      </div>
    </div>
  `
}

function renderCodeMessage(content: string) {
  // Split content by code blocks
  const parts = content.split(/```(\w*)\n([\s\S]*?)```/)

  return html`
    ${
    parts.map((part, index) => {
      if (index % 3 === 0) {
        // Regular text
        return part ? html`<div>${part}</div>` : ""
      } else if (index % 3 === 1) {
        // Language identifier
        return ""
      } else {
        // Code block
        const language = parts[index - 1] || "plaintext"
        return html`
          <div class="message-code">
            <div class="code-header">
              <span class="code-language">${language}</span>
              <button class="message-action-button" title="Copy code">
                <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"></path>
                </svg>
              </button>
            </div>
            <pre><code>${part}</code></pre>
          </div>
        `
      }
    }).join("")
  }
  `
}

const chatStyles = css`
  /* Chat UI Styles - Zinc Theme */

  /* Main container */
  .chat-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--background0);
    color: var(--foreground1);
  }

  /* Header */
  .chat-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    background: var(--background1);
    border-bottom: 1px solid var(--foreground2);
  }

  .chat-header a {
    color: var(--foreground2);
    text-decoration: none;
    transition: color 0.2s;
  }

  .chat-header a:hover {
    color: var(--foreground1);
  }

  .model-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--foreground0);
    font-size: 0.875rem;
  }

  .model-indicator-dot {
    width: 6px;
    height: 6px;
    background: var(--foreground0);
    border-radius: 50%;
    display: inline-block;
  }

  /* Main layout */
  .chat-main {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  /* Sidebar */
  .chat-sidebar {
    width: 260px;
    background: var(--background1);
    border-right: 1px solid var(--foreground2);
    display: flex;
    flex-direction: column;
  }

  .thread-header {
    padding: 1rem;
    border-bottom: 1px solid var(--foreground2);
  }

  .new-thread-button {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.75rem;
    background: var(--background2);
    color: var(--foreground1);
    border: 1px solid var(--foreground2);
    border-radius: 0.375rem;
    cursor: pointer;
    transition: all 0.2s;
    font-family: inherit;
  }

  .new-thread-button:hover {
    background: var(--background3);
    border-color: var(--foreground0);
  }

  .thread-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .thread-item {
    padding: 0.75rem;
    margin-bottom: 0.25rem;
    cursor: pointer;
    border-radius: 0.375rem;
    transition: background 0.2s;
  }

  .thread-item:hover {
    background: var(--background2);
  }

  .thread-item.active {
    background: var(--background2);
    border: 1px solid var(--foreground0);
  }

  .thread-title {
    font-weight: 500;
    color: var(--foreground1);
    margin-bottom: 0.25rem;
  }

  .thread-preview {
    font-size: 0.875rem;
    color: var(--foreground0);
    margin-bottom: 0.25rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .thread-meta {
    font-size: 0.75rem;
    color: var(--overlay0);
  }

  /* Chat content */
  .chat-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--background0);
  }

  /* Messages */
  .messages-container {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 1rem;
  }

  .messages-wrapper {
    max-width: 48rem;
    margin: 0 auto;
  }

  .message {
    display: flex;
    gap: 1rem;
    margin-bottom: 1.5rem;
    align-items: flex-start;
  }

  .message-avatar {
    width: 2rem;
    height: 2rem;
    background: var(--background2);
    border: 1px solid var(--foreground2);
    border-radius: 0.375rem;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 500;
    font-size: 0.875rem;
    flex-shrink: 0;
  }

  .message-avatar.user {
    background: var(--background2);
    color: var(--foreground1);
  }

  .message-avatar.assistant {
    background: var(--foreground0);
    color: var(--background0);
  }

  .message-content {
    flex: 1;
    min-width: 0;
  }

  .message-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .message-name {
    font-weight: 500;
    color: var(--foreground1);
  }

  .message-time {
    font-size: 0.75rem;
    color: var(--overlay0);
  }

  .message-body {
    color: var(--foreground1);
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .message-body.assistant {
    background: var(--background1);
    padding: 1rem;
    border: 1px solid var(--foreground2);
    border-radius: 0.375rem;
  }

  .message-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }

  .message-action-button {
    padding: 0.375rem;
    background: transparent;
    border: 1px solid var(--foreground2);
    border-radius: 0.25rem;
    color: var(--foreground0);
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .message-action-button:hover {
    background: var(--background2);
    border-color: var(--foreground0);
    color: var(--foreground1);
  }

  /* Code blocks */
  .message-code {
    margin: 0.5rem 0;
    border: 1px solid var(--foreground2);
    border-radius: 0.375rem;
    overflow: hidden;
    background: var(--background0);
  }

  .code-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--background2);
    border-bottom: 1px solid var(--foreground2);
  }

  .code-language {
    font-size: 0.75rem;
    color: var(--foreground0);
    text-transform: uppercase;
  }

  .message-code pre {
    margin: 0;
    padding: 1rem;
    overflow-x: auto;
    background: var(--background0);
  }

  .message-code code {
    font-family: "Berkeley Mono", monospace;
    font-size: 0.875rem;
    color: var(--foreground1);
  }

  /* Input area */
  .chat-input-container {
    padding: 1rem;
    background: var(--background1);
    border-top: 1px solid var(--foreground2);
  }

  .chat-input-wrapper {
    max-width: 48rem;
    margin: 0 auto;
  }

  .chat-input-form {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .chat-input {
    flex: 1;
    padding: 0.75rem 1rem;
    background: var(--background0);
    border: 1px solid var(--foreground2);
    border-radius: 0.375rem;
    color: var(--foreground1);
    font-family: inherit;
    resize: none;
    min-height: 2.75rem;
    max-height: 10rem;
    overflow-y: auto;
    transition: border-color 0.2s;
  }

  .chat-input:focus {
    outline: none;
    border-color: var(--foreground0);
  }

  .chat-input::placeholder {
    color: var(--overlay0);
  }

  .chat-send-button {
    padding: 0.75rem 1.5rem;
    background: var(--foreground0);
    color: var(--background0);
    border: 1px solid var(--foreground0);
    border-radius: 0.375rem;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-family: inherit;
  }

  .chat-send-button:hover {
    background: var(--foreground1);
    border-color: var(--foreground1);
  }

  .chat-send-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Utility classes from Tailwind migration */
  .text-xl { font-size: 1.25rem; }
  .font-bold { font-weight: 700; }
  .text-white { color: var(--foreground2); }
  .text-zinc-200 { color: var(--foreground1); }
  .text-zinc-400 { color: var(--foreground0); }
  .text-zinc-700 { color: var(--overlay1); }
  .bg-zinc-700 { background-color: var(--background3); }
  .hover\:text-white:hover { color: var(--foreground2); }
  .hover\:text-zinc-200:hover { color: var(--foreground1); }
  .transition-colors { transition-property: color; transition-duration: 200ms; }
  .flex { display: flex; }
  .items-center { align-items: center; }
  .justify-center { justify-content: center; }
  .gap-6 { gap: 1.5rem; }
  .w-8 { width: 2rem; }
  .h-8 { height: 2rem; }
  .w-4 { width: 1rem; }
  .h-4 { height: 1rem; }
  .w-3 { width: 0.75rem; }
  .h-3 { height: 0.75rem; }
  .rounded-full { border-radius: 9999px; }
  .text-xs { font-size: 0.75rem; }
  .text-sm { font-size: 0.875rem; }

  /* Scrollbar styling */
  ::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  ::-webkit-scrollbar-track {
    background: var(--background1);
  }

  ::-webkit-scrollbar-thumb {
    background: var(--background3);
    border-radius: 4px;
  }

  ::-webkit-scrollbar-thumb:hover {
    background: var(--overlay0);
  }

  /* Responsive */
  @media (max-width: 768px) {
    .chat-sidebar {
      position: absolute;
      left: -260px;
      height: 100%;
      transition: left 0.3s;
      z-index: 10;
    }
    
    .chat-sidebar.open {
      left: 0;
    }
    
    .messages-container {
      padding: 1rem 0.5rem;
    }
  }
`

export async function home() {
  return document({
    title: "OpenAgents",
    styles: css`${baseStyles}
${chatStyles}`,
    body: html`
      <div class="chat-container">
        <!-- Header -->
        <div class="chat-header">
          <div class="flex items-center gap-6">
            <a href="/" class="text-xl font-bold text-white hover:text-zinc-200 transition-colors">
              OpenAgents
            </a>
            <div class="model-indicator">
              <span class="model-indicator-dot"></span>
              <span>claude-3-opus</span>
            </div>
          </div>
          <nav class="flex items-center gap-6">
            <a href="/agents" class="text-sm text-zinc-400 hover:text-white transition-colors">Agents</a>
            <a href="/docs" class="text-sm text-zinc-400 hover:text-white transition-colors">Docs</a>
            <a href="/blog" class="text-sm text-zinc-400 hover:text-white transition-colors">Blog</a>
            <div class="w-8 h-8 rounded-full bg-zinc-700 flex items-center justify-center text-xs font-bold text-zinc-200">
              U
            </div>
          </nav>
        </div>

        <div class="chat-main">
          <!-- Sidebar -->
          <div class="chat-sidebar">
            <div class="thread-header">
              <button class="new-thread-button w-full">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
                </svg>
                <span>New Thread</span>
              </button>
            </div>
            <div class="thread-list">
              ${mockThreads.map((thread) => renderThread(thread)).join("")}
            </div>
          </div>

          <!-- Chat Content -->
          <div class="chat-content">
            <!-- Messages -->
            <div class="messages-container">
              <div class="messages-wrapper">
                ${mockMessages.map((message) => renderMessage(message)).join("")}
              </div>
            </div>

            <!-- Input Area -->
            <div class="chat-input-container">
              <div class="chat-input-wrapper">
                <form class="chat-input-form" onsubmit="event.preventDefault()">
                  <textarea 
                    class="chat-input" 
                    placeholder="Type your message..."
                    rows="1"
                    onkeydown="if(event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); }"
                  ></textarea>
                  <button type="submit" class="chat-send-button">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"></path>
                    </svg>
                    <span>Send</span>
                  </button>
                </form>
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        // Basic interactivity for demo
        document.querySelector('.chat-input').addEventListener('input', function(e) {
          // Auto-resize textarea
          e.target.style.height = 'auto';
          e.target.style.height = Math.min(e.target.scrollHeight, 200) + 'px';
        });

        // Copy code functionality
        document.addEventListener('click', function(e) {
          if (e.target.closest('.message-action-button[title="Copy code"]')) {
            const codeBlock = e.target.closest('.message-code').querySelector('code');
            navigator.clipboard.writeText(codeBlock.textContent);
          }
          if (e.target.closest('.message-action-button[title="Copy"]')) {
            const messageBody = e.target.closest('.message').querySelector('.message-body');
            navigator.clipboard.writeText(messageBody.textContent);
          }
        });
      </script>
    `
  })
}
