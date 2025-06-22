export const title = "Tailwind - Headers"
export const component = "OpenAgents v1 Headers"

export const BasicHeader = {
  name: "Basic Header",
  html: `
    <div class="oa-header">
      <div class="oa-header-content">
        <div class="oa-header-brand">
          <svg class="oa-header-logo" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
          </svg>
          <h1 class="oa-header-title">OpenAgents</h1>
        </div>
        
        <nav class="oa-header-nav">
          <a href="#" class="oa-header-nav-link">Dashboard</a>
          <a href="#" class="oa-header-nav-link active">Agents</a>
          <a href="#" class="oa-header-nav-link">Wallet</a>
          <a href="#" class="oa-header-nav-link">Settings</a>
        </nav>
        
        <div class="oa-header-actions">
          <button class="oa-button-ghost oa-button-sm">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"></path>
            </svg>
          </button>
          <button class="oa-button-primary oa-button-sm">
            <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
            </svg>
            New Agent
          </button>
        </div>
        
        <button class="oa-header-mobile-button">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"></path>
          </svg>
        </button>
      </div>
    </div>
  `,
  description: "Main application header"
}

export const HeaderWithSearch = {
  name: "Header with Search",
  html: `
    <div class="oa-header">
      <div class="oa-header-content">
        <div class="oa-header-brand">
          <h1 class="oa-header-title">OpenAgents</h1>
        </div>
        
        <div class="oa-header-search">
          <div class="oa-input-wrapper">
            <svg class="oa-input-icon-left w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
            </svg>
            <input type="search" class="oa-input-search has-icon-left" placeholder="Search agents...">
          </div>
        </div>
        
        <div class="oa-header-actions">
          <div class="oa-balance">
            <svg class="oa-balance-icon-lightning" fill="currentColor" viewBox="0 0 24 24">
              <path d="M13 2L3 14h9l-1 8 10-12h-9z"/>
            </svg>
            <span class="oa-balance-amount">21,000<span class="oa-balance-unit">sats</span></span>
          </div>
          
          <div class="oa-dropdown">
            <button class="oa-dropdown-trigger p-2">
              <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%234B5563'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="User" class="w-8 h-8 rounded-full">
            </button>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Header with search bar and user menu"
}

export const MinimalHeader = {
  name: "Minimal Header",
  html: `
    <div class="oa-header">
      <div class="oa-header-content">
        <button class="oa-button-ghost oa-button-sm oa-button-icon">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
          </svg>
        </button>
        
        <h2 class="text-lg font-medium text-white">Agent Settings</h2>
        
        <button class="oa-button-primary oa-button-sm">Save</button>
      </div>
    </div>
  `,
  description: "Minimal header for sub-pages"
}

export const HeaderWithTabs = {
  name: "Header with Tabs",
  html: `
    <div class="space-y-0">
      <div class="oa-header">
        <div class="oa-header-content">
          <div class="oa-header-brand">
            <h1 class="oa-header-title">Agent Profile</h1>
          </div>
          
          <div class="oa-header-actions">
            <button class="oa-button-ghost oa-button-sm">Edit</button>
            <button class="oa-button-primary oa-button-sm">Chat</button>
          </div>
        </div>
      </div>
      
      <div class="bg-gray-950 border-b border-gray-800">
        <div class="max-w-7xl mx-auto px-4">
          <nav class="flex space-x-8">
            <a href="#" class="py-3 px-1 border-b-2 border-blue-500 text-white text-sm font-medium">Overview</a>
            <a href="#" class="py-3 px-1 border-b-2 border-transparent text-gray-400 hover:text-white text-sm font-medium">Knowledge Base</a>
            <a href="#" class="py-3 px-1 border-b-2 border-transparent text-gray-400 hover:text-white text-sm font-medium">Plugins</a>
            <a href="#" class="py-3 px-1 border-b-2 border-transparent text-gray-400 hover:text-white text-sm font-medium">Analytics</a>
          </nav>
        </div>
      </div>
    </div>
  `,
  description: "Header with navigation tabs"
}

export const TransparentHeader = {
  name: "Transparent Header",
  html: `
    <div class="relative h-96 bg-gradient-to-b from-blue-900 to-gray-900">
      <div class="oa-header oa-header-transparent">
        <div class="oa-header-content">
          <div class="oa-header-brand">
            <h1 class="oa-header-title text-white">OpenAgents</h1>
          </div>
          
          <nav class="oa-header-nav">
            <a href="#" class="oa-header-nav-link text-white/80 hover:text-white">Features</a>
            <a href="#" class="oa-header-nav-link text-white/80 hover:text-white">Pricing</a>
            <a href="#" class="oa-header-nav-link text-white/80 hover:text-white">Docs</a>
          </nav>
          
          <div class="oa-header-actions">
            <button class="oa-button-ghost text-white border-white/20 hover:border-white/40">Sign In</button>
            <button class="oa-button-primary">Get Started</button>
          </div>
        </div>
      </div>
      
      <div class="absolute inset-0 flex items-center justify-center">
        <h2 class="text-4xl font-bold text-white">Hero Content Here</h2>
      </div>
    </div>
  `,
  description: "Transparent header for landing pages"
}