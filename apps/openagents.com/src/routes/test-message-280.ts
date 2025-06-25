import { document, html } from "@openagentsinc/psionic"

export async function testMessage280() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")

  let messages = []
  try {
    const result = await getConversationWithMessages("claude-code-session-1750816776552")
    messages = result.messages || []
  } catch (error) {
    return document({
      title: "Error",
      body: html`<h1>Error: ${error.message}</h1>`
    })
  }

  // Get messages around 280
  const targetMessages = messages.slice(275, 285)

  const messageHtml = html`
    <style>
      body {
        background: #1a1a1a;
        color: #f5f5f5;
        font-family: monospace;
        padding: 20px;
        max-width: 1200px;
        margin: 0 auto;
      }
      .message {
        margin: 20px 0;
        padding: 15px;
        background: #0a0a0a;
        border: 2px solid #444;
        overflow: auto;
      }
      .message-header {
        background: #2a2a2a;
        padding: 10px;
        margin: -15px -15px 15px -15px;
      }
      pre {
        white-space: pre-wrap;
        word-wrap: break-word;
        overflow-wrap: break-word;
        max-width: 100%;
      }
      .warning {
        background: #550000;
        border: 2px solid #ff0000;
        padding: 10px;
        margin: 10px 0;
      }
    </style>
    
    <h1>Messages Around 280</h1>
    
    ${
    targetMessages.map((msg, index) => {
      const msgIndex = index + 276
      const role = msg.role || "system"
      const content = msg.content || "[No content]"

      // Check if this message has problematic content
      const hasLongContent = content.length > 5000
      const hasHTMLEntities = content.includes("&lt;") || content.includes("&gt;") || content.includes("&quot;")
      const hasBackslashes = content.includes("\\n") || content.includes("\\t") || content.includes("\\\\")

      return html`
        <div class="message">
          <div class="message-header">
            <strong>Message ${msgIndex}</strong> - Role: ${role}
            ${hasLongContent ? html`<span style="color: #ff6666;"> ⚠️ LONG (${content.length} chars)</span>` : ""}
            ${hasHTMLEntities ? html`<span style="color: #ffaa66;"> ⚠️ HTML ENTITIES</span>` : ""}
            ${hasBackslashes ? html`<span style="color: #ffff66;"> ⚠️ ESCAPED CHARS</span>` : ""}
          </div>
          
          <div>
            <strong>Content Preview (first 500 chars):</strong>
            <pre>${content.substring(0, 500)}${content.length > 500 ? "..." : ""}</pre>
          </div>
          
          ${
        msg.metadata ?
          html`
            <div style="margin-top: 10px;">
              <strong>Metadata:</strong>
              <pre>${JSON.stringify(msg.metadata, null, 2)}</pre>
            </div>
          ` :
          ""
      }
          
          ${
        (hasLongContent || hasHTMLEntities || hasBackslashes) ?
          html`
            <div class="warning">
              <strong>⚠️ This message might be breaking the layout!</strong>
              <br>Content length: ${content.length} characters
            </div>
          ` :
          ""
      }
        </div>
      `
    }).join("")
  }
  `

  return document({
    title: "Debug Message 280",
    body: messageHtml
  })
}
