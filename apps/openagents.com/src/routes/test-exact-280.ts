import { document, html } from "@openagentsinc/psionic"
import { renderChatMessage } from "../lib/chat-utils"

export async function testExact280() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")

  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const messages = result.messages || []

  // Get message 280 (index 279)
  const msg280 = messages[279]

  if (!msg280) {
    return document({
      title: "Message 280 not found",
      body: html`<h1>Message 280 not found</h1>`
    })
  }

  // Test rendering this exact message
  const rendered = renderChatMessage(msg280)

  return document({
    title: "Test Message 280",
    body: html`
      <style>
        body {
          background: #1a1a1a;
          color: #f5f5f5;
          font-family: monospace;
          padding: 20px;
          max-width: 1200px;
          margin: 0 auto;
        }
        .debug {
          background: #0a0a0a;
          padding: 15px;
          margin: 20px 0;
          border: 1px solid #333;
        }
        pre {
          white-space: pre-wrap;
          word-wrap: break-word;
        }
        .message-block.system {
          border-left-color: #bb9af7;
        }
      </style>
      
      <h1>Message 280 Debug</h1>
      
      <div class="debug">
        <h3>Message Data:</h3>
        <pre>${
      JSON.stringify(
        {
          id: msg280.id,
          role: msg280.role,
          contentLength: msg280.content?.length || 0,
          contentPreview: msg280.content?.substring(0, 100) || "[no content]",
          hasMetadata: !!msg280.metadata,
          metadataType: msg280.metadata?.entryType
        },
        null,
        2
      )
    }</pre>
      </div>
      
      <div class="debug">
        <h3>Rendered Output:</h3>
        ${rendered}
      </div>
      
      <div class="debug">
        <h3>Raw HTML:</h3>
        <pre>${rendered.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</pre>
      </div>
    `
  })
}
