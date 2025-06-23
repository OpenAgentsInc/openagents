import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

// Mock thread data - in real implementation this would come from database
const mockThreads = {
  "1": {
    id: "1",
    title: "Bitcoin Lightning Integration",
    messages: [
      {
        id: "1",
        role: "user",
        content: "How can I integrate Lightning payments into my agent?",
        timestamp: "2 hours ago"
      },
      {
        id: "2",
        role: "assistant",
        content:
          "Great question! Lightning Network integration for agents involves several key components:\n\n1. **Lightning Node Setup**: You'll need to connect to a Lightning node (LND, CLN, or Eclair)\n2. **Invoice Generation**: Create invoices for payments to your agent\n3. **Payment Processing**: Handle incoming payments and verify them\n4. **Agent Responses**: Gate agent responses behind payment verification\n\nHere's a basic example using LND:\n\n```javascript\nconst lnd = require('@lightninglabs/lnd-grpc')\n\n// Generate invoice\nconst invoice = await lnd.addInvoice({\n  value: 100, // 100 sats\n  memo: 'Agent conversation'\n})\n\n// Check payment status\nconst isPaymentReceived = await lnd.lookupInvoice({\n  payment_hash: invoice.payment_hash\n})\n```\n\nWould you like me to dive deeper into any specific aspect?",
        timestamp: "2 hours ago"
      }
    ]
  },
  "2": {
    id: "2",
    title: "Nostr Protocol Questions",
    messages: [
      {
        id: "3",
        role: "user",
        content: "I need help understanding NIP-06 key derivation for agents. How does it work?",
        timestamp: "Yesterday"
      },
      {
        id: "4",
        role: "assistant",
        content:
          "NIP-06 defines hierarchical deterministic key derivation for Nostr, which is perfect for agent systems!\n\n**Key Concepts:**\n\n1. **Master Seed**: A single seed phrase generates all keys\n2. **Derivation Path**: Each agent gets a unique path like `m/44'/1'/0'/0/0`\n3. **Agent Identity**: Each path creates a unique Nostr keypair\n\n**Implementation:**\n\n```typescript\nimport { generateMnemonic, mnemonicToSeedSync } from 'bip39'\nimport { HDKey } from '@scure/bip32'\nimport { schnorr } from '@noble/curves/secp256k1'\n\n// Generate master seed\nconst mnemonic = generateMnemonic()\nconst seed = mnemonicToSeedSync(mnemonic)\nconst hdkey = HDKey.fromMasterSeed(seed)\n\n// Derive agent key (agent index 0)\nconst agentPath = \"m/44'/1'/0'/0/0\"\nconst agentKey = hdkey.derive(agentPath)\nconst privateKey = agentKey.privateKey\nconst publicKey = schnorr.getPublicKey(privateKey)\n```\n\nThis ensures each agent has a deterministic but unique identity!",
        timestamp: "Yesterday"
      }
    ]
  },
  "3": {
    id: "3",
    title: "Agent Development",
    messages: [
      {
        id: "5",
        role: "user",
        content: "What's the best way to structure an autonomous agent?",
        timestamp: "3 days ago"
      },
      {
        id: "6",
        role: "assistant",
        content:
          "Excellent question! Here's a proven architecture for autonomous agents:\n\n## Core Components\n\n**1. Agent Runtime**\n- Event loop for continuous operation\n- State management and persistence\n- Resource monitoring (balance, compute)\n\n**2. Communication Layer**\n- Nostr for decentralized messaging\n- WebSocket connections for real-time\n- HTTP APIs for external services\n\n**3. Decision Engine**\n- Goal-oriented planning\n- Action selection based on state\n- Learning from outcomes\n\n**4. Tool Integration**\n- Lightning payments\n- External API calls\n- File system operations\n- Web scraping capabilities\n\n## Recommended Structure\n\n```\nagent/\n├── core/\n│   ├── runtime.ts      # Main event loop\n│   ├── state.ts        # State management\n│   └── scheduler.ts    # Task scheduling\n├── communication/\n│   ├── nostr.ts        # Nostr client\n│   └── api.ts          # HTTP client\n├── tools/\n│   ├── lightning.ts    # Payment tools\n│   ├── web.ts          # Web tools\n│   └── file.ts         # File tools\n└── planning/\n    ├── goals.ts        # Goal management\n    └── actions.ts      # Action execution\n```\n\nWant me to elaborate on any specific component?",
        timestamp: "3 days ago"
      }
    ]
  }
}

