import path from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  cacheControlForPathname,
  contentTypeForPath,
  resolveStaticFilePath,
  staticAssetResponse,
} from './static'

const CLIENT_DIR = '/srv/aiur/dist/client'

describe('resolveStaticFilePath', () => {
  test('resolves a plain asset path inside the client dir', () => {
    expect(resolveStaticFilePath('/assets/index-abc.js', CLIENT_DIR)).toBe(
      path.join(CLIENT_DIR, 'assets', 'index-abc.js'),
    )
  })

  test('never escapes the client dir via ..', () => {
    const resolved = resolveStaticFilePath(
      '/assets/../../../etc/passwd',
      CLIENT_DIR,
    )
    expect(
      resolved === undefined || resolved.startsWith(`${CLIENT_DIR}${path.sep}`),
    ).toBe(true)
  })

  test('never escapes the client dir via encoded ..', () => {
    const resolved = resolveStaticFilePath(
      '/%2e%2e/%2e%2e/etc/passwd',
      CLIENT_DIR,
    )
    expect(
      resolved === undefined || resolved.startsWith(`${CLIENT_DIR}${path.sep}`),
    ).toBe(true)
  })

  test('rejects directory-ish and malformed paths', () => {
    expect(resolveStaticFilePath('/assets/', CLIENT_DIR)).toBeUndefined()
    expect(resolveStaticFilePath('/%zz', CLIENT_DIR)).toBeUndefined()
    expect(resolveStaticFilePath('/a%00b', CLIENT_DIR)).toBeUndefined()
  })
})

describe('contentTypeForPath / cacheControlForPathname', () => {
  test('maps common build outputs', () => {
    expect(contentTypeForPath('/x/app.js')).toContain('text/javascript')
    expect(contentTypeForPath('/x/app.css')).toContain('text/css')
    expect(contentTypeForPath('/x/shell.html')).toContain('text/html')
    expect(contentTypeForPath('/x/icon.svg')).toBe('image/svg+xml')
    expect(contentTypeForPath('/x/unknown.bin')).toBe(
      'application/octet-stream',
    )
  })

  test('hashed /assets/ get immutable caching, others do not', () => {
    expect(cacheControlForPathname('/assets/index-abc.js')).toContain(
      'immutable',
    )
    expect(cacheControlForPathname('/icon.svg')).not.toContain('immutable')
  })
})

describe('staticAssetResponse', () => {
  const files = new Map<string, Uint8Array>([
    [
      path.join(CLIENT_DIR, 'assets', 'index-abc.js'),
      new TextEncoder().encode('console.log(1)'),
    ],
  ])

  const readFake = async (filePath: string): Promise<Uint8Array> => {
    const found = files.get(filePath)
    if (found === undefined) throw new Error('ENOENT')
    return found
  }

  test('serves an existing file with content-type and cache headers', async () => {
    const response = await staticAssetResponse(
      new Request('https://aiur.openagents.com/assets/index-abc.js'),
      CLIENT_DIR,
      readFake,
    )
    expect(response?.status).toBe(200)
    expect(response?.headers.get('content-type')).toContain('text/javascript')
    expect(response?.headers.get('cache-control')).toContain('immutable')
    expect(await response?.text()).toBe('console.log(1)')
  })

  test('falls through (undefined) for missing files and non-GET', async () => {
    expect(
      await staticAssetResponse(
        new Request('https://aiur.openagents.com/assets/missing.js'),
        CLIENT_DIR,
        readFake,
      ),
    ).toBeUndefined()
    expect(
      await staticAssetResponse(
        new Request('https://aiur.openagents.com/assets/index-abc.js', {
          method: 'POST',
        }),
        CLIENT_DIR,
        readFake,
      ),
    ).toBeUndefined()
  })
})
