import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  collapsible,
  collapsibleContent,
  collapsibleTrigger,
  scrollbar,
  tooltip,
} from './tooltip'
import { renderHtml } from './test-helpers'

describe('basecoat tooltip, scrollbar, and collapsible components', () => {
  test('renders tooltip data attributes on the selected element', () => {
    const rendered = renderHtml(
      tooltip({
        element: 'button',
        tooltip: 'Archive thread',
        side: 'inline-end',
        align: 'start',
        ariaLabel: 'Archive',
        className: 'inline-flex',
        children: ['Archive'],
      }),
    )

    expect(rendered).toContain('<button')
    expect(rendered).toContain('type="button"')
    expect(rendered).toContain('data-tooltip="Archive thread"')
    expect(rendered).toContain('data-side="inline-end"')
    expect(rendered).toContain('data-align="start"')
    expect(rendered).toContain('aria-label="Archive"')
    expect(rendered).toContain('class="inline-flex"')
  })

  test('renders scrollbar size classes with semantic container options', () => {
    const rendered = renderHtml(
      scrollbar({
        element: 'section',
        size: 'sm',
        role: 'region',
        ariaLabel: 'Activity',
        children: ['Scrollable content'],
      }),
    )

    expect(rendered).toContain('<section')
    expect(rendered).toContain('class="scrollbar-sm"')
    expect(rendered).toContain('role="region"')
    expect(rendered).toContain('aria-label="Activity"')
    expect(rendered).toContain('Scrollable content')
  })

  test('renders native details collapsible with trigger and content slots', () => {
    const rendered = renderHtml(
      collapsible({
        open: true,
        className: 'group',
        summaryClassName: 'cursor-pointer',
        contentClassName: 'pt-2',
        trigger: ['Advanced'],
        children: ['Hidden settings'],
      }),
    )

    expect(rendered).toContain('<details')
    expect(rendered).toContain('open=""')
    expect(rendered).toContain('class="group"')
    expect(rendered).toContain('<summary')
    expect(rendered).toContain('data-slot="collapsible-trigger"')
    expect(rendered).toContain('class="cursor-pointer"')
    expect(rendered).toContain('Advanced')
    expect(rendered).toContain('data-slot="collapsible-content"')
    expect(rendered).toContain('class="pt-2"')
    expect(rendered).toContain('Hidden settings')
  })

  test('renders collapsible slots independently', () => {
    const rendered = renderHtml(
      collapsibleContent({
        children: [
          collapsibleTrigger({
            children: ['Standalone slot'],
          }),
        ],
      }),
    )

    expect(rendered).toContain('data-slot="collapsible-content"')
    expect(rendered).toContain('data-slot="collapsible-trigger"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.tooltip).toBe(tooltip)
    expect(Basecoat.scrollbar).toBe(scrollbar)
    expect(Basecoat.collapsible).toBe(collapsible)
    expect(Basecoat.collapsibleTrigger).toBe(collapsibleTrigger)
    expect(Basecoat.collapsibleContent).toBe(collapsibleContent)
  })
})
