import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

export function chat() {
  return document({
    title: "Chat - OpenAgents",
    styles: baseStyles,
    body: html`
      ${sharedHeader({ current: "chat" })}
      
      <div class="chat-container">
        <!-- Chat History Sidebar -->
        <div class="chat-sidebar" id="chat-sidebar">
          <div class="sidebar-header">
            <button 
              id="new-chat-btn"
              is-="button" 
              box-="square"
              variant-="foreground0"
              class="new-chat-btn"
            >
              + New Chat
            </button>
            <button 
              id="sidebar-toggle"
              is-="button" 
              box-="square"
              variant-="foreground2"
              class="sidebar-toggle mobile-only"
            >
              ×
            </button>
          </div>
          
          <div class="chat-history" id="chat-history">
            <div class="history-loading">
              <p>Loading conversations...</p>
            </div>
          </div>
          
          <div class="sidebar-footer">
            <button 
              id="settings-btn"
              is-="button" 
              box-="square"
              variant-="foreground1"
              class="settings-btn"
            >
              ⚙ Settings
            </button>
          </div>
        </div>

        <!-- Main Chat Area -->
        <div class="chat-main">
          <!-- Model Indicator Bar -->
          <div class="model-bar">
            <button 
              id="sidebar-toggle-main"
              is-="button" 
              box-="square"
              variant-="foreground2"
              class="sidebar-toggle-main"
            >
              ☰
            </button>
            <div class="model-indicator" id="model-indicator">
              <span class="model-label">Model:</span>
              <span class="model-name" id="current-model-name">Not selected</span>
            </div>
          </div>
          
          <!-- Messages Area -->
          <div id="chat-messages" class="chat-messages">
            <div class="empty-state" id="empty-state">
              <h2>How can I help you today?</h2>
              <div class="example-prompts">
                <button class="example-prompt" box-="square" variant-="foreground2">
                  💡 Explain quantum computing in simple terms
                </button>
                <button class="example-prompt" box-="square" variant-="foreground2">
                  🎨 Help me design a RESTful API
                </button>
                <button class="example-prompt" box-="square" variant-="foreground2">
                  📊 Create a Python data analysis script
                </button>
                <button class="example-prompt" box-="square" variant-="foreground2">
                  🔧 Debug my JavaScript code
                </button>
              </div>
            </div>
          </div>
          
          <!-- Input Area -->
          <div class="chat-input-area">
            <div class="input-wrapper">
              <textarea 
                id="chat-input" 
                is-="textarea" 
                box-="square"
                placeholder="Type your message..."
                rows="1"
                disabled
              ></textarea>
              <button 
                id="chat-send" 
                is-="button" 
                box-="square"
                variant-="foreground0"
                disabled
              >
                Send
              </button>
            </div>
          </div>
        </div>
        
        <!-- Settings Modal -->
        <dialog id="settings-modal" class="settings-modal" box-="square">
          <div class="modal-header">
            <h2>Settings</h2>
            <button 
              id="close-settings"
              is-="button" 
              box-="square"
              variant-="foreground2"
              class="close-btn"
            >
              ×
            </button>
          </div>
          
          <div class="modal-content">
            <!-- Model Selection -->
            <div class="settings-section">
              <h3>Model Selection</h3>
              <select id="chat-model-select" box-="square" class="model-select">
                <option value="">Select a model...</option>
              </select>
            </div>
            
            <!-- Provider Status -->
            <div class="settings-section">
              <h3>Provider Status</h3>
              
              <!-- Ollama Status -->
              <div class="provider-status" box-="square">
                <div class="provider-header">
                  <span class="provider-name">Ollama</span>
                  <div class="status-indicator">
                    <span id="ollama-status-dot" class="status-dot"></span>
                    <span id="ollama-status-text" class="status-text">Checking...</span>
                  </div>
                </div>
              </div>
              
              <!-- Cloudflare Status -->
              <div class="provider-status" box-="square">
                <div class="provider-header">
                  <span class="provider-name">Cloudflare</span>
                  <div class="status-indicator">
                    <span id="cloudflare-status-dot" class="status-dot"></span>
                    <span id="cloudflare-status-text" class="status-text">Checking...</span>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- API Keys -->
            <div class="settings-section">
              <h3>API Keys</h3>
              
              <!-- OpenRouter API Key -->
              <div class="api-key-section">
                <label for="openrouter-api-key">OpenRouter API Key</label>
                <div class="api-key-input-wrapper">
                  <input 
                    type="password" 
                    id="openrouter-api-key" 
                    is-="input" 
                    box-="square"
                    placeholder="sk-or-v1-..."
                    autocomplete="off"
                  />
                  <button 
                    type="button"
                    id="openrouter-save" 
                    is-="button" 
                    box-="square"
                    variant-="foreground1"
                  >
                    Save
                  </button>
                </div>
                <div id="openrouter-status" class="api-key-status"></div>
              </div>
            </div>
            
            <!-- Available Models List -->
            <div class="settings-section" id="model-list-section" style="display: none;">
              <h3>Available Models</h3>
              <div id="model-list" class="model-list"></div>
            </div>
          </div>
        </dialog>
      </div>

      <style>
        /* Prevent page scrolling */
        html, body {
          margin: 0;
          padding: 0;
          height: 100vh;
          overflow: hidden;
          position: fixed;
          width: 100%;
        }
        
        /* ASCII Header fix */
        .ascii-header {
          position: sticky;
          top: 0;
          z-index: 100;
          flex-shrink: 0;
        }
        
        /* Main Layout */
        .chat-container {
          display: flex;
          height: calc(100vh - 4rem); /* Adjust for ASCII header height */
          background: var(--background0);
          position: relative;
          overflow: hidden;
        }

        /* Chat Sidebar */
        .chat-sidebar {
          width: 260px;
          background: var(--background1);
          border-right: 1px solid var(--foreground2);
          display: flex;
          flex-direction: column;
          transition: transform 0.3s ease;
        }

        .chat-sidebar.collapsed {
          transform: translateX(-100%);
          position: absolute;
          height: 100%;
          z-index: 10;
        }

        .sidebar-header {
          padding: 1rem;
          border-bottom: 1px solid var(--foreground2);
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .new-chat-btn {
          flex: 1;
          padding: 0.5rem 1rem;
          font-size: 0.9em;
        }

        .sidebar-toggle {
          width: 2rem;
          height: 2rem;
          padding: 0;
          font-size: 1.2em;
          margin-left: 0.5rem;
        }

        .chat-history {
          flex: 1;
          overflow-y: auto;
          padding: 0.5rem;
        }

        .history-loading {
          text-align: center;
          color: var(--foreground0);
          margin-top: 2rem;
          font-size: 0.85em;
        }

        .conversation-item {
          padding: 0.75rem;
          margin-bottom: 0.25rem;
          cursor: pointer;
          border: 1px solid transparent;
          transition: all 0.2s ease;
          border-radius: 4px;
        }

        .conversation-item:hover {
          background: var(--background2);
          border-color: var(--foreground2);
        }

        .conversation-item.active {
          background: var(--background2);
          border-color: var(--foreground0);
        }

        .conversation-title {
          font-weight: 500;
          color: var(--foreground1);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .conversation-meta {
          font-size: 0.75em;
          color: var(--foreground0);
          margin-top: 0.25rem;
        }

        .sidebar-footer {
          padding: 1rem;
          border-top: 1px solid var(--foreground2);
        }

        .settings-btn {
          width: 100%;
          padding: 0.5rem 1rem;
          font-size: 0.9em;
        }

        /* Main Chat Area */
        .chat-main {
          flex: 1;
          display: flex;
          flex-direction: column;
          background: var(--background0);
        }

        .model-bar {
          padding: 0.75rem 1rem;
          background: var(--background1);
          border-bottom: 1px solid var(--foreground2);
          display: flex;
          align-items: center;
          gap: 1rem;
        }

        .sidebar-toggle-main {
          display: none;
          width: 2rem;
          height: 2rem;
          padding: 0;
          font-size: 1.2em;
        }

        .model-indicator {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .model-label {
          color: var(--foreground0);
          font-size: 0.9em;
        }

        .model-name {
          color: var(--foreground1);
          font-weight: 500;
        }

        /* Messages Area */
        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 1.5rem;
          display: flex;
          flex-direction: column;
        }

        .empty-state {
          flex: 1;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          text-align: center;
        }

        .empty-state h2 {
          color: var(--foreground1);
          margin-bottom: 2rem;
          font-size: 1.8rem;
        }

        .example-prompts {
          display: grid;
          grid-template-columns: repeat(2, 1fr);
          gap: 1rem;
          max-width: 600px;
        }

        .example-prompt {
          padding: 1rem;
          text-align: left;
          font-size: 0.9em;
          cursor: pointer;
          transition: all 0.2s ease;
        }

        .example-prompt:hover {
          transform: translateY(-2px);
          box-shadow: 0 4px 8px rgba(0, 0, 0, 0.1);
        }

        .chat-message {
          margin-bottom: 1.5rem;
          max-width: 800px;
          width: 100%;
        }

        .message-role {
          font-size: 0.85em;
          color: var(--foreground0);
          margin-bottom: 0.5rem;
          font-weight: 500;
        }

        .message-content {
          background: var(--background1);
          padding: 1rem;
          border-radius: 8px;
          line-height: 1.6;
          white-space: pre-wrap;
          word-wrap: break-word;
        }

        .message-content.streaming::after {
          content: '▌';
          animation: blink 1s infinite;
        }

        @keyframes blink {
          0%, 50% { opacity: 1; }
          51%, 100% { opacity: 0; }
        }

        /* Input Area */
        .chat-input-area {
          padding: 1rem;
          background: var(--background1);
          border-top: 1px solid var(--foreground2);
        }

        .input-wrapper {
          display: flex;
          gap: 0.5rem;
          max-width: 800px;
          margin: 0 auto;
        }

        #chat-input {
          flex: 1;
          min-height: 2.5rem;
          max-height: 10rem;
          resize: none;
          padding: 0.75rem;
          font-size: 0.95em;
          font-family: inherit;
          line-height: 1.4;
        }

        #chat-send {
          align-self: flex-end;
          padding: 0.75rem 1.5rem;
        }

        /* Settings Modal */
        .settings-modal {
          max-width: 600px;
          width: 90%;
          padding: 0;
          background: var(--background0);
          color: var(--foreground1);
        }

        .settings-modal::backdrop {
          background: rgba(0, 0, 0, 0.5);
        }

        .modal-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 1rem 1.5rem;
          border-bottom: 1px solid var(--foreground2);
        }

        .modal-header h2 {
          margin: 0;
          font-size: 1.25rem;
        }

        .close-btn {
          width: 2rem;
          height: 2rem;
          padding: 0;
          font-size: 1.5em;
          line-height: 1;
        }

        .modal-content {
          padding: 1.5rem;
          max-height: 70vh;
          overflow-y: auto;
        }

        .settings-section {
          margin-bottom: 2rem;
        }

        .settings-section h3 {
          margin: 0 0 1rem 0;
          font-size: 1.1rem;
          color: var(--foreground0);
        }

        .model-select {
          width: 100%;
          padding: 0.75rem;
          background: var(--background1);
          color: var(--foreground1);
          border: 1px solid var(--foreground2);
          border-radius: 4px;
          font-family: inherit;
          font-size: 0.95em;
        }

        .provider-status {
          padding: 1rem;
          margin-bottom: 0.5rem;
          background: var(--background1);
        }

        .provider-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .provider-name {
          font-weight: 500;
        }

        .status-indicator {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .status-dot {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background: var(--foreground2);
        }

        .status-dot.checking {
          background: var(--warning);
          animation: pulse 1s infinite;
        }

        .status-dot.online {
          background: var(--success);
        }

        .status-dot.offline {
          background: var(--danger);
        }

        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }

        .status-text {
          font-size: 0.85em;
          color: var(--foreground0);
        }

        .api-key-section {
          margin-bottom: 1.5rem;
        }

        .api-key-section label {
          display: block;
          margin-bottom: 0.5rem;
          font-weight: 500;
        }

        .api-key-input-wrapper {
          display: flex;
          gap: 0.5rem;
        }

        .api-key-input-wrapper input {
          flex: 1;
        }

        .api-key-status {
          margin-top: 0.5rem;
          font-size: 0.85em;
        }

        .api-key-status.success {
          color: var(--success);
        }

        .api-key-status.error {
          color: var(--danger);
        }

        .model-list {
          max-height: 300px;
          overflow-y: auto;
        }

        .model-item {
          padding: 0.75rem;
          background: var(--background1);
          border: 1px solid var(--foreground2);
          border-radius: 4px;
        }

        .model-item-name {
          font-weight: 500;
          color: var(--foreground1);
          margin-bottom: 0.25rem;
        }

        .model-item-details {
          font-size: 0.85em;
          color: var(--foreground0);
        }

        /* Mobile Styles */
        @media (max-width: 768px) {
          .chat-sidebar {
            position: absolute;
            height: 100%;
            z-index: 10;
          }

          .chat-sidebar:not(.collapsed) {
            box-shadow: 2px 0 4px rgba(0, 0, 0, 0.1);
          }

          .sidebar-toggle-main {
            display: block;
          }

          .mobile-only {
            display: block;
          }

          .chat-messages {
            padding: 1rem;
          }

          .chat-input-area {
            padding: 1rem;
          }

          .example-prompts {
            grid-template-columns: 1fr;
          }
        }

        @media (min-width: 769px) {
          .mobile-only {
            display: none;
          }
        }

        .chat-input-container {
          display: flex;
          gap: 0.5rem;
          padding-top: 0.75rem;
          border-top: 1px solid var(--foreground2);
        }

        #chat-input {
          flex: 1;
        }

        #chat-model-select {
          min-width: 200px;
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
          font-size: var(--font-size);
          padding: 0.5rem 1rem;
          background: var(--background1);
          color: var(--foreground1);
          border: 1px solid var(--foreground2);
          border-radius: 4px;
          cursor: pointer;
        }

        #chat-model-select:focus {
          outline: none;
          border-color: var(--foreground0);
        }

        #chat-model-select option {
          background: var(--background1);
          color: var(--foreground1);
          font-family: "Berkeley Mono", ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, "DejaVu Sans Mono", monospace;
        }

        /* Responsive design */
        @media (max-width: 768px) {
          .chat-layout {
            flex-direction: column;
            height: auto;
          }

          .sidebar {
            width: 100%;
            min-width: 100%;
            max-height: 200px;
            overflow-y: auto;
          }

          .main-chat {
            height: calc(100vh - 300px);
            min-height: 300px;
          }

          .model-list {
            max-height: 150px;
          }
        }
      </style>

      <script type="module">
        // Import the Effect-based ChatClient from the bundled module with cache-busting
        import { ChatClient } from '/js/chat-client.js?v=${Date.now()}'
        
        // Initialize ChatClient
        const chatClient = new ChatClient()
        
        // Chat state
        let currentConversationId = null
        let currentModel = ''
        let currentProvider = 'cloudflare' // Default to cloudflare
        let isStreaming = false
        let openRouterApiKey = localStorage.getItem('openRouterApiKey') || ''
        let cloudflareAvailable = true // Assume available by default
        let messageUnsubscribe = null

        // UI Elements
        let elements = {}

        // Initialize UI elements
        const initElements = () => {
          elements = {
            sidebar: document.getElementById('chat-sidebar'),
            sidebarToggle: document.getElementById('sidebar-toggle'),
            sidebarToggleMain: document.getElementById('sidebar-toggle-main'),
            settingsBtn: document.getElementById('settings-btn'),
            settingsModal: document.getElementById('settings-modal'),
            closeSettings: document.getElementById('close-settings'),
            newChatBtn: document.getElementById('new-chat-btn'),
            chatInput: document.getElementById('chat-input'),
            chatSend: document.getElementById('chat-send'),
            chatMessages: document.getElementById('chat-messages'),
            chatHistory: document.getElementById('chat-history'),
            modelSelect: document.getElementById('chat-model-select'),
            currentModelName: document.getElementById('current-model-name'),
            emptyState: document.getElementById('empty-state'),
            modelList: document.getElementById('model-list'),
            modelListSection: document.getElementById('model-list-section'),
            ollamaStatusDot: document.getElementById('ollama-status-dot'),
            ollamaStatusText: document.getElementById('ollama-status-text'),
            cloudflareStatusDot: document.getElementById('cloudflare-status-dot'),
            cloudflareStatusText: document.getElementById('cloudflare-status-text'),
            openrouterApiKey: document.getElementById('openrouter-api-key'),
            openrouterSave: document.getElementById('openrouter-save'),
            openrouterStatus: document.getElementById('openrouter-status')
          }
        }

        // Format timestamp
        const formatTimestamp = (date) => {
          const now = new Date()
          const messageDate = new Date(date)
          const diffMs = now - messageDate
          const diffMins = Math.floor(diffMs / 60000)
          const diffHours = Math.floor(diffMins / 60)
          const diffDays = Math.floor(diffHours / 24)

          if (diffMins < 1) return 'Just now'
          if (diffMins < 60) return \`\${diffMins}m ago\`
          if (diffHours < 24) return \`\${diffHours}h ago\`
          if (diffDays < 7) return \`\${diffDays}d ago\`
          return messageDate.toLocaleDateString()
        }

        // Load conversations into sidebar
        const loadConversations = async () => {
          try {
            const conversations = await chatClient.listConversations()
            
            if (conversations.length === 0) {
              elements.chatHistory.innerHTML = \`
                <div class="history-loading">
                  <p>No conversations yet</p>
                </div>
              \`
              return
            }

            elements.chatHistory.innerHTML = conversations.map(conv => \`
              <div class="conversation-item \${conv.id === currentConversationId ? 'active' : ''}" 
                   data-id="\${conv.id}"
                   onclick="switchConversation('\${conv.id}')">
                <div class="conversation-title">\${conv.title || 'New Conversation'}</div>
                <div class="conversation-meta">
                  \${conv.model ? conv.model.split('/').pop() : 'No model'} • \${formatTimestamp(conv.lastMessageAt || conv.createdAt)}
                </div>
              </div>
            \`).join('')
          } catch (error) {
            console.error('Failed to load conversations:', error)
            elements.chatHistory.innerHTML = \`
              <div class="history-loading">
                <p>Failed to load conversations</p>
              </div>
            \`
          }
        }

        // Switch to a different conversation
        window.switchConversation = async (conversationId) => {
          if (conversationId === currentConversationId) return
          
          // Unsubscribe from previous conversation
          if (messageUnsubscribe) {
            messageUnsubscribe()
            messageUnsubscribe = null
          }
          
          currentConversationId = conversationId
          
          // Load conversation details
          const conversation = await chatClient.getConversation(conversationId)
          if (conversation) {
            // Update model
            if (conversation.model) {
              currentModel = conversation.model
              elements.modelSelect.value = currentModel
              handleModelSelection()
            }
            
            // Load messages
            await loadMessages(conversationId)
            
            // Subscribe to live updates
            messageUnsubscribe = chatClient.subscribeToConversation(conversationId, (messages) => {
              renderMessages(messages)
            })
          }
          
          // Update UI
          loadConversations()
        }

        // Load messages for a conversation
        const loadMessages = async (conversationId) => {
          try {
            const messages = await chatClient.getMessages(conversationId)
            renderMessages(messages)
          } catch (error) {
            console.error('Failed to load messages:', error)
          }
        }

        // Render messages to UI
        const renderMessages = (messages) => {
          elements.chatMessages.innerHTML = ''
          
          if (messages.length === 0) {
            elements.chatMessages.appendChild(elements.emptyState.cloneNode(true))
            return
          }
          
          messages.forEach(msg => {
            if (msg.role !== 'system') {
              addMessageToUI(msg.role, msg.content, false, false)
            }
          })
          
          elements.chatMessages.scrollTop = elements.chatMessages.scrollHeight
        }

        // Format file size
        const formatSize = (bytes) => {
          const gb = bytes / (1024 * 1024 * 1024)
          return gb.toFixed(2) + ' GB'
        }

        // Auto-resize textarea
        const autoResizeTextarea = () => {
          const textarea = elements.chatInput
          if (!textarea) return
          
          textarea.style.height = 'auto'
          textarea.style.height = Math.min(textarea.scrollHeight, 160) + 'px'
        }

        // Toggle sidebar
        const toggleSidebar = () => {
          if (elements.sidebar) {
            elements.sidebar.classList.toggle('collapsed')
          }
        }

        // Open settings modal
        const openSettings = () => {
          if (elements.settingsModal) {
            elements.settingsModal.showModal()
            checkOllamaStatus()
            checkCloudflareStatus()
          }
        }

        // Close settings modal
        const closeSettings = () => {
          if (elements.settingsModal) {
            elements.settingsModal.close()
          }
        }

        // Update model indicator
        const updateModelIndicator = () => {
          if (elements.currentModelName) {
            elements.currentModelName.textContent = currentModel || 'Not selected'
          }
        }

        // Enable/disable chat input
        const updateChatInput = () => {
          if (!elements.chatInput || !elements.chatSend) return
          
          if (currentModel) {
            elements.chatInput.disabled = false
            elements.chatSend.disabled = false
            elements.chatInput.placeholder = 'Type your message...'
            elements.chatInput.focus()
          } else {
            elements.chatInput.disabled = true
            elements.chatSend.disabled = true
            elements.chatInput.placeholder = 'Select a model first...'
          }
        }

        // Update Ollama status
        const updateOllamaStatus = (status) => {
          if (!elements.ollamaStatusDot || !elements.ollamaStatusText) return

          elements.ollamaStatusDot.classList.remove('checking', 'online', 'offline')

          if (status.online) {
            elements.ollamaStatusDot.classList.add('online')
            elements.ollamaStatusText.textContent = 'Online'

            if (status.modelCount > 0 && elements.modelList && elements.modelListSection) {
              elements.modelListSection.style.display = 'block'
              elements.modelList.innerHTML = ''

              // Update dropdown
              if (elements.modelSelect) {
                // Remove existing Ollama optgroup
                const existingOllamaGroup = elements.modelSelect.querySelector('optgroup[label="Ollama"]')
                if (existingOllamaGroup) existingOllamaGroup.remove()
                
                // Ensure default option exists
                if (!elements.modelSelect.querySelector('option[value=""]')) {
                  const defaultOption = document.createElement('option')
                  defaultOption.value = ''
                  defaultOption.textContent = 'Select a model...'
                  elements.modelSelect.insertBefore(defaultOption, elements.modelSelect.firstChild)
                }
                
                // Create Ollama optgroup
                const ollamaGroup = document.createElement('optgroup')
                ollamaGroup.label = 'Ollama'
                
                const savedModel = localStorage.getItem('selectedModel')
                let modelSelected = false

                status.models.forEach(model => {
                  // Add to dropdown
                  const option = document.createElement('option')
                  option.value = model.name
                  option.textContent = model.name
                  
                  if (savedModel && model.name === savedModel && !currentModel) {
                    option.selected = true
                    currentModel = model.name
                    currentProvider = 'ollama'
                    modelSelected = true
                  }
                  
                  ollamaGroup.appendChild(option)

                  // Add to model list
                  const modelItem = document.createElement('div')
                  modelItem.className = 'model-item'

                  const modelName = document.createElement('div')
                  modelName.className = 'model-item-name'
                  modelName.textContent = model.name

                  const modelDetails = document.createElement('div')
                  modelDetails.className = 'model-item-details'
                  const details = []
                  if (model.details?.parameter_size) details.push(model.details.parameter_size)
                  if (model.details?.quantization_level) details.push(model.details.quantization_level)
                  details.push(formatSize(model.size))
                  modelDetails.textContent = details.join(' • ')

                  modelItem.appendChild(modelName)
                  modelItem.appendChild(modelDetails)
                  elements.modelList.appendChild(modelItem)
                })
                
                elements.modelSelect.appendChild(ollamaGroup)

                if (modelSelected) {
                  updateModelIndicator()
                  updateChatInput()
                }
              }
            }
          } else {
            elements.ollamaStatusDot.classList.add('offline')
            elements.ollamaStatusText.textContent = 'Offline'
            if (elements.modelListSection) elements.modelListSection.style.display = 'none'
          }
        }

        // Check Ollama status
        const checkOllamaStatus = async () => {
          if (elements.ollamaStatusDot) {
            elements.ollamaStatusDot.classList.add('checking')
            try {
              const response = await fetch('/api/ollama/status')
              const status = await response.json()
              updateOllamaStatus(status)
            } catch (error) {
              console.error('Failed to check Ollama status:', error)
              updateOllamaStatus({ online: false })
            }
          }
        }

        // Update Cloudflare status
        const updateCloudflareStatus = (online) => {
          if (!elements.cloudflareStatusDot || !elements.cloudflareStatusText) return

          elements.cloudflareStatusDot.classList.remove('checking', 'online', 'offline')
          
          if (online) {
            elements.cloudflareStatusDot.classList.add('online')
            elements.cloudflareStatusText.textContent = 'Available'
            cloudflareAvailable = true
          } else {
            elements.cloudflareStatusDot.classList.add('offline')
            elements.cloudflareStatusText.textContent = 'Not configured'
            cloudflareAvailable = false
          }
        }

        // Check Cloudflare status
        const checkCloudflareStatus = async () => {
          if (elements.cloudflareStatusDot) {
            elements.cloudflareStatusDot.classList.add('checking')
            try {
              const response = await fetch('/api/cloudflare/status')
              const data = await response.json()
              updateCloudflareStatus(data.available)
            } catch (error) {
              console.error('Failed to check Cloudflare status:', error)
              updateCloudflareStatus(false)
            }
          }
        }

        // Update Cloudflare models
        const updateCloudflareModels = () => {
          if (!elements.modelSelect || !cloudflareAvailable) return

          // Remove existing Cloudflare optgroup
          const existingGroup = elements.modelSelect.querySelector('optgroup[label="Cloudflare"]')
          if (existingGroup) existingGroup.remove()

          // Create Cloudflare optgroup
          const cloudflareGroup = document.createElement('optgroup')
          cloudflareGroup.label = 'Cloudflare'

          const cloudflareModels = [
            { value: '@cf/meta/llama-3.1-70b-instruct', name: 'Llama 3.1 70B' },
            { value: '@cf/meta/llama-3.1-8b-instruct', name: 'Llama 3.1 8B' },
            { value: '@cf/meta/llama-3-8b-instruct', name: 'Llama 3 8B' },
            { value: '@cf/meta/llama-2-7b-chat-fp16', name: 'Llama 2 7B' },
            { value: '@hf/thebloke/neural-chat-7b-v3-1-awq', name: 'Neural Chat 7B' },
            { value: '@cf/deepseek-ai/deepseek-math-7b-instruct', name: 'DeepSeek Math 7B' },
            { value: '@cf/deepseek-ai/deepseek-r1-distill-llama-70b', name: 'DeepSeek R1 Distill 70B' }
          ]

          cloudflareModels.forEach(model => {
            const option = document.createElement('option')
            option.value = model.value
            option.textContent = model.name
            cloudflareGroup.appendChild(option)
          })

          elements.modelSelect.appendChild(cloudflareGroup)
        }

        // Save OpenRouter API key
        const saveOpenRouterApiKey = () => {
          const apiKey = elements.openrouterApiKey.value.trim()
          
          if (!apiKey) {
            elements.openrouterStatus.textContent = 'Please enter an API key'
            elements.openrouterStatus.className = 'api-key-status error'
            return
          }
          
          localStorage.setItem('openRouterApiKey', apiKey)
          openRouterApiKey = apiKey
          
          elements.openrouterStatus.textContent = 'API key saved!'
          elements.openrouterStatus.className = 'api-key-status success'
          
          updateOpenRouterModels()
        }

        // Update OpenRouter models
        const updateOpenRouterModels = () => {
          if (!elements.modelSelect || !openRouterApiKey) return

          // Remove existing OpenRouter optgroup
          const existingGroup = elements.modelSelect.querySelector('optgroup[label="OpenRouter"]')
          if (existingGroup) existingGroup.remove()

          // Create OpenRouter optgroup
          const openRouterGroup = document.createElement('optgroup')
          openRouterGroup.label = 'OpenRouter'

          const openRouterModels = [
            { value: 'openrouter/auto', name: 'Auto (Best Available)' },
            { value: 'openai/gpt-4o', name: 'GPT-4o' },
            { value: 'openai/gpt-4o-mini', name: 'GPT-4o Mini' },
            { value: 'anthropic/claude-3.5-sonnet', name: 'Claude 3.5 Sonnet' },
            { value: 'anthropic/claude-3-haiku', name: 'Claude 3 Haiku' },
            { value: 'google/gemini-pro-1.5', name: 'Gemini Pro 1.5' },
            { value: 'meta-llama/llama-3.1-70b-instruct', name: 'Llama 3.1 70B' },
            { value: 'meta-llama/llama-3.1-8b-instruct', name: 'Llama 3.1 8B' },
            { value: 'mistralai/mistral-large', name: 'Mistral Large' },
            { value: 'deepseek/deepseek-chat', name: 'DeepSeek Chat' }
          ]

          openRouterModels.forEach(model => {
            const option = document.createElement('option')
            option.value = model.value
            option.textContent = model.name
            openRouterGroup.appendChild(option)
          })

          elements.modelSelect.appendChild(openRouterGroup)
        }

        // Handle model selection
        const handleModelSelection = async () => {
          const selected = elements.modelSelect.value
          if (!selected) {
            currentModel = ''
            currentProvider = ''
          } else if (selected.startsWith('@cf/')) {
            currentModel = selected
            currentProvider = 'cloudflare'
          } else if (selected.includes('/')) {
            currentModel = selected
            currentProvider = 'openrouter'
          } else {
            currentModel = selected
            currentProvider = 'ollama'
          }
          
          localStorage.setItem('selectedModel', currentModel)
          updateModelIndicator()
          updateChatInput()
          
          // Update current conversation's model if we have one
          if (currentConversationId) {
            await chatClient.updateConversation(currentConversationId, { model: currentModel })
          }
        }

        // Add message to UI
        const addMessageToUI = (role, content, streaming = false, save = true) => {
          const messageDiv = document.createElement('div')
          messageDiv.className = 'chat-message'
          
          const roleDiv = document.createElement('div')
          roleDiv.className = 'message-role'
          roleDiv.textContent = role === 'user' ? 'You' : 'Assistant'
          
          const contentDiv = document.createElement('div')
          contentDiv.className = \`message-content \${streaming ? 'streaming' : ''}\`
          contentDiv.textContent = content
          
          messageDiv.appendChild(roleDiv)
          messageDiv.appendChild(contentDiv)
          elements.chatMessages.appendChild(messageDiv)
          
          elements.chatMessages.scrollTop = elements.chatMessages.scrollHeight
          
          return contentDiv
        }

        // Handle example prompt click
        const handleExamplePrompt = async (prompt) => {
          // Force stop any streaming in progress
          isStreaming = false
          
          // Create new conversation
          const result = await chatClient.startConversation(prompt, currentModel, {
            systemPrompt: 'You are a helpful assistant.'
          })
          
          currentConversationId = result.conversation.id
          
          // Subscribe to live updates
          if (messageUnsubscribe) {
            messageUnsubscribe()
          }
          messageUnsubscribe = chatClient.subscribeToConversation(currentConversationId, (messages) => {
            renderMessages(messages)
          })
          
          // Clear messages and add the user message
          elements.chatMessages.innerHTML = ''
          addMessageToUI('user', prompt, false, false)
          
          // Update conversations list
          await loadConversations()
          
          // Send to AI
          await sendToAI(prompt, result.message.id)
        }

        // Send message to AI
        const sendToAI = async (content, userMessageId) => {
          isStreaming = true
          
          // Add assistant message placeholder
          const assistantDiv = addMessageToUI('assistant', '', true, false)
          
          try {
            let endpoint = ''
            const headers = {
              'Content-Type': 'application/json'
            }
            
            // Determine endpoint and add auth if needed
            if (currentProvider === 'ollama') {
              endpoint = '/api/ollama/chat'
            } else if (currentProvider === 'openrouter') {
              endpoint = '/api/openrouter/chat'
              headers['X-API-Key'] = openRouterApiKey
            } else if (currentProvider === 'cloudflare') {
              endpoint = '/api/cloudflare/chat'
            }
            
            // Get all messages for context
            const messages = await chatClient.getMessages(currentConversationId)
            const chatMessages = messages.map(m => ({ role: m.role, content: m.content }))
            
            const response = await fetch(endpoint, {
              method: 'POST',
              headers,
              body: JSON.stringify({
                model: currentModel,
                messages: chatMessages
              })
            })
            
            if (!response.ok) {
              throw new Error(\`HTTP error! status: \${response.status}\`)
            }
            
            const reader = response.body.getReader()
            const decoder = new TextDecoder()
            let assistantMessage = ''
            
            while (true) {
              const { done, value } = await reader.read()
              if (done) break
              
              const chunk = decoder.decode(value, { stream: true })
              const lines = chunk.split('\\n')
              
              for (const line of lines) {
                if (line.startsWith('data: ')) {
                  const data = line.slice(6)
                  if (data === '[DONE]') {
                    continue
                  }
                  
                  try {
                    const parsed = JSON.parse(data)
                    if (parsed.choices?.[0]?.delta?.content) {
                      assistantMessage += parsed.choices[0].delta.content
                      assistantDiv.textContent = assistantMessage
                      elements.chatMessages.scrollTop = elements.chatMessages.scrollHeight
                    }
                  } catch (e) {
                    console.error('Error parsing chunk:', e)
                  }
                }
              }
            }
            
            // Remove streaming indicator
            assistantDiv.classList.remove('streaming')
            
            // Save assistant message
            await chatClient.sendMessage({
              conversationId: currentConversationId,
              role: 'assistant',
              content: assistantMessage,
              model: currentModel,
              metadata: {}
            })
            
          } catch (error) {
            console.error('Chat error:', error)
            assistantDiv.textContent = 'Error: ' + error.message
            assistantDiv.classList.remove('streaming')
          } finally {
            isStreaming = false
          }
        }

        // Send chat message
        const sendChatMessage = async () => {
          if (isStreaming || !currentModel || !elements.chatInput.value.trim()) return
          
          const message = elements.chatInput.value.trim()
          elements.chatInput.value = ''
          autoResizeTextarea()
          
          // Create conversation if needed
          if (!currentConversationId) {
            const result = await chatClient.startConversation(message, currentModel, {
              systemPrompt: 'You are a helpful assistant.'
            })
            currentConversationId = result.conversation.id
            
            // Subscribe to live updates
            if (messageUnsubscribe) {
              messageUnsubscribe()
            }
            messageUnsubscribe = chatClient.subscribeToConversation(currentConversationId, (messages) => {
              renderMessages(messages)
            })
            
            // Update conversations list
            await loadConversations()
          } else {
            // Save user message
            await chatClient.sendMessage({
              conversationId: currentConversationId,
              role: 'user',
              content: message,
              metadata: {}
            })
          }
          
          // Hide empty state
          if (elements.emptyState && elements.emptyState.style.display !== 'none') {
            elements.emptyState.style.display = 'none'
          }
          
          // Add to UI
          addMessageToUI('user', message, false, false)
          
          // Send to AI
          await sendToAI(message)
        }

        // Initialize event listeners
        const initEventListeners = () => {
          // Sidebar toggles
          if (elements.sidebarToggle) {
            elements.sidebarToggle.addEventListener('click', toggleSidebar)
          }
          if (elements.sidebarToggleMain) {
            elements.sidebarToggleMain.addEventListener('click', toggleSidebar)
          }
          
          // Settings
          if (elements.settingsBtn) {
            elements.settingsBtn.addEventListener('click', openSettings)
          }
          if (elements.closeSettings) {
            elements.closeSettings.addEventListener('click', closeSettings)
          }
          
          // Model selection
          if (elements.modelSelect) {
            elements.modelSelect.addEventListener('change', handleModelSelection)
          }
          
          // OpenRouter API key
          if (elements.openrouterSave) {
            elements.openrouterSave.addEventListener('click', saveOpenRouterApiKey)
          }
          if (elements.openrouterApiKey) {
            elements.openrouterApiKey.addEventListener('keypress', (e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                saveOpenRouterApiKey()
              }
            })
          }
          
          // Chat input
          if (elements.chatInput) {
            elements.chatInput.addEventListener('input', autoResizeTextarea)
            elements.chatInput.addEventListener('keydown', (e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                sendChatMessage()
              }
            })
          }
          
          if (elements.chatSend) {
            elements.chatSend.addEventListener('click', sendChatMessage)
          }
          
          // Example prompts
          const examplePrompts = document.querySelectorAll('.example-prompt')
          examplePrompts.forEach(prompt => {
            prompt.addEventListener('click', () => {
              // Just strip the first 2 characters (emoji and space)
              const text = prompt.textContent.trim().substring(2)
              handleExamplePrompt(text)
            })
          })
          
          // New chat button
          if (elements.newChatBtn) {
            elements.newChatBtn.addEventListener('click', async () => {
              // Clear current conversation
              currentConversationId = null
              if (messageUnsubscribe) {
                messageUnsubscribe()
                messageUnsubscribe = null
              }
              
              // Clear messages
              elements.chatMessages.innerHTML = ''
              
              // Re-add the empty state
              const emptyStateHTML = '<div class="empty-state" id="empty-state">' +
                '<h2>How can I help you today?</h2>' +
                '<div class="example-prompts">' +
                  '<button class="example-prompt" box-="square" variant-="foreground2">' +
                    '💡 Explain quantum computing in simple terms' +
                  '</button>' +
                  '<button class="example-prompt" box-="square" variant-="foreground2">' +
                    '🎨 Help me design a RESTful API' +
                  '</button>' +
                  '<button class="example-prompt" box-="square" variant-="foreground2">' +
                    '📊 Create a Python data analysis script' +
                  '</button>' +
                  '<button class="example-prompt" box-="square" variant-="foreground2">' +
                    '🔧 Debug my JavaScript code' +
                  '</button>' +
                '</div>' +
              '</div>'
              elements.chatMessages.innerHTML = emptyStateHTML
              
              // Re-attach event listeners to new example prompts
              const newPrompts = elements.chatMessages.querySelectorAll('.example-prompt')
              newPrompts.forEach(prompt => {
                prompt.addEventListener('click', () => {
                  // Just strip the first 2 characters (emoji and space)
                  const text = prompt.textContent.trim().substring(2)
                  handleExamplePrompt(text)
                })
              })
              
              // Update elements reference
              elements.emptyState = document.getElementById('empty-state')
              
              updateChatInput()
              loadConversations()
            })
          }
          
          // Close modal on backdrop click
          if (elements.settingsModal) {
            elements.settingsModal.addEventListener('click', (e) => {
              if (e.target === elements.settingsModal) {
                closeSettings()
              }
            })
          }
        }

        // Initialize on page load
        window.addEventListener('DOMContentLoaded', async () => {
          // Reset streaming state on page load
          isStreaming = false
          
          initElements()
          initEventListeners()
          
          // Set default model to Cloudflare Llama
          const defaultModel = '@cf/meta/llama-3.1-70b-instruct'
          const savedModel = localStorage.getItem('selectedModel')
          
          // Use saved model or default
          currentModel = savedModel || defaultModel
          
          // Determine provider based on model format
          if (currentModel.startsWith('@cf/')) {
            currentProvider = 'cloudflare'
          } else if (currentModel.includes('/')) {
            currentProvider = 'openrouter'
          } else {
            currentProvider = 'ollama'
          }
          
          // Update UI to reflect selected model
          if (elements.modelSelect) {
            // Add Cloudflare models first
            updateCloudflareModels()
            elements.modelSelect.value = currentModel
          }
          updateModelIndicator()
          updateChatInput()
          
          // Load saved API key
          if (openRouterApiKey && elements.openrouterApiKey) {
            elements.openrouterApiKey.value = openRouterApiKey
            updateOpenRouterModels()
          }
          
          // Check provider status
          checkOllamaStatus()
          checkCloudflareStatus()
          
          // Auto-check Ollama status periodically
          setInterval(checkOllamaStatus, 30000)
          
          // Load conversations
          await loadConversations()
          
          // If we have conversations, load the most recent one
          const conversations = await chatClient.listConversations()
          if (conversations.length > 0) {
            await switchConversation(conversations[0].id)
          }
        })
      </script>
    `
  })
}
