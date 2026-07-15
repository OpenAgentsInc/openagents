import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after, before, test } from 'node:test'

import { makeRequestHandler, resolveDocsFile } from './server.mjs'

const root = mkdtempSync(join(tmpdir(), 'openagents-docs-'))
mkdirSync(join(root, 'guide'), { recursive: true })
mkdirSync(join(root, '_astro'), { recursive: true })
writeFileSync(join(root, 'index.html'), '<h1>Docs</h1>')
writeFileSync(join(root, 'guide', 'index.html'), '<h1>Guide</h1>')
writeFileSync(join(root, '_astro', 'app.abc.css'), 'body{}')
writeFileSync(join(root, '404.html'), '<h1>Missing</h1>')

let origin
let server

before(async () => {
  server = createServer(makeRequestHandler(root))
  await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen))
  const address = server.address()
  origin = `http://127.0.0.1:${address.port}`
})

after(async () => {
  await new Promise((resolveClose, rejectClose) =>
    server.close((error) => error === undefined ? resolveClose() : rejectClose(error)),
  )
})

test('resolves root, nested clean URLs, and exact assets beneath /docs', () => {
  assert.equal(resolveDocsFile(root, '/docs'), join(root, 'index.html'))
  assert.equal(resolveDocsFile(root, '/docs/guide'), join(root, 'guide', 'index.html'))
  assert.equal(resolveDocsFile(root, '/docs/_astro/app.abc.css'), join(root, '_astro', 'app.abc.css'))
})

test('rejects paths outside the docs mount and traversal attempts', () => {
  assert.equal(resolveDocsFile(root, '/'), undefined)
  assert.equal(resolveDocsFile(root, '/docs/%2e%2e/secret'), undefined)
  assert.equal(resolveDocsFile(root, '/docs/%E0%A4%A'), undefined)
})

test('serves HTML, immutable assets, HEAD, health, and docs-owned 404', async () => {
  const rootResponse = await fetch(`${origin}/docs`)
  assert.equal(rootResponse.status, 200)
  assert.match(rootResponse.headers.get('content-type'), /^text\/html/)
  assert.equal(await rootResponse.text(), '<h1>Docs</h1>')

  const assetResponse = await fetch(`${origin}/docs/_astro/app.abc.css`, { method: 'HEAD' })
  assert.equal(assetResponse.status, 200)
  assert.equal(assetResponse.headers.get('cache-control'), 'public, max-age=31536000, immutable')
  assert.equal(await assetResponse.text(), '')

  const healthResponse = await fetch(`${origin}/internal/healthz`)
  assert.deepEqual(await healthResponse.json(), { ok: true, service: 'openagents-docs' })

  const missingResponse = await fetch(`${origin}/docs/missing`)
  assert.equal(missingResponse.status, 404)
  assert.equal(await missingResponse.text(), '<h1>Missing</h1>')
})

test('allows only GET and HEAD', async () => {
  const response = await fetch(`${origin}/docs`, { method: 'POST' })
  assert.equal(response.status, 405)
  assert.equal(response.headers.get('allow'), 'GET, HEAD')
})
