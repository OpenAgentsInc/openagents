import { document, html } from "@openagentsinc/psionic"

export async function debugChatFull() {
  const { getConversationWithMessages } = await import("../lib/chat-client-convex")
  
  console.log("\n=== DEBUG-CHAT-FULL: Starting ===")
  
  try {
    const result = await getConversationWithMessages("claude-code-session-1750816776552")
    const messages = result.messages || []
    
    console.log("Total messages:", messages.length)
    
    // Find messages with potential breaking content
    const problematicMessages = []
    
    messages.forEach((msg, index) => {
      if (msg.content) {
        // Check for content that might break template literals
        const hasTripleBackticks = msg.content.includes('```')
        const hasTemplateString = msg.content.includes('${')
        const hasBacktick = msg.content.includes('`')
        const hasUnclosedQuote = (msg.content.match(/"/g) || []).length % 2 !== 0
        const hasUnclosedSingleQuote = (msg.content.match(/'/g) || []).length % 2 !== 0
        
        if (hasTripleBackticks || hasTemplateString || hasBacktick) {
          problematicMessages.push({
            index: index + 1,
            id: msg.id,
            role: msg.role,
            issues: {
              tripleBackticks: hasTripleBackticks,
              templateString: hasTemplateString,
              backtick: hasBacktick,
              unclosedQuote: hasUnclosedQuote,
              unclosedSingleQuote: hasUnclosedSingleQuote
            },
            preview: msg.content.substring(0, 100) + '...'
          })
        }
      }
    })
    
    console.log(`\nFound ${problematicMessages.length} potentially problematic messages`)
    
    // Log first 10 problematic messages
    problematicMessages.slice(0, 10).forEach(pm => {
      console.log(`\nMessage ${pm.index} (${pm.id}):`);
      console.log("Role:", pm.role);
      console.log("Issues:", pm.issues);
      console.log("Preview:", pm.preview);
    })
    
    return document({
      title: "Debug Chat Full",
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
          .problem {
            margin: 20px 0;
            padding: 15px;
            background: #0a0a0a;
            border: 1px solid #f7768e;
            border-radius: 4px;
          }
          .issue {
            display: inline-block;
            padding: 2px 8px;
            margin: 2px;
            background: #f7768e;
            color: #000;
            border-radius: 3px;
            font-size: 12px;
          }
          pre {
            background: #000;
            padding: 10px;
            overflow-x: auto;
            margin: 10px 0;
          }
        </style>
        
        <h1>Debug Chat Full - Problematic Messages</h1>
        <p>Total messages: ${messages.length}</p>
        <p>Messages with potential issues: ${problematicMessages.length}</p>
        
        <h2>First 10 Problematic Messages:</h2>
        
        ${problematicMessages.slice(0, 10).map(pm => {
          // Escape the preview for safe display
          const safePreview = pm.preview
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
          
          return `
            <div class="problem">
              <h3>Message ${pm.index} (${pm.role})</h3>
              <p><strong>ID:</strong> ${pm.id}</p>
              <div>
                <strong>Issues:</strong>
                ${pm.issues.tripleBackticks ? '<span class="issue">Triple Backticks</span>' : ''}
                ${pm.issues.templateString ? '<span class="issue">Template String ${}</span>' : ''}
                ${pm.issues.backtick ? '<span class="issue">Backtick</span>' : ''}
                ${pm.issues.unclosedQuote ? '<span class="issue">Unclosed Quote</span>' : ''}
                ${pm.issues.unclosedSingleQuote ? '<span class="issue">Unclosed Single Quote</span>' : ''}
              </div>
              <pre>${safePreview}</pre>
            </div>
          `
        }).join('')}
        
        <div style="margin-top: 40px; padding: 20px; background: #111; border: 1px solid #444;">
          <h3>Analysis</h3>
          <p>The syntax error at line 19153 suggests that message content is breaking the JavaScript template literal in the chat view.</p>
          <p>Most likely cause: Backticks or template strings in message content are not being properly escaped.</p>
        </div>
      `
    })
  } catch (error) {
    console.error("Error in debugChatFull:", error)
    return document({
      title: "Error",
      body: html`<h1>Error: ${error instanceof Error ? error.message : 'Unknown error'}</h1>`
    })
  }
}