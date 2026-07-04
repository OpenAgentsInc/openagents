import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import * as KhalaCodeDownload from './page/khalaCodeDownload'
import { KhalaCodeDownloadRoute, urlToAppRoute } from './route'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
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
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe('khala code download route', () => {
  test('parses the public /code/download path', () => {
    expect(urlToAppRoute(appUrl('/code/download'))).toEqual(
      KhalaCodeDownloadRoute(),
    )
  })

  test('keeps unauthenticated users on the install-truth page with no auth bootstrap', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/code/download'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'KhalaCodeDownload' },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders Codex-required install paths and the exact-row counter link', () => {
    const rendered = renderHtml(
      KhalaCodeDownload.view({ _tag: 'LoggedOut' }),
    )

    expect(rendered).toContain('data-route="khala-code-download"')
    expect(rendered).toContain(KhalaCodeDownload.CODEX_INSTALL_COMMAND)
    expect(rendered).toContain(KhalaCodeDownload.CODEX_LOGIN_COMMAND)
    expect(rendered).toContain(
      KhalaCodeDownload.KHALA_CODE_CLI_INSTALL_COMMAND,
    )
    expect(rendered).toContain('public artifact pending')
    expect(rendered).toContain(
      KhalaCodeDownload.KHALA_CODE_DOWNLOAD_COUNTER_ENDPOINT,
    )
    expect(rendered).toContain('empty counts array')
  })

  test('keeps public copy inside the khala_code desktop wrapper promise gate', () => {
    const rendered = renderHtml(
      KhalaCodeDownload.view({ _tag: 'LoggedOut' }),
    )
    const visibleCopy = rendered.replace(/<[^>]+>/g, ' ')

    expect(rendered).toContain(
      KhalaCodeDownload.KHALA_CODE_DOWNLOAD_COPY_GATE.promiseId,
    )
    expect(rendered).toContain(
      KhalaCodeDownload.KHALA_CODE_DOWNLOAD_COPY_GATE.safeCopy,
    )
    expect(
      KhalaCodeDownload.khalaCodeDownloadCopyViolations(visibleCopy),
    ).toEqual([])

    expect(
      KhalaCodeDownload.khalaCodeDownloadCopyViolations(
        'Khala Code works without Codex and the paid plan is live.',
      ).map(violation => violation.phraseRef),
    ).toEqual([
      'phrase.public.khala_code.works_without_codex',
      'phrase.public.khala_code.live_plan_economics',
    ])
  })
})
