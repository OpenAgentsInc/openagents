#!/usr/bin/env bun

/**
 * Test the complete flow including what the UI would see
 */

import { getConversationWithMessages } from "../../apps/openagents.com/src/lib/chat-client-convex"

async function main() {
  console.log("🧪 Testing complete UI flow...")

  try {
    // This is exactly what the UI calls
    const result = await getConversationWithMessages("claude-code-session-1750816776552")

    console.log("\n📊 Conversation:")
    console.log(JSON.stringify(result.conversation, null, 2))

    console.log("\n📊 Messages (first 10):")
    result.messages.slice(0, 10).forEach((msg, idx) => {
      console.log(`\n[${idx + 1}] ${msg.id}`)
      console.log(`  Role: ${msg.role}`)
      console.log(`  Content: ${msg.content ? `"${msg.content.substring(0, 100)}..."` : "EMPTY"}`)
      console.log(`  Content length: ${msg.content?.length || 0}`)
      console.log(`  Metadata:`, {
        entryType: msg.metadata.entryType,
        hasThinking: !!msg.metadata.thinking,
        hasSummary: !!msg.metadata.summary,
        toolName: msg.metadata.toolName
      })
    })

    // Count messages by type
    const typeCounts = result.messages.reduce((acc, msg) => {
      const type = msg.metadata.entryType
      acc[type] = (acc[type] || 0) + 1
      return acc
    }, {} as Record<string, number>)

    console.log("\n📊 Message type counts:")
    console.log(typeCounts)

    // Count empty content by type
    const emptyByType = result.messages.reduce((acc, msg) => {
      if (!msg.content || msg.content.length === 0) {
        const type = msg.metadata.entryType
        acc[type] = (acc[type] || 0) + 1
      }
      return acc
    }, {} as Record<string, number>)

    console.log("\n📊 Empty content by type:")
    console.log(emptyByType)
  } catch (error) {
    console.error("Error:", error)
  }
}

main().catch(console.error)
