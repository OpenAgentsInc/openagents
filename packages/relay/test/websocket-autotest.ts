import { Effect, Console } from "effect"
import { WebSocket } from "ws"

/**
 * Autotest script for testing WebSocket connections to the Nostr relay
 * This verifies the relay is accessible and responds to NIP-01 protocol messages
 */

const testRelayConnection = (relayUrl: string) => Effect.gen(function*() {
  yield* Console.log(`Testing WebSocket connection to: ${relayUrl}`)
  
  const ws = new WebSocket(relayUrl)
  
  // Track messages
  const messages: any[] = []
  let isConnected = false
  
  // Set up event handlers
  ws.on('message', (data) => {
    const msg = JSON.parse(data.toString())
    messages.push(msg)
    yield* Console.log(`Received: ${JSON.stringify(msg)}`)
  })
  
  ws.on('error', (error) => {
    yield* Console.error(`WebSocket error: ${error}`)
  })
  
  // Wait for connection
  yield* Effect.promise(() => new Promise<void>((resolve, reject) => {
    ws.on('open', () => {
      isConnected = true
      resolve()
    })
    ws.on('error', reject)
    setTimeout(() => reject(new Error('Connection timeout')), 5000)
  }))
  
  yield* Console.log("✅ WebSocket connected successfully")
  
  // Test 1: Send a REQ message
  const subId = `test-${Date.now()}`
  const reqMessage = ["REQ", subId, { kinds: [1], limit: 10 }]
  ws.send(JSON.stringify(reqMessage))
  yield* Console.log(`Sent REQ: ${JSON.stringify(reqMessage)}`)
  
  // Wait for EOSE
  yield* Effect.sleep("500 millis")
  
  const eoseMsg = messages.find(m => m[0] === "EOSE" && m[1] === subId)
  if (eoseMsg) {
    yield* Console.log("✅ Received EOSE response")
  } else {
    yield* Console.error("❌ No EOSE response received")
  }
  
  // Test 2: Send a test event
  const testEvent = {
    id: "test" + Date.now().toString(16).padEnd(64, '0').slice(0, 64),
    pubkey: "test".padEnd(64, '0'),
    created_at: Math.floor(Date.now() / 1000),
    kind: 1,
    tags: [["test", "autotest"]],
    content: "Autotest verification event",
    sig: "0".repeat(128)
  }
  
  messages.length = 0
  ws.send(JSON.stringify(["EVENT", testEvent]))
  yield* Console.log("Sent test EVENT")
  
  // Wait for OK response
  yield* Effect.sleep("500 millis")
  
  const okMsg = messages.find(m => m[0] === "OK")
  if (okMsg) {
    yield* Console.log(`✅ Received OK response: ${JSON.stringify(okMsg)}`)
  } else {
    yield* Console.error("❌ No OK response received")
  }
  
  // Test 3: Close subscription
  ws.send(JSON.stringify(["CLOSE", subId]))
  yield* Console.log("Sent CLOSE message")
  
  yield* Effect.sleep("200 millis")
  
  // Close connection
  ws.close()
  yield* Console.log("✅ WebSocket test completed")
  
  return {
    connected: isConnected,
    receivedEose: !!eoseMsg,
    receivedOk: !!okMsg,
    totalMessages: messages.length
  }
})

// Run the test
const relayUrl = process.argv[2] || "ws://localhost:3003/relay"

Effect.runPromise(testRelayConnection(relayUrl))
  .then(result => {
    console.log("\n📊 Test Results:", result)
    if (result.connected && result.receivedEose) {
      console.log("✅ All WebSocket tests passed!")
      process.exit(0)
    } else {
      console.log("❌ Some tests failed")
      process.exit(1)
    }
  })
  .catch(error => {
    console.error("Test failed:", error)
    process.exit(1)
  })