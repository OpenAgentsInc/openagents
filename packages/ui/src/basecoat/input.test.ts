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
            autocomplete: 'email',
            id: 'email',
            name: 'email',
            placeholder: 'agent@example.com',
            required: true,
            type: 'email',
          }),
          textarea({
            id: 'notes',
            name: 'notes',
            placeholder: 'Notes',
            rows: 4,
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

  test('renders inputs and textareas with Basecoat classes and native attrs', () => {
    const rendered = renderHtml(
      field({
        children: [
          label({ for: 'email', children: ['Email'] }),
          input({
            id: 'email',
            name: 'email',
            placeholder: 'm@example.com',
            required: true,
            type: 'email',
          }),
          textarea({
            id: 'message',
            name: 'message',
            placeholder: 'Message',
            rows: 4,
            value: 'Hello',
          }),
        ],
      }),
    )

    expect(rendered).toContain('<div role="group" class="field">')
    expect(rendered).toContain('<label htmlFor="email" class="label">Email</label>')
    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('type="email"')
    expect(rendered).toContain('placeholder="m@example.com"')
    expect(rendered).toContain('required')
    expect(rendered).toContain('class="textarea"')
    expect(rendered).toContain('rows="4"')
    expect(rendered).toContain('Hello')
  })

  test('renders field states, fieldsets, helper text, errors, and separators', () => {
    const rendered = renderHtml(
      fieldset({
        children: [
          fieldLegend({ variant: 'label', children: ['Profile'] }),
          field({
            invalid: true,
            orientation: 'horizontal',
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
                describedBy: 'username-error',
                id: 'username',
                invalid: true,
                type: 'text',
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

  test('renders field, fieldset, description, section, and separator markup', () => {
    const rendered = renderHtml(
      fieldset({
        children: [
          field({
            invalid: true,
            orientation: 'responsive',
            children: [
              fieldSection({
                children: [
                  label({ for: 'username', children: ['Username'] }),
                  fieldDescription({
                    alert: true,
                    id: 'username-error',
                    children: ['Choose another username.'],
                  }),
                ],
              }),
              input({
                id: 'username',
                value: 'taken',
                attrs: [
                  Basecoat.basecoatDataAttr('debug-invalid', 'true')[0],
                ],
              }),
            ],
          }),
          fieldSeparator(),
        ],
      }),
    )

    expect(rendered).toContain('<fieldset class="fieldset">')
    expect(rendered).toContain('data-orientation="responsive"')
    expect(rendered).toContain('data-invalid="true"')
    expect(rendered).toContain('<section>')
    expect(rendered).toContain('<p role="alert" id="username-error">')
    expect(rendered).toContain('data-debug-invalid="true"')
    expect(rendered).toContain('<div class="field-separator"><hr role="separator"></hr></div>')
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
            align: 'end',
            as: 'footer',
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

  test('renders input group addons with alignment and vertical orientation', () => {
    const rendered = renderHtml(
      inputGroup({
        orientation: 'vertical',
        role: 'group',
        children: [
          textarea({ placeholder: 'Write a comment...' }),
          inputGroupAddon({
            align: 'block-start',
            element: 'header',
            children: ['script.js'],
          }),
          inputGroupAddon({
            align: 'block-end',
            element: 'footer',
            children: ['0/280'],
          }),
        ],
      }),
    )

    expect(rendered).toContain('role="group"')
    expect(rendered).toContain('class="input-group"')
    expect(rendered).toContain('data-orientation="vertical"')
    expect(rendered).toContain('<header data-align="block-start">script.js</header>')
    expect(rendered).toContain('<footer data-align="block-end">0/280</footer>')
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
