export const title = "Tailwind - Thread List"
export const component = "OpenAgents v1 Threads"

export const ThreadItem = {
  name: "Thread Item",
  html: `
    <div class="bg-gray-950 w-80">
      <div class="oa-thread-item">
        <div class="oa-thread-content">
          <h4 class="oa-thread-title">Python Fibonacci Function</h4>
          <p class="oa-thread-preview">Can you help me write a Python function to...</p>
          <div class="oa-thread-meta">
            <span class="oa-thread-time">2 min ago</span>
            <span class="oa-thread-badge">Claude</span>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Single thread item"
}

export const ActiveThread = {
  name: "Active Thread",
  html: `
    <div class="bg-gray-950 w-80">
      <div class="oa-thread-item active">
        <div class="oa-thread-content">
          <h4 class="oa-thread-title">React Component Help</h4>
          <p class="oa-thread-preview">I need to create a modal component that...</p>
          <div class="oa-thread-meta">
            <span class="oa-thread-time">Just now</span>
            <span class="oa-thread-badge">GPT-4</span>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Currently active thread"
}

export const ThreadWithAvatar = {
  name: "Thread with Avatar",
  html: `
    <div class="bg-gray-950 w-80">
      <div class="oa-thread-item">
        <div class="oa-thread-item-with-avatar">
          <img class="oa-thread-avatar" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%234B5563'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="Agent">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Code Review Assistant</h4>
            <p class="oa-thread-preview">Please review this TypeScript code...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">1 hour ago</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Thread with agent avatar"
}

export const ThreadList = {
  name: "Complete Thread List",
  html: `
    <div class="oa-thread-list h-96">
      <div class="oa-thread-list-header">
        <h3 class="oa-thread-list-title">Conversations</h3>
        <button class="oa-button-ghost oa-button-sm oa-button-icon">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
          </svg>
        </button>
      </div>
      
      <button class="oa-thread-new">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
        </svg>
        <span>New Thread</span>
      </button>
      
      <div class="oa-thread-items">
        <div class="oa-thread-item active">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Current Conversation</h4>
            <p class="oa-thread-preview">Working on the UI components...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">Just now</span>
            </div>
          </div>
        </div>
        
        <div class="oa-thread-item">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">API Integration Help</h4>
            <p class="oa-thread-preview">How do I connect to the Lightning...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">5 min ago</span>
            </div>
          </div>
        </div>
        
        <div class="oa-thread-item">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Database Schema Design</h4>
            <p class="oa-thread-preview">I need help designing a schema for...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">2 hours ago</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Full thread list sidebar"
}

export const GroupedThreads = {
  name: "Grouped Threads",
  html: `
    <div class="bg-gray-950 w-80">
      <div class="oa-thread-group">
        <h3 class="oa-thread-group-title">Today</h3>
        <div class="oa-thread-item">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Morning standup notes</h4>
            <p class="oa-thread-preview">Team updates and blockers...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">9:00 AM</span>
            </div>
          </div>
        </div>
        <div class="oa-thread-item">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Bug fix discussion</h4>
            <p class="oa-thread-preview">The issue with user auth...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">11:30 AM</span>
            </div>
          </div>
        </div>
      </div>
      
      <div class="oa-thread-group">
        <h3 class="oa-thread-group-title">Yesterday</h3>
        <div class="oa-thread-item">
          <div class="oa-thread-content">
            <h4 class="oa-thread-title">Code review</h4>
            <p class="oa-thread-preview">Please check the PR for...</p>
            <div class="oa-thread-meta">
              <span class="oa-thread-time">4:45 PM</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Threads grouped by date"
}

export const EmptyThreadList = {
  name: "Empty Thread List",
  html: `
    <div class="bg-gray-950 w-80 p-4">
      <div class="oa-thread-empty">
        <svg class="w-12 h-12 mx-auto mb-3 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"></path>
        </svg>
        <p class="oa-thread-empty-text">No conversations yet</p>
        <button class="oa-button-primary oa-button-sm mt-3">Start New Chat</button>
      </div>
    </div>
  `,
  description: "Empty state for thread list"
}