#!/usr/bin/env bun

// Test message rendering
import { renderMarkdown } from "@openagentsinc/psionic"
import { renderChatMessage } from "../../apps/openagents.com/src/lib/chat-utils"

async function testMessageRendering() {
  console.log("Testing message rendering...")
  
  // Test cases
  const testMessages = [
    {
      role: "user" as const,
      content: "Can you help me with markdown rendering? I need to **bold** text and `inline code`.",
      timestamp: Date.now()
    },
    {
      role: "assistant" as const,
      content: `Sure! Here's how markdown works:

**Bold text** is created with double asterisks.
*Italic text* uses single asterisks.
\`Inline code\` uses backticks.

\`\`\`javascript
// Code blocks use triple backticks
function hello() {
  console.log("Hello, world!");
}
\`\`\`

You can also create:
- Bullet lists
- With multiple items

1. Numbered lists
2. Work too!`,
      timestamp: Date.now()
    },
    {
      role: "user" as const,
      content: "[User message content was not properly imported]",
      timestamp: Date.now()
    }
  ]
  
  console.log("\n=== Testing Message Rendering ===\n")
  
  for (const msg of testMessages) {
    console.log(`\n--- ${msg.role.toUpperCase()} MESSAGE ---`)
    console.log("Original content:", msg.content.substring(0, 100) + "...")
    
    // Render markdown
    const rendered = await renderMarkdown(msg.content)
    
    // Test with rendered content
    const messageWithRendered = { ...msg, rendered }
    const htmlOutput = renderChatMessage(messageWithRendered)
    
    console.log("\nRendered HTML (first 500 chars):")
    console.log(htmlOutput.substring(0, 500) + "...")
    
    // Test without rendered content (fallback)
    const htmlFallback = renderChatMessage(msg)
    console.log("\nFallback HTML (first 200 chars):")
    console.log(htmlFallback.substring(0, 200) + "...")
  }
}

// Run the test
testMessageRendering()