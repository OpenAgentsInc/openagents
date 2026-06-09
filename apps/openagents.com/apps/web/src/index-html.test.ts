/// <reference types="vite/client" />
import { describe, expect, test } from 'vitest'

const indexHtml = import.meta.glob<string>('../index.html', {
  eager: true,
  import: 'default',
  query: '?raw',
})

describe('index html', () => {
  test('loads the Fathom analytics script on the app shell', () => {
    const html = indexHtml['../index.html'] ?? ''

    expect(html).toContain('src="https://cdn.usefathom.com/script.js"')
    expect(html).toContain('data-site="IVAXCCIT"')
    expect(html).toContain('defer')
  })
})
