import { describe, expect, test } from 'bun:test'

import { Basecoat } from '../index'
import {
  field,
  fieldDescription,
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
  test('renders inputs and textareas with Basecoat classes and native attrs', () => {
    const rendered = renderHtml(
      field({
        children: [
          label({ for: 'email', children: ['Email'] }),
          input({
            id: 'email',
            name: 'email',
            type: 'email',
            placeholder: 'm@example.com',
            required: true,
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

  test('renders field, fieldset, description, section, and separator markup', () => {
    const rendered = renderHtml(
      fieldset({
        children: [
          field({
            orientation: 'responsive',
            invalid: true,
            children: [
              fieldSection({
                children: [
                  label({ for: 'username', children: ['Username'] }),
                  fieldDescription({
                    id: 'username-error',
                    alert: true,
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

  test('renders input group addons with alignment and vertical orientation', () => {
    const rendered = renderHtml(
      inputGroup({
        orientation: 'vertical',
        role: 'group',
        children: [
          textarea({ placeholder: 'Write a comment...' }),
          inputGroupAddon({
            element: 'header',
            align: 'block-start',
            children: ['script.js'],
          }),
          inputGroupAddon({
            element: 'footer',
            align: 'block-end',
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
    expect(Basecoat.fieldSection).toBe(fieldSection)
    expect(Basecoat.fieldDescription).toBe(fieldDescription)
    expect(Basecoat.fieldSeparator).toBe(fieldSeparator)
    expect(Basecoat.inputGroup).toBe(inputGroup)
    expect(Basecoat.inputGroupAddon).toBe(inputGroupAddon)
  })
})
