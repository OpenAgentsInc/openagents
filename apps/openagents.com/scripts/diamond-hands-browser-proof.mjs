#!/usr/bin/env node

import { chromium } from 'playwright'

const targetUrl = process.argv[2] ?? 'http://127.0.0.1:8081/dh/'
const relayUrl = 'wss://relay.openagents.com/'
const expectedLiveEventId = process.env['DIAMOND_HANDS_EXPECT_LIVE_EVENT_ID']
const projectApiRequests = []
const relayFrames = []
const browserErrors = []

const browser = await chromium.launch({
  headless: true,
  args: ['--enable-unsafe-webgpu', '--enable-features=SharedArrayBuffer'],
})

try {
  const page = await browser.newPage()
  page.on('console', message => {
    if (message.type() === 'error') browserErrors.push(message.text())
  })
  page.on('pageerror', error => browserErrors.push(error.message))
  page.on('request', request => {
    const url = new URL(request.url())
    if (url.pathname.startsWith('/api/')) projectApiRequests.push(request.url())
  })

  const eose = new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error('relay did not send EOSE within 20 seconds')),
      20_000,
    )
    page.on('websocket', socket => {
      if (socket.url() !== relayUrl) return
      socket.on('framesent', frame =>
        relayFrames.push({ direction: 'sent', ...frame }),
      )
      socket.on('framereceived', frame => {
        relayFrames.push({ direction: 'received', ...frame })
        if (typeof frame.payload !== 'string') return
        const parsed = JSON.parse(frame.payload)
        if (parsed[0] !== 'EOSE') return
        clearTimeout(timeout)
        resolve()
      })
    })
  })

  const liveEvent = new Promise((resolve, reject) => {
    if (expectedLiveEventId === undefined) {
      resolve()
      return
    }
    const timeout = setTimeout(
      () =>
        reject(new Error(`live event ${expectedLiveEventId} did not arrive`)),
      20_000,
    )
    page.on('websocket', socket => {
      if (socket.url() !== relayUrl) return
      let eoseSeen = false
      socket.on('framereceived', frame => {
        if (typeof frame.payload !== 'string') return
        const parsed = JSON.parse(frame.payload)
        if (parsed[0] === 'EOSE') {
          eoseSeen = true
          return
        }
        if (
          eoseSeen &&
          parsed[0] === 'EVENT' &&
          parsed[2]?.id === expectedLiveEventId
        ) {
          clearTimeout(timeout)
          resolve()
        }
      })
    })
  })

  const response = await page.goto(targetUrl, { waitUntil: 'domcontentloaded' })
  if (response === null || !response.ok()) {
    throw new Error(
      `project page failed to load: ${response?.status() ?? 'no response'}`,
    )
  }
  await eose
  if (expectedLiveEventId !== undefined) {
    process.stdout.write(
      `${JSON.stringify({
        eoseObserved: true,
        waitingForLiveEvent: expectedLiveEventId,
      })}\n`,
    )
  }
  await liveEvent

  const req = relayFrames.find(frame => {
    if (frame.direction !== 'sent' || typeof frame.payload !== 'string')
      return false
    const parsed = JSON.parse(frame.payload)
    return parsed[0] === 'REQ' && parsed[1] === 'dh-project-v1'
  })
  if (req === undefined) throw new Error('browser sent no bounded project REQ')
  if (projectApiRequests.length > 0) {
    throw new Error(
      `project page called an OpenAgents API: ${projectApiRequests.join(', ')}`,
    )
  }
  if (browserErrors.length > 0) {
    throw new Error(`browser errors: ${browserErrors.join(' | ')}`)
  }

  process.stdout.write(
    `${JSON.stringify({
      targetUrl,
      relayUrl,
      reqObserved: true,
      eoseObserved: true,
      liveEventObserved: expectedLiveEventId !== undefined,
      projectApiRequests: 0,
      relayFrameCount: relayFrames.length,
    })}\n`,
  )
} finally {
  await browser.close()
}
