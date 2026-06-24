import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import {
  AiElements,
  button,
  headingBlock,
  inputGroup,
  linkButton,
  textareaGroup,
  workroomChatRoute,
  workroomTimeline,
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

describe('component class migration coverage', () => {
  test('renders shared button and heading primitives through the class helper', () => {
    const rendered = renderHtml(
      headingBlock({
        eyebrow: 'Classes',
        title: 'Primitive surface',
        body: 'Token-backed styles, same Foldkit API.',
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

  test('renders first form controls through the class helper', () => {
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

  test('renders prompt-input AI Elements through the class helper', () => {
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

  test('renders workroom timeline surfaces through shared component classes', () => {
    const rendered = renderHtml(
      workroomChatRoute(
        workroomTimeline({
          messages: [
            {
              id: 'message-class-user',
              author: 'user',
              label: 'Operator',
              time: 'now',
              status: 'complete',
              parts: [{ kind: 'text', body: ['Ship the migration.'] }],
            },
            {
              id: 'message-class-agent',
              author: 'assistant',
              label: 'Autopilot',
              time: 'now',
              status: 'streaming',
              parts: [
                { kind: 'text', body: ['Working through the gate.'] },
                {
                  kind: 'tool',
                  title: 'Shell',
                  subtitle: 'shell command',
                  status: 'running',
                  detail: ['bun run check:deploy'],
                },
                {
                  kind: 'diff',
                  files: [
                    {
                      path: 'apps/openagents.com/apps/web/src/styles.css',
                      added: 2,
                      removed: 200,
                      status: 'modified',
                    },
                  ],
                },
                {
                  kind: 'file',
                  path: 'packages/ui/src/workroom-styles.ts',
                  language: 'ts',
                  excerpt: ['export const workroomStyles = {}'],
                },
              ],
            },
          ],
        }),
      ),
    )

    expect(rendered).toContain('oa-ui-workroom-chat-surface')
    expect(rendered).toContain('oa-ui-workroom-session-turn')
    expect(rendered).toContain('oa-ui-workroom-text-part')
    expect(rendered).toContain('oa-ui-workroom-bash-output')
    expect(rendered).toContain('oa-ui-workroom-tool-trigger')
    expect(rendered).toContain('oa-ui-workroom-diffs')
    expect(rendered).toContain('oa-ui-workroom-write-content')
    expect(rendered).toContain('data-component="session-turn"')
    expect(rendered).toContain('data-slot="session-turn-diff-filename"')
  })
})
