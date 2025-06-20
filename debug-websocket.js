// Debug WebSocket connection to relay
import { WebSocket } from 'ws'
import { schnorr } from '@noble/curves/secp256k1'
import { sha256 } from '@noble/hashes/sha256'
import { bytesToHex, randomBytes } from '@noble/hashes/utils'

// Generate a proper Nostr event
function createChannelEvent() {
  const privateKey = bytesToHex(randomBytes(32))
  const publicKey = bytesToHex(schnorr.getPublicKey(privateKey))
  
  const event = {
    pubkey: publicKey,
    created_at: Math.floor(Date.now() / 1000),
    kind: 40,
    tags: [],
    content: JSON.stringify({
      name: "Debug WebSocket Channel",
      about: "Testing direct WebSocket connection"
    })
  }
  
  // Calculate ID
  const serialized = JSON.stringify([
    0,
    event.pubkey,
    event.created_at,
    event.kind,
    event.tags,
    event.content
  ])
  
  const hash = sha256(new TextEncoder().encode(serialized))
  event.id = bytesToHex(hash)
  
  // Sign
  event.sig = bytesToHex(schnorr.sign(event.id, privateKey))
  
  return event
}

const ws = new WebSocket('ws://localhost:3003/relay')

ws.on('open', () => {
  console.log('🔗 Connected to relay')
  
  const event = createChannelEvent()
  console.log('📝 Generated event:', {
    id: event.id,
    kind: event.kind,
    pubkey: event.pubkey,
    content: event.content
  })
  
  const message = ["EVENT", event]
  console.log('📤 Sending EVENT message...')
  ws.send(JSON.stringify(message))
})

ws.on('message', (data) => {
  const message = JSON.parse(data.toString())
  console.log('📥 Received:', message)
  
  if (message[0] === 'OK') {
    const [, eventId, success, reason] = message
    if (success) {
      console.log('✅ Event accepted by relay!')
    } else {
      console.log('❌ Event rejected:', reason)
    }
    ws.close()
  }
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
}, 10000)