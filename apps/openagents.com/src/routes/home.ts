import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function home() {
  return document({
    title: "OpenAgents - Chat with Bitcoin-Powered AI",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "home" })}
      
      <!-- Welcome Modal -->
      <dialog id="welcome-modal" size-="default" box-="square">
        <div style="padding: 2rem; text-align: center;">
          <h2 style="margin: 0 0 1.5rem 0; color: var(--foreground1);">Welcome to OpenAgents ⚡</h2>
          
          <p style="margin: 0 0 1.5rem 0; color: var(--foreground2); line-height: 1.6;">
            Chat with autonomous AI agents powered by Bitcoin. Each agent must earn to survive, 
            ensuring they provide real value.
          </p>
          
          <div style="background: var(--background1); padding: 1.5rem; margin: 1.5rem 0; border-radius: 4px;">
            <h3 style="margin: 0 0 1rem 0; color: var(--foreground1); font-size: 1rem;">🚀 Quick Start</h3>
            <ul style="margin: 0; padding: 0; list-style: none; text-align: left; color: var(--foreground2);">
              <li style="margin-bottom: 0.5rem;">✓ Select an AI model from the sidebar</li>
              <li style="margin-bottom: 0.5rem;">✓ Start chatting - no API keys needed</li>
              <li style="margin-bottom: 0.5rem;">✓ Agents earn Bitcoin for helpful responses</li>
              <li>✓ Your conversations stay private</li>
            </ul>
          </div>
          
          <div style="display: flex; gap: 1rem; justify-content: center; margin-top: 2rem;">
            <button is-="button" box-="square" variant-="background1" onclick="closeWelcome()">
              Maybe Later
            </button>
            <button is-="button" box-="square" variant-="foreground1" onclick="closeWelcome()">
              Start Chatting
            </button>
          </div>
        </div>
      </dialog>
      
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
              <h3>Chat with OpenAgents</h3>
              <select id="chat-model-select" box-="square">
                <option value="">Select a model...</option>
              </select>
            </div>
            
            <div id="chat-messages" class="chat-messages">
              <div class="empty-state">
                <div style="text-align: center; color: var(--foreground0);">
                  <h3 style="margin-bottom: 1rem;">Ready to Chat! ⚡</h3>
                  <p style="margin-bottom: 1rem;">Select an AI model from the sidebar and start your conversation.</p>
                  <p style="font-size: 0.9rem;">Each agent earns Bitcoin for helpful responses, ensuring quality interactions.</p>
                </div>
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

        .status-card, .model-list-card {
          padding: 0.75rem;
        }

        .status-card h4, .model-list-card h4 {
          margin: 0 0 0.5rem 0;
          font-size: 0.85em;
          color: var(--foreground1);
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
        // Welcome modal logic
        function closeWelcome() {
          document.getElementById('welcome-modal').close()
          localStorage.setItem('openagents-welcomed', 'true')
        }

        // Show welcome modal on first visit
        if (!localStorage.getItem('openagents-welcomed')) {
          setTimeout(() => {
            document.getElementById('welcome-modal').showModal()
          }, 500)
        }

        // Chat state
        let chatMessages = []
        let currentModel = ''
        let isStreaming = false

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

              // Update dropdown
              if (modelDropdown) {
                modelDropdown.innerHTML = '<option value="">Select a model...</option>'
                
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
                  
                  modelDropdown.appendChild(option)

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

        // Send chat message
        const sendChatMessage = async () => {
          const input = document.getElementById('chat-input')
          const sendButton = document.getElementById('chat-send')
          const message = input.value.trim()
          
          if (!message || !currentModel || isStreaming) return
          
          chatMessages.push({ role: 'user', content: message })
          addMessageToUI('user', message)
          
          input.value = ''
          input.disabled = true
          sendButton.disabled = true
          isStreaming = true
          
          const assistantDiv = addMessageToUI('assistant', '', true)
          let assistantContent = ''
          
          try {
            const response = await fetch('/api/ollama/chat', {
              method: 'POST',
              headers: {
                'Content-Type': 'application/json'
              },
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
                    if (parsed.message && parsed.message.content) {
                      assistantContent += parsed.message.content
                      assistantDiv.textContent = assistantContent
                      
                      const messagesContainer = document.getElementById('chat-messages')
                      messagesContainer.scrollTop = messagesContainer.scrollHeight
                    }
                    
                    if (parsed.done) {
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
          // Check status
          checkOllamaStatus()
          setInterval(checkOllamaStatus, 10000)

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
