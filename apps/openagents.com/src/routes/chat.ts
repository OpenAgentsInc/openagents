import { document, html } from "@openagentsinc/psionic"
import { navigation } from "../components/navigation"
import { baseStyles } from "../styles"

export function chat() {
  return document({
    title: "Chat - OpenAgents",
    styles: baseStyles,
    body: html`
      ${navigation({ current: "chat" })}
      
      <div class="container">
        <div class="chat-container">
          <!-- Ollama Status Card -->
          <div class="status-card" box-="square">
            <div class="status-header">
              <h3>Ollama Status</h3>
              <div class="status-indicator">
                <span id="ollama-status-dot" class="status-dot"></span>
                <span id="ollama-status-text" class="status-text">Checking...</span>
              </div>
            </div>
          </div>

          <!-- Models List Card -->
          <div id="model-list-card" class="model-list-card" box-="square" style="display: none;">
            <h3>Available Models</h3>
            <div id="model-list" class="model-list"></div>
          </div>

          <!-- Chat Interface -->
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
        .container {
          max-width: 1200px;
          margin: 0 auto;
          padding: 2rem;
        }

        .chat-container {
          display: grid;
          gap: 1rem;
        }

        .status-card, .model-list-card {
          padding: 1rem;
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
          gap: 0.5rem;
          margin-top: 1rem;
        }

        .model-item {
          padding: 0.75rem;
          background: var(--background1);
          border-radius: 4px;
        }

        .model-name {
          font-weight: bold;
          margin-bottom: 0.25rem;
        }

        .model-details {
          font-size: 0.9em;
          color: var(--foreground0);
        }

        .chat-interface {
          display: flex;
          flex-direction: column;
          height: 600px;
          padding: 1rem;
        }

        .chat-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding-bottom: 1rem;
          border-bottom: 1px solid var(--foreground2);
        }

        .chat-messages {
          flex: 1;
          overflow-y: auto;
          padding: 1rem 0;
          display: flex;
          flex-direction: column;
          gap: 1rem;
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
          padding-top: 1rem;
          border-top: 1px solid var(--foreground2);
        }

        #chat-input {
          flex: 1;
        }

        #chat-model-select {
          min-width: 200px;
        }
      </style>

      <script type="module">
        import { checkOllama, Inference } from '@openagentsinc/sdk'

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
                  if (model.details.parameter_size) details.push(model.details.parameter_size)
                  if (model.details.quantization_level) details.push(model.details.quantization_level)
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
              const status = await checkOllama()
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
            const chatRequest = {
              model: currentModel,
              messages: chatMessages,
              stream: true,
              options: {
                temperature: 0.7,
                num_ctx: 4096
              }
            }
            
            for await (const chunk of Inference.chat(chatRequest)) {
              if (chunk.message && chunk.message.content) {
                assistantContent += chunk.message.content
                assistantDiv.textContent = assistantContent
                
                const messagesContainer = document.getElementById('chat-messages')
                messagesContainer.scrollTop = messagesContainer.scrollHeight
              }
              
              if (chunk.done) {
                assistantDiv.classList.remove('streaming')
                break
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