// V1 exact styling (reuse from home.ts but make it modular)
const chatStyles = css`
  /* V1 Color Palette */
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

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  /* Sidebar styling */
  .sidebar { transition: border-color 0.3s ease-in-out, width 0.3s ease-in-out; }
  .sidebar-open { width: 260px; border-right: 1px solid var(--sidebar-border); }
  .sidebar-closed { width: 0px; border-right: 1px solid rgba(0, 0, 0, 0); }
  .hmmm { transition: margin-left 0.3s ease-in-out; }

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

  .message-avatar.user { padding: 2px; }
  .message-avatar.assistant { padding: 5px; }
  .message-avatar svg { width: 100%; height: 100%; color: var(--white); }

  .message-content { flex: 1; max-width: 936px; }
  .message-author { font-weight: 600; color: var(--white); margin-bottom: 4px; }
  .message-body { color: var(--text); line-height: 1.6; white-space: pre-wrap; }

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

  .message-body pre code { background: none; padding: 0; }

  /* Chat input */
  .chat-input {
    background-color: transparent;
    border: 2px solid var(--input-border);
    border-radius: 6px;
    padding: 10px 40px 10px 16px;
    color: var(--white);
    font-size: 16px;
    min-height: 48px;
    resize: none;
    font-family: inherit;
    width: 100%;
    transition: all 300ms ease-in-out;
  }

  .chat-input:focus {
    outline: none !important;
    border-color: var(--white) !important;
    box-shadow: none !important;
  }

  .chat-input::placeholder { color: var(--placeholder); }

  .send-button {
    position: absolute;
    top: 10px;
    right: 6px;
    width: 28px;
    height: 28px;
    background-color: var(--white);
    color: var(--black);
    border: none;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background-color 0.2s;
    padding: 0;
    margin: 0;
  }

  .send-button:hover:not(:disabled) { background-color: rgba(255, 255, 255, 0.9); }
  .send-button:disabled { background-color: var(--gray); cursor: not-allowed; }
  .send-button svg { width: 24px; height: 24px; }

  .model-selector { color: var(--gray); font-size: 14px; }

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

  .sidebar-footer .footer-bottom a { color: inherit; text-decoration: none; }
  .sidebar-footer .footer-bottom a:hover { color: var(--white); }
`

function renderThread(thread: { id: string; title: string; active: boolean }) {
  return html`
    <div class="relative z-[15]">
      <div class="group relative rounded-lg active:opacity-90 px-3 ${thread.active ? "bg-[#262626]" : ""}">
        <a href="/chat/${thread.id}" class="flex items-center gap-2 py-1">
          <div class="relative grow overflow-hidden whitespace-nowrap text-white">
            ${thread.title}
          </div>
        </a>
      </div>
    </div>
  `
}

function renderMessage(message: { role: string; content: string; timestamp: string }) {
  return html`
    <div class="message">
      <div class="message-avatar ${message.role}">
        ${
    message.role === "user" ?
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
        <div class="message-author">${message.role === "user" ? "You" : "Assistant"}</div>
        <div class="message-body">${message.content}</div>
      </div>
    </div>
  `
}

