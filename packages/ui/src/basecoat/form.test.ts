import { describe, expect, test } from 'bun:test'
import { html } from 'foldkit/html'

import { Basecoat } from '../index'
import { button } from './button'
import {
  form,
  formDescription,
  formField,
  formFieldGroup,
  formOptionLabel,
  formSection,
  formSectionTitle,
  formSwitchContent,
  formSwitchField,
  formSwitchLabel,
} from './form'
import { input, label, textarea } from './input'
import { renderHtml } from './test-helpers'

describe('basecoat form components', () => {
  test('renders the Basecoat form root with native form attributes', () => {
    const rendered = renderHtml(
      form({
        id: 'profile-form',
        name: 'profile',
        action: '/profile',
        method: 'post',
        target: '_self',
        ariaLabel: 'Profile settings',
        className: 'max-w-sm',
        children: [
          formField({
            children: [
              label({ for: 'username', children: ['Username'] }),
              input({
                id: 'username',
                name: 'username',
                placeholder: 'hunvreus',
              }),
              formDescription({
                id: 'username-help',
                children: ['This is your public display name.'],
              }),
            ],
          }),
          button({ type: 'submit', children: ['Submit'] }),
        ],
      }),
    )

    expect(rendered).toContain('<form')
    expect(rendered).toContain('id="profile-form"')
    expect(rendered).toContain('name="profile"')
    expect(rendered).toContain('action="/profile"')
    expect(rendered).toContain('method="post"')
    expect(rendered).toContain('target="_self"')
    expect(rendered).toContain('aria-label="Profile settings"')
    expect(rendered).toContain('class="form grid gap-6 max-w-sm"')
    expect(rendered).toContain('<div class="grid gap-2">')
    expect(rendered).toContain('<label htmlFor="username" class="label">Username</label>')
    expect(rendered).toContain('class="input"')
    expect(rendered).toContain('<p id="username-help" class="text-muted-foreground text-sm">')
    expect(rendered).toContain('<button type="submit" class="btn">Submit</button>')
  })

  test('renders Basecoat form sections, option labels, and switch rows', () => {
    const rendered = renderHtml(
      form({
        children: [
          formField({
            children: [
              label({ for: 'bio', children: ['Bio'] }),
              textarea({ id: 'bio', name: 'bio', rows: 3 }),
              formDescription({
                children: ['You can @mention other users and organizations.'],
              }),
            ],
          }),
          formFieldGroup({
            children: [
              label({ for: 'notify', children: ['Notify me about...'] }),
              formOptionLabel({
                children: [
                  input({
                    id: 'notify-all',
                    name: 'notify',
                    type: 'radio',
                    value: 'all',
                    checked: true,
                  }),
                  'All new messages',
                ],
              }),
            ],
          }),
          formSection({
            children: [
              formSectionTitle({ children: ['Email Notifications'] }),
              formSwitchField({
                children: [
                  formSwitchContent({
                    disabled: true,
                    children: [
                      formSwitchLabel({
                        for: 'security-email',
                        children: ['Security emails'],
                      }),
                      formDescription({
                        children: ['Receive emails about your account security.'],
                      }),
                    ],
                  }),
                  input({
                    id: 'security-email',
                    name: 'security-email',
                    type: 'checkbox',
                    disabled: true,
                    attrs: [html().Role('switch')],
                  }),
                ],
              }),
            ],
          }),
        ],
      }),
    )

    expect(rendered).toContain('class="form grid gap-6"')
    expect(rendered).toContain('<textarea')
    expect(rendered).toContain('rows="3"')
    expect(rendered).toContain('<div class="flex flex-col gap-3">')
    expect(rendered).toContain('<label class="font-normal">')
    expect(rendered).toContain('type="radio"')
    expect(rendered).toContain('checked')
    expect(rendered).toContain('<section class="grid gap-4">')
    expect(rendered).toContain('<h3 class="text-lg font-medium">Email Notifications</h3>')
    expect(rendered).toContain('class="gap-2 flex flex-row items-start justify-between rounded-lg border p-4 shadow-xs"')
    expect(rendered).toContain('class="flex flex-col gap-0.5 opacity-60"')
    expect(rendered).toContain('<label htmlFor="security-email" class="leading-normal">Security emails</label>')
    expect(rendered).toContain('role="switch"')
    expect(rendered).toContain('disabled')
  })

  test('is exported from the Basecoat namespace', () => {
    expect(Basecoat.form).toBe(form)
    expect(Basecoat.formField).toBe(formField)
    expect(Basecoat.formFieldGroup).toBe(formFieldGroup)
    expect(Basecoat.formSection).toBe(formSection)
    expect(Basecoat.formSectionTitle).toBe(formSectionTitle)
    expect(Basecoat.formDescription).toBe(formDescription)
    expect(Basecoat.formOptionLabel).toBe(formOptionLabel)
    expect(Basecoat.formSwitchField).toBe(formSwitchField)
    expect(Basecoat.formSwitchContent).toBe(formSwitchContent)
    expect(Basecoat.formSwitchLabel).toBe(formSwitchLabel)
  })
})
