#!/usr/bin/env bun

/**
 * Check a specific message that should have content
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api"

const CONVEX_URL = process.env.CONVEX_URL || "https://proficient-panther-764.convex.cloud"

async function main() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get the message with UUID 81b34e9d-ad91-4b29-9ba3-72282bc03cb0
  // This should have content: "I'll continue fixing the TypeScript errors..."
  const message = await client.query(api.messages.getByUuid, {
    entryUuid: "81b34e9d-ad91-4b29-9ba3-72282bc03cb0"
  })
  
  console.log("Message lookup result:")
  console.log(JSON.stringify(message, null, 2))
  
  // Also try to get the user message that should have content
  const userMessage = await client.query(api.messages.getByUuid, {
    entryUuid: "c7322bbc-fad9-49a2-ad20-1ee929a0a779"
  })
  
  console.log("\n\nUser message lookup result:")
  console.log(JSON.stringify(userMessage, null, 2))
}

main().catch(console.error)