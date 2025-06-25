/**
 * Escape HTML to prevent template literal issues
 */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/`/g, "&#96;") // Critical: escape backticks
}

/**
 * Generate HTML for a chat message (using string concatenation instead of template literals)
 */
export function renderChatMessage(message: {
  role: "user" | "assistant"
  content: string
  timestamp?: number
  rendered?: string
  metadata?: any
  [key: string]: any // Allow additional properties for debugging
}) {
  // Use the rendered content if available, otherwise escape the raw content
  let content = message.rendered || escapeHtml(message.content)
  
  // Add tool information if present
  if (message.metadata?.hasEmbeddedTool && message.metadata?.toolName) {
    const toolInput = message.metadata.toolInput ? 
      escapeHtml(JSON.stringify(message.metadata.toolInput, null, 2)) : "";
    
    const toolInfo = 
      "<div class=\"tool-section\" style=\"border-left: 3px solid #a855f7; margin-bottom: 0.5rem;\">" +
      "<div class=\"tool-header\">" +
      "<span style=\"color: #a855f7; margin-right: 0.5rem;\">🔧</span>" +
      "<span class=\"tool-name\" style=\"color: #a855f7; font-weight: 600;\">" + escapeHtml(message.metadata.toolName) + "</span>" +
      "</div>" +
      (toolInput ? "<div class=\"tool-content\" style=\"font-size: 12px; margin-top: 0.25rem; opacity: 0.8;\">" +
        "<pre style=\"margin: 0; font-size: 11px; max-height: 100px; overflow-y: auto;\">" + toolInput + "</pre>" +
      "</div>" : "") +
      "</div>";
    
    // Prepend tool info to content
    content = toolInfo + content
  }
  
  // Create debug object that includes all fields and flattens metadata
  const debugObject = {
    ...message,
    // Flatten metadata fields for easier viewing
    ...(message.metadata ? {
      metadata: message.metadata,
      // Also include flattened metadata fields at top level for visibility
      entryType: message.metadata.entryType,
      toolName: message.metadata.toolName,
      toolInput: message.metadata.toolInput,
      toolUseId: message.metadata.toolUseId,
      toolOutput: message.metadata.toolOutput,
      toolIsError: message.metadata.toolIsError,
      thinking: message.metadata.thinking,
      summary: message.metadata.summary,
      tokenUsage: message.metadata.tokenUsage,
      cost: message.metadata.cost,
      turnCount: message.metadata.turnCount
    } : {})
  }
  
  // Create debug JSON (escaped for HTML)
  const debugJson = escapeHtml(JSON.stringify(debugObject, null, 2))

  return (
    "<div class=\"message\">" +
    "<div class=\"message-block " + message.role + "\">" +
    "<div class=\"message-body\">" + content + "</div>" +
    "<div class=\"message-debug\">" +
    "<details>" +
    "<summary>Debug JSON</summary>" +
    "<pre class=\"debug-json\">" + debugJson + "</pre>" +
    "</details>" +
    "</div>" +
    "</div>" +
    "</div>"
  )
}

/**
 * Generate HTML for a thread item in the sidebar (using string concatenation)
 */
export function renderThreadItem(thread: {
  id: string
  title: string
  lastMessageAt?: number
  active?: boolean
}) {
  const bgClass = thread.active ? "bg-[#262626]" : ""
  const safeTitle = escapeHtml(thread.title)

  return (
    "<div class=\"relative z-[15]\">" +
    "<div class=\"group relative rounded-lg active:opacity-90 px-3 " + bgClass + "\">" +
    "<a href=\"/chat/" + thread.id + "\" class=\"flex items-center gap-2 py-1\">" +
    "<div class=\"relative grow overflow-hidden whitespace-nowrap text-white\">" +
    safeTitle +
    "</div>" +
    "</a>" +
    "</div>" +
    "</div>"
  )
}

/**
 * Shared chat JavaScript that handles message submission and streaming
 * This is injected into the page's script tag
 */
export const chatClientScript = `
  // Initialize chat state
  let isGenerating = false;
  let currentStreamReader = null;
  let currentConversationId = window.CONVERSATION_ID || null;

  // Initialize chat functions directly without importing
  async function initializeChatClient() {
    // Define chat client functions inline for browser use
    window.chatClient = {
      createConversation: async (title) => {
        const response = await fetch('/api/conversations', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title })
        });
        const data = await response.json();
        return data.id;
      },
      
      addMessage: async (conversationId, role, content) => {
        await fetch(\`/api/conversations/\${conversationId}/messages\`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ role, content })
        });
      },
      
      getConversations: async () => {
        const response = await fetch('/api/conversations');
        return response.json();
      },
      
      updateConversationTitle: async (conversationId, title) => {
        await fetch(\`/api/conversations/\${conversationId}\`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title })
        });
      }
    };
    
    // Load conversations for sidebar
    await loadConversations();
  }

  // Load and render conversations in sidebar
  async function loadConversations() {
    try {
      const conversations = await window.chatClient.getConversations();
      const threadList = document.querySelector('#thread-list');
      if (threadList && conversations.length > 0) {
        threadList.innerHTML = conversations.map(conv => \`
          <li>
            <div class="relative z-[15]">
              <div class="group relative rounded-lg active:opacity-90 px-3 \${conv.id === currentConversationId ? 'bg-[#262626]' : ''}">
                <a href="/chat/\${conv.id}" class="flex items-center gap-2 py-1">
                  <div class="relative grow overflow-hidden whitespace-nowrap text-white">
                    \${conv.title}
                  </div>
                </a>
              </div>
            </div>
          </li>
        \`).join('');
      }
    } catch (error) {
      console.error('Failed to load conversations:', error);
    }
  }

  // Handle message submission
  async function handleSubmit() {
    const input = document.getElementById('chat-input');
    const submitButton = document.getElementById('submit-button');
    const message = input.value.trim();
    
    if (!message || isGenerating) return;
    
    isGenerating = true;
    submitButton.disabled = true;
    input.disabled = true;
    
    // Create conversation if needed
    if (!currentConversationId) {
      try {
        // Use first 50 chars of message as title
        const title = message.slice(0, 50) + (message.length > 50 ? '...' : '');
        currentConversationId = await window.chatClient.createConversation(title);
        
        // Update title after a slight delay to ensure it's saved
        setTimeout(() => {
          window.chatClient.updateConversationTitle(currentConversationId, title);
        }, 100);
        
        // Update URL without reload
        window.history.replaceState({}, '', \`/chat/\${currentConversationId}\`);
      } catch (error) {
        console.error('Failed to create conversation:', error);
        isGenerating = false;
        submitButton.disabled = false;
        input.disabled = false;
        return;
      }
    }
    
    // Add user message to UI
    const messagesContainer = document.getElementById('messages-container');
    const messagesDiv = messagesContainer.querySelector('div');
    
    const userMessageHtml = \`
      <div class="message">
        <div class="message-block user">
          <div class="message-body">\${escapeHtml(message)}</div>
        </div>
      </div>
    \`;
    
    messagesDiv.insertAdjacentHTML('beforeend', userMessageHtml);
    
    // Save user message
    try {
      await window.chatClient.addMessage(currentConversationId, 'user', message);
    } catch (error) {
      console.error('Failed to save user message:', error);
    }
    
    // Clear input
    input.value = '';
    autoResize();
    
    // Add assistant message placeholder with unique ID
    const messageId = \`assistant-message-\${Date.now()}\`;
    const assistantMessageHtml = \`
      <div class="message" id="\${messageId}">
        <div class="message-block assistant">
          <div class="message-body assistant-response-body"><div class="dot-flashing"></div></div>
        </div>
      </div>
    \`;
    
    messagesDiv.insertAdjacentHTML('beforeend', assistantMessageHtml);
    messagesContainer.scrollTop = messagesContainer.scrollHeight;
    
    // Build conversation history
    const messages = [];
    
    // Add existing messages from the DOM (excluding loading dots)
    const existingMessages = messagesDiv.querySelectorAll('.message');
    existingMessages.forEach(msg => {
      const isUser = msg.querySelector('.message-block.user');
      const messageBody = msg.querySelector('.message-body');
      const hasLoadingDots = messageBody?.querySelector('.dot-flashing');
      const content = messageBody?.textContent?.trim();
      
      // Only add messages that have content and aren't just loading dots
      if (content && !hasLoadingDots) {
        messages.push({
          role: isUser ? 'user' : 'assistant',
          content: content
        });
      }
    });
    
    // Add the new user message
    messages.push({ role: 'user', content: message });
    
    // Get selected model and provider
    const modelInfo = window.getSelectedModel ? window.getSelectedModel() : { 
      id: '@cf/meta/llama-3.3-70b-instruct-fp8-fast', 
      provider: 'cloudflare' 
    };
    
    // Determine API endpoint based on provider
    const apiEndpoint = modelInfo.provider === 'openrouter' ? '/api/openrouter/chat' : '/api/cloudflare/chat';
    
    // Build headers
    const headers = { 'Content-Type': 'application/json' };
    if (modelInfo.provider === 'openrouter') {
      const openrouterApiKey = localStorage.getItem('openrouterApiKey');
      // Only add header if we have a local key - server might have its own
      if (openrouterApiKey) {
        headers['x-api-key'] = openrouterApiKey;
      }
    }
    
    // Stream response from API
    try {
      const response = await fetch(apiEndpoint, {
        method: 'POST',
        headers: headers,
        body: JSON.stringify({
          messages: messages,
          model: modelInfo.id
        })
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        console.error('API error response:', errorText);
        throw new Error('API request failed');
      }
      
      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      currentStreamReader = reader;
      
      // Find the current assistant message body (the one we just added)
      const currentAssistantMessage = document.getElementById(messageId);
      const assistantResponse = currentAssistantMessage?.querySelector('.assistant-response-body');
      
      if (!assistantResponse) {
        console.error('Could not find assistant response element');
        throw new Error('Failed to find assistant response element');
      }
      
      let fullResponse = '';
      
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        
        const chunk = decoder.decode(value);
        const lines = chunk.split('\\n');
        
        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6);
            if (data === '[DONE]') continue;
            
            try {
              const parsed = JSON.parse(data);
              // Handle OpenAI format from Cloudflare
              if (parsed.choices?.[0]?.delta?.content) {
                fullResponse += parsed.choices[0].delta.content;
                assistantResponse.textContent = fullResponse;
                messagesContainer.scrollTop = messagesContainer.scrollHeight;
              }
              // Handle old format (fallback)
              else if (parsed.response) {
                fullResponse += parsed.response;
                assistantResponse.textContent = fullResponse;
                messagesContainer.scrollTop = messagesContainer.scrollHeight;
              }
            } catch (e) {
              console.error('Failed to parse SSE data:', e);
            }
          }
        }
      }
      
      // Save assistant message
      if (fullResponse) {
        try {
          await window.chatClient.addMessage(currentConversationId, 'assistant', fullResponse);
          
          // Render markdown
          const rendered = await renderMarkdown(fullResponse);
          assistantResponse.innerHTML = rendered;
        } catch (error) {
          console.error('Failed to save assistant message:', error);
        }
      } else {
        // No response received
        assistantResponse.textContent = 'No response received. Please check if Cloudflare API is configured.';
      }
      
      // Reload conversations to update sidebar
      await loadConversations();
      
    } catch (error) {
      console.error('Failed to get response:', error);
      if (assistantResponse) {
        assistantResponse.textContent = 'Sorry, I encountered an error. Please try again.';
      }
    } finally {
      isGenerating = false;
      submitButton.disabled = false;
      input.disabled = false;
      input.focus();
      currentStreamReader = null;
    }
  }

  // Auto-resize textarea
  function autoResize() {
    const textarea = document.getElementById('chat-input');
    textarea.style.height = 'auto';
    textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
  }

  // Escape HTML for security
  function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  // Render markdown with syntax highlighting via API
  async function renderMarkdown(text) {
    try {
      const response = await fetch('/api/markdown', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: text })
      });
      
      if (!response.ok) {
        console.error('Markdown rendering failed');
        return escapeHtml(text);
      }
      
      const data = await response.json();
      return data.html;
    } catch (error) {
      console.error('Failed to render markdown:', error);
      // Fallback to escaped HTML
      return escapeHtml(text);
    }
  }

  // Setup event listeners
  document.addEventListener('DOMContentLoaded', async () => {
    await initializeChatClient();
    
    const input = document.getElementById('chat-input');
    const submitButton = document.getElementById('submit-button');
    
    // Auto-resize on input
    input.addEventListener('input', autoResize);
    
    // Submit on Enter (without Shift)
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    });
    
    // Submit on button click
    submitButton.addEventListener('click', handleSubmit);
    
    // Initial resize
    autoResize();
    input.focus();
  });
`

/**
 * Common chat styles
 */
export const chatStyles = `
  /* Message styling */
  .message {
    display: flex;
    margin-bottom: 1.5rem;
    max-width: 700px;
    margin-left: auto;
    margin-right: auto;
    width: 100%;
  }
  
  .message.user {
    justify-content: flex-end;
  }
  
  .message.assistant,
  .message.tool {
    justify-content: flex-start;
  }
  
  .message-block {
    border-left: 4px solid var(--color-terminal-accent);
    padding: 0.75rem 1rem;
    background: #101010;
    border-radius: 0;
    display: inline-block;
    word-wrap: break-word;
  }
  
  /* User message styling */
  .message-block.user {
    border-left-color: #9ece6a; /* Terminal success green */
  }
  
  /* Assistant message styling */
  .message-block.assistant {
    border-left-color: #7aa2f7; /* Terminal accent blue */
  }
  
  /* Tool message styling */
  .message-block.tool {
    border-left-color: #a855f7; /* Purple for tools */
  }
  
  /* Message header styles - removed as headers are no longer displayed */
  /*
  .message-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }
  
  .message-role {
    font-weight: 600;
    color: var(--white);
    font-size: 14px;
    font-family: var(--font-family-mono);
  }
  
  .message-role.user {
    color: #9ece6a;
  }
  
  .message-role.assistant {
    color: #7aa2f7;
  }
  
  .message-time {
    font-size: 12px;
    color: var(--gray);
    font-weight: normal;
    font-family: var(--font-family-mono);
  }
  */
  
  .message-body {
    color: var(--text);
    line-height: 1.6;
    font-family: var(--font-family-mono);
    font-size: 14px;
    max-width: none;
  }
  
  /* 3-dot loading animation */
  .dot-flashing {
    position: relative;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--white);
    color: var(--white);
    animation: dot-flashing 1s infinite linear alternate;
    animation-delay: 0.5s;
    margin: 0 0 0 14px;
    display: inline-block;
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
    color: var(--white);
    animation: dot-flashing 1s infinite alternate;
    animation-delay: 0s;
  }

  .dot-flashing::after {
    left: 15px;
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background-color: var(--white);
    color: var(--white);
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

  /* Inline code */
  .message-body code {
    background-color: var(--offblack);
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 14px;
  }

  /* Code blocks - WebTUI pre with Shiki highlighting */
  .message-body pre[is-="pre"] {
    margin: 16px 0;
    overflow-x: auto;
  }

  .message-body pre[is-="pre"] code { 
    background: none; 
    padding: 0; 
    font-size: 14px;
    line-height: 1.5;
  }

  /* Markdown content styling */
  .message-body p { margin: 0 0 16px 0; }
  .message-body p:last-child { margin-bottom: 0; }
  
  .message-body ul, .message-body ol { 
    margin: 0 0 16px 0; 
    padding-left: 24px;
  }
  
  .message-body li { margin: 4px 0; }
  
  .message-body blockquote {
    border-left: 4px solid var(--darkgray);
    padding-left: 16px;
    margin: 16px 0;
    color: var(--gray);
  }
  
  .message-body h1, .message-body h2, .message-body h3, 
  .message-body h4, .message-body h5, .message-body h6 {
    margin: 24px 0 16px 0;
    font-weight: 600;
  }
  
  .message-body h1 { font-size: 1.5em; }
  .message-body h2 { font-size: 1.3em; }
  .message-body h3 { font-size: 1.1em; }
  
  .message-body a {
    color: var(--white);
    text-decoration: underline;
  }
  
  .message-body a:hover {
    color: var(--gray);
  }

  /* Chat input */
  .chat-input {
    background-color: transparent;
    border: 1px solid var(--offblack);
    border-radius: 6px;
    color: var(--white);
    font-family: inherit;
    font-size: 14px;
    line-height: 1.5;
    padding: 12px 50px 12px 16px;
    resize: none;
    width: 100%;
    outline: none;
    transition: border-color 0.2s;
  }
  
  .chat-input:focus-visible {
    outline: none;
    box-shadow: none !important;
  }

  .chat-input:focus {
    border-color: var(--white);
    outline: none !important;
    box-shadow: none !important;
    --tw-ring-color: transparent !important;
    --tw-ring-offset-shadow: 0 0 #0000 !important;
    --tw-ring-shadow: 0 0 #0000 !important;
  }

  .chat-input::placeholder {
    color: var(--gray);
    opacity: 0.6;
  }

  /* Submit button */
  .submit-button {
    position: absolute;
    right: 8px;
    bottom: 8px;
    background: none;
    border: none;
    color: var(--gray);
    cursor: pointer;
    padding: 6px;
    border-radius: 4px;
    transition: color 0.2s, background-color 0.2s;
  }

  .submit-button:hover:not(:disabled) {
    color: var(--white);
    background-color: var(--offblack);
  }

  .submit-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Debug section styling */
  .message-debug {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--offblack);
  }

  .message-debug details {
    margin: 0;
  }

  .message-debug summary {
    color: var(--gray);
    font-size: 11px;
    font-family: var(--font-family-mono);
    cursor: pointer;
    user-select: none;
    padding: 2px 0;
  }

  .message-debug summary:hover {
    color: var(--white);
  }

  .debug-json {
    background-color: var(--black);
    border: 1px solid var(--offblack);
    border-radius: 4px;
    padding: 8px;
    margin: 4px 0 0 0;
    font-size: 10px;
    font-family: var(--font-family-mono);
    color: var(--gray);
    overflow-x: auto;
    white-space: pre;
    max-height: 200px;
    overflow-y: auto;
  }
`
