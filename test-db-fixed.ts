// Test database after migration
import { RelayDatabase, RelayDatabaseLive } from "@openagentsinc/relay"
import { Effect, Runtime } from "effect"

const runtime = Runtime.defaultRuntime

async function testDatabase() {
  console.log("Testing database after migration...\n")
  
  try {
    const program = Effect.gen(function*() {
      const database = yield* RelayDatabase
      
      console.log("✅ Database service acquired")
      
      // Test channels query
      const channels = yield* database.getChannels()
      console.log(`Found ${channels.length} channels`)
      
      if (channels.length > 0) {
        console.log("\nExisting channels:")
        channels.forEach(ch => {
          console.log(`- ${ch.name} (${ch.id})`)
        })
      }
      
      // Test storing a kind 40 (channel creation) event
      const testEvent = {
        id: "test_channel_" + Date.now(),
        pubkey: "a".repeat(64),
        created_at: Math.floor(Date.now() / 1000),
        kind: 40, // Channel creation
        tags: [],
        content: JSON.stringify({ name: "Test Database Channel", about: "Testing database" }),
        sig: "b".repeat(128)
      }
      
      console.log("\nStoring channel creation event...")
      const stored = yield* database.storeEvent(testEvent as any)
      console.log("Event stored:", stored)
      
      if (stored) {
        console.log("\nChecking channels after event creation...")
        const newChannels = yield* database.getChannels()
        console.log(`Now found ${newChannels.length} channels`)
        
        newChannels.forEach(ch => {
          console.log(`- ${ch.name} (${ch.id}) by ${ch.creator_pubkey || 'unknown'}`)
        })
      }
      
      return { success: true }
    })
    
    const result = await Runtime.runPromise(runtime)(
      program.pipe(Effect.provide(RelayDatabaseLive))
    )
    
    console.log("\n✅ Database test complete:", result)
  } catch (error) {
    console.error("❌ Database test failed:", error)
    console.error("Stack:", error.stack)
  }
}

testDatabase()