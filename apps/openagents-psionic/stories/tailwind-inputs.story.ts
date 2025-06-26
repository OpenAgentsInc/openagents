export const title = "Tailwind - Input Fields"
export const component = "OpenAgents v1 Inputs"

export const BasicInput = {
  name: "Basic Input",
  html: `<input type="text" class="oa-input" placeholder="Enter your name">`,
  description: "Basic text input with dark theme"
}

export const InputStates = {
  name: "Input States",
  html: `
    <div class="space-y-4 max-w-sm">
      <input type="text" class="oa-input" placeholder="Normal input">
      <input type="text" class="oa-input oa-input-error" placeholder="Error state">
      <input type="text" class="oa-input oa-input-success" placeholder="Success state">
      <input type="text" class="oa-input" placeholder="Disabled input" disabled>
    </div>
  `,
  description: "Different input states"
}

export const InputSizes = {
  name: "Input Sizes",
  html: `
    <div class="space-y-4 max-w-sm">
      <input type="text" class="oa-input oa-input-sm" placeholder="Small input">
      <input type="text" class="oa-input" placeholder="Default input">
      <input type="text" class="oa-input oa-input-lg" placeholder="Large input">
    </div>
  `,
  description: "Different input sizes"
}

export const InputWithIcons = {
  name: "Input with Icons",
  html: `
    <div class="space-y-4 max-w-sm">
      <div class="oa-input-wrapper">
        <svg class="oa-input-icon-left w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
        </svg>
        <input type="text" class="oa-input has-icon-left" placeholder="Search...">
      </div>
      
      <div class="oa-input-wrapper">
        <input type="email" class="oa-input has-icon-right" placeholder="Email address">
        <svg class="oa-input-icon-right w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
        </svg>
      </div>
    </div>
  `,
  description: "Inputs with left or right icons"
}

export const SearchInput = {
  name: "Search Input",
  html: `
    <div class="max-w-sm">
      <div class="oa-input-wrapper">
        <svg class="oa-input-icon-left w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
        </svg>
        <input type="search" class="oa-input-search has-icon-left" placeholder="Search agents...">
      </div>
    </div>
  `,
  description: "Rounded search input"
}

export const FormLayout = {
  name: "Form Layout",
  html: `
    <form class="space-y-4 max-w-md">
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Username</label>
        <input type="text" class="oa-input" placeholder="Enter username">
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Email</label>
        <input type="email" class="oa-input" placeholder="name@example.com">
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Password</label>
        <input type="password" class="oa-input" placeholder="••••••••">
      </div>
      
      <button type="submit" class="oa-button-primary oa-button-full">
        Create Account
      </button>
    </form>
  `,
  description: "Complete form layout example"
}