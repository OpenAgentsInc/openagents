import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../styles"

/*
TypeScript types for JSONL conversation data (documentation only):

interface TextContentPart {
  type: "text"
  text: string
}

interface ThinkingContentPart {
  type: "thinking"
  thinking: string
  signature: string
}

interface ToolUseContentPart {
  type: "tool_use"
  id: string
  name: string
  input: any
}

interface ToolResultContentPart {
  type: "tool_result"
  tool_use_id: string
  content: any
  is_error: boolean
}

interface UserMessage {
  role: "user"
  content: (TextContentPart | ToolResultContentPart)[]
}

interface AssistantMessageUsage {
  input_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
  output_tokens: number
  service_tier: string
}

interface AssistantMessage {
  id: string
  type: "message"
  role: "assistant"
  model: string
  content: (TextContentPart | ThinkingContentPart | ToolUseContentPart)[]
  stop_reason: string | null
  stop_sequence: string | null
  usage: AssistantMessageUsage
}

interface LogEntryBase {
  uuid: string
  timestamp: string
  isSidechain: boolean
  userType: "external"
  cwd: string
  sessionId: string
  version: string
}

interface SummaryEntry {
  type: "summary"
  summary: string
  leafUuid: string
}

interface UserEntry extends LogEntryBase {
  type: "user"
  parentUuid: string | null
  message: UserMessage
  isCompactSummary?: boolean
  toolUseResult?: any
}

interface AssistantEntry extends LogEntryBase {
  type: "assistant"
  parentUuid: string
  message: AssistantMessage
  requestId: string
}

type LogEntry = SummaryEntry | UserEntry | AssistantEntry
*/

// Component styles
const importStyles = css`
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
    --success: #22c55e;
    --error: #ef4444;
    --warning: #f59e0b;
  }

  body {
    background-color: var(--black) !important;
    color: var(--white) !important;
    font-family: "Berkeley Mono", "JetBrains Mono", ui-monospace, monospace !important;
  }

  .import-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
  }

  .drop-zone {
    border: 2px dashed var(--input-border);
    border-radius: 12px;
    padding: 4rem 2rem;
    text-align: center;
    background: rgba(255, 255, 255, 0.02);
    transition: all 0.3s ease;
    cursor: pointer;
  }

  .drop-zone:hover,
  .drop-zone.drag-over {
    border-color: var(--white);
    background: rgba(255, 255, 255, 0.05);
  }

  .drop-zone-icon {
    width: 48px;
    height: 48px;
    margin: 0 auto 1rem;
    color: var(--gray);
  }

  .drop-zone-title {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--white);
    margin-bottom: 0.5rem;
  }

  .drop-zone-subtitle {
    color: var(--gray);
    margin-bottom: 1rem;
  }

  .file-input {
    display: none;
  }

  .browse-button {
    background: var(--white);
    color: var(--black);
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .browse-button:hover {
    background: rgba(255, 255, 255, 0.9);
  }

  .progress-container {
    margin-top: 2rem;
    display: none;
  }

  .progress-bar {
    width: 100%;
    height: 8px;
    background: var(--darkgray);
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--success);
    transition: width 0.3s ease;
    width: 0%;
  }

  .progress-text {
    text-align: center;
    color: var(--gray);
    margin-top: 0.5rem;
    font-size: 0.875rem;
  }

  .conversation-container {
    margin-top: 2rem;
    display: none;
  }

  .conversation-header {
    border-bottom: 1px solid var(--darkgray);
    padding-bottom: 1rem;
    margin-bottom: 2rem;
  }

  .conversation-title {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--white);
    margin-bottom: 0.5rem;
  }

  .conversation-meta {
    color: var(--gray);
    font-size: 0.875rem;
  }

  .message {
    margin-bottom: 2rem;
    padding: 1.5rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
  }

  .message-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
  }

  .message-avatar {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.875rem;
    font-weight: 600;
  }

  .message-avatar.user {
    background: var(--darkgray);
    color: var(--white);
  }

  .message-avatar.assistant {
    background: var(--success);
    color: var(--black);
  }

  .message-info {
    flex: 1;
  }

  .message-author {
    font-weight: 600;
    color: var(--white);
  }

  .message-timestamp {
    color: var(--gray);
    font-size: 0.75rem;
  }

  .message-content {
    color: var(--text);
    line-height: 1.6;
  }

  .message-text {
    white-space: pre-wrap;
    margin-bottom: 1rem;
  }

  .thinking-section {
    background: rgba(139, 69, 19, 0.1);
    border: 1px solid rgba(139, 69, 19, 0.3);
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .thinking-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #deb887;
    font-weight: 600;
    margin-bottom: 0.5rem;
    cursor: pointer;
  }

  .thinking-content {
    color: #f4f4f4;
    font-size: 0.875rem;
    line-height: 1.5;
    white-space: pre-wrap;
    display: none;
  }

  .thinking-content.expanded {
    display: block;
  }

  .tool-call {
    background: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.3);
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .tool-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #93c5fd;
    font-weight: 600;
    margin-bottom: 0.5rem;
    cursor: pointer;
  }

  .tool-name {
    color: var(--white);
    font-weight: 700;
  }

  .tool-content {
    background: var(--offblack);
    border-radius: 4px;
    padding: 1rem;
    font-family: 'Berkeley Mono', monospace;
    font-size: 0.875rem;
    overflow-x: auto;
    display: none;
  }

  .tool-content.expanded {
    display: block;
  }

  .tool-result {
    background: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.3);
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .tool-result.error {
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .expand-icon {
    width: 16px;
    height: 16px;
    transition: transform 0.2s;
  }

  .expand-icon.expanded {
    transform: rotate(90deg);
  }

  .error-message {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 6px;
    padding: 1rem;
    color: #fecaca;
    margin-top: 1rem;
  }

  .summary-section {
    background: rgba(168, 85, 247, 0.1);
    border: 1px solid rgba(168, 85, 247, 0.3);
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  .summary-header {
    color: #c4b5fd;
    font-weight: 600;
    margin-bottom: 0.5rem;
  }

  .summary-content {
    color: var(--text);
    line-height: 1.5;
  }
`

