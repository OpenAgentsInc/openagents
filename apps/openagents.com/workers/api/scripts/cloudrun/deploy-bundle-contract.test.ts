import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

describe('Cloud Run Vite Plus bundle contract', () => {
  test('preserves the server bundle while packing preload', () => {
    const deployScript = readFileSync(
      fileURLToPath(new URL('../deploy-cloudrun.sh', import.meta.url)),
      'utf8',
    )

    expect(deployScript).toMatch(
      /vp pack src\/cloudrun\/preload\.ts[\s\S]*?--no-clean/,
    )
    expect(deployScript).toContain('! -f dist-cloudrun/server.mjs')
    expect(deployScript).toContain('! -f dist-cloudrun/preload.mjs')
  })

  test('ships Vite Plus split chunks beside the server entry', () => {
    const dockerfile = readFileSync(
      fileURLToPath(new URL('../../Dockerfile', import.meta.url)),
      'utf8',
    )

    expect(dockerfile).toContain('COPY dist-cloudrun/*.mjs ./dist-cloudrun/')
  })
})
