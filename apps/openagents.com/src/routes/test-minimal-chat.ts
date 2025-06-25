import { document, html } from "@openagentsinc/psionic"

export async function testMinimalChat() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  const result = await getConversationWithMessages("claude-code-session-1750816776552")
  const messages = result.messages || []
  
  // Get message 798 (index 797) - the one with line 405
  const targetMessage = messages[797]
  
  console.log("\n=== TEST-MINIMAL-CHAT: Looking for message 798 ===")
  console.log("Total messages loaded:", messages.length)
  console.log("Target message exists:", !!targetMessage)
  
  if (targetMessage) {
    console.log("\nMessage details:")
    console.log("- ID:", targetMessage.id)
    console.log("- Role:", targetMessage.role)
    console.log("- Content type:", typeof targetMessage.content)
    console.log("- Content length:", targetMessage.content?.length || 0)
    console.log("- Metadata:", JSON.stringify(targetMessage.metadata, null, 2))
    
    // Log first 200 chars of content
    if (targetMessage.content) {
      console.log("\nContent preview (first 200 chars):")
      console.log(targetMessage.content.substring(0, 200))
      console.log("...")
      
      // Check for problematic characters
      console.log("\nContent analysis:")
      console.log("- Contains backticks:", targetMessage.content.includes('`'))
      console.log("- Contains triple backticks:", targetMessage.content.includes('```'))
      console.log("- Contains line 405:", targetMessage.content.includes('405→'))
    }
  }
  
  const messagesToShow = targetMessage ? [targetMessage] : []
  
  return document({
    title: "Message 798 Test",
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
        .message {
          margin: 20px 0;
          padding: 15px;
          border: 1px solid #333;
          background: #0a0a0a;
        }
        .role-user { border-left: 4px solid #9ece6a; }
        .role-assistant { border-left: 4px solid #7aa2f7; }
        .role-system { border-left: 4px solid #bb9af7; }
        .metadata {
          margin-top: 15px;
          padding: 10px;
          background: #111;
          font-size: 12px;
          color: #888;
          border: 1px solid #444;
        }
        .content-box {
          margin-top: 15px;
          padding: 15px;
          background: #050505;
          border: 1px solid #444;
          overflow-x: auto;
        }
        pre {
          margin: 0;
          white-space: pre-wrap;
          word-break: break-word;
        }
      </style>
      
      <h1>Message 798 (The Line 405 Message)</h1>
      <p>Looking for message with: "405→ context.params[param] = routeMatch[index + 1]"</p>
      
      ${messagesToShow.length > 0 ? messagesToShow.map((msg) => {
        // Manually escape content for safe display
        const safeContent = (msg.content || "[no content]")
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
        
        return `
          <div class="message role-${msg.role}">
            <div><strong>Message 798 - ${msg.role}</strong></div>
            <div><strong>ID:</strong> ${msg.id}</div>
            
            <div class="metadata">
              <strong>Metadata:</strong>
              <pre>${JSON.stringify(msg.metadata, null, 2)}</pre>
            </div>
            
            <div class="content-box">
              <strong>Content (${msg.content?.length || 0} chars):</strong>
              <pre>${safeContent}</pre>
            </div>
          </div>
        `
      }).join('') : '<p style="color: #f7768e;">Message 798 not found!</p>'}
      
      <div style="margin-top: 40px; padding: 20px; background: #111; border: 1px solid #444;">
        <h3>Check Browser Console for Detailed Logs</h3>
        <p>Open DevTools (F12) and check the console for detailed analysis of this message.</p>
      </div>
    `
  })
}