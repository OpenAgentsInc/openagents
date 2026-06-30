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
  test('keeps the empty composer compact before users start typing', async () => {
    const css = await Bun.file(
      new URL('../src/ai-elements/command-composer.css', import.meta.url),
    ).text()

    expect(css).toContain('--oa-command-composer-height: 8rem')
    expect(css).toContain('min-height: 4rem')
    expect(css).toContain('padding: 0.75rem')
    expect(css).not.toContain('--oa-command-composer-height: 10rem')
    expect(css).not.toContain('min-height: 7rem')
  })

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
                previewUrl: 'blob:screen',
                dimensions: { width: 320, height: 200 },
              },
            ],
          },
        },
        props: {
          name: 'prompt',
          preview: true,
          status: 'streaming',
          selectedAttachmentId: 'att-1',
          dragActive: true,
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
    expect(rendered).toContain('src="blob:screen"')
    expect(rendered).toContain('width="320"')
    expect(rendered).toContain('height="200"')
    expect(rendered).toContain('data-selected="true"')
    expect(rendered).toContain('data-oa-command-composer-gapcursor="before"')
    expect(rendered).toContain('data-oa-command-composer-gapcursor="after"')
    expect(rendered).toContain('data-oa-command-composer-dropcursor=""')
    expect(rendered).toContain('data-oa-command-composer-drag-active="true"')
    expect(rendered).toContain('data-oa-command-composer-attachment-action="preview"')
    expect(rendered).toContain('data-oa-command-composer-attachment-action="remove"')
    expect(rendered).toContain('<strong')
    expect(rendered).toContain('Hello')
    expect(rendered).toContain('data-oa-command-composer-submit="stop"')
    expect(rendered).toContain('type="button"')
  })

  test('renders error attachment retry action without claiming readiness', () => {
    const rendered = renderHtml(
      AiElements.commandComposer({
        props: { name: 'prompt' },
        attachments: [
          {
            id: 'att-error',
            kind: 'file',
            name: 'archive.zip',
            mime: 'application/zip',
            sizeBytes: 4096,
            status: 'error',
            errorText: 'Upload failed',
          },
        ],
      }),
    )

    expect(rendered).toContain('archive.zip')
    expect(rendered).toContain('Error')
    expect(rendered).toContain('Upload failed')
    expect(rendered).toContain('data-status="error"')
    expect(rendered).toContain('data-oa-command-composer-attachment-action="retry"')
    expect(rendered).toContain('data-oa-command-composer-attachment-action="remove"')
    expect(rendered).not.toContain(
      'oa-ai-command-composer-attachment-status"><node>Ready',
    )
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

  test('keeps a 100k-character prompt editable', () => {
    const largePrompt = 'x'.repeat(100_000)
    const rendered = renderHtml(
      AiElements.commandComposer({
        props: {
          name: 'prompt',
          value: largePrompt,
          sizeLabel: '100 KB',
        },
      }),
    )

    expect(rendered).toContain(`>${largePrompt}</node>`)
    expect(rendered).toContain('100 KB')
    expect(rendered).toContain('100000 characters')
    expect(rendered).not.toContain('disabled="true"')
  })
})
