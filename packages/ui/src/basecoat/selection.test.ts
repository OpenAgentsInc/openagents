import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  checkbox,
  checkboxGroup,
  radio,
  radioGroup,
  switch as basecoatSwitch,
  switchControl,
} from './selection'
import { renderHtml } from './test-helpers'

describe('basecoat selection components', () => {
  test('renders checkbox input markup with Basecoat input class and state attrs', () => {
    const rendered = renderHtml(
      checkbox({
        id: 'terms-checkbox',
        name: 'terms',
        checked: true,
        required: true,
        invalid: true,
        describedBy: 'terms-description',
      }),
    )

    expect(rendered).toContain('<input')
    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('type="checkbox"')
    expect(rendered).toContain('id="terms-checkbox"')
    expect(rendered).toContain('name="terms"')
    expect(rendered).toContain('checked')
    expect(rendered).toContain('required')
    expect(rendered).toContain('aria-invalid="true"')
    expect(rendered).toContain('aria-describedby="terms-description"')
  })

  test('renders radio group and radio item markup', () => {
    const rendered = renderHtml(
      radioGroup({
        ariaLabel: 'View density',
        className: 'w-fit',
        children: [
          radio({
            id: 'density-comfortable',
            name: 'density',
            value: 'comfortable',
            checked: true,
          }),
        ],
      }),
    )

    expect(rendered).toContain('role="radiogroup"')
    expect(rendered).toContain('aria-label="View density"')
    expect(rendered).toContain('data-slot="radio-group"')
    expect(rendered).toContain('class="w-fit"')
    expect(rendered).toContain('type="radio"')
    expect(rendered).toContain('value="comfortable"')
  })

  test('renders switch as a checkbox with role switch and Basecoat size attr', () => {
    const rendered = renderHtml(
      switchControl({
        id: 'airplane-mode',
        ariaLabel: 'Airplane mode',
        size: 'sm',
        disabled: true,
      }),
    )

    expect(rendered).toContain('<input')
    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('type="checkbox"')
    expect(rendered).toContain('role="switch"')
    expect(rendered).toContain('data-size="sm"')
    expect(rendered).toContain('disabled')
    expect(rendered).toContain('aria-label="Airplane mode"')
  })

  test('renders checkbox group slot markup', () => {
    const rendered = renderHtml(
      checkboxGroup({
        ariaLabel: 'Desktop items',
        children: [
          checkbox({
            id: 'finder-hard-disks',
            name: 'finder-hard-disks',
            checked: true,
          }),
        ],
      }),
    )

    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('aria-label="Desktop items"')
    expect(rendered).toContain('data-slot="checkbox-group"')
    expect(rendered).toContain('type="checkbox"')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.checkbox).toBe(checkbox)
    expect(Basecoat.checkboxGroup).toBe(checkboxGroup)
    expect(Basecoat.radio).toBe(radio)
    expect(Basecoat.radioGroup).toBe(radioGroup)
    expect(Basecoat.switch).toBe(basecoatSwitch)
    expect(Basecoat.switchControl).toBe(switchControl)
  })
})
