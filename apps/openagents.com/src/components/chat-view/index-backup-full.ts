import { css } from "@openagentsinc/psionic"
import fs from "fs"
import path from "path"
import { chatStyles } from "../../lib/chat-utils"
import { AVAILABLE_MODELS, DEFAULT_MODEL } from "../../lib/models-config"
import { baseStyles } from "../../styles"

// File paths for HTML and CSS
const chatViewHTMLPath = path.join(process.cwd(), "src", "components", "chat-view", "chat-view.html")
const chatViewCSSPath = path.join(process.cwd(), "src", "components", "chat-view", "chat-view.css")

// Cache for production
let cachedHTML: string | null = null
let cachedCSS: string | null = null

export interface ChatViewProps {
  conversationId?: string
}

// Helper function to escape HTML
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
}

// Helper function to get additional classes based on metadata
function getAdditionalClasses(metadata: any): string {
  if (!metadata) return ""

  let classes = ""
  if (metadata.entryType === "tool_use") classes += " tool-use"
  if (metadata.entryType === "tool_result") classes += " tool-result"
  if (metadata.entryType === "summary") classes += " summary"

  return classes
}

// Safe version of renderChatMessage that builds HTML as string
function renderChatMessageSafe(message: {
  role: "user" | "assistant" | "system"
  content: string
  timestamp?: number
  rendered?: string
  metadata?: any
}): string {
  // For system messages (tool use, summaries, etc.), use a different style
  const blockClass = message.role === "system" ? "system" : message.role

  // Check if this is a tool-related message based on metadata
  const additionalClasses = getAdditionalClasses(message.metadata)

  // Skip empty messages
  if (!message.content || message.content.trim() === "") {
    return ""
  }

  // Build HTML as string to safely contain content with special characters
  return `
    <div class="message ${message.role}">
      <div class="message-block ${blockClass}${additionalClasses}">
        <div class="message-body">
          <pre style="white-space: pre-wrap; word-wrap: break-word; font-family: inherit; margin: 0; background: transparent; padding: 0;">${
    escapeHtml(message.content)
  }</pre>
        </div>
      </div>
    </div>
  `
}

