#!/usr/bin/env bun

import { ConvexClient } from "convex/browser"
import { api } from "./convex/_generated/api"

// Initialize client with the Convex URL
const CONVEX_URL = "https://proficient-panther-764.convex.cloud"
const client = new ConvexClient(CONVEX_URL)

async function examineMessage() {
  const messageId = "10c5f73f-41e8-461b-a952-5e3becb35a78"

  console.log(`\n=== Examining Message ID: ${messageId} ===\n`)

  try {
    // Get the message by UUID
    const message = await client.query(api.messages.getByUuid, {
      entryUuid: messageId
    })

    if (!message) {
      console.error(`Message with ID ${messageId} not found!`)
      return
    }

    console.log("Complete Database Record:")
    console.log("------------------------")
    console.log(JSON.stringify(message, null, 2))

    console.log("\n\nField Analysis:")
    console.log("---------------")

    // List all fields with their values
    const fields = Object.keys(message).sort()
    for (const field of fields) {
      const value = message[field as keyof typeof message]
      const valueType = typeof value
      let displayValue = value

      if (valueType === "string" && value.length > 100) {
        displayValue = value.substring(0, 100) + "... (truncated)"
      } else if (valueType === "object" && value !== null) {
        displayValue = JSON.stringify(value)
      }

      console.log(`${field} (${valueType}): ${displayValue}`)
    }

    // Special analysis for tool-related fields
    console.log("\n\nTool-Related Fields:")
    console.log("--------------------")
    console.log(`entry_type: ${message.entry_type}`)
    console.log(`tool_name: ${message.tool_name || "(not set)"}`)
    console.log(`tool_use_id: ${message.tool_use_id || "(not set)"}`)
    console.log(`tool_input: ${message.tool_input ? JSON.stringify(message.tool_input, null, 2) : "(not set)"}`)
    console.log(`tool_output: ${message.tool_output || "(not set)"}`)
    console.log(`tool_is_error: ${message.tool_is_error || "(not set)"}`)

    // Check content field
    console.log("\n\nContent Analysis:")
    console.log("-----------------")
    if (message.content) {
      console.log(`Content length: ${message.content.length} characters`)
      console.log(`Content preview: ${message.content.substring(0, 200)}...`)
    } else {
      console.log("Content field is empty/null")
    }

    // Compare with what should be displayed
    console.log("\n\nDisplay Comparison:")
    console.log("-------------------")
    console.log("Fields currently shown in debug info:")
    console.log("- entry_uuid")
    console.log("- entry_type")
    console.log("- role")
    console.log("- content (if entry_type is 'user' or 'assistant')")
    console.log("- tool_name (if entry_type is 'tool_use')")
    console.log("- tool_output (if entry_type is 'tool_result')")

    console.log("\nFields NOT shown in debug info:")
    const shownFields = [
      "_id",
      "_creationTime",
      "entry_uuid",
      "entry_type",
      "role",
      "content",
      "tool_name",
      "tool_output"
    ]
    const notShown = fields.filter((f) =>
      !shownFields.includes(f) && message[f as keyof typeof message] !== null &&
      message[f as keyof typeof message] !== undefined
    )
    for (const field of notShown) {
      console.log(`- ${field}: ${JSON.stringify(message[field as keyof typeof message])}`)
    }
  } catch (error) {
    console.error("Error examining message:", error)
  }
}

// Run the examination
examineMessage().catch(console.error)
