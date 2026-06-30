import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  nativeSelect,
  nativeSelectOptgroup,
  nativeSelectOption,
} from './native-select'
import { renderHtml } from './test-helpers'

describe('basecoat native select components', () => {
  test('renders native select markup with Basecoat class, size, and state attrs', () => {
    const rendered = renderHtml(
      nativeSelect({
        id: 'timezone',
        name: 'timezone',
        size: 'sm',
        required: true,
        invalid: true,
        describedBy: 'timezone-description',
        children: [
          nativeSelectOption({
            value: 'utc',
            selected: true,
            children: ['UTC'],
          }),
          nativeSelectOption({
            value: 'america-chicago',
            children: ['America/Chicago'],
          }),
        ],
      }),
    )

    expect(rendered).toContain('<select')
    expect(rendered).toContain('class="select"')
    expect(rendered).toContain('id="timezone"')
    expect(rendered).toContain('name="timezone"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('required')
    expect(rendered).toContain('aria-invalid="true"')
    expect(rendered).toContain('aria-describedby="timezone-description"')
    expect(rendered).toContain('<option value="utc" selected>UTC</option>')
    expect(rendered).toContain('<option value="america-chicago">America/Chicago</option>')
  })

  test('renders optgroups, disabled states, and multiple selection', () => {
    const rendered = renderHtml(
      nativeSelect({
        ariaLabel: 'Deployment region',
        multiple: true,
        disabled: true,
        className: 'w-full',
        children: [
          nativeSelectOptgroup({
            label: 'US',
            children: [
              nativeSelectOption({
                value: 'iad',
                children: ['Washington, DC'],
              }),
            ],
          }),
          nativeSelectOptgroup({
            label: 'EU',
            disabled: true,
            children: [
              nativeSelectOption({
                value: 'fra',
                disabled: true,
                children: ['Frankfurt'],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="select w-full"')
    expect(rendered).toContain('aria-label="Deployment region"')
    expect(rendered).toContain('multiple')
    expect(rendered).toContain('disabled')
    expect(rendered).not.toContain('data-size="default"')
    expect(rendered).toContain('<optgroup label="US">')
    expect(rendered).toContain('<optgroup label="EU" disabled>')
    expect(rendered).toContain('<option value="fra" disabled>Frankfurt</option>')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.nativeSelect).toBe(nativeSelect)
    expect(Basecoat.nativeSelectOption).toBe(nativeSelectOption)
    expect(Basecoat.nativeSelectOptgroup).toBe(nativeSelectOptgroup)
  })
})
