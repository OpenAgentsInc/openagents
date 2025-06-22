export const title = "Tailwind - Utilities"
export const component = "OpenAgents v1 Utilities"

export const Badges = {
  name: "Badges",
  html: `
    <div class="space-y-4">
      <div class="flex items-center gap-2">
        <span class="oa-badge oa-badge-default">Default</span>
        <span class="oa-badge oa-badge-primary">Primary</span>
        <span class="oa-badge oa-badge-success">Success</span>
        <span class="oa-badge oa-badge-warning">Warning</span>
        <span class="oa-badge oa-badge-danger">Danger</span>
        <span class="oa-badge oa-badge-info">Info</span>
      </div>
      
      <div class="flex items-center gap-2">
        <span class="oa-badge oa-badge-sm oa-badge-primary">Small</span>
        <span class="oa-badge oa-badge-primary">Default</span>
        <span class="oa-badge oa-badge-lg oa-badge-primary">Large</span>
      </div>
      
      <div class="flex items-center gap-2">
        <span class="oa-badge oa-badge-primary oa-badge-dot">Online</span>
        <span class="oa-badge oa-badge-success oa-badge-dot">Active</span>
        <span class="oa-badge oa-badge-danger oa-badge-dot">Error</span>
      </div>
      
      <div class="flex items-center gap-2">
        <button class="oa-button-secondary">
          Messages
          <span class="oa-badge oa-badge-count ml-2">12</span>
        </button>
        <span class="oa-badge oa-badge-warning oa-badge-pulse">
          <span class="relative z-10">Live</span>
        </span>
      </div>
    </div>
  `,
  description: "Various badge styles and states"
}

export const Spinners = {
  name: "Loading Spinners",
  html: `
    <div class="space-y-6">
      <div class="flex items-center gap-4">
        <div class="oa-spinner-circle"></div>
        <div class="oa-spinner-circle oa-spinner-sm"></div>
        <div class="oa-spinner-circle oa-spinner-lg"></div>
        <div class="oa-spinner-circle oa-spinner-xl"></div>
      </div>
      
      <div class="flex items-center gap-4">
        <div class="oa-spinner-dots">
          <div class="oa-spinner-dot"></div>
          <div class="oa-spinner-dot"></div>
          <div class="oa-spinner-dot"></div>
        </div>
        
        <div class="oa-spinner-bars">
          <div class="oa-spinner-bar"></div>
          <div class="oa-spinner-bar"></div>
          <div class="oa-spinner-bar"></div>
        </div>
        
        <div class="oa-spinner-pulse"></div>
      </div>
      
      <div class="oa-spinner-text">
        <div class="oa-spinner-circle"></div>
        <span class="oa-spinner-text-label">Loading agents...</span>
      </div>
    </div>
  `,
  description: "Different loading spinner styles"
}

export const Alerts = {
  name: "Alert Messages",
  html: `
    <div class="space-y-4 max-w-2xl">
      <div class="oa-alert oa-alert-info">
        <svg class="oa-alert-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
        </svg>
        <div class="oa-alert-content">
          <h4 class="oa-alert-title">Information</h4>
          <p class="oa-alert-message">This is an informational message with helpful details.</p>
        </div>
        <button class="oa-alert-close">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
          </svg>
        </button>
      </div>
      
      <div class="oa-alert oa-alert-success">
        <svg class="oa-alert-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
        </svg>
        <div class="oa-alert-content">
          <p class="oa-alert-message">Payment completed successfully!</p>
        </div>
      </div>
      
      <div class="oa-alert oa-alert-warning">
        <svg class="oa-alert-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/>
        </svg>
        <div class="oa-alert-content">
          <p class="oa-alert-message">Your balance is running low. Consider adding funds.</p>
        </div>
      </div>
      
      <div class="oa-alert oa-alert-error">
        <svg class="oa-alert-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
        </svg>
        <div class="oa-alert-content">
          <p class="oa-alert-message">Failed to connect to the model. Please try again.</p>
        </div>
      </div>
    </div>
  `,
  description: "Alert notification styles"
}

export const CopyButtons = {
  name: "Copy Buttons",
  html: `
    <div class="space-y-6">
      <div class="flex items-center gap-4">
        <button class="oa-copy-button">
          <svg class="oa-copy-button-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"></path>
          </svg>
          <span>Copy</span>
        </button>
        
        <button class="oa-copy-button copied">
          <svg class="oa-copy-button-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
          </svg>
          <span>Copied!</span>
        </button>
        
        <button class="oa-copy-button-icon-only">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"></path>
          </svg>
        </button>
      </div>
      
      <div class="oa-lightning-address max-w-md">
        <svg class="oa-lightning-address-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M13 2L3 14h9l-1 8 10-12h-9z"/>
        </svg>
        <span class="oa-lightning-address-text">satoshi@openagents.com</span>
        <button class="oa-copy-button-icon-only ml-auto">
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"></path>
          </svg>
        </button>
      </div>
      
      <div class="oa-code-copy max-w-md">
        <pre class="bg-gray-900 p-4 rounded-lg text-sm text-gray-300">lnbc500u1p3q8fznpp5qyur6y...</pre>
        <button class="oa-copy-button oa-copy-button-sm">Copy Invoice</button>
      </div>
    </div>
  `,
  description: "Copy button variations"
}

export const SkeletonLoaders = {
  name: "Skeleton Loaders",
  html: `
    <div class="space-y-6 max-w-md">
      <div class="space-y-3">
        <div class="oa-skeleton-title"></div>
        <div class="oa-skeleton-text"></div>
        <div class="oa-skeleton-text"></div>
        <div class="oa-skeleton-text w-3/4"></div>
      </div>
      
      <div class="flex items-center space-x-3">
        <div class="oa-skeleton-avatar"></div>
        <div class="flex-1 space-y-2">
          <div class="oa-skeleton h-4 w-32"></div>
          <div class="oa-skeleton h-3 w-full"></div>
        </div>
      </div>
      
      <div class="flex space-x-3">
        <div class="oa-skeleton-button"></div>
        <div class="oa-skeleton-button"></div>
      </div>
      
      <div class="oa-card">
        <div class="oa-skeleton h-40 mb-4"></div>
        <div class="oa-skeleton-title mb-2"></div>
        <div class="oa-skeleton-text"></div>
        <div class="oa-skeleton-text"></div>
      </div>
    </div>
  `,
  description: "Skeleton loading states"
}