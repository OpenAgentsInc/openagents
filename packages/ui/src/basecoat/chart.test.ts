import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  chartInit,
  chartPoints,
  chartUpdate,
  chartView,
  type ChartMessage,
} from './chart'
import { renderHtml } from './test-helpers'

const message = (input: ChartMessage): ChartMessage => input

const data = [
  { month: 'Jan', desktop: 186, mobile: 80 },
  { month: 'Feb', desktop: 305, mobile: 200 },
  { month: 'Mar', desktop: 237, mobile: 120 },
]

const series = {
  desktop: { label: 'Desktop', color: 'var(--chart-1)' },
  mobile: { label: 'Mobile', color: 'var(--chart-2)' },
}

describe('basecoat chart component', () => {
  test('renders Basecoat chart markup with canvas data, tooltip, and legend', () => {
    const model = chartInit({
      type: 'bar',
      labelKey: 'month',
      data,
      series,
      open: true,
      activePointId: 'desktop-0',
      focusedSeriesKey: 'desktop',
    })

    const rendered = renderHtml(
      chartView({
        model,
        id: 'visitors-chart',
        ariaLabel: 'Monthly visitors',
        legend: true,
        toMessage: message,
      }),
    )

    expect(rendered).toContain('class="chart"')
    expect(rendered).toContain('<canvas')
    expect(rendered).toContain('id="visitors-chart"')
    expect(rendered).toContain('role="img"')
    expect(rendered).toContain('aria-label="Monthly visitors"')
    expect(rendered).toContain('data-chart-type="bar"')
    expect(rendered).toContain('data-chart-label-key="month"')
    expect(rendered).toContain('data-chart-point-count="6"')
    expect(rendered).toContain('class="chart-tooltip"')
    expect(rendered).toContain('class="chart-tooltip-title"')
    expect(rendered).toContain('Jan')
    expect(rendered).toContain('Desktop')
    expect(rendered).toContain('186')
    expect(rendered).toContain('class="chart-legend"')
    expect(rendered).toContain('class="chart-legend-item"')
    expect(rendered).toContain('data-series-key="desktop"')
    expect(rendered).toContain('aria-pressed')
    expect(rendered).toContain('class="chart-tooltip-indicator"')
    expect(rendered).toContain('class="chart-legend-indicator"')
  })

  test('normalizes rows and series into visible chart points', () => {
    const model = chartInit({
      labelKey: 'month',
      data,
      series,
      hiddenSeriesKeys: ['mobile'],
    })

    expect(chartPoints(model)).toEqual([
      {
        id: 'desktop-0',
        label: 'Jan',
        seriesKey: 'desktop',
        seriesLabel: 'Desktop',
        value: 186,
        color: 'var(--chart-1)',
        index: 0,
      },
      {
        id: 'desktop-1',
        label: 'Feb',
        seriesKey: 'desktop',
        seriesLabel: 'Desktop',
        value: 305,
        color: 'var(--chart-1)',
        index: 1,
      },
      {
        id: 'desktop-2',
        label: 'Mar',
        seriesKey: 'desktop',
        seriesLabel: 'Desktop',
        value: 237,
        color: 'var(--chart-1)',
        index: 2,
      },
    ])
  })

  test('opens, closes, focuses, and selects points through update messages', () => {
    const model = chartInit({ labelKey: 'month', data, series })

    const opened = chartUpdate(model, {
      _tag: 'ChartOpened',
      pointId: 'mobile-1',
    })
    expect(opened.open).toBe(true)
    expect(opened.activePointId).toBe('mobile-1')

    const focused = chartUpdate(opened, {
      _tag: 'ChartFocused',
      pointId: 'desktop-2',
    })
    expect(focused.focusedPointId).toBe('desktop-2')
    expect(focused.activePointId).toBe('desktop-2')

    const selected = chartUpdate(focused, {
      _tag: 'ChartSelected',
      pointId: 'desktop-2',
    })
    expect(selected.selectedPointId).toBe('desktop-2')
    expect(selected.open).toBe(true)

    const closed = chartUpdate(selected, { _tag: 'ChartClosed' })
    expect(closed.open).toBe(false)
    expect(closed.activePointId).toBeNull()
  })

  test('navigates chart points with keyboard keys', () => {
    const model = chartInit({
      labelKey: 'month',
      data,
      series,
      activePointId: 'desktop-0',
    })

    const next = chartUpdate(model, {
      _tag: 'ChartKeyDown',
      key: 'ArrowRight',
    })
    expect(next.activePointId).toBe('mobile-0')
    expect(next.open).toBe(true)

    const end = chartUpdate(next, { _tag: 'ChartKeyDown', key: 'End' })
    expect(end.activePointId).toBe('mobile-2')

    const selected = chartUpdate(end, { _tag: 'ChartKeyDown', key: 'Enter' })
    expect(selected.selectedPointId).toBe('mobile-2')

    const closed = chartUpdate(selected, { _tag: 'ChartKeyDown', key: 'Escape' })
    expect(closed.open).toBe(false)
  })

  test('toggles legend series and preserves keyboard focus', () => {
    const model = chartInit({
      labelKey: 'month',
      data,
      series,
      focusedSeriesKey: 'desktop',
    })

    const moved = chartUpdate(model, {
      _tag: 'ChartLegendKeyDown',
      key: 'ArrowRight',
    })
    expect(moved.focusedSeriesKey).toBe('mobile')

    const hidden = chartUpdate(moved, {
      _tag: 'ChartLegendKeyDown',
      key: 'Enter',
    })
    expect(hidden.hiddenSeriesKeys).toEqual(['mobile'])
    expect(hidden.focusedSeriesKey).toBe('mobile')

    const shown = chartUpdate(hidden, {
      _tag: 'ChartLegendToggled',
      seriesKey: 'mobile',
    })
    expect(shown.hiddenSeriesKeys).toEqual([])
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.chartInit).toBe(chartInit)
    expect(Basecoat.chartUpdate).toBe(chartUpdate)
    expect(Basecoat.chartView).toBe(chartView)
  })
})
