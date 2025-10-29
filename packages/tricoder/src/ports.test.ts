import test from 'node:test'
import assert from 'node:assert/strict'
import net from 'node:net'
import { findAvailablePort, isPortAvailable } from './ports.js'

test('isPortAvailable returns boolean for an unused port', async () => {
  const base = 47321
  const ok = await isPortAvailable(base)
  assert.equal(typeof ok, 'boolean')
})

test('findAvailablePort skips a busy port', async () => {
  const base = 48321
  const srv = net.createServer()
  await new Promise<void>((resolve, reject) => srv.listen({ port: base, host: '0.0.0.0' }, () => resolve()))
  try {
    const p = await findAvailablePort(base, 3)
    assert.notEqual(p, base)
  } finally {
    await new Promise<void>((resolve) => srv.close(() => resolve()))
  }
})
