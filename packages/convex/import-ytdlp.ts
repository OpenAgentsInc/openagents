import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"
import * as fs from "fs"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"
const JSONL_FILE = "/Users/christopherdavid/.claude/projects/-Users-christopherdavid-code-yt-dlp/78a490a4-8631-4ef3-a523-b796b88a67fb.jsonl"

// Import the parse function from delete-and-reimport.ts
function parseCompleteJSONL(content: string) {
  const lines = content.trim().split('\n').filter(line => line.trim())
  console.log(`📄 Found ${lines.length} lines in JSONL file`)
  
  const entries = []
  let parseErrors = 0
  
  for (const [index, line] of lines.entries()) {
    try {
      const parsed = JSON.parse(line)
      entries.push(parsed)
    } catch (error) {
      parseErrors++
      console.error(`❌ Failed to parse line ${index + 1}:`, error)
    }
  }
  
  console.log(`✅ Successfully parsed ${entries.length} entries (${parseErrors} errors)`)
  return entries
}

// Simple conversion for demo
function convertToConvexMessages(entries: any[]) {
  const messages = []
  
  for (const entry of entries) {
    if (entry.type === "user" && entry.message?.content) {
      messages.push({
        session_id: "yt-dlp-demo",
        entry_uuid: entry.uuid,
        entry_type: "user",
        role: "user",
        content: typeof entry.message.content === "string" 
          ? entry.message.content 
          : entry.message.content[0]?.text || "[No content]",
        timestamp: new Date(entry.timestamp).getTime()
      })
    } else if (entry.type === "assistant" && entry.message?.content) {
      // Extract text from content array
      let textContent = ""
      if (Array.isArray(entry.message.content)) {
        textContent = entry.message.content
          .filter((c: any) => c.type === "text")
          .map((c: any) => c.text)
          .join("\n")
      } else {
        textContent = entry.message.content
      }
      
      if (textContent) {
        messages.push({
          session_id: "yt-dlp-demo",
          entry_uuid: entry.uuid,
          entry_type: "assistant",
          role: "assistant",
          content: textContent,
          thinking: entry.message.thinking,
          model: entry.message.model,
          timestamp: new Date(entry.timestamp).getTime()
        })
      }
    }
  }
  
  return messages
}

async function main() {
  console.log("🚀 Importing yt-dlp session...")
  
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Create session
  await client.mutation(api.sessions.create, {
    id: "yt-dlp-demo",
    user_id: "claude-code-user",
    project_path: "/Users/christopherdavid/code/yt-dlp",
    project_name: "yt-dlp Demo",
    status: "completed",
    started_at: Date.now() - 1000000,
    last_activity: Date.now(),
    message_count: 0,
    total_cost: 0
  })
  
  // Read and parse file
  const content = fs.readFileSync(JSONL_FILE, 'utf-8')
  const entries = parseCompleteJSONL(content)
  const messages = convertToConvexMessages(entries)
  
  console.log(`📦 Importing ${messages.length} messages...`)
  
  // Import messages
  let imported = 0
  for (const msg of messages) {
    try {
      await client.mutation(api.messages.create, msg)
      imported++
    } catch (error) {
      console.error(`Failed to import message:`, error)
    }
  }
  
  // Update session count
  await client.mutation(api.sessions.updateStats, {
    sessionId: "yt-dlp-demo",
    messageCount: imported,
    totalCost: 0.05
  })
  
  console.log(`✅ Imported ${imported} messages!`)
}

main().catch(console.error)