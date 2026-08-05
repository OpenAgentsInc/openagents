import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act } from 'react'
import { type Root, createRoot } from 'react-dom/client'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import { TASSADAR_AGENT_INSTRUCTIONS, TassadarPage } from './-tassadar-page'

;(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true

let container: HTMLDivElement | null = null
let root: Root | null = null

const stubClipboard = (writeText: () => Promise<void>): void => {
  Object.defineProperty(globalThis.navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
    writable: true,
  })
}

const mountTassadarPage = async (): Promise<HTMLDivElement> => {
  container = document.createElement('div')
  document.body.appendChild(container)
  root = createRoot(container)
  await act(async () => {
    root?.render(<TassadarPage />)
  })
  return container
}

beforeEach(() => {
  container = null
  root = null
})

afterEach(async () => {
  if (root !== null) {
    await act(async () => {
      root?.unmount()
    })
  }
  container?.remove()
  vi.restoreAllMocks()
})

describe('Start /tassadar route', () => {
  test('server-renders the landing content instead of a mount shim', () => {
    const html = renderToStaticMarkup(<TassadarPage />)

    expect(html).toContain('data-route="tassadar"')
    expect(html).toContain('aria-label="Tassadar - OpenAgents training run"')
    expect(html).toContain('OpenAgents Training Run')
    expect(html).toMatch(/<h1[^>]*>Tassadar<\/h1>/)
    expect(html).toContain(
      'open, distributed AI model training run. Agents and Pylons claim bounded work, independent validators replay accepted work, and small spend-capped Lightning settlements are recorded with public receipts.',
    )
    expect(html).toContain('LLM-computer idea')
    expect(html).toContain('01 What Tassadar is')
    expect(html).toContain('02 How to join')
  })

  test('renders the trust cards and the back link to home', () => {
    const html = renderToStaticMarkup(<TassadarPage />)

    expect(html).toContain('Open and joinable')
    expect(html).toContain(
      'Install Pylon, check the run status, and claim an open lease.',
    )
    expect(html).toContain('Verified by replay')
    expect(html).toContain(
      'A separate validator re-executes work and compares digests.',
    )
    expect(html).toContain('Paid in Bitcoin')
    expect(html).toContain(
      'Accepted work settles over Lightning with dereferenceable receipts.',
    )
    expect(html).toContain('href="/"')
    expect(html).toContain('← OpenAgents')
  })

  test('agent instructions render as preformatted text, verbatim', () => {
    const html = renderToStaticMarkup(<TassadarPage />)

    expect(html).toMatch(/<pre[^>]*><code>/)
    expect(html).toContain(
      'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
    )
    expect(html).toContain(
      'pylon training claim --base-url https://openagents.com --lease-seconds 300',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'curl -X POST https://openagents.com/api/agents/register',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain('npx @openagentsinc/pylon')
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'Accepted work is paid in Bitcoin over Lightning, with public receipts.',
    )
  })

  test('the copy control is a real button, labelled for the idle state', () => {
    const html = renderToStaticMarkup(<TassadarPage />)

    expect(html).toMatch(/<button[^>]*type="button"/)
    expect(html).toContain('Copy Agent Instructions')
    expect(html).not.toContain('>Copied<')
    expect(html).toContain('Hand this to your agent to get started.')
  })

  test('pressing copy writes the instructions and flips the label to Copied', async () => {
    const writeText = vi.fn(() => Promise.resolve())
    stubClipboard(writeText)

    const mounted = await mountTassadarPage()
    const button = mounted.querySelector('button')
    expect(button?.textContent).toBe('Copy Agent Instructions')

    await act(async () => {
      button?.click()
    })

    expect(writeText).toHaveBeenCalledWith(TASSADAR_AGENT_INSTRUCTIONS)
    expect(mounted.querySelector('button')?.textContent).toBe('Copied')
  })

  test('a rejected clipboard write still flips the label instead of breaking the page', async () => {
    const writeText = vi.fn(() => Promise.reject(new Error('denied')))
    stubClipboard(writeText)

    const mounted = await mountTassadarPage()

    await act(async () => {
      mounted.querySelector('button')?.click()
    })

    expect(writeText).toHaveBeenCalledTimes(1)
    expect(mounted.querySelector('button')?.textContent).toBe('Copied')
  })

  test('the page module carries no Effect Native dependency', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-tassadar-page.tsx'),
      'utf8',
    )

    expect(source).not.toContain('@effect-native')
    expect(source).not.toContain('lucide-react')
  })

  test('the route shell mounts the plain-React tassadar page', () => {
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/tassadar.tsx'),
      'utf8',
    )

    expect(routeSource).toContain("from './-tassadar-page'")
    expect(routeSource).toContain('TassadarPage')
    expect(routeSource).not.toContain('effect-native')
  })
})
