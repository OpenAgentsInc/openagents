import { document, html } from "@openagentsinc/psionic"
import { renderChatMessage } from "../lib/chat-utils"

export async function testChatRender() {
  // Test message with problematic content (backticks)
  const testMessage = {
    role: "assistant" as const,
    content: `Here's some code:
\`\`\`typescript
function test() {
  return "hello"
}
\`\`\`
And some more text.`,
    timestamp: Date.now()
  }

  // Render using the same function as chat view
  const rendered = renderChatMessage(testMessage)

  return document({
    title: "Test Chat Render",
    body: html`
      <style>
        body {
          background: #1a1a1a;
          color: #f5f5f5;
          font-family: monospace;
          padding: 20px;
          max-width: 800px;
          margin: 0 auto;
        }
        .message {
          margin: 20px 0;
        }
        .message-block {
          border-left: 4px solid #7aa2f7;
          padding: 15px;
          background: #0a0a0a;
        }
        .message-body {
          color: #f5f5f5;
          line-height: 1.6;
        }
        pre {
          background: #1a1a1a;
          padding: 10px;
          border-radius: 4px;
          overflow-x: auto;
        }
      </style>
      
      <h1>Test Chat Message Rendering</h1>
      <p>This tests rendering a message with backticks using renderChatMessage:</p>
      
      <div id="rendered-message">
        ${rendered}
      </div>
      
      <hr style="margin: 40px 0; border-color: #333;">
      
      <h2>Raw HTML Output:</h2>
      <pre>${rendered.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</pre>
    `
  })
}
