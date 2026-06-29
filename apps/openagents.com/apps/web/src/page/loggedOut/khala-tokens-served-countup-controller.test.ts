import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import { installKhalaTokensServedCountUp } from './khala-tokens-served-countup-controller'

// Integration test for the DOM controller against happy-dom. We force reduced
// motion so the animator SNAPS deterministically (no fake-clock needed) and we
// can assert the controller wires the target through to the displayed text on
// both initial attach and on later `data-value` mutations — including a node
// that is added to the DOM AFTER install (route re-render).

const makeCounterNode = (value: string): HTMLElement => {
  const span = document.createElement('span')
  span.setAttribute('data-counter-display', 'khala-tokens-served')
  span.setAttribute('data-value', value)
  span.textContent = value
  return span
}

const flushMutations = async (): Promise<void> => {
  // MutationObserver callbacks are microtask-scheduled; yield to let them run.
  await Promise.resolve()
  await Promise.resolve()
}

let teardown: (() => void) | null = null

beforeEach(() => {
  document.body.innerHTML = ''
  // Reduced motion → the animator snaps straight to the target.
  vi.stubGlobal(
    'matchMedia',
    (query: string) =>
      ({
        matches: query.includes('prefers-reduced-motion'),
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
      }) as unknown as MediaQueryList,
  )
})

afterEach(() => {
  teardown?.()
  teardown = null
  vi.unstubAllGlobals()
})

describe('khala count-up controller (DOM, #6324)', () => {
  test('shows the initial target on an element present at install', async () => {
    const node = makeCounterNode('1,000')
    document.body.appendChild(node)

    teardown = installKhalaTokensServedCountUp()
    await flushMutations()

    expect(node.textContent).toBe('1,000')
  })

  test('updates the displayed value when data-value changes (no freeze)', async () => {
    const node = makeCounterNode('1,000')
    document.body.appendChild(node)

    teardown = installKhalaTokensServedCountUp()
    await flushMutations()

    // Simulate the view pushing the next ≤3/sec server total.
    node.setAttribute('data-value', '1,334')
    await flushMutations()
    expect(node.textContent).toBe('1,334')

    // A larger post-burst reconcile delta still lands on the exact target.
    node.setAttribute('data-value', '9,001,334')
    await flushMutations()
    expect(node.textContent).toBe('9,001,334')
  })

  test('attaches to a counter node added after install (route re-render)', async () => {
    teardown = installKhalaTokensServedCountUp()
    await flushMutations()

    const node = makeCounterNode('42')
    document.body.appendChild(node)
    await flushMutations()

    node.setAttribute('data-value', '500')
    await flushMutations()
    expect(node.textContent).toBe('500')
  })

  test('is a no-op for the em-dash placeholder until a real value arrives', async () => {
    const node = makeCounterNode('—')
    document.body.appendChild(node)

    teardown = installKhalaTokensServedCountUp()
    await flushMutations()
    // placeholder stays as-is
    expect(node.textContent).toBe('—')

    node.setAttribute('data-value', '7')
    node.textContent = '—'
    await flushMutations()
    expect(node.textContent).toBe('7')
  })
})
