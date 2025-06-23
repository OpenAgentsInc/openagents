import { css, document, html } from "@openagentsinc/psionic"
import { baseStyles } from "../../styles"
import fs from "fs"
import path from "path"

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

export function createChatView({ conversationId }: ChatViewProps) {
  const title = conversationId ? `Chat ${conversationId} - OpenAgents` : "Chat - OpenAgents"

  return document({
    title,
    styles: baseStyles + css`${chatViewCSS}`,
    body: html`${chatViewHTML}`
  })
}