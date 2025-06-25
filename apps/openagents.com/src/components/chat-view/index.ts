import { css, document, html, renderMarkdown } from "@openagentsinc/psionic"
import fs from "fs"
import path from "path"
import { chatStyles, renderChatMessage } from "../../lib/chat-utils"
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

// HTML escape function to prevent template literal issues
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/`/g, "&#96;") // Critical: escape backticks to prevent template literal issues
}

// Note: Using escapeHtml for user content safety

export async function createChatView({ conversationId }: ChatViewProps) {
  console.log("=== createChatView called ===")
  console.log("conversationId:", conversationId)
  console.log("typeof conversationId:", typeof conversationId)
  
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

  // Import Effect
  const { Effect } = await import("effect")

  // Load all conversations for sidebar
  let allConversations: Array<any> = []
  try {
    console.log("Loading conversations...")
    allConversations = await Effect.runPromise(getConversations()) as Array<any>
    console.log("Loaded conversations:", allConversations.length)
  } catch (error) {
    console.error("Failed to load conversations:", error)
  }

  // Load messages if conversationId is provided
  let messages: Array<any> = []
  let conversation: any = null
  if (conversationId) {
    try {
      console.log("Loading conversation with messages for:", conversationId)
      const result = await Effect.runPromise(getConversationWithMessages(conversationId))
      conversation = result.conversation
      messages = result.messages as Array<any>
      console.log("Loaded messages:", messages.length)
      console.log("Message range 66-80:", messages.slice(65, 80).length)
    } catch (error) {
      console.error("Failed to load conversation:", error)
    }
  }

  // Show all messages (template literal issue is now fixed)
  const limitedMessages = messages
  console.log("Showing all messages:", limitedMessages.length)

  // Render messages with markdown
  console.log("Starting markdown rendering...")
  const renderedMessages = await Promise.all(
    limitedMessages.map(async (msg, index) => {
      try {
        const rendered = await renderMarkdown(msg.content)
        return {
          ...msg,
          rendered
        }
      } catch (error) {
        console.error(`Failed to render message ${index + 1}:`, error)
        return {
          ...msg,
          rendered: escapeHtml(msg.content)
        }
      }
    })
  )
  console.log("Markdown rendering complete")

  const title = escapeHtml(conversation?.title || "Chat - OpenAgents")

  // Generate thread list HTML with safe escaping
  const threadListHTML = allConversations.length > 0 ?
    html`
    <div class="mt-2">
      <div class="px-3 py-1 mb-0.5">
        <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
      </div>
      <ul class="flex flex-col gap-0.5">
        ${
      allConversations.map((conv) => {
        // Escape the title to prevent template literal issues
        const safeTitle = escapeHtml(conv.title || "Untitled Chat")
        const activeClass = conv.id === conversationId
          ? "bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]"
          : "text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"

        return html`
          <li>
            <a href="/chat/${conv.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${activeClass}">
              <span>${safeTitle}</span>
            </a>
          </li>
        `
      }).join("")
    }
      </ul>
    </div>
  ` :
    ""

  // Generate messages HTML
  console.log("Generating messages HTML...")
  const messagesHTML = renderedMessages.length > 0 ?
    renderedMessages.map((message, index) => {
      try {
        const html = renderChatMessage(message)
        return html
      } catch (error) {
        console.error(`Failed to generate HTML for message ${index + 1}:`, error)
        return `<div class="message"><div class="message-block assistant"><div class="message-body">Error rendering message</div></div></div>`
      }
    }).join("") :
    ""
  console.log("Messages HTML generation complete, total length:", messagesHTML.length)

  // Generate model options HTML
  const modelOptionsHTML = html`
    <div class="model-group">Cloudflare (Free)</div>
    ${
    AVAILABLE_MODELS
      .filter((m) => m.provider === "cloudflare")
      .map((model) =>
        html`
        <div class="model-option" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
          <div class="model-name">${model.name}</div>
          ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
        </div>
      `
      )
      .join("")
  }
    
    <div class="model-group">OpenRouter (API Key Required)</div>
    <div id="openrouter-models">
      ${
    AVAILABLE_MODELS
      .filter((m) => m.provider === "openrouter")
      .map((model) =>
        html`
          <div class="model-option openrouter-model" data-model-id="${model.id}" onclick="selectModel('${model.id}')">
            <div class="model-name">
              ${model.name}
              <svg class="lock-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="5" y="11" width="14" height="11" rx="2" ry="2"/>
                <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
              </svg>
            </div>
            ${model.description ? html`<div class="model-description">${model.description}</div>` : ""}
          </div>
        `
      )
      .join("")
  }
    </div>
    
    <div class="api-key-notice" id="api-key-notice" style="display: none;">
      <a href="/settings">Add OpenRouter API key in Settings</a>
    </div>
  `

  // Escape content to prevent template literal issues
  const safeMessagesHTML = messagesHTML.replace(/`/g, "&#96;")
  const safeThreadListHTML = threadListHTML.replace(/`/g, "&#96;")
  const safeModelOptionsHTML = modelOptionsHTML.replace(/`/g, "&#96;")
  
  console.log("Escaped HTML content:")
  console.log("- messagesHTML backticks replaced:", (messagesHTML.match(/`/g) || []).length)
  console.log("- threadListHTML backticks replaced:", (threadListHTML.match(/`/g) || []).length)
  console.log("- modelOptionsHTML backticks replaced:", (modelOptionsHTML.match(/`/g) || []).length)

  // Replace placeholders in HTML
  const processedHTML = chatViewHTML
    .replace("<!-- Thread groups will be inserted here -->", safeThreadListHTML)
    .replace("<!-- Messages will be dynamically added here -->", safeMessagesHTML)
    .replace("<!-- Model options will be populated dynamically -->", safeModelOptionsHTML)

  // Script base URL for development vs production
  const scriptBase = isDev ? "http://localhost:5173/src/client" : "/js"

  return document({
    title,
    head: isDev ?
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
      "<link rel=\"stylesheet\" href=\"/css/client.css\">",
    styles: baseStyles + css`${chatViewCSS}` + css`${chatStyles}`,
    body: html`
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
    `
  })
}
