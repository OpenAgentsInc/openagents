export const title = "Tailwind - Select & Dropdowns"
export const component = "OpenAgents v1 Selects"

export const BasicSelect = {
  name: "Basic Select",
  html: `
    <div class="max-w-sm">
      <select class="oa-select">
        <option>Select an option</option>
        <option>Option 1</option>
        <option>Option 2</option>
        <option>Option 3</option>
      </select>
    </div>
  `,
  description: "Native select with custom styling"
}

export const SelectSizes = {
  name: "Select Sizes",
  html: `
    <div class="space-y-4 max-w-sm">
      <select class="oa-select oa-select-sm">
        <option>Small select</option>
        <option>Option 1</option>
      </select>
      
      <select class="oa-select">
        <option>Default select</option>
        <option>Option 1</option>
      </select>
      
      <select class="oa-select oa-select-lg">
        <option>Large select</option>
        <option>Option 1</option>
      </select>
    </div>
  `,
  description: "Different select sizes"
}

export const CustomDropdown = {
  name: "Custom Dropdown",
  html: `
    <div class="max-w-sm">
      <div class="oa-dropdown">
        <button class="oa-dropdown-trigger">
          <span>Select Model</span>
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
          </svg>
        </button>
        <div class="oa-dropdown-menu">
          <div class="oa-dropdown-item">Claude 3.5 Sonnet</div>
          <div class="oa-dropdown-item selected">GPT-4</div>
          <div class="oa-dropdown-item">Llama 3</div>
          <div class="oa-dropdown-item">Mistral</div>
        </div>
      </div>
    </div>
  `,
  description: "Custom dropdown component"
}

export const ModelDropdown = {
  name: "Model Dropdown",
  html: `
    <div class="max-w-sm">
      <div class="oa-model-dropdown open">
        <button class="oa-dropdown-trigger">
          <img class="oa-model-image" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' fill='%233B82F6'%3E%3Crect width='24' height='24' rx='12'/%3E%3C/svg%3E" alt="Model">
          <span>Claude 3.5 Sonnet</span>
          <svg class="w-4 h-4 ml-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
          </svg>
        </button>
        <div class="oa-dropdown-menu">
          <div class="p-2 border-b border-gray-800">
            <p class="text-xs text-gray-500 uppercase px-2">AI Models</p>
          </div>
          <div class="py-1">
            <div class="oa-dropdown-item">
              <img class="oa-dropdown-item-image" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%233B82F6'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="Claude">
              <div>
                <p class="font-medium">Claude 3.5 Sonnet</p>
                <p class="text-xs text-gray-500">Most capable model</p>
              </div>
            </div>
            <div class="oa-dropdown-item selected">
              <img class="oa-dropdown-item-image" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%2310B981'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="GPT">
              <div>
                <p class="font-medium">GPT-4</p>
                <p class="text-xs text-gray-500">OpenAI flagship</p>
              </div>
            </div>
            <div class="oa-dropdown-item">
              <img class="oa-dropdown-item-image" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%23F59E0B'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="Llama">
              <div>
                <p class="font-medium">Llama 3</p>
                <p class="text-xs text-gray-500">Open source</p>
              </div>
            </div>
          </div>
          <div class="p-2 border-t border-gray-800">
            <p class="text-xs text-gray-500 uppercase px-2">Recent Agents</p>
          </div>
          <div class="py-1">
            <div class="oa-dropdown-item">
              <img class="oa-dropdown-item-image" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='32' height='32' fill='%236366F1'%3E%3Crect width='32' height='32' rx='16'/%3E%3C/svg%3E" alt="Agent">
              <div>
                <p class="font-medium">Code Assistant</p>
                <p class="text-xs text-gray-500">100 sats/msg</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Model/Agent selection dropdown"
}

export const FormWithSelects = {
  name: "Form with Selects",
  html: `
    <form class="space-y-4 max-w-md">
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Country</label>
        <select class="oa-select">
          <option>Select country</option>
          <option>United States</option>
          <option>Canada</option>
          <option>United Kingdom</option>
          <option>Germany</option>
          <option>Japan</option>
        </select>
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Language</label>
        <select class="oa-select">
          <option>English</option>
          <option>Spanish</option>
          <option>French</option>
          <option>German</option>
          <option>Japanese</option>
        </select>
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Timezone</label>
        <select class="oa-select oa-select-sm">
          <option>UTC-8 (Pacific Time)</option>
          <option>UTC-5 (Eastern Time)</option>
          <option>UTC+0 (GMT)</option>
          <option>UTC+1 (CET)</option>
          <option>UTC+9 (JST)</option>
        </select>
      </div>
    </form>
  `,
  description: "Form with multiple select fields"
}