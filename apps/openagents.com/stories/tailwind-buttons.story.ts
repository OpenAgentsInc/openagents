export const title = "Tailwind - Buttons"
export const component = "OpenAgents v1 Buttons"

export const Primary = {
  name: "Primary Button",
  html: `<button class="oa-button-primary">Primary Action</button>`,
  description: "Primary button for main actions"
}

export const Secondary = {
  name: "Secondary Button",
  html: `<button class="oa-button-secondary">Secondary Action</button>`,
  description: "Secondary button for less important actions"
}

export const Ghost = {
  name: "Ghost Button",
  html: `<button class="oa-button-ghost">Ghost Action</button>`,
  description: "Ghost button with transparent background"
}

export const Danger = {
  name: "Danger Button",
  html: `<button class="oa-button-danger">Delete Account</button>`,
  description: "Danger button for destructive actions"
}

export const Sizes = {
  name: "Button Sizes",
  html: `
    <div class="flex items-center gap-4">
      <button class="oa-button-primary oa-button-sm">Small</button>
      <button class="oa-button-primary">Default</button>
      <button class="oa-button-primary oa-button-lg">Large</button>
    </div>
  `,
  description: "Different button sizes"
}

export const States = {
  name: "Button States",
  html: `
    <div class="flex items-center gap-4">
      <button class="oa-button-primary">Normal</button>
      <button class="oa-button-primary" disabled>Disabled</button>
      <button class="oa-button-primary oa-button-icon">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
        </svg>
      </button>
    </div>
  `,
  description: "Different button states"
}

export const FullWidth = {
  name: "Full Width Button",
  html: `
    <div class="max-w-sm">
      <button class="oa-button-primary oa-button-full">Full Width Button</button>
    </div>
  `,
  description: "Button that spans full container width"
}

export const WithIcons = {
  name: "Buttons with Icons",
  html: `
    <div class="flex items-center gap-4">
      <button class="oa-button-primary">
        <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path>
        </svg>
        Lightning Pay
      </button>
      <button class="oa-button-secondary">
        Settings
        <svg class="w-4 h-4 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"></path>
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path>
        </svg>
      </button>
    </div>
  `,
  description: "Buttons with leading or trailing icons"
}