export async function createChatView({ conversationId }: ChatViewProps) {
  // Determine if we're in development mode
  const isDev = process.env.NODE_ENV !== "production"

  // Read HTML and CSS files - fresh in dev, cached in production
  let chatViewHTML: string
  let chatViewCSS: string

  if (isDev) {
    // Read fresh files in development mode for hot reloading
    chatViewHTML = fs.readFileSync(chatViewHTMLPath, "utf-8")
    chatViewCSS = fs.readFileSync(chatViewCSSPath, "utf-8")
  } else {
    // Use cached files in production
    if (!cachedHTML) {
      cachedHTML = fs.readFileSync(chatViewHTMLPath, "utf-8")
    }
    if (!cachedCSS) {
      cachedCSS = fs.readFileSync(chatViewCSSPath, "utf-8")
    }
    chatViewHTML = cachedHTML
    chatViewCSS = cachedCSS
  }

  // Import server-side only here to avoid bundling issues
  const { getConversationWithMessages, getConversations } = await import("../../lib/chat-client-convex")

  // Load all conversations for sidebar
  let allConversations: Array<any> = []
  try {
    allConversations = await getConversations() as Array<any>
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }

  // Load messages if conversationId is provided
  let messages: Array<any> = []
  let conversation: any = null
  if (conversationId) {
    try {
      const result = await getConversationWithMessages(conversationId)
      conversation = result.conversation
      messages = result.messages as Array<any>
    } catch (error) {
      console.error("Failed to load conversation:", error)
    }
  }

  // Render messages - DO NOT render Claude Code HTML as markdown
  // TEMPORARY: Render all messages with safe approach
  const renderedMessages = messages.map((msg) => {
    // For now, just pass content through without rendering
    return {
      ...msg,
      content: msg.content || ""
    }
  })

  const title = conversation?.title || "Chat - OpenAgents"

  // Generate thread list HTML as a string
  let threadListHTML = ""
  if (allConversations.length > 0) {
    threadListHTML = `
    <div class="mt-2">
      <div class="px-3 py-1 mb-0.5">
        <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
      </div>
      <ul class="flex flex-col gap-0.5">`

    for (const conv of allConversations) {
      const isActive = conv.id === conversationId
      const className = isActive
        ? "bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]"
        : "text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"

      threadListHTML += `
        <li>
          <a href="/chat/${conv.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${className}">
            <span>${escapeHtml(conv.title)}</span>
          </a>
        </li>`
    }

    threadListHTML += `
      </ul>
    </div>`
  }

  // Generate messages HTML as a string
  let messagesHTML = ""
  if (renderedMessages.length > 0) {
    for (const message of renderedMessages) {
      messagesHTML += renderChatMessageSafe(message)
    }
  }

  // Generate model options HTML as a string
  let modelOptionsHTML = `<div class="model-group">Cloudflare (Free)</div>`

  const cloudflareModels = AVAILABLE_MODELS.filter((m) => m.provider === "cloudflare")
  for (const model of cloudflareModels) {
    modelOptionsHTML += `
      <div class="model-option" data-model-id="${escapeHtml(model.id)}" onclick="selectModel('${
      escapeHtml(model.id)
    }')">
        <div class="model-name">${escapeHtml(model.name)}</div>
        ${model.description ? `<div class="model-description">${escapeHtml(model.description)}</div>` : ""}
      </div>`
  }

  modelOptionsHTML += `
    <div class="model-group">OpenRouter (API Key Required)</div>
    <div id="openrouter-models">`

  const openrouterModels = AVAILABLE_MODELS.filter((m) => m.provider === "openrouter")
  for (const model of openrouterModels) {
    modelOptionsHTML += `
      <div class="model-option openrouter-model" data-model-id="${escapeHtml(model.id)}" onclick="selectModel('${
      escapeHtml(model.id)
    }')">
        <div class="model-name">
          ${escapeHtml(model.name)}
          <svg class="lock-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="5" y="11" width="14" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
        </div>
        ${model.description ? `<div class="model-description">${escapeHtml(model.description)}</div>` : ""}
      </div>`
  }

  modelOptionsHTML += `
    </div>
    
    <div class="api-key-notice" id="api-key-notice" style="display: none;">
      <a href="/settings">Add OpenRouter API key in Settings</a>
    </div>`

  // Replace placeholders in HTML
  const processedHTML = chatViewHTML
    .replace("<!-- Thread groups will be inserted here -->", threadListHTML)
    .replace("<!-- Messages will be dynamically added here -->", messagesHTML)
    .replace("<!-- Model options will be populated dynamically -->", modelOptionsHTML)

  // Script base URL for development vs production
  const scriptBase = isDev ? "http://localhost:5173/src/client" : "/js"

  // Build the head content
  const headContent = isDev ?
    `<link rel="preload" href="http://localhost:5173/src/client/main.css" as="style" onload="this.onload=null;this.rel='stylesheet'">
     <noscript><link rel="stylesheet" href="http://localhost:5173/src/client/main.css"></noscript>
     <style>
       /* Critical CSS to prevent FOUC - basic layout and colors */
       body { 
         background-color: #1a1a1a; 
         color: #f5f5f5; 
         font-family: ui-monospace, monospace;
         margin: 0;
       }
       .chat-container { 
         display: flex; 
         height: 100vh; 
       }
       .sidebar { 
         width: 16rem; 
         background-color: #0a0a0a; 
         border-right: 1px solid #333;
       }
       .main-content { 
         flex: 1; 
         background-color: #1a1a1a; 
       }
     </style>` :
    "<link rel=\"stylesheet\" href=\"/css/client.css\">"

  // Build the complete HTML document as a string
  const html = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)}</title>
  <link rel="icon" type="image/x-icon" href="/favicon.ico">
  ${headContent}
  <style>${baseStyles}${css`${chatViewCSS}`}${css`${chatStyles}`}</style>
</head>
<body>
  ${processedHTML}
  
  ${isDev ? "<script type=\"module\" src=\"http://localhost:5173/@vite/client\"></script>" : ""}
  <script type="module">
    // Import client initialization (includes CSS in dev mode)
    import { initializeClient } from '${scriptBase}/index.${isDev ? "ts" : "js"}';
    
    // Import chat module
    import { initializeChat } from '${scriptBase}/chat.${isDev ? "ts" : "js"}';
    import { initializeModelSelector } from '${scriptBase}/model-selector.${isDev ? "ts" : "js"}';
    
    // Set conversation ID globally
    window.CONVERSATION_ID = ${conversationId ? `"${conversationId}"` : "null"};
    
    // Set model config globally
    window.AVAILABLE_MODELS = ${JSON.stringify(AVAILABLE_MODELS)};
    window.DEFAULT_MODEL = '${DEFAULT_MODEL}';
    
    // Initialize components
    initializeClient();
    initializeModelSelector();
    initializeChat();
  </script>
</body>
</html>`

  return new Response(html, {
    headers: {
      "content-type": "text/html; charset=utf-8"
    }
  })
}
