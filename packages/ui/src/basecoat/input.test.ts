import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  field,
  fieldDescription,
  fieldError,
  fieldLegend,
  fieldSection,
  fieldSeparator,
  fieldset,
  input,
  inputGroup,
  inputGroupAddon,
  label,
  textarea,
} from './input'
import { renderHtml } from './test-helpers'

describe('basecoat input components', () => {
  test('renders standalone input, textarea, and label classes', () => {
    const rendered = renderHtml(
      field({
        children: [
          label({ htmlFor: 'email', children: ['Email'] }),
          input({
            id: 'email',
            name: 'email',
            type: 'email',
            placeholder: 'agent@example.com',
            autocomplete: 'email',
            required: true,
          }),
          textarea({
            id: 'notes',
            name: 'notes',
            rows: 4,
            placeholder: 'Notes',
            value: 'Ready',
          }),
        ],
      }),
    )

    expect(rendered).toContain('<div role="group" class="field">')
    expect(rendered).toContain('<label htmlFor="email" class="label">Email</label>')
    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('type="email"')
    expect(rendered).toContain('autocomplete="email"')
    expect(rendered).toContain('required')
    expect(rendered).toContain('class="textarea"')
    expect(rendered).toContain('rows="4"')
    expect(rendered).toContain('>Ready</textarea>')
  })

  test('renders field states, fieldsets, helper text, errors, and separators', () => {
    const rendered = renderHtml(
      fieldset({
        children: [
          fieldLegend({ variant: 'label', children: ['Profile'] }),
          field({
            orientation: 'horizontal',
            invalid: true,
            children: [
              fieldSection({
                children: [
                  label({ htmlFor: 'username', children: ['Username'] }),
                  fieldDescription({
                    id: 'username-desc',
                    children: ['Choose a public handle.'],
                  }),
                ],
              }),
              input({
                id: 'username',
                type: 'text',
                describedBy: 'username-error',
                invalid: true,
              }),
              fieldError({
                id: 'username-error',
                children: ['Username is already taken'],
              }),
            ],
          }),
          fieldSeparator({ children: ['Billing'] }),
        ],
      }),
    )

    expect(rendered).toContain('<fieldset class="fieldset">')
    expect(rendered).toContain('<legend data-variant="label">Profile</legend>')
    expect(rendered).toContain('data-orientation="horizontal"')
    expect(rendered).toContain('data-invalid="true"')
    expect(rendered).toContain('<section>')
    expect(rendered).toContain('aria-describedby="username-error"')
    expect(rendered).toContain('aria-invalid="true"')
    expect(rendered).toContain('<p role="alert" id="username-error">Username is already taken</p>')
    expect(rendered).toContain('class="field-separator"')
    expect(rendered).toContain('<hr role="separator"></hr>')
    expect(rendered).toContain('<span>Billing</span>')
  })

  test('renders input groups with orientation and aligned addons', () => {
    const rendered = renderHtml(
      inputGroup({
        orientation: 'vertical',
        children: [
          textarea({
            control: true,
            placeholder: 'Write a comment...',
          }),
          inputGroupAddon({
            as: 'footer',
            align: 'end',
            children: ['120 characters left'],
          }),
          inputGroupAddon({
            align: 'start',
            ariaHidden: true,
            children: ['Search'],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="input-group"')
    expect(rendered).toContain('data-orientation="vertical"')
    expect(rendered).toContain('data-control=""')
    expect(rendered).toContain('<footer data-align="end">120 characters left</footer>')
    expect(rendered).toContain('<span data-align="start" aria-hidden="true">Search</span>')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.input).toBe(input)
    expect(Basecoat.textarea).toBe(textarea)
    expect(Basecoat.label).toBe(label)
    expect(Basecoat.field).toBe(field)
    expect(Basecoat.fieldset).toBe(fieldset)
    expect(Basecoat.fieldLegend).toBe(fieldLegend)
    expect(Basecoat.fieldDescription).toBe(fieldDescription)
    expect(Basecoat.fieldError).toBe(fieldError)
    expect(Basecoat.fieldSection).toBe(fieldSection)
    expect(Basecoat.fieldSeparator).toBe(fieldSeparator)
    expect(Basecoat.inputGroup).toBe(inputGroup)
    expect(Basecoat.inputGroupAddon).toBe(inputGroupAddon)
  })
})
