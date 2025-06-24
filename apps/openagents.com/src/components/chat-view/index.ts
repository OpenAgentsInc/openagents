import { css, document, html, renderMarkdown } from "@openagentsinc/psionic"
import fs from "fs"
import path from "path"
import { chatStyles, renderChatMessage } from "../../lib/chat-utils"
import { AVAILABLE_MODELS, DEFAULT_MODEL } from "../../lib/models-config"
import { baseStyles } from "../../styles"

// Read HTML and CSS files at runtime from the source directory
const chatViewHTML = fs.readFileSync(
  path.join(process.cwd(), "src", "components", "chat-view", "chat-view.html"),
  "utf-8"
)
const chatViewCSS = fs.readFileSync(
  path.join(process.cwd(), "src", "components", "chat-view", "chat-view.css"),
  "utf-8"
)

export interface ChatViewProps {
  conversationId?: string
}

export async function createChatView({ conversationId }: ChatViewProps) {
  // Import server-side only here to avoid bundling issues
  const { getConversationWithMessages, getConversations } = await import("../../lib/chat-client")

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

  // Render messages with markdown
  const renderedMessages = await Promise.all(
    messages.map(async (msg) => ({
      ...msg,
      rendered: await renderMarkdown(msg.content)
    }))
  )

  const title = conversation?.title || "Chat - OpenAgents"

  // Generate thread list HTML
  const threadListHTML = allConversations.length > 0 ?
    html`
    <div class="mt-2">
      <div class="px-3 py-1 mb-0.5">
        <span class="text-xs font-medium text-[rgba(255,255,255,0.5)] uppercase">Recent</span>
      </div>
      <ul class="flex flex-col gap-0.5">
        ${
      allConversations.map((conv) =>
        html`
          <li>
            <a href="/chat/${conv.id}" class="block px-3 py-1.5 text-sm rounded-md transition-colors ${
          conv.id === conversationId
            ? "bg-[rgba(255,255,255,0.1)] text-[#D7D8E5]"
            : "text-[rgba(255,255,255,0.7)] hover:bg-[rgba(255,255,255,0.05)] hover:text-[#D7D8E5]"
        }">
              <span>${conv.title}</span>
            </a>
          </li>
        `
      ).join("")
    }
      </ul>
    </div>
  ` :
    ""

  // Generate messages HTML
  const messagesHTML = renderedMessages.length > 0 ?
    renderedMessages.map((message) => renderChatMessage(message)).join("") :
    ""

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

  // Replace placeholders in HTML
  const processedHTML = chatViewHTML
    .replace("<!-- Thread groups will be inserted here -->", threadListHTML)
    .replace("<!-- Messages will be dynamically added here -->", messagesHTML)
    .replace("<!-- Model options will be populated dynamically -->", modelOptionsHTML)

  // Determine if we're in development mode
  const isDev = process.env.NODE_ENV !== "production"
  const scriptBase = isDev ? "http://localhost:5173/src/client" : "/js"

  return document({
    title,
    head: "<link rel=\"stylesheet\" href=\"" + (isDev ? "http://localhost:5173/src/client/main.css" : "/css/client.css") + "\">",
    styles: baseStyles + css`${chatViewCSS}` + css`${chatStyles}`,
    body: html`
      ${processedHTML}
      
      <script type="module">
        // Import chat module
        import { initializeChat } from '${scriptBase}/chat.${isDev ? "ts" : "js"}';
        import { initializeModelSelector } from '${scriptBase}/model-selector.${isDev ? "ts" : "js"}';
        
        // Set conversation ID globally
        window.CONVERSATION_ID = ${conversationId ? `"${conversationId}"` : "null"};
        
        // Set model config globally
        window.AVAILABLE_MODELS = ${JSON.stringify(AVAILABLE_MODELS)};
        window.DEFAULT_MODEL = '${DEFAULT_MODEL}';
        
        // Initialize components
        initializeModelSelector();
        initializeChat();
      </script>
    `
  })
}
