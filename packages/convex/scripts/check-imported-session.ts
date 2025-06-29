#!/usr/bin/env bun

import { Effect } from "effect"
import { ConvexClient } from "./src/client.js"

const main = Effect.gen(function*() {
  console.log("📊 Checking imported session...")

  const sessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
  console.log(`\nFound ${sessions.length} session(s) in database`)

  if (sessions.length > 0) {
    const session = sessions[0]
    console.log(`\n✅ Session Details:`)
    console.log(`ID: ${session.id}`)
    console.log(`Project: ${session.project_name || session.project_path}`)
    console.log(`Status: ${session.status}`)
    console.log(`Messages: ${session.message_count}`)
    console.log(`Started: ${new Date(session.started_at).toLocaleString()}`)
    console.log(`Last Activity: ${new Date(session.last_activity).toLocaleString()}`)

    // Get a few messages to verify
    const messages = yield* ConvexClient.messages.listBySession(session.id, 5)
    console.log(`\nFirst ${messages.length} messages:`)
    messages.forEach((msg, i) => {
      const content = msg.content ? msg.content.substring(0, 80) + "..." : "[Empty]"
      console.log(`${i + 1}. ${msg.entry_type} (${msg.role || "N/A"}): ${content}`)
    })

    console.log(`\n🔗 View at: http://localhost:3003/chat/${session.id}`)
  }
})

Effect.runPromise(main).catch(console.error)
