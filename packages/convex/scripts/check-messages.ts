#!/usr/bin/env bun

/**
 * Quick diagnostic script to check message content in Convex
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api"

const CONVEX_URL = process.env.CONVEX_URL || "https://proficient-panther-764.convex.cloud"

async function main() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get all sessions
  const sessions = await client.query(api.sessions.listRecent, { limit: 10 })
  console.log(`Found ${sessions.length} sessions`)

  if (sessions.length === 0) {
    console.log("No sessions found!")
    return
  }

  // Check the first session
  const session = sessions[0]
  console.log(`\nChecking session: ${session.id}`)

  // Get first 10 messages
  const messages = await client.query(api.messages.listBySession, {
    sessionId: session.id,
    limit: 10
  })

  console.log(`\nFound ${messages.length} messages`)

  // Check each message
  for (const msg of messages) {
    console.log(`\n--- Message ${msg.entry_uuid} ---`)
    console.log(`Type: ${msg.entry_type}`)
    console.log(`Role: ${msg.role || "N/A"}`)
    console.log(`Content: ${msg.content ? `${msg.content.substring(0, 100)}...` : "EMPTY"}`)
    console.log(`Content length: ${msg.content?.length || 0}`)
    console.log(`Has thinking: ${!!msg.thinking}`)
    console.log(`Has summary: ${!!msg.summary}`)
    console.log(`Has tool_output: ${!!msg.tool_output}`)
  }

  // Get message type breakdown
  const allMessages = await client.query(api.messages.listBySession, {
    sessionId: session.id,
    limit: 1000
  })

  const typeCount = allMessages.reduce((acc, msg) => {
    acc[msg.entry_type] = (acc[msg.entry_type] || 0) + 1
    return acc
  }, {} as Record<string, number>)

  const emptyContent =
    allMessages.filter((msg) => msg.entry_type === "assistant" && (!msg.content || msg.content.length === 0)).length

  console.log(`\n--- Message Type Breakdown ---`)
  console.log(typeCount)
  console.log(`\nAssistant messages with empty content: ${emptyContent}`)
}

main().catch(console.error)
