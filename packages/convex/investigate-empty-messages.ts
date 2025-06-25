#!/usr/bin/env tsx

import { ConvexHttpClient } from "convex/browser";
import { api } from "./convex/_generated/api";

const CONVEX_URL = process.env.NEXT_PUBLIC_CONVEX_URL || "https://humorous-marten-231.convex.cloud";

async function investigateEmptyMessages() {
  const client = new ConvexHttpClient(CONVEX_URL);
  
  const messageIds = [
    "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42", // Blue box (assistant)
    "75f5d516-754d-4d9a-bb42-a272fa37c30b"  // Green box with "[Empty message]" (user)
  ];

  console.log("Investigating empty messages...\n");

  for (const messageId of messageIds) {
    console.log(`\n${"=".repeat(80)}`);
    console.log(`Message ID: ${messageId}`);
    console.log(`${"=".repeat(80)}`);

    try {
      // Get the message
      const message = await client.query(api.messages.getByUuid, { entryUuid: messageId });
      
      if (!message) {
        console.log("❌ Message not found");
        continue;
      }

      // Display all fields
      console.log("\n📄 Raw Message Data:");
      console.log(JSON.stringify(message, null, 2));

      // Check specific fields
      console.log("\n🔍 Field Analysis:");
      console.log(`- ID: ${message._id}`);
      console.log(`- Created: ${new Date(message._creationTime).toISOString()}`);
      console.log(`- Session ID: ${message.sessionId}`);
      console.log(`- Entry Type: ${message.entry_type}`);
      console.log(`- Content Type: ${typeof message.content}`);
      console.log(`- Content Length: ${message.content ? message.content.length : 'null/undefined'}`);
      console.log(`- Has rendered_content: ${!!message.rendered_content}`);
      
      if (message.content) {
        console.log(`\n📝 Content Preview (first 500 chars):`);
        console.log(message.content.substring(0, 500));
        
        // Check for common patterns that might cause empty rendering
        console.log(`\n🔎 Content Patterns:`);
        console.log(`- Starts with whitespace: ${/^\s/.test(message.content)}`);
        console.log(`- Only whitespace: ${/^\s*$/.test(message.content)}`);
        console.log(`- Contains HTML tags: ${/<[^>]+>/.test(message.content)}`);
        console.log(`- Contains escaped HTML: ${/&lt;|&gt;|&amp;/.test(message.content)}`);
        console.log(`- Contains tool_result tags: ${/<tool_result>/.test(message.content)}`);
        console.log(`- Contains thinking tags: ${/<thinking>/.test(message.content)}`);
      }

      // Check rendered content
      if (message.rendered_content) {
        console.log(`\n🎨 Rendered Content Preview (first 500 chars):`);
        console.log(message.rendered_content.substring(0, 500));
      }

      // Check for tool-related fields
      if (message.entry_type === 'tool_result' || message.entry_type === 'tool_use') {
        console.log(`\n🔧 Tool Information:`);
        console.log(`- Tool Name: ${message.tool_name || 'N/A'}`);
        console.log(`- Tool Use ID: ${message.tool_use_id || 'N/A'}`);
        console.log(`- Is Error: ${message.is_error || false}`);
      }

      // Check for metadata
      if (message.metadata) {
        console.log(`\n📊 Metadata:`);
        console.log(JSON.stringify(message.metadata, null, 2));
      }

    } catch (error) {
      console.error(`❌ Error querying message ${messageId}:`, error);
    }
  }
}

// Run the investigation
investigateEmptyMessages().then(() => {
  console.log("\n✅ Investigation complete");
  process.exit(0);
}).catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});