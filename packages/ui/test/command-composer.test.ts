import { describe, expect, test } from 'bun:test'
import {
  composerAttachmentId,
  emptyComposerState,
} from '@openagentsinc/composer-state'
import type { Html } from 'foldkit/html'

import { AiElements } from '../src/index'

type VNodeLike = {
  sel?: string
  data?: {
    attrs?: Record<string, unknown>
    class?: Record<string, boolean>
    props?: Record<string, unknown>
    style?: Record<string, string>
  }
  children?: ReadonlyArray<VNodeLike | string>
  text?: string
}

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const styles = node.data?.style ?? {}
  const classNames = Object.entries(node.data?.class ?? {}).flatMap(
    ([className, enabled]) => (enabled ? [className] : []),
  )
  const entries = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(Object.keys(styles).length === 0
      ? []
      : [
          [
            'style',
            Object.entries(styles)
              .map(([key, value]) => `${key}:${value}`)
              .join(';'),
          ],
        ]),
    ...(classNames.length > 0 ? [['class', classNames.join(' ')]] : []),
  ]
  return entries
    .map(([key, value]) =>
      value === '' ? ` ${key}=""` : ` ${key}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) return ''
  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child => (typeof child === 'string' ? child : renderHtml(child as Html)))
    .join('')
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

describe('ai-elements command composer', () => {
  test('renders the shared composer contract with native text editing intact', () => {
    const rendered = renderHtml(
      AiElements.commandComposer({
        props: {
          name: 'chat-prompt',
          label: 'Message Khala',
          placeholder: 'Send a message',
          value: 'Run `bun test` after this.',
          rows: 4,
          autofocus: true,
          heightPx: 220,
          tokenEstimate: 42,
          keymapLabel: 'Enter sends',
        },
      }),
    )

    expect(rendered).toContain('ai-elements:command-composer/CommandComposer')
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerFrame',
    )
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerTextarea',
    )
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerSubmit',
    )
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerResizeHandle',
    )

    expect(rendered).toContain('name="chat-prompt"')
    expect(rendered).toContain('placeholder="Send a message"')
    expect(rendered).toContain('autofocus="true"')
    expect(rendered).toContain(
      'data-oa-command-composer-native-editing="true"',
    )
    expect(rendered).toContain(
      'data-oa-command-composer-focus-after-submit=""',
    )
    expect(rendered).not.toContain('disabled="true"')
    expect(rendered).toContain('--oa-command-composer-height:220px')
    expect(rendered).toContain('42 tok')
    expect(rendered).toContain('Enter sends')
  })

  test('renders attachments and markdown preview from the state contract', () => {
    const state = emptyComposerState()
    const rendered = renderHtml(
      AiElements.commandComposer({
        state: {
          ...state,
          doc: {
            ...state.doc,
            blocks: [
              {
                id: state.doc.blocks[0]?.id ?? ('block-1' as never),
                kind: 'paragraph',
                text: '**Hello** from composer',
                marks: [],
              },
            ],
            attachments: [
              {
                id: composerAttachmentId('att-1'),
                kind: 'image',
                name: 'screen.png',
                mime: 'image/png',
                sizeBytes: 1536,
                status: 'ready',
              },
            ],
          },
        },
        props: {
          name: 'prompt',
          preview: true,
          status: 'streaming',
        },
      }),
    )

    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerRail',
    )
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerAttachment',
    )
    expect(rendered).toContain(
      'ai-elements:command-composer/CommandComposerMarkdownPreview',
    )
    expect(rendered).toContain('screen.png')
    expect(rendered).toContain('image/png - 1.5 KB')
    expect(rendered).toContain('<strong')
    expect(rendered).toContain('Hello')
    expect(rendered).toContain('data-oa-command-composer-submit="stop"')
    expect(rendered).toContain('type="button"')
  })

  test('keeps text editable while a turn is submitted', () => {
    const rendered = renderHtml(
      AiElements.commandComposer({
        props: {
          name: 'prompt',
          value: 'I can keep typing here',
          status: 'submitted',
        },
      }),
    )

    expect(rendered).toContain('I can keep typing here')
    expect(rendered).toContain('data-oa-command-composer-status="submitted"')
    expect(rendered).toContain('data-oa-command-composer-submit="stop"')
    expect(rendered).not.toContain('textarea disabled="true"')
    expect(rendered).not.toContain('disabled="true"')
  })
})
