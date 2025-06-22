export const title = "Tailwind - Textareas"
export const component = "OpenAgents v1 Textareas"

export const BasicTextarea = {
  name: "Basic Textarea",
  html: `<textarea class="oa-textarea" placeholder="Enter your message..."></textarea>`,
  description: "Basic textarea with resizing"
}

export const TextareaSizes = {
  name: "Textarea Sizes",
  html: `
    <div class="space-y-4 max-w-md">
      <textarea class="oa-textarea oa-textarea-sm" placeholder="Small textarea"></textarea>
      <textarea class="oa-textarea" placeholder="Default textarea"></textarea>
      <textarea class="oa-textarea oa-textarea-lg" placeholder="Large textarea"></textarea>
    </div>
  `,
  description: "Different textarea sizes"
}

export const TextareaVariants = {
  name: "Textarea Variants",
  html: `
    <div class="space-y-4 max-w-md">
      <textarea class="oa-textarea oa-textarea-auto" placeholder="Auto-resizing textarea"></textarea>
      <textarea class="oa-textarea oa-textarea-fixed" rows="4" placeholder="Fixed size textarea"></textarea>
      <textarea class="oa-textarea oa-textarea-error" placeholder="Error state"></textarea>
      <textarea class="oa-textarea oa-textarea-success" placeholder="Success state"></textarea>
    </div>
  `,
  description: "Different textarea variants"
}

export const ChatInput = {
  name: "Chat Input Textarea",
  html: `
    <div class="max-w-2xl">
      <div class="bg-gray-950 border-t border-gray-800 p-4">
        <div class="flex items-end space-x-3">
          <textarea class="oa-textarea-chat" placeholder="Type a message..."></textarea>
          <button class="oa-button-primary oa-button-icon rounded-full">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"></path>
            </svg>
          </button>
        </div>
      </div>
    </div>
  `,
  description: "Chat input with send button"
}

export const FormWithTextarea = {
  name: "Form with Textarea",
  html: `
    <form class="space-y-4 max-w-md">
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Title</label>
        <input type="text" class="oa-input" placeholder="Enter title">
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Description</label>
        <textarea class="oa-textarea" rows="4" placeholder="Enter description..."></textarea>
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-1">Notes</label>
        <textarea class="oa-textarea oa-textarea-sm" rows="3" placeholder="Additional notes..."></textarea>
      </div>
      
      <button type="submit" class="oa-button-primary">Save</button>
    </form>
  `,
  description: "Form with multiple textareas"
}