import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  alert,
  alertDescription,
  alertFooter,
  alertTitle,
  avatar,
  avatarBadge,
  avatarFallback,
  avatarGroup,
  avatarGroupCount,
  avatarImage,
  skeleton,
} from './display'
import { renderHtml } from './test-helpers'

describe('basecoat display components', () => {
  test('renders Basecoat alerts with shadcn destructive variant slots', () => {
    const rendered = renderHtml(
      alert({
        variant: 'destructive',
        children: [
          alertTitle({ level: 4, children: ['Build failed'] }),
          alertDescription({ children: ['Verification did not pass'] }),
          alertFooter({ children: ['Retry'] }),
        ],
      }),
    )

    expect(rendered).toContain('<div')
    expect(rendered).toContain('class="alert"')
    expect(rendered).toContain('role="alert"')
    expect(rendered).toContain('data-variant="destructive"')
    expect(rendered).toContain('<h4>Build failed</h4>')
    expect(rendered).toContain('<section>Verification did not pass</section>')
    expect(rendered).toContain('<footer>Retry</footer>')
  })

  test('renders avatar image, fallback, badge, and group count selectors', () => {
    const rendered = renderHtml(
      avatarGroup({
        children: [
          avatar({
            size: 'sm',
            children: [
              avatarImage({
                src: 'https://example.com/avatar.png',
                alt: 'AtlantisPleb',
              }),
              avatarFallback({ children: ['AP'] }),
              avatarBadge({ children: ['1'] }),
            ],
          }),
          avatarGroupCount({ children: ['+3'] }),
        ],
      }),
    )

    expect(rendered).toContain('class="avatar-group"')
    expect(rendered).toContain('class="avatar"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('src="https://example.com/avatar.png"')
    expect(rendered).toContain('alt="AtlantisPleb"')
    expect(rendered).toContain('<span>AP</span>')
    expect(rendered).toContain('class="avatar-badge"')
    expect(rendered).toContain('data-count=""')
    expect(rendered).toContain('+3')
  })

  test('renders skeleton placeholders', () => {
    const rendered = renderHtml(
      skeleton({
        className: 'h-4 w-24',
      }),
    )

    expect(rendered).toContain('<div')
    expect(rendered).toContain('class="skeleton h-4 w-24"')
  })

  test('omits default Basecoat variant and size data attributes', () => {
    const rendered = renderHtml(
      alert({
        variant: 'default',
        children: [
          alertTitle({ children: ['Heads up'] }),
          avatar({ size: 'default', children: [avatarFallback({ children: ['OA'] })] }),
        ],
      }),
    )

    expect(rendered).not.toContain('data-variant="default"')
    expect(rendered).not.toContain('data-size="default"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.alert).toBe(alert)
    expect(Basecoat.alertTitle).toBe(alertTitle)
    expect(Basecoat.alertDescription).toBe(alertDescription)
    expect(Basecoat.alertFooter).toBe(alertFooter)
    expect(Basecoat.avatar).toBe(avatar)
    expect(Basecoat.avatarImage).toBe(avatarImage)
    expect(Basecoat.avatarFallback).toBe(avatarFallback)
    expect(Basecoat.avatarBadge).toBe(avatarBadge)
    expect(Basecoat.avatarGroup).toBe(avatarGroup)
    expect(Basecoat.avatarGroupCount).toBe(avatarGroupCount)
    expect(Basecoat.skeleton).toBe(skeleton)
  })
})
