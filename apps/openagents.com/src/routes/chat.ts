import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function chat() {
  return document({
    title: "Chat - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "chat" })}
      
      <div class="chat-layout">
        <!-- Left Sidebar -->
        <div class="sidebar">
          <!-- Ollama Status Card -->
          <div class="status-card" box-="square">
            <div class="status-header">
              <h4>Ollama Status</h4>
              <div class="status-indicator">
                <span id="ollama-status-dot" class="status-dot"></span>
                <span id="ollama-status-text" class="status-text">Checking...</span>
              </div>
            </div>
          </div>

          <!-- API Keys Card -->
          <div class="api-keys-card" box-="square">
            <h4>API Keys</h4>
            <form class="api-key-section">
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
            </form>

          </div>

          <!-- Models List Card -->
          <div id="model-list-card" class="model-list-card" box-="square" style="display: none;">
            <h4>Available Models</h4>
            <div id="model-list" class="model-list"></div>
          </div>
        </div>

        <!-- Main Chat Area -->
        <div class="main-chat">
          <div class="chat-interface" box-="square">
            <div class="chat-header">
              <h3>Chat Interface</h3>
              <select id="chat-model-select" box-="square">
                <option value="">Select a model...</option>
              </select>
            </div>
            
            <div id="chat-messages" class="chat-messages">
              <div class="empty-state">
                <p>Select a model to start chatting</p>
              </div>
            </div>
            
            <div class="chat-input-container">
              <input 
                type="text" 
                id="chat-input" 
                is-="input" 
                box-="square"
                placeholder="Select a model first..."
                disabled
              />
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
      </div>

      <style>
        .chat-layout {
          display: flex;
          height: calc(100vh - 60px); /* Adjust for compact navigation height */
          gap: 0.5rem;
          padding: 0.5rem;
        }

        .sidebar {
          width: 280px;
          min-width: 280px;
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
          overflow-y: auto;
        }

        .main-chat {
          flex: 1;
          min-width: 0; /* Allow flex item to shrink */
        }

        .status-card, .api-keys-card, .model-list-card {
          padding: 0.75rem;
        }

        .status-card h4, .api-keys-card h4, .model-list-card h4 {
          margin: 0 0 0.5rem 0;
          font-size: 0.85em;
          color: var(--foreground1);
        }

        .api-key-section {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .api-key-section label {
          font-size: 0.8em;
          color: var(--foreground0);
        }

        .api-key-input-wrapper {
          display: flex;
          gap: 0.5rem;
        }

        .api-key-input-wrapper input {
          flex: 1;
          font-size: 0.85em;
        }

        .api-key-input-wrapper button {
          font-size: 0.85em;
          padding: 0.25rem 0.75rem;
        }

        .api-key-status {
          font-size: 0.75em;
          color: var(--foreground0);
          min-height: 1.2em;
        }

        .api-key-status.success {
          color: #10b981;
        }

        .api-key-status.error {
          color: #ef4444;
        }

        .status-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
        }

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

        .model-list {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
          max-height: 400px;
          overflow-y: auto;
        }

        .model-item {
          padding: 0.5rem;
          background: var(--background1);
          border-radius: 4px;
          font-size: 0.85em;
        }

        .model-name {
          font-weight: bold;
          margin-bottom: 0.125rem;
          font-size: 0.9em;
        }

        .model-details {
          font-size: 0.75em;
          color: var(--foreground0);
          line-height: 1.2;
        }

        .chat-interface {
          display: flex;
          flex-direction: column;
          height: 100%;
          padding: 0.75rem;
        }

        .chat-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--foreground2);
        }

        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 0.75rem 0;
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .empty-state {
          text-align: center;
          color: var(--foreground0);
          margin: auto;
        }

        .chat-message {
          display: flex;
          flex-direction: column;
        }

        .message-content {
          padding: 0.75rem 1rem;
          border-radius: 4px;
          max-width: 70%;
          word-wrap: break-word;
        }

        .message-content.user {
          align-self: flex-end;
          background: var(--background2);
          color: var(--foreground2);
        }

        .message-content.assistant {
          align-self: flex-start;
          background: var(--background1);
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
        let cloudflareAvailable = false // Determined by server configuration

        // Format file size
        const formatSize = (bytes) => {
          const gb = bytes / (1024 * 1024 * 1024)
          return gb.toFixed(2) + ' GB'
        }

        // Update Ollama status
        const updateOllamaStatus = (status) => {
          const statusDot = document.getElementById('ollama-status-dot')
          const statusText = document.getElementById('ollama-status-text')
          const modelListCard = document.getElementById('model-list-card')
          const modelList = document.getElementById('model-list')
          const modelDropdown = document.getElementById('chat-model-select')

          if (!statusDot || !statusText) return

          statusDot.classList.remove('checking', 'online', 'offline')

          if (status.online) {
            statusDot.classList.add('online')
            statusText.textContent = 'Online'

            if (status.modelCount > 0 && modelListCard && modelList) {
              modelListCard.style.display = 'block'
              modelList.innerHTML = ''

              // Update dropdown - preserve OpenRouter models
              if (modelDropdown) {
                // Remove existing Ollama optgroup if it exists
                const existingOllamaGroup = modelDropdown.querySelector('optgroup[label="Ollama"]')
                if (existingOllamaGroup) {
                  existingOllamaGroup.remove()
                }
                
                // Ensure we have the default option if dropdown is empty or only has optgroups
                const hasDefaultOption = modelDropdown.querySelector('option[value=""]')
                const hasOnlyOptgroups = modelDropdown.children.length > 0 && 
                  Array.from(modelDropdown.children).every(child => child.tagName === 'OPTGROUP')
                
                if (!hasDefaultOption || hasOnlyOptgroups) {
                  // Remove any existing default option first
                  const existingDefault = modelDropdown.querySelector('option[value=""]')
                  if (existingDefault) existingDefault.remove()
                  
                  const defaultOption = document.createElement('option')
                  defaultOption.value = ''
                  defaultOption.textContent = 'Select a model...'
                  modelDropdown.insertBefore(defaultOption, modelDropdown.firstChild)
                }
                
                // Create Ollama optgroup
                const ollamaGroup = document.createElement('optgroup')
                ollamaGroup.label = 'Ollama'
                
                const savedModel = localStorage.getItem('selectedModel')
                let largestModel = null
                let largestSize = 0

                status.models.forEach(model => {
                  if (model.size > largestSize) {
                    largestSize = model.size
                    largestModel = model
                  }
                })

                let modelSelected = false
                status.models.forEach(model => {
                  // Add to dropdown
                  const option = document.createElement('option')
                  option.value = model.name
                  option.textContent = model.name
                  
                  if (savedModel && model.name === savedModel) {
                    option.selected = true
                    currentModel = model.name
                    modelSelected = true
                  } else if (!savedModel && largestModel && model.name === largestModel.name && !modelSelected) {
                    option.selected = true
                    currentModel = model.name
                    modelSelected = true
                    localStorage.setItem('selectedModel', currentModel)
                  }
                  
                  ollamaGroup.appendChild(option)

                  // Add to list
                  const modelItem = document.createElement('div')
                  modelItem.className = 'model-item'

                  const modelName = document.createElement('div')
                  modelName.className = 'model-name'
                  modelName.textContent = model.name

                  const modelDetails = document.createElement('div')
                  modelDetails.className = 'model-details'
                  const details = []
                  if (model.details?.parameter_size) details.push(model.details.parameter_size)
                  if (model.details?.quantization_level) details.push(model.details.quantization_level)
                  details.push(formatSize(model.size))
                  modelDetails.textContent = details.join(' • ')

                  modelItem.appendChild(modelName)
                  modelItem.appendChild(modelDetails)
                  modelList.appendChild(modelItem)
                })
                
                // Add the Ollama group to dropdown
                modelDropdown.appendChild(ollamaGroup)

                if (modelSelected) {
                  setTimeout(() => enableChatInput(), 100)
                }
              }
            }
          } else {
            statusDot.classList.add('offline')
            statusText.textContent = 'Offline'
            if (modelListCard) modelListCard.style.display = 'none'
          }
        }

        // Check Ollama status
        const checkOllamaStatus = async () => {
          const statusDot = document.getElementById('ollama-status-dot')
          if (statusDot) {
            statusDot.classList.add('checking')
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

        // Enable chat input
        const enableChatInput = () => {
          const input = document.getElementById('chat-input')
          const sendButton = document.getElementById('chat-send')
          const messagesContainer = document.getElementById('chat-messages')
          
          if (!input || !sendButton) return
          
          if (currentModel) {
            input.disabled = false
            sendButton.disabled = false
            input.placeholder = \`Type your message... (\${currentModel})\`
            input.focus()
            
            if (messagesContainer && messagesContainer.querySelector('.empty-state')) {
              messagesContainer.innerHTML = ''
              chatMessages = [{
                role: 'system',
                content: 'You are a helpful assistant. Do not respond in markdown. Use plain text only.'
              }]
            }
          } else {
            input.disabled = true
            sendButton.disabled = true
            input.placeholder = 'Select a model first...'
          }
        }

        // Add message to UI
        const addMessageToUI = (role, content, streaming = false) => {
          const messagesContainer = document.getElementById('chat-messages')
          
          const messageDiv = document.createElement('div')
          messageDiv.className = 'chat-message'
          
          const contentDiv = document.createElement('div')
          contentDiv.className = \`message-content \${role} \${streaming ? 'streaming' : ''}\`
          contentDiv.textContent = content
          
          messageDiv.appendChild(contentDiv)
          messagesContainer.appendChild(messageDiv)
          
          messagesContainer.scrollTop = messagesContainer.scrollHeight
          
          return contentDiv
        }

        // Handle OpenRouter API key
        const saveOpenRouterApiKey = () => {
          const input = document.getElementById('openrouter-api-key')
          const statusDiv = document.getElementById('openrouter-status')
          const dropdown = document.getElementById('chat-model-select')
          
          if (!input || !statusDiv) return
          
          const apiKey = input.value.trim()
          if (!apiKey) {
            statusDiv.textContent = 'API key is required'
            statusDiv.className = 'api-key-status error'
            return
          }
          
          // Save to localStorage
          localStorage.setItem('openRouterApiKey', apiKey)
          openRouterApiKey = apiKey
          
          // Update status
          statusDiv.textContent = 'API key saved!'
          statusDiv.className = 'api-key-status success'
          
          // Add OpenRouter models to dropdown
          if (dropdown && apiKey) {
            // Check if OpenRouter option group already exists
            let openRouterGroup = dropdown.querySelector('optgroup[label="OpenRouter"]')
            if (!openRouterGroup) {
              openRouterGroup = document.createElement('optgroup')
              openRouterGroup.label = 'OpenRouter'
              dropdown.appendChild(openRouterGroup)
            } else {
              openRouterGroup.innerHTML = ''
            }
            
            // Add popular OpenRouter models
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
            
            const savedModel = localStorage.getItem('selectedModel')
            let autoSelected = false
            
            openRouterModels.forEach((model, index) => {
              const option = document.createElement('option')
              option.value = model.value
              option.textContent = model.name
              
              // Auto-select the first model (Auto) if no saved model or if saved model doesn't exist
              if ((index === 0 && !savedModel) || (savedModel === model.value)) {
                option.selected = true
                currentModel = model.value
                autoSelected = true
                localStorage.setItem('selectedModel', currentModel)
              }
              
              openRouterGroup.appendChild(option)
            })
            
            // Enable chat input if auto-selected
            if (autoSelected) {
              setTimeout(() => enableChatInput(), 100)
            }
          }
          
          setTimeout(() => {
            statusDiv.textContent = ''
            statusDiv.className = 'api-key-status'
          }, 3000)
        }

        // Check if Cloudflare is available and add models
        const checkCloudflareAvailability = async () => {
          try {
            const response = await fetch('/api/cloudflare/status')
            const data = await response.json()
            
            if (data.available) {
              cloudflareAvailable = true
              addCloudflareModels()
              updateCloudflareModelsList()
            }
          } catch (error) {
            console.log('Cloudflare not configured on server')
            cloudflareAvailable = false
          }
        }

        // Update the Available Models list with Cloudflare models
        const updateCloudflareModelsList = () => {
          const modelListCard = document.getElementById('model-list-card')
          const modelList = document.getElementById('model-list')
          
          if (!modelListCard || !modelList) return
          
          // Show the model list card
          modelListCard.style.display = 'block'
          
          // Add Cloudflare models to the list
          const cloudflareModels = [
            { value: '@cf/meta/llama-3.1-70b-instruct', name: 'Llama 3.1 70B', size: '70B' },
            { value: '@cf/meta/llama-3.1-8b-instruct', name: 'Llama 3.1 8B', size: '8B' },
            { value: '@cf/meta/llama-3.2-11b-vision-instruct', name: 'Llama 3.2 11B Vision', size: '11B' },
            { value: '@cf/meta/llama-3.2-3b-instruct', name: 'Llama 3.2 3B', size: '3B' },
            { value: '@cf/google/gemma-2-9b-it', name: 'Gemma 2 9B', size: '9B' },
            { value: '@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq', name: 'DeepSeek Coder', size: '6.7B' },
            { value: '@cf/qwen/qwen1.5-7b-chat-awq', name: 'Qwen 1.5 7B Chat', size: '7B' },
            { value: '@cf/microsoft/phi-2', name: 'Phi-2', size: '2.7B' }
          ]
          
          // Add a header for Cloudflare models
          const cloudflareHeader = document.createElement('div')
          cloudflareHeader.style.marginTop = modelList.children.length > 0 ? '1rem' : '0'
          cloudflareHeader.style.marginBottom = '0.5rem'
          cloudflareHeader.style.fontWeight = 'bold'
          cloudflareHeader.style.color = 'var(--foreground0)'
          cloudflareHeader.textContent = 'Cloudflare Models'
          modelList.appendChild(cloudflareHeader)
          
          cloudflareModels.forEach(model => {
            const item = document.createElement('div')
            item.className = 'model-item'
            item.innerHTML = \`
              <div class="model-name">\${model.name}</div>
              <div style="font-size: 0.8em; color: var(--foreground2);">
                \${model.value} • \${model.size}
              </div>
            \`
            modelList.appendChild(item)
          })
        }

        // Add Cloudflare models to dropdown (server-configured)
        const addCloudflareModels = () => {
          const dropdown = document.getElementById('chat-model-select')
          
          if (!dropdown || !cloudflareAvailable) return
          
          // Check if Cloudflare option group already exists
          let cloudflareGroup = dropdown.querySelector('optgroup[label="Cloudflare"]')
          if (!cloudflareGroup) {
            cloudflareGroup = document.createElement('optgroup')
            cloudflareGroup.label = 'Cloudflare'
            dropdown.appendChild(cloudflareGroup)
          } else {
            cloudflareGroup.innerHTML = ''
          }
          
          // Add popular Cloudflare models
          const cloudflareModels = [
            { value: '@cf/meta/llama-3.1-70b-instruct', name: 'Llama 3.1 70B (Recommended)' },
            { value: '@cf/meta/llama-3.1-8b-instruct', name: 'Llama 3.1 8B' },
            { value: '@cf/meta/llama-3.2-11b-vision-instruct', name: 'Llama 3.2 11B Vision' },
            { value: '@cf/meta/llama-3.2-3b-instruct', name: 'Llama 3.2 3B' },
            { value: '@cf/google/gemma-2-9b-it', name: 'Gemma 2 9B' },
            { value: '@cf/deepseek-ai/deepseek-coder-6.7b-instruct-awq', name: 'DeepSeek Coder 6.7B' },
            { value: '@cf/qwen/qwen1.5-7b-chat-awq', name: 'Qwen 1.5 7B Chat' },
            { value: '@cf/microsoft/phi-2', name: 'Phi-2' }
          ]
          
          const savedModel = localStorage.getItem('selectedModel')
          let autoSelected = false
          
          cloudflareModels.forEach((model, index) => {
            const option = document.createElement('option')
            option.value = model.value
            option.textContent = model.name
            
            // Auto-select the first model (Llama 3.1 70B) if no saved model or if saved model doesn't exist
            if ((index === 0 && !savedModel) || (savedModel === model.value)) {
              option.selected = true
              currentModel = model.value
              autoSelected = true
              localStorage.setItem('selectedModel', currentModel)
            }
            
            cloudflareGroup.appendChild(option)
          })
          
          // Enable chat input if auto-selected
          if (autoSelected) {
            setTimeout(() => enableChatInput(), 100)
          }
        }

        // Update send message to handle all providers
        const sendChatMessage = async () => {
          const input = document.getElementById('chat-input')
          const sendButton = document.getElementById('chat-send')
          const message = input.value.trim()
          
          if (!message || !currentModel || isStreaming) return
          
          // Determine provider from model name
          if (currentModel.startsWith('@cf/')) {
            currentProvider = 'cloudflare'
          } else if (currentModel.includes('/')) {
            currentProvider = 'openrouter'
          } else {
            currentProvider = 'ollama'
          }
          
          chatMessages.push({ role: 'user', content: message })
          addMessageToUI('user', message)
          
          input.value = ''
          input.disabled = true
          sendButton.disabled = true
          isStreaming = true
          
          const assistantDiv = addMessageToUI('assistant', '', true)
          let assistantContent = ''
          
          try {
            let endpoint = '/api/ollama/chat'
            if (currentProvider === 'openrouter') {
              endpoint = '/api/openrouter/chat'
            } else if (currentProvider === 'cloudflare') {
              endpoint = '/api/cloudflare/chat'
            }
            
            const headers = {
              'Content-Type': 'application/json'
            }
            
            if (currentProvider === 'openrouter' && openRouterApiKey) {
              headers['X-API-Key'] = openRouterApiKey
            }
            // Cloudflare uses server-side configuration, no client credentials needed
            
            const response = await fetch(endpoint, {
              method: 'POST',
              headers,
              body: JSON.stringify({
                model: currentModel,
                messages: chatMessages,
                options: {
                  temperature: 0.7,
                  num_ctx: 4096
                }
              })
            })
            
            if (!response.ok) {
              throw new Error(\`HTTP error! status: \${response.status}\`)
            }
            
            const reader = response.body.getReader()
            const decoder = new TextDecoder()
            
            while (true) {
              const { done, value } = await reader.read()
              if (done) break
              
              const chunk = decoder.decode(value)
              const lines = chunk.split('\\n')
              
              for (const line of lines) {
                if (line.startsWith('data: ')) {
                  const data = line.slice(6)
                  if (data === '[DONE]') {
                    assistantDiv.classList.remove('streaming')
                    break
                  }
                  
                  try {
                    const parsed = JSON.parse(data)
                    if (parsed.error) {
                      throw new Error(parsed.error)
                    }
                    
                    // Handle Ollama, OpenRouter, and Cloudflare response formats
                    let content = ''
                    if (currentProvider === 'ollama' && parsed.message?.content) {
                      content = parsed.message.content
                    } else if ((currentProvider === 'openrouter' || currentProvider === 'cloudflare') && parsed.choices?.[0]?.delta?.content) {
                      content = parsed.choices[0].delta.content
                    }
                    
                    if (content) {
                      assistantContent += content
                      assistantDiv.textContent = assistantContent
                      
                      const messagesContainer = document.getElementById('chat-messages')
                      messagesContainer.scrollTop = messagesContainer.scrollHeight
                    }
                    
                    if (parsed.done || (parsed.choices?.[0]?.finish_reason)) {
                      assistantDiv.classList.remove('streaming')
                    }
                  } catch (e) {
                    console.error('Error parsing chunk:', e)
                  }
                }
              }
            }
            
            chatMessages.push({ role: 'assistant', content: assistantContent })
            
          } catch (error) {
            console.error('Chat error:', error)
            assistantDiv.textContent = \`Error: \${error.message}\`
            assistantDiv.classList.remove('streaming')
            assistantDiv.style.color = 'var(--danger)'
          } finally {
            isStreaming = false
            input.disabled = false
            sendButton.disabled = false
            input.focus()
          }
        }

        // Initialize
        document.addEventListener('DOMContentLoaded', () => {
          // Load saved OpenRouter API key first
          const apiKeyInput = document.getElementById('openrouter-api-key')
          if (apiKeyInput && openRouterApiKey) {
            apiKeyInput.value = openRouterApiKey
            // Trigger save to populate models immediately
            saveOpenRouterApiKey()
          }
          
          // Check status after a short delay to avoid conflicts
          setTimeout(() => {
            checkOllamaStatus()
            setInterval(checkOllamaStatus, 10000)
          }, 100)
          
          // OpenRouter API key handler
          const saveButton = document.getElementById('openrouter-save')
          if (saveButton) {
            saveButton.addEventListener('click', saveOpenRouterApiKey)
          }
          
          if (apiKeyInput) {
            apiKeyInput.addEventListener('keypress', (e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                saveOpenRouterApiKey()
              }
            })
          }

          // Check if Cloudflare is available on server
          checkCloudflareAvailability()

          // Model dropdown handler
          const modelDropdown = document.getElementById('chat-model-select')
          if (modelDropdown) {
            modelDropdown.addEventListener('change', (e) => {
              currentModel = e.target.value
              localStorage.setItem('selectedModel', currentModel)
              enableChatInput()
            })
          }

          // Send button handler
          const sendButton = document.getElementById('chat-send')
          if (sendButton) {
            sendButton.addEventListener('click', sendChatMessage)
          }

          // Input handler
          const input = document.getElementById('chat-input')
          if (input) {
            input.addEventListener('keypress', (e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                sendChatMessage()
              }
            })
          }
        })
      </script>
    `
  })
}
