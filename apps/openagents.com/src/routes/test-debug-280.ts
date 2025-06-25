import { document, html } from "@openagentsinc/psionic"

export async function testDebug280() {
  // Use the existing chat client
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")

  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const allMessages = result.messages || []

  // Get messages 279-281 (0-indexed, so 279 = index 279)
  const targetMessages = allMessages.slice(279, 282)

  console.log("\n=== DEBUGGING MESSAGES 280-282 ===")

  const debugInfo = targetMessages.map((msg, idx) => {
    const msgNum = 280 + idx
    console.log(`\n--- Message ${msgNum} ---`)
    console.log("Entry type:", msg.entry_type)
    console.log("Role:", msg.role)
    console.log("Content exists:", !!msg.content)
    console.log("Content type:", typeof msg.content)
    console.log("Content length:", msg.content ? msg.content.length : 0)

    if (msg.content) {
      console.log("First 200 chars:", msg.content.substring(0, 200))
      console.log("Contains backslash-n:", msg.content.includes("\\n"))
      console.log("Contains HTML tags:", msg.content.includes("<") || msg.content.includes(">"))
      console.log("Contains quotes:", msg.content.includes("\"") || msg.content.includes("'"))
    }

    if (msg.metadata) {
      console.log("Metadata:", JSON.stringify(msg.metadata, null, 2))
    }

    return {
      num: msgNum,
      entry_type: msg.entry_type,
      role: msg.role || "system",
      content: msg.content || "[No content]",
      metadata: msg.metadata,
      contentLength: msg.content ? msg.content.length : 0,
      hasProblematicChars: msg.content ?
        (
          msg.content.includes("\\n") ||
          msg.content.includes("\\t") ||
          msg.content.includes("<") ||
          msg.content.includes(">")
        ) :
        false
    }
  })

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
      }
      .message.problematic {
        border-color: #ff6666;
        background: #220000;
      }
      .debug-info {
        background: #2a2a2a;
        padding: 10px;
        margin: 10px 0;
        font-size: 12px;
      }
      .content-box {
        background: #1a1a1a;
        padding: 10px;
        margin: 10px 0;
        white-space: pre-wrap;
        word-wrap: break-word;
        overflow-wrap: break-word;
        max-width: 100%;
        overflow-x: auto;
      }
      .warning {
        color: #ff6666;
        font-weight: bold;
      }
    </style>
    
    <h1>Debug Messages 280-282</h1>
    <p>Check the console for detailed debug logs</p>
    
    ${
    debugInfo.map((info) =>
      html`
      <div class="message ${info.hasProblematicChars ? "problematic" : ""}">
        <h3>Message ${info.num}</h3>
        
        <div class="debug-info">
          <strong>Entry Type:</strong> ${info.entry_type}<br>
          <strong>Role:</strong> ${info.role}<br>
          <strong>Content Length:</strong> ${info.contentLength} chars<br>
          ${info.hasProblematicChars ? html`<span class="warning">⚠️ Contains problematic characters</span><br>` : ""}
        </div>
        
        <h4>Content:</h4>
        <div class="content-box">${info.content}</div>
        
        ${
        info.metadata ?
          html`
          <h4>Metadata:</h4>
          <div class="content-box">${JSON.stringify(info.metadata, null, 2)}</div>
        ` :
          ""
      }
      </div>
    `
    ).join("")
  }
  `

  return document({
    title: "Debug Messages 280",
    body: messageHtml
  })
}
