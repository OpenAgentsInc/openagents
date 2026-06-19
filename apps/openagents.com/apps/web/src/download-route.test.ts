import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import * as Download from './page/download'
import { DownloadRoute, urlToAppRoute } from './route'

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

describe('download route', () => {
  test('parses the public /download path', () => {
    expect(urlToAppRoute(appUrl('/download'))).toEqual(DownloadRoute())
  })

  test('keeps unauthenticated users on the download page with no auth bootstrap', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/download'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Download' },
    })
    // Public route: no auth bootstrap command is dispatched.
    expect(commands).toHaveLength(0)
  })

  test('renders the signed DMG download link, honest platform copy, and the Pylon alternative', () => {
    const rendered = renderHtml(Download.view({ _tag: 'LoggedOut' }))

    // The discoverable download points at the published signed DMG asset.
    expect(rendered).toContain('data-cta="download-autopilot"')
    expect(rendered).toContain(Download.AUTOPILOT_DESKTOP_DMG_URL)
    expect(rendered).toContain('Download for Mac (Apple Silicon)')

    // Honest platform availability.
    expect(rendered).toContain('macOS · Apple Silicon')
    expect(rendered).toContain('macOS · Intel')
    expect(rendered).toContain('Not published yet')
    expect(rendered).toContain('Windows')
    expect(rendered).toContain('Pending the Authenticode signing certificate')
    expect(rendered).toContain('Linux')

    // Pylon-CLI alternative stays available.
    expect(rendered).toContain('npx @openagentsinc/pylon')
  })

  test('one-click experience is gated until the fresh signed DMG ships', () => {
    // Hard gate: until the owner builds + signs a fresh DMG from current main,
    // the page must NOT imply one-click auto-onboarding works.
    expect(Download.DOWNLOAD_ONE_CLICK_READY).toBe(false)

    const rendered = renderHtml(Download.view({ _tag: 'LoggedOut' }))
    expect(rendered).toContain('data-download-status="gated"')
    expect(rendered).toContain('Status: auto-onboarding not in this build yet')
  })
})
