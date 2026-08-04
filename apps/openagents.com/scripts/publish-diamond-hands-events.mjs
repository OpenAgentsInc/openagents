#!/usr/bin/env node

import { readFile } from 'node:fs/promises'
import { WebSocket } from 'ws'

const inputPath = process.argv[2]
const relayUrl = process.argv[3] ?? 'wss://relay.openagents.com/'
const maximumEvents = 32
const timeoutMilliseconds = 20_000

if (inputPath === undefined) {
  throw new Error(
    'usage: publish-diamond-hands-events.mjs <signed-events.json> [relay-url]',
  )
}

const source = await readFile(inputPath, 'utf8')
const events = JSON.parse(source)
if (!Array.isArray(events) || events.length === 0) {
  throw new Error('input must be a non-empty JSON array')
}
if (events.length > maximumEvents) {
  throw new Error(`input exceeds the ${maximumEvents}-event operator limit`)
}

const socket = new WebSocket(relayUrl)
const acknowledgements = new Map()

const completion = new Promise((resolve, reject) => {
  const timeout = setTimeout(() => {
    reject(new Error('relay acknowledgements timed out'))
  }, timeoutMilliseconds)

  socket.on('error', reject)
  socket.on('message', payload => {
    const message = JSON.parse(payload.toString())
    if (message[0] !== 'OK' || typeof message[1] !== 'string') return
    acknowledgements.set(message[1], {
      accepted: message[2] === true,
      message: typeof message[3] === 'string' ? message[3] : '',
    })
    if (acknowledgements.size !== events.length) return
    clearTimeout(timeout)
    resolve()
  })
})

await new Promise((resolve, reject) => {
  socket.once('open', resolve)
  socket.once('error', reject)
})

for (const event of events) {
  if (typeof event?.id !== 'string') {
    throw new Error('every input entry must be a signed event with an id')
  }
  socket.send(JSON.stringify(['EVENT', event]))
}

try {
  await completion
} finally {
  socket.close()
}

const results = events.map(event => ({
  id: event.id,
  ...acknowledgements.get(event.id),
}))
const refusal = results.find(result => result.accepted !== true)
process.stdout.write(`${JSON.stringify({ relayUrl, results })}\n`)
if (refusal !== undefined) {
  throw new Error(`relay refused event ${refusal.id}: ${refusal.message}`)
}
