// Debug simple WebSocket message
import { WebSocket } from 'ws'

const ws = new WebSocket('ws://localhost:3003/relay')

ws.on('open', () => {
  console.log('🔗 Connected to relay')
  
  // Send a simple text message first
  console.log('📤 Sending simple text message...')
  ws.send('hello')
  
  setTimeout(() => {
    // Then send a JSON message
    console.log('📤 Sending JSON message...')
    ws.send(JSON.stringify({test: "message"}))
    
    setTimeout(() => {
      // Then send an EVENT message
      console.log('📤 Sending EVENT message...')
      ws.send(JSON.stringify(["EVENT", {id: "test", kind: 1}]))
    }, 1000)
  }, 1000)
})

ws.on('message', (data) => {
  console.log('📥 Received:', data.toString())
})

ws.on('error', (error) => {
  console.error('🚨 WebSocket error:', error)
})

ws.on('close', () => {
  console.log('🔌 Connection closed')
})

// Timeout after 10 seconds
setTimeout(() => {
  console.log('⏰ Timeout - closing connection')
  if (ws.readyState === WebSocket.OPEN) {
    ws.close()
  }
}, 5000)