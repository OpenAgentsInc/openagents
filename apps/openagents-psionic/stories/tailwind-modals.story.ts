export const title = "Tailwind - Modals"
export const component = "OpenAgents v1 Modals"

export const BasicModal = {
  name: "Basic Modal",
  html: `
    <div class="relative h-96">
      <div class="oa-modal-backdrop show"></div>
      <div class="oa-modal show">
        <div class="oa-modal-dialog">
          <div class="oa-modal-header">
            <h3 class="oa-modal-title">Modal Title</h3>
            <button class="oa-modal-close">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
              </svg>
            </button>
          </div>
          <div class="oa-modal-body">
            <p>This is a basic modal dialog. It can contain any content you need.</p>
          </div>
          <div class="oa-modal-footer">
            <button class="oa-button-secondary">Cancel</button>
            <button class="oa-button-primary">Confirm</button>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Standard modal dialog"
}

export const ModalSizes = {
  name: "Modal Sizes",
  html: `
    <div class="space-y-4">
      <button class="oa-button-secondary">Small Modal (max-w-sm)</button>
      <button class="oa-button-secondary">Default Modal (max-w-lg)</button>
      <button class="oa-button-secondary">Large Modal (max-w-3xl)</button>
      <button class="oa-button-secondary">XL Modal (max-w-5xl)</button>
    </div>
  `,
  description: "Different modal sizes"
}

export const AlertModal = {
  name: "Alert Modal",
  html: `
    <div class="relative h-64">
      <div class="oa-modal-backdrop show"></div>
      <div class="oa-modal oa-modal-alert show">
        <div class="oa-modal-dialog">
          <div class="oa-modal-body">
            <svg class="w-12 h-12 mx-auto mb-4 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path>
            </svg>
            <h3 class="text-lg font-semibold text-white mb-2">Delete Account?</h3>
            <p class="text-gray-400 mb-6">This action cannot be undone. All your data will be permanently deleted.</p>
            <div class="flex justify-center space-x-3">
              <button class="oa-button-secondary">Cancel</button>
              <button class="oa-button-danger">Delete</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Alert/confirmation modal"
}

export const FormModal = {
  name: "Form Modal",
  html: `
    <div class="relative h-96">
      <div class="oa-modal-backdrop show"></div>
      <div class="oa-modal oa-modal-form show">
        <div class="oa-modal-dialog">
          <div class="oa-modal-header">
            <h3 class="oa-modal-title">Create New Agent</h3>
            <button class="oa-modal-close">
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
              </svg>
            </button>
          </div>
          <div class="oa-modal-body">
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Agent Name</label>
              <input type="text" class="oa-input" placeholder="My Assistant">
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Description</label>
              <textarea class="oa-textarea oa-textarea-sm" rows="3" placeholder="What does this agent do?"></textarea>
            </div>
            <div>
              <label class="block text-sm font-medium text-gray-300 mb-1">Base Model</label>
              <select class="oa-select">
                <option>Claude 3.5 Sonnet</option>
                <option>GPT-4</option>
                <option>Llama 3</option>
              </select>
            </div>
          </div>
          <div class="oa-modal-footer">
            <button class="oa-button-ghost">Cancel</button>
            <button class="oa-button-primary">Create Agent</button>
          </div>
        </div>
      </div>
    </div>
  `,
  description: "Modal with form content"
}