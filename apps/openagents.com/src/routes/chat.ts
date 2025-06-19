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
          
          <div class="chat-history">
            <div class="history-placeholder">
              <p>Chat history will appear here</p>
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

        .history-placeholder {
          text-align: center;
          color: var(--foreground0);
          margin-top: 2rem;
          font-size: 0.85em;
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
        }

        .model-bar {
          padding: 0.75rem 1rem;
          border-bottom: 1px solid var(--foreground2);
          display: flex;
          align-items: center;
          gap: 1rem;
          background: var(--background1);
        }

        .sidebar-toggle-main {
          width: 2.5rem;
          height: 2.5rem;
          padding: 0;
          font-size: 1.2em;
          display: none;
        }

        .model-indicator {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          font-size: 0.9em;
        }

        .model-label {
          color: var(--foreground0);
        }

        .model-name {
          color: var(--foreground1);
          font-weight: 500;
        }

        /* Messages Area */
        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 2rem;
          display: flex;
          flex-direction: column;
          gap: 1rem;
          max-width: 48rem;
          margin: 0 auto;
          width: 100%;
        }

        /* Empty State */
        .empty-state {
          margin: auto;
          text-align: center;
          padding: 2rem;
        }

        .empty-state h2 {
          color: var(--foreground1);
          margin-bottom: 2rem;
          font-size: 1.75em;
        }

        .example-prompts {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
          gap: 1rem;
          max-width: 600px;
          margin: 0 auto;
        }

        .example-prompt {
          padding: 1rem;
          text-align: left;
          font-size: 0.9em;
          line-height: 1.4;
          cursor: pointer;
          background: var(--background1);
          border: 1px solid var(--foreground2);
          transition: all 0.2s;
        }

        .example-prompt:hover {
          background: var(--background2);
          border-color: var(--foreground1);
        }

        /* Chat Messages */
        .chat-message {
          display: flex;
          gap: 1rem;
        }

        .message-role {
          font-weight: 600;
          font-size: 0.9em;
          color: var(--foreground1);
          min-width: 80px;
        }

        .message-content {
          flex: 1;
          line-height: 1.6;
          color: var(--foreground1);
        }

        .message-content.streaming::after {
          content: "▋";
          animation: blink 1s infinite;
        }

        @keyframes blink {
          0%, 50% { opacity: 1; }
          51%, 100% { opacity: 0; }
        }

        /* Input Area */
        .chat-input-area {
          padding: 1rem 2rem 2rem;
          max-width: 48rem;
          margin: 0 auto;
          width: 100%;
        }

        .input-wrapper {
          display: flex;
          gap: 0.5rem;
          align-items: flex-end;
        }

        #chat-input {
          flex: 1;
          min-height: 2.5rem;
          max-height: 10rem;
          resize: none;
          font-size: 0.95em;
          line-height: 1.5;
          padding: 0.5rem 0.75rem;
        }

        #chat-send {
          padding: 0.5rem 1.5rem;
          height: 2.5rem;
        }

        /* Settings Modal */
        .settings-modal {
          width: 90%;
          max-width: 600px;
          max-height: 80vh;
          padding: 0;
          border: 1px solid var(--foreground2);
          background: var(--background0);
          margin: auto;
          overflow: hidden;
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
          font-size: 1.25em;
          color: var(--foreground1);
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
          overflow-y: auto;
          max-height: calc(80vh - 4rem);
        }

        .settings-section {
          margin-bottom: 2rem;
        }

        .settings-section h3 {
          margin: 0 0 1rem 0;
          font-size: 1.1em;
          color: var(--foreground1);
        }

        .model-select {
          width: 100%;
          padding: 0.5rem;
          font-size: 0.95em;
        }

        /* Provider Status */
        .provider-status {
          padding: 0.75rem;
          margin-bottom: 0.5rem;
        }

        .provider-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

        .provider-name {
          font-weight: 500;
          color: var(--foreground1);
        }

        /* API Key Section */
        .api-key-section {
          margin-bottom: 1rem;
        }

        .api-key-section label {
          display: block;
          margin-bottom: 0.5rem;
          font-size: 0.9em;
          color: var(--foreground1);
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
          color: var(--foreground0);
        }

        .api-key-status.success {
          color: #10b981;
        }

        .api-key-status.error {
          color: #ef4444;
        }

        /* Status Indicators */
        .status-indicator {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .status-dot {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background-color: var(--foreground2);
          transition: background-color 0.3s;
        }

        .status-dot.checking {
          animation: pulse 1s infinite;
        }

        .status-dot.online {
          background-color: #10b981;
        }

        .status-dot.offline {
          background-color: #ef4444;
        }

        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }

        /* Model List */
        .model-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
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

      <script>
        // Chat state
        let chatMessages = []
        let currentModel = ''
        let currentProvider = 'ollama' // 'ollama', 'openrouter', or 'cloudflare'
        let isStreaming = false
        let openRouterApiKey = localStorage.getItem('openRouterApiKey') || ''
        let cloudflareAvailable = false

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
            
            // Clear empty state if needed
            if (elements.emptyState && elements.emptyState.style.display !== 'none') {
              elements.emptyState.style.display = 'none'
              chatMessages = [{
                role: 'system',
                content: 'You are a helpful assistant.'
              }]
            }
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
              console.error('Error checking Ollama:', error)
              updateOllamaStatus({ online: false })
            }
          }
        }

        // Check Cloudflare status
        const checkCloudflareStatus = async () => {
          if (elements.cloudflareStatusDot) {
            elements.cloudflareStatusDot.classList.add('checking')
            try {
              const response = await fetch('/api/cloudflare/status')
              const status = await response.json()
              updateCloudflareStatus(status)
            } catch (error) {
              console.error('Error checking Cloudflare:', error)
              updateCloudflareStatus({ available: false })
            }
          }
        }

        // Update Cloudflare status
        const updateCloudflareStatus = (status) => {
          if (!elements.cloudflareStatusDot || !elements.cloudflareStatusText) return

          elements.cloudflareStatusDot.classList.remove('checking', 'online', 'offline')

          if (status.available) {
            elements.cloudflareStatusDot.classList.add('online')
            elements.cloudflareStatusText.textContent = 'Configured'
            cloudflareAvailable = true
            updateCloudflareModels()
          } else {
            elements.cloudflareStatusDot.classList.add('offline')
            elements.cloudflareStatusText.textContent = 'Not configured'
            cloudflareAvailable = false
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
            { value: '@cf/meta/llama-3.2-11b-vision-instruct', name: 'Llama 3.2 11B Vision' },
            { value: '@cf/meta/llama-3.2-3b-instruct', name: 'Llama 3.2 3B' },
            { value: '@cf/google/gemma-2-9b-it', name: 'Gemma 2 9B' },
            { value: '@cf/deepseek-ai/deepseek-r1-distill-qwen-32b', name: 'DeepSeek R1 Distill' },
            { value: '@cf/qwen/qwen1.5-7b-chat-awq', name: 'Qwen 1.5 7B Chat' },
            { value: '@cf/microsoft/phi-2', name: 'Phi-2' }
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
          if (!elements.openrouterApiKey || !elements.openrouterStatus) return
          
          const apiKey = elements.openrouterApiKey.value.trim()
          if (!apiKey) {
            elements.openrouterStatus.textContent = 'API key is required'
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
        const handleModelSelection = () => {
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
        }

        // Add message to UI
        const addMessageToUI = (role, content, streaming = false) => {
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
        const handleExamplePrompt = (prompt) => {
          if (!currentModel) {
            openSettings()
            return
          }
          
          elements.chatInput.value = prompt
          autoResizeTextarea()
          elements.chatInput.focus()
        }

        // Send chat message
        const sendChatMessage = async () => {
          if (isStreaming || !currentModel || !elements.chatInput.value.trim()) return
          
          const message = elements.chatInput.value.trim()
          elements.chatInput.value = ''
          autoResizeTextarea()
          
          // Add user message
          chatMessages.push({ role: 'user', content: message })
          addMessageToUI('user', message)
          
          // Add assistant message placeholder
          const assistantDiv = addMessageToUI('assistant', '', true)
          isStreaming = true
          
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
            
            // Add to messages
            chatMessages.push({ role: 'assistant', content: assistantMessage })
            
          } catch (error) {
            console.error('Chat error:', error)
            assistantDiv.textContent = 'Error: ' + error.message
            assistantDiv.classList.remove('streaming')
          } finally {
            isStreaming = false
          }
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
              const text = prompt.textContent.trim().replace(/^[^\s]+ /, '') // Remove emoji
              handleExamplePrompt(text)
            })
          })
          
          // New chat button
          if (elements.newChatBtn) {
            elements.newChatBtn.addEventListener('click', () => {
              chatMessages = [{
                role: 'system',
                content: 'You are a helpful assistant.'
              }]
              elements.chatMessages.innerHTML = ''
              elements.emptyState.style.display = 'flex'
              updateChatInput()
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
        window.addEventListener('DOMContentLoaded', () => {
          initElements()
          initEventListeners()
          
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
        })
      </script>
    `
  })
}
