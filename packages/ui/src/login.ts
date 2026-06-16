import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { inputGroup } from './forms'
import { button, linkButton } from './shared'

// A composed login form (email + send-sign-in-link), built from the kit's
// input-group and button primitives so the auth surface inherits the same
// dark-only / compact-mono / thin-border design contract. Reusable both in the
// component library and on the real /login page.
export type LoginFormProps<Message> = Readonly<{
  emailValue?: string
  emailPlaceholder?: string
  submitLabel?: string
  // Attributes for the <form> element (e.g. method/action, or a Message hook).
  formAttrs?: ReadonlyArray<Attribute<Message>>
  // Extra attributes for the submit button (the form submit type is always set).
  submitAttrs?: ReadonlyArray<Attribute<Message>>
  // When provided, render a secondary "Continue with GitHub" button.
  githubHref?: string
}>

export const loginForm = <Message>(
  props: LoginFormProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.form(
    [...(props.formAttrs ?? []), h.Class('grid w-full gap-4')],
    [
      inputGroup<Message>({
        id: 'login-email',
        name: 'email',
        label: 'Email address',
        type: 'email',
        ...(props.emailValue === undefined ? {} : { value: props.emailValue }),
        placeholder: props.emailPlaceholder ?? 'you@example.com',
        attrs: [h.Required(true)],
      }),
      button<Message>({
        label: props.submitLabel ?? 'Send sign-in link',
        variant: 'primary',
        block: true,
        attrs: [h.Type('submit'), ...(props.submitAttrs ?? [])],
      }),
      ...(props.githubHref === undefined
        ? []
        : [
            linkButton<Message>({
              href: props.githubHref,
              label: 'Continue with GitHub',
              variant: 'secondary',
              block: true,
            }),
          ]),
    ],
  )
}

// Full-page centered login screen: the OpenAgents wordmark above the login form
// on a pure-black background. This is what the real /login page renders.
export const loginScreen = <Message>(
  props: LoginFormProps<Message> = {},
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.Class(
        'grid min-h-dvh place-items-center bg-[#000] px-4 py-16 text-[#f1efe8]',
      ),
    ],
    [
      h.div(
        [h.Class('grid w-full max-w-[400px] gap-10')],
        [
          h.div(
            [
              h.Class(
                'text-center text-2xl font-medium tracking-tight text-[#f1efe8]',
              ),
            ],
            ['OpenAgents'],
          ),
          loginForm<Message>(props),
        ],
      ),
    ],
  )
}
