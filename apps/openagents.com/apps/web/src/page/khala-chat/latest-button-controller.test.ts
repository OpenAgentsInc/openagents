import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  installKhalaChatLatestButtonController,
  khalaChatLatestButtonIsActionable,
} from './latest-button-controller'

const flushMutations = async (): Promise<void> => {
  await Promise.resolve()
  await Promise.resolve()
}

const defineScrollMetrics = (
  region: HTMLElement,
  metrics: {
    scrollHeight: number
    clientHeight: number
    scrollTop: number
  },
): void => {
  Object.defineProperties(region, {
    scrollHeight: {
      configurable: true,
      get: () => metrics.scrollHeight,
    },
    clientHeight: {
      configurable: true,
      get: () => metrics.clientHeight,
    },
    scrollTop: {
      configurable: true,
      get: () => metrics.scrollTop,
      set: value => {
        metrics.scrollTop = Number(value)
      },
    },
  })
}

const makeChatNodes = (metrics: {
  scrollHeight: number
  clientHeight: number
  scrollTop: number
}): {
  root: HTMLElement
  region: HTMLElement
  button: HTMLButtonElement
  metrics: typeof metrics
} => {
  const root = document.createElement('div')
  root.setAttribute('data-khala-chat', '')

  const region = document.createElement('div')
  region.setAttribute('data-khala-chat-scroll-region', '')
  defineScrollMetrics(region, metrics)

  const button = document.createElement('button')
  button.setAttribute('data-khala-chat-latest-button', '')
  button.hidden = true

  root.append(region, button)
  return { root, region, button, metrics }
}

let teardown: (() => void) | null = null

beforeEach(() => {
  document.body.innerHTML = ''
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => {
    callback(0)
    return 1
  })
  vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {})
})

afterEach(() => {
  teardown?.()
  teardown = null
  vi.restoreAllMocks()
})

describe('Khala chat latest button controller', () => {
  test('marks the jump button actionable only when newer content is below view', () => {
    const { region, metrics } = makeChatNodes({
      scrollHeight: 700,
      clientHeight: 300,
      scrollTop: 120,
    })

    expect(khalaChatLatestButtonIsActionable(region)).toBe(true)

    metrics.scrollTop = 380
    expect(khalaChatLatestButtonIsActionable(region)).toBe(false)

    metrics.scrollHeight = 280
    metrics.clientHeight = 300
    metrics.scrollTop = 0
    expect(khalaChatLatestButtonIsActionable(region)).toBe(false)
  })

  test('starts hidden when the transcript is already at latest', () => {
    const { root, button } = makeChatNodes({
      scrollHeight: 700,
      clientHeight: 300,
      scrollTop: 400,
    })
    document.body.append(root)

    teardown = installKhalaChatLatestButtonController()

    expect(button.hidden).toBe(true)
    expect(button.dataset.khalaChatLatestActionable).toBe('false')
  })

  test('shows while the scroll region has newer hidden content below', () => {
    const { root, button } = makeChatNodes({
      scrollHeight: 700,
      clientHeight: 300,
      scrollTop: 0,
    })
    document.body.append(root)

    teardown = installKhalaChatLatestButtonController()

    expect(button.hidden).toBe(false)
    expect(button.dataset.khalaChatLatestActionable).toBe('true')
  })

  test('hides again after the user scrolls back to the latest content', () => {
    const { root, region, button, metrics } = makeChatNodes({
      scrollHeight: 700,
      clientHeight: 300,
      scrollTop: 0,
    })
    document.body.append(root)

    teardown = installKhalaChatLatestButtonController()
    expect(button.hidden).toBe(false)

    metrics.scrollTop = 400
    region.dispatchEvent(new Event('scroll'))

    expect(button.hidden).toBe(true)
    expect(button.dataset.khalaChatLatestActionable).toBe('false')
  })

  test('attaches to the chat after a route render', async () => {
    teardown = installKhalaChatLatestButtonController()

    const { root, button } = makeChatNodes({
      scrollHeight: 700,
      clientHeight: 300,
      scrollTop: 0,
    })
    document.body.append(root)
    await flushMutations()

    expect(button.hidden).toBe(false)
  })
})
