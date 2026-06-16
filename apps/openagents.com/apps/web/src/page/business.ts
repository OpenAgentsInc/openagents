import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Public `openagents.com/business` landing page + signup form.
//
// This renders in the real Foldkit app shell on `@openagentsinc/ui` and is
// reachable logged-out, mirroring the docs/blog/components public-style routes.
// It honors the dark-only / pure-black / compact-mono / thin-border design
// contract documented in `packages/ui/src/README.md`.
//
// The signup form is a plain server-posted HTML form (no Foldkit message
// wiring) so the page stays static and public. It posts to a placeholder
// intake endpoint; the real intake/handoff is owned by other issues (see the
// clearly-marked TODOs below).
//
// SCOPE / STUBS (do NOT implement here -- these are separate issues):
//   - C1 (#5092): "create a prefilled workspace on submit" handoff. On submit
//     we should seed a prefilled Autopilot workspace from the signup fields and
//     hand the user into it. NOT built here; the form posts to a no-op/intake
//     placeholder for now.
//   - C2 (#5093): lead-enrichment hook via our own API (operator seeding,
//     invite link, engagement tracking). NOT built here.
//   - C4 (#5095): the actual shared-Slack-channel Slack Connect invite. This
//     page only renders the OPT-IN checkbox; sending the Slack Connect invite
//     is a separate issue and is NOT built here.
//
// Naming note: keep all copy/code generic product language. Do NOT reference
// any partner/company/person names anywhere.

// Placeholder intake endpoint. TODO(#5092/C1, #5093/C2): replace this no-op
// target with the real intake + prefilled-workspace handoff plus the
// lead-enrichment hook. For now the form posts here so the page is functional
// without implementing other issues.
const intakeAction = '/api/public/business-signup'

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const sectionLabelClass = 'm-0 font-mono text-base text-white/35 sm:text-sm'

const fieldLabelClass = 'font-mono text-xs uppercase tracking-wide text-white/45'

const fieldInputClass =
  'mt-1.5 w-full min-w-0 border border-[#222] bg-[#030303] px-3 py-2.5 font-mono text-[0.8125rem] leading-[1.35] text-[#f1efe8] outline-none focus:border-[#ffb400] focus:ring-1 focus:ring-[#ffb400]'

const fieldTextareaClass = `${fieldInputClass} min-h-32 resize-y`

const heroView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-8')],
    [
      h.p([Ui.className<Message>(sectionLabelClass)], ['For your business']),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
          ),
        ],
        ['Put an AI workforce to work on your business'],
      ),
      h.p(
        [Ui.className<Message>('mt-1 max-w-[68ch] text-base/7 text-white/65')],
        [
          'Tell us what you want help with. We set up a workspace seeded with your details so you can hand off work and watch it get done.',
        ],
      ),
    ],
  )
}

const labelledField = <Message>(input: {
  readonly id: string
  readonly name: string
  readonly label: string
  readonly type?: string
  readonly placeholder?: string
  readonly required?: boolean
  readonly help?: string
  readonly autocomplete?: string
  readonly inputmode?: string
}): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid')],
    [
      h.label(
        [h.For(input.id), Ui.className<Message>(fieldLabelClass)],
        [
          input.label,
          input.required === true
            ? h.span([Ui.className<Message>('ml-1 text-[#ffb400]')], ['*'])
            : h.empty,
        ],
      ),
      h.input([
        h.Id(input.id),
        h.Name(input.name),
        h.Type(input.type ?? 'text'),
        ...(input.placeholder === undefined
          ? []
          : [h.Placeholder(input.placeholder)]),
        ...(input.required === true ? [h.Required(true)] : []),
        ...(input.autocomplete === undefined
          ? []
          : [h.Attribute('autocomplete', input.autocomplete)]),
        ...(input.inputmode === undefined
          ? []
          : [h.Attribute('inputmode', input.inputmode)]),
        Ui.className<Message>(fieldInputClass),
      ]),
      input.help === undefined
        ? h.empty
        : h.span(
            [Ui.className<Message>('mt-1 font-mono text-xs text-white/35')],
            [input.help],
          ),
    ],
  )
}

