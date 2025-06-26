export const title = "Tailwind - File Upload"
export const component = "OpenAgents v1 Upload"

export const FileUploadButton = {
  name: "Upload Button",
  html: `
    <div class="oa-file-upload">
      <input type="file" id="file1" class="oa-file-input" accept="image/*">
      <label for="file1" class="oa-file-upload-button">
        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
        </svg>
        Upload File
      </label>
    </div>
  `,
  description: "Simple file upload button"
}

export const DragDropZone = {
  name: "Drag & Drop Zone",
  html: `
    <div class="oa-file-dropzone">
      <svg class="oa-file-dropzone-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
      </svg>
      <p class="oa-file-dropzone-text">Drop files here or click to upload</p>
      <p class="oa-file-dropzone-hint">PNG, JPG, PDF up to 10MB</p>
      <input type="file" class="oa-file-input" multiple>
    </div>
  `,
  description: "Drag and drop upload area"
}

export const FilePreview = {
  name: "File Preview",
  html: `
    <div>
      <div class="oa-file-dropzone mb-4">
        <svg class="oa-file-dropzone-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"></path>
        </svg>
        <p class="oa-file-dropzone-text">Drop images here</p>
        <p class="oa-file-dropzone-hint">PNG, JPG up to 20MB each</p>
      </div>
      
      <div class="oa-file-preview">
        <div class="oa-file-preview-item">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='96' height='96' fill='%23374151'%3E%3Crect width='96' height='96' rx='8'/%3E%3C/svg%3E" class="oa-file-preview-image" alt="Preview">
          <p class="oa-file-preview-name">image1.png</p>
          <button class="oa-file-preview-remove">×</button>
        </div>
        <div class="oa-file-preview-item">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='96' height='96' fill='%23374151'%3E%3Crect width='96' height='96' rx='8'/%3E%3C/svg%3E" class="oa-file-preview-image" alt="Preview">
          <p class="oa-file-preview-name">screenshot.jpg</p>
          <button class="oa-file-preview-remove">×</button>
        </div>
      </div>
    </div>
  `,
  description: "File upload with preview grid"
}

export const ChatUpload = {
  name: "Chat File Upload",
  html: `
    <div class="bg-gray-950 p-4 rounded-lg">
      <div class="oa-chat-upload-area">
        <div class="oa-chat-upload-item">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='80' height='80' fill='%23374151'%3E%3Crect width='80' height='80' rx='8'/%3E%3C/svg%3E" class="oa-chat-upload-image" alt="Upload">
          <button class="oa-chat-upload-remove">×</button>
        </div>
        <div class="oa-chat-upload-item">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='80' height='80' fill='%23374151'%3E%3Crect width='80' height='80' rx='8'/%3E%3C/svg%3E" class="oa-chat-upload-image" alt="Upload">
          <button class="oa-chat-upload-remove">×</button>
        </div>
        <button class="oa-file-upload-button h-20">
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
          </svg>
        </button>
      </div>
      
      <div class="flex items-end space-x-3 mt-3">
        <textarea class="oa-chat-input" placeholder="Describe these images..."></textarea>
        <button class="oa-chat-send-button">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"></path>
          </svg>
        </button>
      </div>
    </div>
  `,
  description: "Chat interface with file uploads"
}

export const MultipleFileTypes = {
  name: "Multiple File Types",
  html: `
    <form class="space-y-4 max-w-md">
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-2">Profile Picture</label>
        <div class="oa-file-upload">
          <input type="file" id="avatar" class="oa-file-input" accept="image/*">
          <label for="avatar" class="oa-file-upload-button">
            <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"></path>
            </svg>
            Choose Image
          </label>
        </div>
      </div>
      
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-2">Knowledge Base Documents</label>
        <div class="oa-file-dropzone">
          <svg class="oa-file-dropzone-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
          </svg>
          <p class="oa-file-dropzone-text">Drop PDFs, DOCX, TXT files here</p>
          <p class="oa-file-dropzone-hint">Maximum 10 files, 50MB each</p>
          <input type="file" class="oa-file-input" multiple accept=".pdf,.docx,.txt,.md">
        </div>
      </div>
    </form>
  `,
  description: "Different file type uploads"
}