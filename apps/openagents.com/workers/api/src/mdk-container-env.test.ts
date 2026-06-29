import { describe, expect, test } from 'vitest'
import { readFile } from 'node:fs/promises'

import { mdkContainerEnvVars } from './mdk-container-env'

describe('MDK sidecar container env', () => {
  test('passes the revenue withdrawal destination only as a container env secret', () => {
    expect(
      mdkContainerEnvVars({
        MDK_ACCESS_TOKEN: ' access-token ',
        MDK_MNEMONIC: ' mnemonic ',
        MDK_WEBHOOK_SECRET: ' webhook-secret ',
        WITHDRAWAL_DESTINATION: ' treasury-destination ',
      }),
    ).toEqual({
      MDK_ACCESS_TOKEN: 'access-token',
      MDK_MNEMONIC: 'mnemonic',
      MDK_WEBHOOK_SECRET: 'webhook-secret',
      WITHDRAWAL_DESTINATION: 'treasury-destination',
    })
  })

  test('omits blank sidecar secrets instead of pinning empty values', () => {
    expect(
      mdkContainerEnvVars({
        MDK_ACCESS_TOKEN: ' ',
        MDK_MNEMONIC: '',
        MDK_WEBHOOK_SECRET: undefined,
        WITHDRAWAL_DESTINATION: '   ',
      }),
    ).toEqual({})
  })
})

describe('MDK sidecar route contract', () => {
  test('keeps dashboard webhooks on the core route handler inside the container', async () => {
    const source = await readFile(
      new URL('../../../services/mdk-sidecar/src/server.mjs', import.meta.url),
      'utf8',
    )

    expect(source).toContain(
      "import { GET, POST } from '@moneydevkit/core/route'",
    )
    expect(source).toContain("url.pathname = '/api/mdk'")
    expect(source).toContain('return POST(await toMdkRequest(request))')
    expect(source).toContain('return GET(await toMdkRequest(request))')
    expect(source).toContain('withdrawalDestinationConfigured')
    expect(source).not.toContain('JSON.stringify(process.env)')
  })
})
