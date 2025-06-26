export const title = "Tailwind - Chat Interface"
export const component = "OpenAgents v1 Chat"

export const ChatMessage = {
  name: "Chat Messages",
  html: `
    <div class="bg-black p-6 space-y-6">
      <!-- User message -->
      <div class="oa-message">
        <div class="oa-message-avatar">
          <span class="oa-message-avatar-text">U</span>
        </div>
        <div class="oa-message-content">
          <div class="oa-message-header">
            <span class="oa-message-name">User</span>
            <span class="oa-message-time">2 min ago</span>
          </div>
          <div class="oa-message-body oa-message-user">
            <div class="oa-message-text">Can you help me write a Python function to calculate fibonacci numbers?</div>
          </div>
        </div>
      </div>
      
      <!-- Assistant message -->
      <div class="oa-message">
        <div class="oa-message-avatar">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%233B82F6'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="AI">
        </div>
        <div class="oa-message-content">
          <div class="oa-message-header">
            <span class="oa-message-name">Claude</span>
            <span class="oa-message-time">1 min ago</span>
          </div>
          <div class="oa-message-body oa-message-assistant">
            <div class="oa-message-text">I'll help you create a Python function to calculate Fibonacci numbers. Here's an efficient implementation:</div>
            <div class="oa-message-code">def fibonacci(n):
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(2, n + 1):
        a, b = b, a + b
    return b</div>
            <div class="oa-message-text">This function uses iteration rather than recursion, making it more efficient for larger numbers.</div>
          </div>
          <div class="oa-message-actions">
            <button class="oa-message-action">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"></path>
              </svg>
            </button>
            <button class="oa-message-action">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"></path>
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "User and assistant chat messages"
}

export const MessageWithImages = {
  name: "Message with Images",
  html: `
    <div class="bg-black p-6">
      <div class="oa-message">
        <div class="oa-message-avatar">
          <span class="oa-message-avatar-text">U</span>
        </div>
        <div class="oa-message-content">
          <div class="oa-message-header">
            <span class="oa-message-name">User</span>
            <span class="oa-message-time">Just now</span>
          </div>
          <div class="oa-message-body oa-message-user">
            <div class="oa-message-text">What's in these images?</div>
          </div>
          <div class="oa-message-images">
            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='192' height='192' fill='%23374151'%3E%3Crect width='192' height='192' rx='8'/%3E%3C/svg%3E" class="oa-message-image" alt="Upload 1">
            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='192' height='192' fill='%23374151'%3E%3Crect width='192' height='192' rx='8'/%3E%3C/svg%3E" class="oa-message-image" alt="Upload 2">
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Message with uploaded images"
}

export const SystemMessage = {
  name: "System Messages",
  html: `
    <div class="bg-black p-6 space-y-4">
      <div class="oa-message-system">
        Thread started with Claude 3.5 Sonnet
      </div>
      
      <div class="oa-message">
        <div class="oa-message-avatar">
          <span class="oa-message-avatar-text">U</span>
        </div>
        <div class="oa-message-content">
          <div class="oa-message-body oa-message-user">
            <div class="oa-message-text">Hello!</div>
          </div>
        </div>
      </div>
      
      <div class="oa-message-system">
        Model switched to GPT-4
      </div>
    </div>
  `,
  description: "System notification messages"
}

export const ErrorMessage = {
  name: "Error Message",
  html: `
    <div class="bg-black p-6">
      <div class="oa-message">
        <div class="oa-message-avatar">
          <span class="oa-message-avatar-text">!</span>
        </div>
        <div class="oa-message-content">
          <div class="oa-message-error">
            Failed to process request. The model is currently unavailable. Please try again later.
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Error state message"
}

export const LoadingMessage = {
  name: "Loading Message",
  html: `
    <div class="bg-black p-6">
      <div class="oa-message">
        <div class="oa-message-avatar">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%233B82F6'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="AI">
        </div>
        <div class="oa-message-content">
          <div class="oa-message-header">
            <span class="oa-message-name">Claude</span>
          </div>
          <div class="oa-chat-loading">
            <div class="oa-chat-loading-dots">
              <div class="oa-chat-loading-dot"></div>
              <div class="oa-chat-loading-dot"></div>
              <div class="oa-chat-loading-dot"></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "AI typing indicator"
}

export const FullChatInterface = {
  name: "Full Chat Interface",
  html: `
    <div class="oa-chat-container h-96">
      <div class="oa-chat-header">
        <div class="oa-chat-header-title">Chat with Claude</div>
        <div class="oa-chat-header-actions">
          <button class="oa-button-ghost oa-button-sm">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z"></path>
            </svg>
          </button>
        </div>
      </div>
      
      <div class="oa-chat-messages">
        <div class="oa-chat-messages-inner">
          <div class="oa-message-system">New conversation started</div>
        </div>
      </div>
      
      <div class="oa-chat-input-area">
        <div class="oa-chat-input-container">
          <div class="oa-chat-input-wrapper">
            <textarea class="oa-chat-input" placeholder="Type your message..."></textarea>
          </div>
          <button class="oa-chat-send-button">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"></path>
            </svg>
          </button>
        </div>
      </div>
    </div>
  `,
  description: "Complete chat interface layout"
}