export async function importRoute() {
  return document({
    title: "Import Conversation - OpenAgents",
    styles: baseStyles + importStyles,
    body: html`
      <div class="import-container">
        <h1 style="font-size: 2rem; font-weight: 700; color: var(--white); margin-bottom: 0.5rem;">
          Import Conversation
        </h1>
        <p style="color: var(--gray); margin-bottom: 2rem;">
          Import Claude Code conversation logs from JSONL files to view and analyze your chat history.
        </p>

        <!-- Drop Zone -->
        <div class="drop-zone" id="dropZone">
          <svg class="drop-zone-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"/>
          </svg>
          <div class="drop-zone-title">Drop your JSONL file here</div>
          <div class="drop-zone-subtitle">or click to browse</div>
          <button class="browse-button" onclick="document.getElementById('fileInput').click()">
            Choose File
          </button>
          <input type="file" id="fileInput" class="file-input" accept=".jsonl,.json" />
        </div>

        <!-- Progress Bar -->
        <div class="progress-container" id="progressContainer">
          <div class="progress-bar">
            <div class="progress-fill" id="progressFill"></div>
          </div>
          <div class="progress-text" id="progressText">Processing file...</div>
        </div>

        <!-- Error Message -->
        <div class="error-message" id="errorMessage" style="display: none;"></div>

        <!-- Conversation Display -->
        <div class="conversation-container" id="conversationContainer">
          <div class="conversation-header">
            <div class="conversation-title" id="conversationTitle">Conversation</div>
            <div class="conversation-meta" id="conversationMeta"></div>
          </div>
          <div id="messagesContainer"></div>
        </div>
      </div>

      <script>
        // File upload handling
        const dropZone = document.getElementById('dropZone')
        const fileInput = document.getElementById('fileInput')
        const progressContainer = document.getElementById('progressContainer')
        const progressFill = document.getElementById('progressFill')
        const progressText = document.getElementById('progressText')
        const errorMessage = document.getElementById('errorMessage')
        const conversationContainer = document.getElementById('conversationContainer')
        const conversationTitle = document.getElementById('conversationTitle')
        const conversationMeta = document.getElementById('conversationMeta')
        const messagesContainer = document.getElementById('messagesContainer')

        // Drag and drop events
        dropZone.addEventListener('dragover', (e) => {
          e.preventDefault()
          dropZone.classList.add('drag-over')
        })

        dropZone.addEventListener('dragleave', () => {
          dropZone.classList.remove('drag-over')
        })

        dropZone.addEventListener('drop', (e) => {
          e.preventDefault()
          dropZone.classList.remove('drag-over')
          const files = e.dataTransfer.files
          if (files.length > 0) {
            handleFile(files[0])
          }
        })

        dropZone.addEventListener('click', () => {
          fileInput.click()
        })

        fileInput.addEventListener('change', (e) => {
          if (e.target.files.length > 0) {
            handleFile(e.target.files[0])
          }
        })

        // File processing
        async function handleFile(file) {
          if (!file.name.endsWith('.jsonl') && !file.name.endsWith('.json')) {
            showError('Please select a JSONL or JSON file.')
            return
          }

          showProgress()
          hideError()

          try {
            const text = await file.text()
            const entries = parseJSONL(text)
            displayConversation(entries, file.name)
            hideProgress()
            showConversation()
          } catch (error) {
            hideProgress()
            showError('Error processing file: ' + error.message)
          }
        }

        function parseJSONL(text) {
          const lines = text.trim().split('\\n')
          const entries = []

          for (let i = 0; i < lines.length; i++) {
            const line = lines[i].trim()
            if (line) {
              try {
                const entry = JSON.parse(line)
                entries.push(entry)
              } catch (error) {
                console.warn('Failed to parse line', i + 1, ':', error.message)
              }
            }
          }

          return entries
        }

        function displayConversation(entries, filename) {
          // Separate entries by type
          const summaries = entries.filter(e => e.type === 'summary')
          const userEntries = entries.filter(e => e.type === 'user')
          const assistantEntries = entries.filter(e => e.type === 'assistant')

          // Build conversation metadata
          const totalMessages = userEntries.length + assistantEntries.length
          const sessionIds = [...new Set([...userEntries, ...assistantEntries].map(e => e.sessionId))]
          
          conversationTitle.textContent = filename
          conversationMeta.innerHTML = \`
            <div>\${totalMessages} messages • \${sessionIds.length} session(s)</div>
            \${summaries.length > 0 ? \`<div>\${summaries.length} summary entries</div>\` : ''}
          \`

          // Build message thread
          messagesContainer.innerHTML = ''

          // Show summaries first
          summaries.forEach(summary => {
            const summaryEl = createSummaryElement(summary)
            messagesContainer.appendChild(summaryEl)
          })

          // Build message map by parent relationships
          const messageMap = new Map()
          const allMessages = [...userEntries, ...assistantEntries]
          
          // Sort by timestamp
          allMessages.sort((a, b) => new Date(a.timestamp) - new Date(b.timestamp))

          allMessages.forEach(entry => {
            const messageEl = createMessageElement(entry)
            messagesContainer.appendChild(messageEl)
          })
        }

        function createSummaryElement(summary) {
          const div = document.createElement('div')
          div.className = 'summary-section'
          div.innerHTML = \`
            <div class="summary-header">📝 Conversation Summary</div>
            <div class="summary-content">\${escapeHtml(summary.summary)}</div>
          \`
          return div
        }

        function createMessageElement(entry) {
          const div = document.createElement('div')
          div.className = 'message'
          
          const isUser = entry.type === 'user'
          const timestamp = new Date(entry.timestamp).toLocaleString()
          
          let contentHtml = ''
          
          if (isUser) {
            // Handle user message
            entry.message.content.forEach(part => {
              if (part.type === 'text') {
                contentHtml += \`<div class="message-text">\${escapeHtml(part.text)}</div>\`
              } else if (part.type === 'tool_result') {
                contentHtml += createToolResultHtml(part)
              }
            })
          } else {
            // Handle assistant message
            entry.message.content.forEach(part => {
              if (part.type === 'text') {
                contentHtml += \`<div class="message-text">\${escapeHtml(part.text)}</div>\`
              } else if (part.type === 'thinking') {
                contentHtml += createThinkingHtml(part)
              } else if (part.type === 'tool_use') {
                contentHtml += createToolUseHtml(part)
              }
            })
          }
          
          div.innerHTML = \`
            <div class="message-header">
              <div class="message-avatar \${isUser ? 'user' : 'assistant'}">
                \${isUser ? 'U' : 'A'}
              </div>
              <div class="message-info">
                <div class="message-author">\${isUser ? 'User' : 'Assistant'}</div>
                <div class="message-timestamp">\${timestamp}</div>
              </div>
            </div>
            <div class="message-content">
              \${contentHtml}
            </div>
          \`
          
          return div
        }

        function createThinkingHtml(part) {
          const id = 'thinking-' + Math.random().toString(36).substr(2, 9)
          return \`
            <div class="thinking-section">
              <div class="thinking-header" onclick="toggleExpand('\${id}')">
                <svg class="expand-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
                🤔 Thinking
              </div>
              <div class="thinking-content" id="\${id}">
                \${escapeHtml(part.thinking)}
              </div>
            </div>
          \`
        }

        function createToolUseHtml(part) {
          const id = 'tool-' + Math.random().toString(36).substr(2, 9)
          return \`
            <div class="tool-call">
              <div class="tool-header" onclick="toggleExpand('\${id}')">
                <svg class="expand-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
                🔧 Tool: <span class="tool-name">\${escapeHtml(part.name)}</span>
              </div>
              <div class="tool-content" id="\${id}">
                \${JSON.stringify(part.input, null, 2)}
              </div>
            </div>
          \`
        }

        function createToolResultHtml(part) {
          const id = 'result-' + Math.random().toString(36).substr(2, 9)
          const isError = part.is_error
          return \`
            <div class="tool-result \${isError ? 'error' : ''}">
              <div class="tool-header" onclick="toggleExpand('\${id}')">
                <svg class="expand-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
                \${isError ? '❌' : '✅'} Tool Result \${isError ? '(Error)' : ''}
              </div>
              <div class="tool-content" id="\${id}">
                \${typeof part.content === 'string' ? escapeHtml(part.content) : JSON.stringify(part.content, null, 2)}
              </div>
            </div>
          \`
        }

        function toggleExpand(id) {
          const element = document.getElementById(id)
          const icon = element.previousElementSibling.querySelector('.expand-icon')
          
          element.classList.toggle('expanded')
          icon.classList.toggle('expanded')
        }

        function escapeHtml(text) {
          const div = document.createElement('div')
          div.textContent = text
          return div.innerHTML
        }

        function showProgress() {
          progressContainer.style.display = 'block'
          progressFill.style.width = '0%'
          
          // Simulate progress
          let progress = 0
          const interval = setInterval(() => {
            progress += Math.random() * 30
            if (progress >= 100) {
              progress = 100
              clearInterval(interval)
            }
            progressFill.style.width = progress + '%'
            progressText.textContent = \`Processing file... \${Math.round(progress)}%\`
          }, 100)
        }

        function hideProgress() {
          progressContainer.style.display = 'none'
        }

        function showError(message) {
          errorMessage.textContent = message
          errorMessage.style.display = 'block'
        }

        function hideError() {
          errorMessage.style.display = 'none'
        }

        function showConversation() {
          conversationContainer.style.display = 'block'
        }
      </script>
    `
  })
}
