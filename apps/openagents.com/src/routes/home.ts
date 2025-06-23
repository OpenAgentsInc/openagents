import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

// Mock data for demo - using v1 structure
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
    author: "You",
    content: "How can I integrate Lightning payments into my agent?",
    timestamp: "2:34 PM"
  },
  {
    id: "2",
    type: "assistant",
    author: "Assistant",
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
    author: "You",
    content: "That looks great! Can agents also make payments autonomously?",
    timestamp: "2:36 PM"
  },
  {
    id: "4",
    type: "assistant",
    author: "Assistant",
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

// V1 exact styling
const v1Styles = css`
  /* V1 Color Palette from tailwind.config.js */
  :root {
    --text: #D7D8E5;
    --offblack: #1e1e1e;
    --darkgray: #3D3D40;
    --gray: #8B8585;
    --lightgray: #A7A7A7;
    --white: #fff;
    --black: #000000;
    --input-border: #3D3E42;
    --placeholder: #777A81;
    --active-thread: #262626;
    --sidebar-border: rgba(255, 255, 255, 0.15);
  }

  /* Override any conflicting styles */
  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  /* V1 Sidebar styling */
  .sidebar {
    transition: border-color 0.3s ease-in-out, width 0.3s ease-in-out;
  }

  .sidebar-open {
    width: 260px;
    border-right: 1px solid var(--sidebar-border);
  }

  .sidebar-closed {
    width: 0px;
    border-right: 1px solid rgba(0, 0, 0, 0);
  }

  .hmmm {
    transition: margin-left 0.3s ease-in-out;
  }

  /* Thread list */
  .thread-item {
    position: relative;
    z-index: 15;
  }

  .thread-item-inner {
    position: relative;
    border-radius: 8px;
    padding: 0 12px;
    cursor: pointer;
  }

  .thread-item-inner.active {
    background-color: var(--active-thread);
  }

  .thread-item-inner:active {
    opacity: 0.9;
  }

  .thread-link {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    text-decoration: none;
    color: var(--white);
  }

  .thread-title {
    position: relative;
    flex-grow: 1;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 14px;
  }

  /* Messages */
  .message {
    display: flex;
    gap: 12px;
    padding-left: 50px;
    margin-bottom: 24px;
  }

  .message-avatar {
    width: 28px;
    height: 28px;
    border: 1px solid var(--darkgray);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .message-avatar.user {
    padding: 2px;
  }

  .message-avatar.assistant {
    padding: 5px;
  }

  .message-avatar svg {
    width: 100%;
    height: 100%;
    color: var(--white);
  }

  .message-content {
    flex: 1;
    max-width: 936px;
  }

  .message-author {
    font-weight: 600;
    color: var(--white);
    margin-bottom: 4px;
  }

  .message-body {
    color: var(--text);
    line-height: 1.6;
    white-space: pre-wrap;
  }

  .message-body code {
    background-color: var(--offblack);
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 14px;
  }

  .message-body pre {
    background-color: var(--offblack);
    border: 1px solid var(--darkgray);
    border-radius: 6px;
    padding: 16px;
    margin: 16px 0;
    overflow-x: auto;
  }

  .message-body pre code {
    background: none;
    padding: 0;
  }

  /* Chat input */
  .chat-input {
    background-color: transparent;
    border: 2px solid var(--input-border);
    border-radius: 6px;
    padding: 12px 16px;
    padding-right: 56px;
    color: var(--white);
    font-size: 16px;
    min-height: 48px;
    resize: none;
    font-family: inherit;
  }

  .chat-input:focus {
    outline: none;
    border-color: var(--white);
  }

  .chat-input::placeholder {
    color: var(--placeholder);
  }

  .send-button {
    position: absolute;
    bottom: 10px;
    right: 10px;
    width: 36px;
    height: 28px;
    background-color: var(--white);
    color: var(--black);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .send-button:hover:not(:disabled) {
    background-color: rgba(255, 255, 255, 0.9);
  }

  .send-button:disabled {
    background-color: var(--gray);
    cursor: not-allowed;
  }

  .send-button svg {
    width: 16px;
    height: 16px;
  }

  /* Model selector */
  .model-selector {
    color: var(--gray);
    font-size: 14px;
  }

  /* Sidebar footer */
  .sidebar-footer {
    border-top: 1px solid var(--offblack);
    padding: 4px 4px;
    color: var(--gray);
    font-size: 14px;
    margin-top: auto;
  }

  .sidebar-footer ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sidebar-footer .footer-bottom {
    display: flex;
    flex-direction: row;
    gap: 4px;
    padding: 12px 12px 0;
    font-size: 12px;
    opacity: 0.75;
    color: var(--gray);
  }
  
  .sidebar-footer .footer-bottom a {
    color: inherit;
    text-decoration: none;
  }
  
  .sidebar-footer .footer-bottom a:hover {
    color: var(--white);
  }

  /* Dot flashing animation from v1 */
  .dot-flashing {
    position: relative;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--white);
    animation: dot-flashing 1s infinite linear alternate;
    animation-delay: 0.5s;
    margin: 10px 20px;
  }

  .dot-flashing::before, .dot-flashing::after {
    content: "";
    display: inline-block;
    position: absolute;
    top: 0;
  }

  .dot-flashing::before {
    left: -15px;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--white);
    animation: dot-flashing 1s infinite alternate;
    animation-delay: 0s;
  }

  .dot-flashing::after {
    left: 15px;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--white);
    animation: dot-flashing 1s infinite alternate;
    animation-delay: 1s;
  }

  @keyframes dot-flashing {
    0% {
      background-color: var(--white);
    }
    50%, 100% {
      background-color: rgba(255, 255, 255, 0.2);
    }
  }
`

function renderThread(thread: typeof mockThreads[0]) {
  return html`
    <div class="thread-item">
      <div class="thread-item-inner ${thread.active ? "active" : ""}">
        <a href="/chat/${thread.id}" class="thread-link">
          <div class="thread-title">${thread.title}</div>
        </a>
      </div>
    </div>
  `
}

function renderMessage(message: typeof mockMessages[0]) {
  const isUser = message.type === "user"

  return html`
    <div class="message">
      <div class="message-avatar ${message.type}">
        ${
    isUser ?
      html`
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
            <circle cx="12" cy="7" r="4"></circle>
          </svg>
        ` :
      html`
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-1-13h2v6h-2zm0 8h2v2h-2z"/>
          </svg>
        `
  }
      </div>
      <div class="message-content">
        <div class="message-author">${message.author}</div>
        <div class="message-body">${message.content}</div>
      </div>
    </div>
  `
}

export async function home() {
  return document({
    title: "OpenAgents",
    styles: baseStyles + v1Styles,
    body: html`
      <div style="display: flex; height: 100vh; overflow: hidden; background: black;">
        <!-- Header -->
        <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black;">
          <div style="display: flex; align-items: center; gap: 20px;">
            <button onclick="document.getElementById('sidebar').classList.toggle('sidebar-open'); document.getElementById('sidebar').classList.toggle('sidebar-closed'); document.getElementById('main').classList.toggle('hmmm')" style="background: none; border: none; color: white; cursor: pointer; padding: 4px;">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="3" y1="12" x2="21" y2="12"></line>
                <line x1="3" y1="6" x2="21" y2="6"></line>
                <line x1="3" y1="18" x2="21" y2="18"></line>
              </svg>
            </button>
            <a href="/" style="color: white; text-decoration: none; font-size: 18px; font-weight: 600;">OpenAgents</a>
          </div>
          <div class="model-selector">claude-3-opus</div>
        </div>

        <!-- Sidebar -->
        <div id="sidebar" class="sidebar sidebar-open" style="position: fixed; left: 0; top: 0; height: 100vh; background: black; overflow: hidden;">
          <div style="width: 260px; height: 100%; display: flex; flex-direction: column;">
            <!-- New thread button area -->
            <div style="height: 54px; display: flex; align-items: center; justify-content: flex-end; padding: 0 16px;">
              <button style="background: none; border: none; color: white; cursor: pointer; padding: 6px;">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </button>
            </div>
            
            <!-- Thread list -->
            <div style="flex: 1; overflow-y: auto;">
              <div style="display: flex; flex-direction: column; gap: 8px; padding: 12px 4px;">
                <ol style="list-style: none; margin: 0; padding: 0;">
                  ${mockThreads.map((thread) => `<li>${renderThread(thread)}</li>`).join("")}
                </ol>
              </div>
            </div>

            <!-- Sidebar footer -->
            <div class="sidebar-footer">
              <ol>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/store" class="hover:text-white flex flex-row items-center gap-2 py-1" style="display: flex; justify-content: space-between;">
                        <div class="select-none cursor-pointer relative overflow-hidden whitespace-nowrap">Agent Store</div>
                        <div class="text-xs text-gray opacity-50">Beta</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/blog" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Blog</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://docs.openagents.com" class="hover:text-white flex flex-row items-center gap-2 py-1">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Docs</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://stacker.news/~openagents" target="_blank" class="hover:text-white flex flex-row items-center gap-2 py-1">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Community</div>
                        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                          <polyline points="15 3 21 3 21 9"></polyline>
                          <line x1="10" y1="14" x2="21" y2="3"></line>
                        </svg>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="https://github.com/OpenAgentsInc/openagents" target="_blank" class="hover:text-white flex flex-row items-center gap-2 py-1">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Source code</div>
                        <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
                          <polyline points="15 3 21 3 21 9"></polyline>
                          <line x1="10" y1="14" x2="21" y2="3"></line>
                        </svg>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div>
                    <div class="footer-bottom">
                      <a href="/terms">Terms</a>
                      <span>·</span>
                      <a href="/privacy">Privacy</a>
                    </div>
                  </div>
                </li>
              </ol>
            </div>
          </div>
        </div>

        <!-- Main content -->
        <div id="main" class="hmmm" style="flex: 1; display: flex; flex-direction: column; margin-left: 260px; transition: margin-left 0.3s ease-in-out;">
          <!-- Messages -->
          <div style="flex: 1; overflow-y: auto; padding: 80px 20px 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              ${mockMessages.map((message) => renderMessage(message)).join("")}
            </div>
          </div>

          <!-- Input area -->
          <div style="border-top: 1px solid var(--offblack); padding: 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              <div style="position: relative;">
                <textarea 
                  class="chat-input" 
                  placeholder="Send a message"
                  rows="1"
                  style="width: 100%; outline: none;"
                  oninput="this.style.height = 'auto'; this.style.height = Math.min(this.scrollHeight, 200) + 'px'; document.getElementById('sendBtn').disabled = !this.value.trim();"
                ></textarea>
                <button id="sendBtn" class="send-button" disabled>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="22" y1="2" x2="11" y2="13"></line>
                    <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    `
  })
}
