import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import {
  AiElements,
  button,
  headingBlock,
  inputGroup,
  linkButton,
  textareaGroup,
} from '../src/index'

type VNodeLike = {
  sel?: string
  data?: {
    attrs?: Record<string, unknown>
    class?: Record<string, boolean>
    props?: Record<string, unknown>
  }
  children?: ReadonlyArray<VNodeLike | string>
  text?: string
}

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classNames = Object.entries(node.data?.class ?? {}).flatMap(
    ([className, enabled]) => (enabled ? [className] : []),
  )
  const entries = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classNames.length > 0 ? [['class', classNames.join(' ')]] : []),
  ]

  if (entries.length === 0) {
    return ''
  }

  return entries
    .map(([key, value]) =>
      value === '' ? ` ${key}=""` : ` ${key}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string' ? child : renderHtml(child as Html),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe('StyleX migration coverage', () => {
  test('renders shared button and heading primitives through the StyleX adapter', () => {
    const rendered = renderHtml(
      headingBlock({
        eyebrow: 'StyleX',
        title: 'Primitive surface',
        body: 'Compiled styles, same Foldkit API.',
        level: 1,
      }),
    )
    const primary = renderHtml(button({ label: 'Ship' }))
    const secondary = renderHtml(
      linkButton({
        href: '/docs',
        label: 'Read',
        variant: 'secondary',
        size: 'sm',
      }),
    )

    expect(rendered).toContain('oa-ui-heading-root')
    expect(rendered).toContain('oa-ui-heading-title')
    expect(rendered).toContain('Primitive surface')
    expect(primary).toContain('oa-ui-button')
    expect(primary).toContain('oa-ui-button-primary')
    expect(secondary).toContain('href="/docs"')
    expect(secondary).toContain('oa-ui-button-secondary')
  })

  test('renders first form controls through the StyleX adapter', () => {
    const rendered = renderHtml(
      inputGroup({
        id: 'email',
        name: 'email',
        label: 'Email',
        placeholder: 'ops@example.com',
        help: 'Used for receipts.',
      }),
    )
    const textarea = renderHtml(
      textareaGroup({
        id: 'brief',
        name: 'brief',
        label: 'Brief',
        value: 'Do the thing.',
      }),
    )

    expect(rendered).toContain('data-ui-family="forms/input-groups"')
    expect(rendered).toContain('oa-ui-form-group')
    expect(rendered).toContain('oa-ui-form-input')
    expect(textarea).toContain('data-ui-family="forms/textareas"')
    expect(textarea).toContain('oa-ui-form-textarea')
  })

  test('renders prompt-input AI Elements through the StyleX adapter', () => {
    const rendered = renderHtml(
      AiElements.promptInput({
        props: {
          name: 'prompt',
          placeholder: 'Ask the agent',
          value: 'Summarize this.',
          status: 'ready',
        },
        tools: [AiElements.promptInputButton({ label: 'Attach' })],
      }),
    )

    expect(rendered).toContain('data-ui-base="ai-elements:prompt-input/PromptInput"')
    expect(rendered).toContain('oa-ai-prompt-input')
    expect(rendered).toContain('oa-ai-prompt-input-textarea')
    expect(rendered).toContain('oa-ai-prompt-input-button')
    expect(rendered).toContain('oa-ai-prompt-input-submit')
  })
})