export async function chat(ctx: { params: { id: string } }) {
  const chatId = ctx.params.id
  const currentThread = mockThreads[chatId as keyof typeof mockThreads]

  if (!currentThread) {
    return document({
      title: "Chat Not Found - OpenAgents",
      styles: baseStyles + chatStyles,
      body: html`
        <div style="display: flex; height: 100vh; align-items: center; justify-content: center; background: black; color: white;">
          <div style="text-align: center;">
            <h1>Chat Not Found</h1>
            <p>The chat thread "${chatId}" could not be found.</p>
            <a href="/" style="color: white; text-decoration: underline;">← Back to Home</a>
          </div>
        </div>
      `
    })
  }

  // Create threads list with current one marked as active
  const allThreads = Object.values(mockThreads).map((thread) => ({
    ...thread,
    active: thread.id === chatId
  }))

  return document({
    title: `${currentThread.title} - OpenAgents`,
    styles: baseStyles + chatStyles,
    body: html`
      <div style="display: flex; height: 100vh; overflow: hidden; background: black;">
        <!-- Header -->
        <div style="position: fixed; top: 0; left: 0; right: 0; height: 52px; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; z-index: 20; background: black; border-bottom: 1px solid var(--darkgray);">
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
          <div class="model-selector">llama-3.3-70b</div>
        </div>

        <!-- Sidebar -->
        <div id="sidebar" class="sidebar sidebar-open" style="position: fixed; left: 0; top: 0; height: 100vh; background: black; overflow: hidden;">
          <div style="width: 260px; height: 100%; display: flex; flex-direction: column;">
            <!-- New thread button area -->
            <div style="height: 54px; display: flex; align-items: center; justify-content: flex-end; padding: 0 16px;">
              <a href="/" style="background: none; border: none; color: white; cursor: pointer; padding: 6px; text-decoration: none;">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </a>
            </div>

            <!-- Thread list -->
            <div style="flex: 1; overflow-y: auto;">
              <div style="display: flex; flex-direction: column; gap: 8px; padding: 12px 4px;">
                <ol style="list-style: none; margin: 0; padding: 0;">
                  ${allThreads.map((thread) => `<li>${renderThread(thread)}</li>`).join("")}
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
                      <a href="/agents" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">My Agents</div>
                      </a>
                    </div>
                  </div>
                </li>
                <li>
                  <div class="relative z-[15]">
                    <div class="group relative rounded-lg active:opacity-90 px-3">
                      <a href="/settings" class="flex items-center gap-2 py-1 hover:text-white">
                        <div class="select-none cursor-pointer relative grow overflow-hidden whitespace-nowrap">Settings</div>
                      </a>
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
          <div id="messages-container" style="flex: 1; overflow-y: auto; padding: 80px 20px 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              ${currentThread.messages.map((message) => renderMessage(message)).join("")}
            </div>
          </div>

          <!-- Input area -->
          <div style="border-top: 1px solid var(--offblack); padding-top: 20px;">
            <div style="max-width: 800px; margin: 0 auto;">
              <div style="position: relative;">
                <textarea
                  id="chat-input"
                  class="chat-input"
                  placeholder="Continue the conversation..."
                  rows="1"
                  autocomplete="off"
                  autofocus
                  style="outline: none;"
                  oninput="this.style.height = 'auto'; this.style.height = Math.min(this.scrollHeight, 200) + 'px'; document.getElementById('sendBtn').disabled = !this.value.trim();"
                ></textarea>
                <button id="sendBtn" class="send-button" disabled onclick="sendMessage()">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="22" y1="2" x2="11" y2="13"></line>
                    <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
                  </svg>
                </button>
              </div>
              <div style="text-align: center; color: var(--gray); font-size: 12px; margin: 8px 0;">
                Continue existing conversation with llama-3.3-70b
              </div>
            </div>
          </div>
        </div>
      </div>

      <script>
        // Simple message continuation functionality
        async function sendMessage() {
          const input = document.getElementById('chat-input')
          const sendBtn = document.getElementById('sendBtn')
          const message = input.value.trim()
          
          if (!message) return
          
          // Add user message to UI
          const container = document.getElementById('messages-container')
          const messagesWrapper = container.querySelector('div')
          
          const userMessageHtml = \`
            <div class="message">
              <div class="message-avatar user">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                  <circle cx="12" cy="7" r="4"></circle>
                </svg>
              </div>
              <div class="message-content">
                <div class="message-author">You</div>
                <div class="message-body">\${message}</div>
              </div>
            </div>
          \`
          
          messagesWrapper.insertAdjacentHTML('beforeend', userMessageHtml)
          container.scrollTop = container.scrollHeight
          
          // Clear input
          input.value = ''
          input.style.height = 'auto'
          sendBtn.disabled = true
          
          // Show typing indicator
          const typingHtml = \`
            <div class="message" id="typing-message">
              <div class="message-avatar assistant">
                <svg viewBox="0 0 24 24" fill="currentColor">
                  <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-1-13h2v6h-2zm0 8h2v2h-2z"/>
                </svg>
              </div>
              <div class="message-content">
                <div class="message-author">Assistant</div>
                <div class="message-body">Typing...</div>
              </div>
            </div>
          \`
          
          messagesWrapper.insertAdjacentHTML('beforeend', typingHtml)
          container.scrollTop = container.scrollHeight
          
          // Simulate response (in real implementation, call API)
          setTimeout(() => {
            const typingMsg = document.getElementById('typing-message')
            if (typingMsg) {
              typingMsg.remove()
            }
            
            const responseHtml = \`
              <div class="message">
                <div class="message-avatar assistant">
                  <svg viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-1-13h2v6h-2zm0 8h2v2h-2z"/>
                  </svg>
                </div>
                <div class="message-content">
                  <div class="message-author">Assistant</div>
                  <div class="message-body">I understand you'd like to continue our conversation about "\${message}". In a full implementation, this would connect to the streaming chat API to provide real responses. For now, this demonstrates the message thread UI working properly.</div>
                </div>
              </div>
            \`
            
            messagesWrapper.insertAdjacentHTML('beforeend', responseHtml)
            container.scrollTop = container.scrollHeight
            
            input.focus()
          }, 1500)
        }
        
        // Enter key to send
        document.getElementById('chat-input').addEventListener('keydown', (e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault()
            sendMessage()
          }
        })
      </script>
    `
  })
}
