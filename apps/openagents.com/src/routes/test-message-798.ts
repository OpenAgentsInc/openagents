import { document, html } from "@openagentsinc/psionic"

export async function testMessage798() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")

  console.log("\n=== TEST-MESSAGE-798: Starting ===")

  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const messages = result.messages || []

  console.log("Total messages loaded:", messages.length)

  // Get message 798 (index 797)
  const msg = messages[797]

  if (!msg) {
    console.log("ERROR: Message 798 not found!")
    return document({
      title: "Message 798 Not Found",
      body: html`<h1>Message 798 not found in ${messages.length} messages</h1>`
    })
  }

  console.log("\n=== Message 798 Details ===")
  console.log("ID:", msg.id)
  console.log("Role:", msg.role)
  console.log("Content type:", typeof msg.content)
  console.log("Content length:", msg.content?.length || 0)

  // Check if content has the line we're looking for
  const hasLine405 = msg.content?.includes("405") && msg.content?.includes("context.params[param]")
  console.log("Contains line 405:", hasLine405)

  // Analyze content for problematic characters
  if (msg.content) {
    const backtickCount = (msg.content.match(/`/g) || []).length
    const arrowCount = (msg.content.match(/→/g) || []).length
    console.log("\nContent analysis:")
    console.log("- Backticks found:", backtickCount)
    console.log("- Arrow chars found:", arrowCount)
    console.log("- Has triple backticks:", msg.content.includes("```"))

    // Find the line with 405
    const lines = msg.content.split("\n")
    const line405 = lines.find((line) => line.includes("405"))
    if (line405) {
      console.log("\nLine 405 found:")
      console.log(line405)
    }
  }

  // Create a completely safe version by manually escaping
  let displayContent = msg.content || "[no content]"

  // First escape HTML entities
  displayContent = displayContent
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")

  // The arrow character should be safe now

  return document({
    title: "Message 798 Test",
    body: html`
      <style>
        body {
          background: #1a1a1a;
          color: #f5f5f5;
          font-family: 'Berkeley Mono', monospace;
          padding: 20px;
          max-width: 1400px;
          margin: 0 auto;
          line-height: 1.6;
        }
        h1 { color: #7aa2f7; }
        .info {
          background: #0a0a0a;
          padding: 15px;
          margin: 20px 0;
          border: 1px solid #333;
          border-radius: 4px;
        }
        .message-content {
          background: #000;
          padding: 20px;
          margin: 20px 0;
          border-left: 4px solid #bb9af7;
          overflow-x: auto;
          font-size: 14px;
        }
        pre {
          margin: 0;
          white-space: pre;
          font-family: inherit;
        }
        .highlight {
          background: #3a3a00;
          color: #ffff00;
          font-weight: bold;
        }
      </style>
      
      <h1>Message 798 - Tool Result</h1>
      
      <div class="info">
        <p><strong>Message ID:</strong> ${msg.id}</p>
        <p><strong>Role:</strong> ${msg.role}</p>
        <p><strong>Type:</strong> ${msg.metadata?.entryType || "unknown"}</p>
        <p><strong>Content Length:</strong> ${msg.content?.length || 0} characters</p>
        <p><strong>Contains Line 405:</strong> ${hasLine405 ? "YES" : "NO"}</p>
      </div>
      
      <h2>Message Content:</h2>
      <div class="message-content">
        <pre>${displayContent}</pre>
      </div>
      
      <div class="info">
        <p><strong>Note:</strong> This is message #798 from the conversation. It's a tool_result message containing code output.</p>
        <p>The arrow characters (\u2192) are Unicode arrows used for line numbers in the output.</p>
      </div>
    `
  })
}
