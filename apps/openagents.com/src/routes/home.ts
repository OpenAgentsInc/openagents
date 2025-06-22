import { document, html } from "@openagentsinc/psionic"
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

export async function home() {
  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <div class="chat-container">
        <!-- Header -->
        <div class="chat-header">
          <div class="flex items-center gap-6">
            <a href="/" class="text-xl font-bold text-white hover:text-gray-200 transition-colors">
              OpenAgents
            </a>
            <div class="model-indicator">
              <span class="model-indicator-dot"></span>
              <span>claude-3-opus</span>
            </div>
          </div>
          <nav class="flex items-center gap-6">
            <a href="/agents" class="text-sm text-gray-400 hover:text-white transition-colors">Agents</a>
            <a href="/docs" class="text-sm text-gray-400 hover:text-white transition-colors">Docs</a>
            <a href="/blog" class="text-sm text-gray-400 hover:text-white transition-colors">Blog</a>
            <div class="w-8 h-8 rounded-full bg-gray-700 flex items-center justify-center text-xs font-bold text-gray-200">
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
