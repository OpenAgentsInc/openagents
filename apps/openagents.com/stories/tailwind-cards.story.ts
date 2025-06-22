export const title = "Tailwind - Cards & Panes"
export const component = "OpenAgents v1 Cards"

export const BasicCard = {
  name: "Basic Card",
  html: `
    <div class="oa-card">
      <h3 class="text-lg font-semibold text-white mb-2">Card Title</h3>
      <p class="text-gray-400">This is a basic card component with some content inside. Cards are great for grouping related information.</p>
    </div>
  `,
  description: "Basic card container"
}

export const CardWithHeader = {
  name: "Card with Header",
  html: `
    <div class="oa-card">
      <div class="oa-card-header">
        <h3 class="oa-card-title">OpenAgents Stats</h3>
        <p class="oa-card-subtitle">Last updated 5 minutes ago</p>
      </div>
      <div class="oa-card-body">
        <p>Total Agents: 1,234</p>
        <p>Active Users: 5,678</p>
        <p>Messages Today: 45,678</p>
      </div>
    </div>
  `,
  description: "Card with header section"
}

export const CardWithFooter = {
  name: "Card with Footer",
  html: `
    <div class="oa-card">
      <div class="oa-card-body">
        <h3 class="text-lg font-semibold text-white mb-2">Premium Plan</h3>
        <p class="text-gray-400 mb-4">Get unlimited access to all AI models and agents.</p>
        <ul class="space-y-2 text-sm text-gray-300">
          <li>✓ 100 messages per day</li>
          <li>✓ Access to Pro models</li>
          <li>✓ Create custom agents</li>
        </ul>
      </div>
      <div class="oa-card-footer">
        <span class="text-2xl font-bold text-white">$20/mo</span>
        <button class="oa-button-primary">Subscribe</button>
      </div>
    </div>
  `,
  description: "Card with footer actions"
}

export const AgentCard = {
  name: "Agent Card",
  html: `
    <div class="oa-agent-card max-w-xs">
      <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='64' height='64' fill='%234B5563'%3E%3Crect width='64' height='64' rx='8'/%3E%3C/svg%3E" alt="Agent" class="oa-agent-card-image">
      <h3 class="oa-agent-card-name">Code Assistant</h3>
      <p class="oa-agent-card-description">Expert AI assistant for programming tasks, code review, and debugging help.</p>
      <div class="oa-agent-card-meta">
        <span>100 sats/msg</span>
        <span>1.2k users</span>
      </div>
    </div>
  `,
  description: "Agent profile card"
}

export const BasicPane = {
  name: "Basic Pane",
  html: `
    <div class="oa-pane">
      <h3 class="oa-pane-title">Configuration</h3>
      <div class="oa-pane-content">
        <p>This is a pane with a floating title. The title appears on the border, creating a grouped section effect.</p>
      </div>
    </div>
  `,
  description: "Pane with floating title"
}

export const PaneVariants = {
  name: "Pane Variants",
  html: `
    <div class="space-y-6">
      <div class="oa-pane oa-pane-info">
        <h3 class="oa-pane-title">Information</h3>
        <div class="oa-pane-content">
          <p>This is an info pane with blue accent.</p>
        </div>
      </div>
      
      <div class="oa-pane oa-pane-success">
        <h3 class="oa-pane-title">Success</h3>
        <div class="oa-pane-content">
          <p>Operation completed successfully!</p>
        </div>
      </div>
      
      <div class="oa-pane oa-pane-warning">
        <h3 class="oa-pane-title">Warning</h3>
        <div class="oa-pane-content">
          <p>Please review before proceeding.</p>
        </div>
      </div>
      
      <div class="oa-pane oa-pane-error">
        <h3 class="oa-pane-title">Error</h3>
        <div class="oa-pane-content">
          <p>Something went wrong. Please try again.</p>
        </div>
      </div>
    </div>
  `,
  description: "Different pane color variants"
}

export const NestedPanes = {
  name: "Nested Panes",
  html: `
    <div class="oa-pane">
      <h3 class="oa-pane-title">Parent Settings</h3>
      <div class="oa-pane-content">
        <p class="mb-4">Main configuration options</p>
        
        <div class="oa-pane-nested">
          <h4 class="oa-pane-title">Advanced Options</h4>
          <div class="oa-pane-content">
            <p>Nested configuration settings go here.</p>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Pane with nested pane inside"
}