const pricingNoteView = <Message>(): Html => {
  const h = html<Message>()

  return h.p(
    [
      Ui.className<Message>(
        'm-0 border border-[#222] bg-[#010102] px-4 py-3 text-sm/6 text-white/70',
      ),
    ],
    [
      // Exact pricing framing required by the issue. Do not paraphrase.
      'Usage is billed as clear token-based credits — buy credits and spend them as you go. No monthly AI subscription, and your credits never expire.',
    ],
  )
}

const slackOptInView = <Message>(): Html => {
  const h = html<Message>()

  // Opt-in shared Slack channel. This is ONLY the opt-in UI. The actual Slack
  // Connect invite is a separate issue (#5095 / C4) and is NOT built here.
  return h.label(
    [
      h.For('business-slack-optin'),
      Ui.className<Message>(
        'flex cursor-pointer items-start gap-2.5 border border-[#222] bg-[#030303] px-3 py-2.5',
      ),
    ],
    [
      h.input([
        h.Id('business-slack-optin'),
        h.Name('requestSlackChannel'),
        h.Type('checkbox'),
        h.Value('yes'),
        Ui.className<Message>('mt-0.5 size-4 shrink-0 accent-[#ffb400]'),
      ]),
      h.span(
        [Ui.className<Message>('grid gap-0.5')],
        [
          h.span(
            [Ui.className<Message>('text-sm text-[#f1efe8]')],
            ['Request a shared Slack channel'],
          ),
          h.span(
            [Ui.className<Message>('font-mono text-xs text-white/40')],
            [
              'We can set up a shared Slack channel so your team and your AI workforce can talk in one place.',
            ],
          ),
        ],
      ),
    ],
  )
}

const signupFormView = <Message>(): Html => {
  const h = html<Message>()

  return h.form(
    [
      h.Method('post'),
      h.Action(intakeAction),
      h.AriaLabel('Business signup'),
      Ui.className<Message>(
        'grid gap-4 border border-[#222] bg-[#010102] p-5 sm:p-6',
      ),
    ],
    [
      labelledField<Message>({
        id: 'business-name',
        name: 'businessName',
        label: 'Business name',
        required: true,
        placeholder: 'Acme Co.',
        autocomplete: 'organization',
      }),
      labelledField<Message>({
        id: 'business-website',
        name: 'website',
        label: 'Website / URL',
        type: 'url',
        placeholder: 'https://example.com',
        autocomplete: 'url',
        inputmode: 'url',
        help: 'We use your public site to set up your workspace.',
      }),
      // First-class phone number field -- required and prominent, not buried.
      labelledField<Message>({
        id: 'business-phone',
        name: 'phone',
        label: 'Phone number',
        type: 'tel',
        required: true,
        placeholder: '+1 555 000 0000',
        autocomplete: 'tel',
        inputmode: 'tel',
        help: 'So we can reach you to get started.',
      }),
      h.div(
        [Ui.className<Message>('grid')],
        [
          h.label(
            [h.For('business-help'), Ui.className<Message>(fieldLabelClass)],
            ['What do you want help with?'],
          ),
          h.textarea(
            [
              h.Id('business-help'),
              h.Name('helpWith'),
              h.Rows(4),
              h.Placeholder(
                'Describe the work you want done, in your own words.',
              ),
              Ui.className<Message>(fieldTextareaClass),
            ],
            [],
          ),
        ],
      ),
      slackOptInView<Message>(),
      pricingNoteView<Message>(),
      h.button(
        [
          h.Type('submit'),
          Ui.className<Message>(
            'min-h-10 cursor-pointer border border-[#f1efe8] bg-[#f1efe8] px-4 font-mono text-[0.8125rem] text-[#000] hover:bg-white',
          ),
        ],
        ['Get started'],
      ),
      h.p(
        [Ui.className<Message>('m-0 font-mono text-xs text-white/35')],
        [
          'We only use your details to set up your workspace and get in touch.',
        ],
      ),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Business'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,920px)] gap-8 px-4 py-8 lg:grid-cols-[minmax(0,1fr)_minmax(22rem,26rem)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid content-start gap-6')],
            [heroView<Message>()],
          ),
          signupFormView<Message>(),
        ],
      ),
    ],
  )
}
