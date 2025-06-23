import { html } from "@openagentsinc/psionic"

/**
 * Generate HTML for a chat message
 */
export function renderChatMessage(message: {
  role: "user" | "assistant"
  content: string
  timestamp?: number
  rendered?: string
}) {
  const displayTime = message.timestamp
    ? new Date(message.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : ""

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
        <div class="message-author">
          ${message.role === "user" ? "You" : "Assistant"}
          ${displayTime ? html`<span class="message-time">${displayTime}</span>` : ""}
        </div>
        <div class="message-body">${message.rendered || message.content}</div>
      </div>
    </div>
  `
}

/**
 * Generate HTML for a thread item in the sidebar
 */
export function renderThreadItem(thread: {
  id: string
  title: string
  lastMessageAt?: number
  active?: boolean
}) {
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
        <div class="message-avatar user">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
            <circle cx="12" cy="7" r="4"></circle>
          </svg>
        </div>
        <div class="message-content">
          <div class="message-author">You</div>
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
        <div class="message-avatar assistant">
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-1-13h2v6h-2zm0 8h2v2h-2z"/>
          </svg>
        </div>
        <div class="message-content">
          <div class="message-author">Assistant</div>
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
      const isUser = msg.querySelector('.message-avatar.user');
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
    gap: 12px;
    padding-left: 50px;
    margin-bottom: 24px;
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
  .message-author { 
    font-weight: 600; 
    color: var(--white); 
    margin-bottom: 4px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  
  .message-time {
    font-size: 12px;
    color: var(--gray);
    font-weight: normal;
  }
  
  .message-body { color: var(--text); line-height: 1.6; }

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

  .chat-input:focus {
    border-color: var(--darkgray);
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
`
