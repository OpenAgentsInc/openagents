export const title = "Tailwind Components"
export const component = "OpenAgents v1 Component Library"

export const Overview = {
  name: "Component Library Overview",
  html: `
    <div class="max-w-4xl mx-auto p-8 space-y-8">
      <div>
        <h1 class="text-3xl font-bold text-white mb-4">OpenAgents v1 Component Library</h1>
        <p class="text-gray-400 text-lg">
          A comprehensive collection of dark-themed UI components migrated from the OpenAgents v1 Laravel/Blade application.
          These components are built with pure HTML and Tailwind CSS, requiring no JavaScript frameworks.
        </p>
      </div>
      
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Forms</h3>
          <p class="text-gray-400 text-sm mb-3">Buttons, inputs, textareas, selects, and file uploads</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-primary">5 components</span>
          </div>
        </div>
        
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Layout</h3>
          <p class="text-gray-400 text-sm mb-3">Cards, panes, modals, and headers</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-primary">4 components</span>
          </div>
        </div>
        
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Chat</h3>
          <p class="text-gray-400 text-sm mb-3">Messages, threads, and chat interface</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-primary">3 components</span>
          </div>
        </div>
        
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Payment</h3>
          <p class="text-gray-400 text-sm mb-3">Lightning invoices, balances, and transactions</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-warning">3 components</span>
            <span class="oa-badge oa-badge-default">Bitcoin/Lightning</span>
          </div>
        </div>
        
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Utilities</h3>
          <p class="text-gray-400 text-sm mb-3">Badges, spinners, alerts, and copy buttons</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-primary">5 components</span>
          </div>
        </div>
        
        <div class="oa-card">
          <h3 class="text-lg font-semibold text-white mb-2">Design System</h3>
          <p class="text-gray-400 text-sm mb-3">Dark theme with consistent colors and spacing</p>
          <div class="flex flex-wrap gap-2">
            <span class="oa-badge oa-badge-success">Ready to use</span>
          </div>
        </div>
      </div>
      
      <div class="oa-pane">
        <h3 class="oa-pane-title">Usage</h3>
        <div class="oa-pane-content space-y-4">
          <p>Import the Tailwind components CSS in your project:</p>
          <div class="bg-gray-900 p-3 rounded font-mono text-sm">
            <span class="text-gray-500">@import</span> <span class="text-green-400">"@openagentsinc/ui/tailwind"</span><span class="text-gray-500">;</span>
          </div>
          <p class="text-sm text-gray-500">
            All components use the <code class="text-orange-400">oa-</code> prefix to avoid conflicts with other CSS frameworks.
          </p>
        </div>
      </div>
    </div>
  `,
  description: "Overview of the OpenAgents v1 component library"
}

export const ColorPalette = {
  name: "Color Palette",
  html: `
    <div class="max-w-3xl space-y-6">
      <h3 class="text-xl font-semibold text-white mb-4">Dark Theme Color System</h3>
      
      <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div>
          <div class="h-20 bg-black rounded-lg mb-2 border border-gray-800"></div>
          <p class="text-sm font-mono text-gray-400">black</p>
          <p class="text-xs text-gray-500">#000000</p>
        </div>
        
        <div>
          <div class="h-20 rounded-lg mb-2 border border-gray-800" style="background: #0a0a0a"></div>
          <p class="text-sm font-mono text-gray-400">offblack</p>
          <p class="text-xs text-gray-500">#0a0a0a</p>
        </div>
        
        <div>
          <div class="h-20 rounded-lg mb-2 border border-gray-800" style="background: #333333"></div>
          <p class="text-sm font-mono text-gray-400">darkgray</p>
          <p class="text-xs text-gray-500">#333333</p>
        </div>
        
        <div>
          <div class="h-20 rounded-lg mb-2 border border-gray-800" style="background: #666666"></div>
          <p class="text-sm font-mono text-gray-400">gray</p>
          <p class="text-xs text-gray-500">#666666</p>
        </div>
        
        <div>
          <div class="h-20 rounded-lg mb-2" style="background: #999999"></div>
          <p class="text-sm font-mono text-gray-400">lightgray</p>
          <p class="text-xs text-gray-500">#999999</p>
        </div>
        
        <div>
          <div class="h-20 bg-white rounded-lg mb-2"></div>
          <p class="text-sm font-mono text-gray-400">white</p>
          <p class="text-xs text-gray-500">#ffffff</p>
        </div>
        
        <div>
          <div class="h-20 rounded-lg mb-2" style="background: #e5e5e5"></div>
          <p class="text-sm font-mono text-gray-400">text</p>
          <p class="text-xs text-gray-500">#e5e5e5</p>
        </div>
      </div>
      
      <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mt-8">
        <div>
          <div class="h-20 bg-blue-600 rounded-lg mb-2"></div>
          <p class="text-sm font-mono text-gray-400">primary</p>
          <p class="text-xs text-gray-500">blue-600</p>
        </div>
        
        <div>
          <div class="h-20 bg-green-600 rounded-lg mb-2"></div>
          <p class="text-sm font-mono text-gray-400">success</p>
          <p class="text-xs text-gray-500">green-600</p>
        </div>
        
        <div>
          <div class="h-20 bg-yellow-600 rounded-lg mb-2"></div>
          <p class="text-sm font-mono text-gray-400">warning</p>
          <p class="text-xs text-gray-500">yellow-600</p>
        </div>
        
        <div>
          <div class="h-20 bg-red-600 rounded-lg mb-2"></div>
          <p class="text-sm font-mono text-gray-400">danger</p>
          <p class="text-xs text-gray-500">red-600</p>
        </div>
      </div>
    </div>
  `,
  description: "Color palette used throughout the components"
}