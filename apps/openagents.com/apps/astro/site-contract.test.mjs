import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const appRoot = dirname(fileURLToPath(import.meta.url))
const readSource = (...parts) =>
  readFileSync(join(appRoot, 'src', ...parts), 'utf8')

test('human landing navigation uses docs rather than product promises', () => {
  const layout = readSource('layouts', 'Layout.astro')
  const page = readSource('pages', 'index.astro')

  assert.doesNotMatch(layout, />Promises</)
  assert.doesNotMatch(layout, />Product promises</)
  assert.doesNotMatch(page, />Product promises</)
  assert.match(layout, /class="brand" href="\/"/)
  assert.match(layout, /href=\{DOCS_URL\}>Docs</)
  assert.match(page, /href=\{DOCS_URL\}>\s*Read the docs/)
})

test('the landing foundation uses the canonical Khala theme background', () => {
  const layout = readSource('layouts', 'Layout.astro')

  assert.match(layout, /<meta name="theme-color" content="#05070d"/)
  assert.match(layout, /--void:\s*#05070d;/)
  assert.doesNotMatch(layout, /--void:\s*#000000;/)
})
