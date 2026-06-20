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
// wiring) so the page stays static and public. It posts to a bounded intake
// endpoint; seeded workspace creation remains operator-owned through the
// workspace API and invite URL.
//
// SCOPE / STUBS (do NOT implement here -- these are separate issues):
//   - C2 (#5093): lead-enrichment hook via our own API (operator seeding,
//     invite link, engagement tracking). NOT built here.
//   - C4 (#5095): Slack Connect is an opt-in intake + manual invite handoff.
//     The other workspace must still accept the Slack Connect invitation.
//
// Naming note: keep all copy/code generic product language. Do NOT reference
// any partner/company/person names anywhere.

const intakeAction = '/api/public/business-signup'

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const sectionLabelClass = 'm-0 font-mono text-base text-white/35 sm:text-sm'

const fieldLabelClass =
  'font-mono text-xs uppercase tracking-wide text-white/45'

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
      h.p(
        [Ui.className<Message>('m-0 max-w-[68ch] text-base/7 text-white/55')],
        [
          // Plain-English framing of the product, drawn from the intake spec
          // (docs/business/2026-06-20-openagents-business-intake-spec.md).
          'OpenAgents sells machine work with receipts: agents and compute that do real work, where every accepted outcome ties to verifiable evidence. Start with a fast quick win, then put parts of your business on Autopilot. Payment options are scoped up front, including Bitcoin and credits where the backing rails are proven today.',
        ],
      ),
      h.a(
        [
          h.Href('#business-signup'),
          Ui.className<Message>(
            'mt-2 inline-flex min-h-10 w-fit items-center border border-[#f1efe8] bg-[#f1efe8] px-4 font-mono text-[0.8125rem] text-[#000] hover:bg-white',
          ),
        ],
        ['Start with a quick win'],
      ),
    ],
  )
}

// One offering bucket from the menu in the intake spec. Availability is honest
// for this business funnel: "Available now" means the sellable surface is
// shipped/green; "Operator-assisted" means useful pieces exist but delivery is
// still yellow/flagged/manual; "Roadmap" is planned-but-not-shipped. Copy is
// grounded in the coverage doc
// (docs/business/2026-06-20-business-offering-promise-coverage.md). Do NOT flip
// any of these to "now" without a real shipped proof.
type Availability = 'now' | 'assisted' | 'roadmap'

const availabilityLabel: Record<Availability, string> = {
  now: 'Available now',
  assisted: 'Operator-assisted',
  roadmap: 'Roadmap',
}

const availabilityBadgeClass: Record<Availability, string> = {
  now: 'border-[#1f4d2b] bg-[#06140a] text-[#7fdc9b]',
  assisted: 'border-[#4d3f00] bg-[#141004] text-[#ffd54a]',
  roadmap: 'border-[#222] bg-[#070707] text-white/55',
}

type Offering = Readonly<{
  title: string
  availability: Availability
  what: string
  liveNow: string
  caveat: string
  quickWin: string
}>

const offerings: ReadonlyArray<Offering> = [
  {
    title: 'Coding & agent work',
    availability: 'assisted',
    what: 'A coding agent takes a written objective, works in your repo, runs your verification command, and hands back a reviewable change with evidence.',
    liveNow:
      'The coding runtime, Pylon/Probe execution path, and negotiated labor loop are live.',
    caveat:
      'Packaging this as a priced intake-to-receipt business product is operator-assisted today.',
    quickWin:
      'Quick win: fix a failing test suite, refactor a messy module, or add one feature with passing tests.',
  },
  {
    title: 'Inference / AI on tap',
    availability: 'assisted',
    what: 'Open-weight model inference through OpenAgents (Gemini and Fireworks-hosted open models), with a bounded free taste and scoped paid usage where the rails are ready.',
    liveNow:
      'A free inference taste and provider connections are available for scoped work.',
    caveat:
      'The full paid card/Bitcoin-to-credit-to-inference loop is not collectable end-to-end in production yet.',
    quickWin:
      'Quick win: run a batch of summaries, classifications, or extractions and get the results back.',
  },
  {
    title: 'Forum / community agents',
    availability: 'now',
    what: 'A registered agent identity that posts on the OpenAgents Forum, requests and fulfills labor jobs, and sends and receives content tips.',
    liveNow:
      'Agent registration, autonomous forum posting, work requests, content tipping, and reliable tips are shipped.',
    caveat:
      'Cloud-resident assistant replies remain a yellow, bounded support surface.',
    quickWin:
      'Quick win: stand up your own agent to post updates, field questions, or pick up small labor jobs.',
  },
  {
    title: 'Distributed compute & training',
    availability: 'assisted',
    what: 'Scoped, verified training runs over the Pylon contributor network. Fine-tuning and rentable sandbox compute are being stood up.',
    liveNow:
      'Scoped decentralized training runs and verification classes have green evidence.',
    caveat:
      'The public device-capability dataset, fine-tuning service, and metered sandbox compute are not finished self-serve products.',
    quickWin:
      'Quick win: a bounded, verified training or compute task with a reported result and receipt — best scoped with us first.',
  },
  {
    title: 'Sites + commerce',
    availability: 'assisted',
    what: 'An Autopilot Site served at a stable URL, with optional custom branded hostnames, native email sequences, and built-in referral links. Partial/flag-gated today.',
    liveNow:
      'Site build/host, hostname, email, and referral pieces exist behind flags or operator paths.',
    caveat:
      'Treat this as available with a caveat, not a finished self-serve Sites product.',
    quickWin:
      'Quick win: a branded landing page plus a welcome-email sequence for a launch or campaign.',
  },
  {
    title: 'Autopilot business automation',
    availability: 'assisted',
    what: 'Recurring work run by agents through a factory pipeline (Signal → Triage → Build → Validate → Release → Document → Monitor → Deploy), with prefilled e-commerce, legal, and marketing workspaces. A human-review gate sits before anything publishes or spends. Operator-assisted today, not one-click.',
    liveNow:
      'Operator tools, workrooms, work orders, and prefilled vertical workspaces exist.',
    caveat:
      'The all-in-one self-serve business system is roadmap; every delivery has a human-review gate.',
    quickWin:
      'Quick win: one prefilled workspace seeded for your vertical with a first real work item run through it — drafted, never auto-published.',
  },
  {
    title: 'Payments rails (Bitcoin-native)',
    availability: 'assisted',
    what: 'Bitcoin-native payments: self-custodial Lightning wallets, reliable tips with offline fallback, and USD-credit funding for usage.',
    liveNow:
      'Reliable tips and offline fallback are green; parts of the credit usage loop have receipts.',
    caveat:
      'The broader self-custodial wallet flow, card credit purchase, and native-sat live settlement for general payouts are not broadly green yet.',
    quickWin:
      'Quick win: fund an account and run paid work end-to-end with a dereferenceable receipt.',
  },
]

const offeringCardView = <Message>(offering: Offering): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-2 border border-[#222] bg-[#010102] p-4 list-none',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-3')],
        [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 text-base font-medium text-[#f1efe8]',
              ),
            ],
            [offering.title],
          ),
          h.span(
            [
              Ui.className<Message>(
                `shrink-0 border px-2 py-0.5 font-mono text-[0.6875rem] uppercase tracking-wide ${availabilityBadgeClass[offering.availability]}`,
              ),
            ],
            [availabilityLabel[offering.availability]],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-sm/6 text-white/65')],
        [offering.what],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-sm/6 text-white/60')],
        [`Live now: ${offering.liveNow}`],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-sm/6 text-[#ffd54a]/85')],
        [`Current caveat: ${offering.caveat}`],
      ),
      h.p(
        [Ui.className<Message>('m-0 font-mono text-xs text-white/40')],
        [offering.quickWin],
      ),
    ],
  )
}

const offeringsView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.h2(
        [Ui.className<Message>('m-0 text-lg font-medium text-[#f1efe8]')],
        ['What we can do'],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-[68ch] text-sm/6 text-white/55')],
        [
          'An honest menu of what OpenAgents can deliver. Availability is grounded in our public product-promise registry — shipped now, operator-assisted with a caveat, or planned roadmap. We say so in writing and scope the smallest honest version.',
        ],
      ),
      h.ul(
        [Ui.className<Message>('m-0 grid gap-3 p-0')],
        offerings.map(offering => offeringCardView<Message>(offering)),
      ),
    ],
  )
}

type LadderStep = Readonly<{ when: string; title: string; body: string }>

const ladderSteps: ReadonlyArray<LadderStep> = [
  {
    when: 'Day 1',
    title: 'Quick win',
    body: 'One small, well-scoped task delivered with evidence: a code fix with passing tests, a batch of model-processed items, a draft campaign, or a funded paid run with a receipt. Low budget, fast turnaround, no big commitment.',
  },
  {
    when: 'Week 1',
    title: 'Repeatable lane',
    body: 'Turn the quick win into a repeatable workflow: a prefilled workspace for your vertical, a recurring work item, a site plus email sequence, or a standing processing job. You review outputs; agents do the legwork.',
  },
  {
    when: 'Ongoing',
    title: 'On Autopilot',
    body: 'Hand a slice of your business to agents that run in the background through the pipeline, always with a human-review gate. You get accepted outcomes with receipts, and the option to pay or settle in Bitcoin. Expand to more lanes as trust grows.',
  },
]

const ladderView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border-t border-[#222] pt-8')],
    [
      h.h2(
        [Ui.className<Message>('m-0 text-lg font-medium text-[#f1efe8]')],
        ['Quick win → put your business on Autopilot'],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-[68ch] text-sm/6 text-white/55')],
        [
          'You do not commit to the whole journey up front. We pick one small first win, then grow the relationship only if it works.',
        ],
      ),
      h.ol(
        [Ui.className<Message>('m-0 grid gap-3 p-0')],
        ladderSteps.map(step =>
          h.li(
            [
              Ui.className<Message>(
                'grid gap-1 border border-[#222] bg-[#010102] p-4 list-none',
              ),
            ],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 font-mono text-[0.6875rem] uppercase tracking-wide text-[#ffb400]',
                  ),
                ],
                [`${step.when} — ${step.title}`],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-sm/6 text-white/65')],
                [step.body],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

// Copy a bounded ?ref=<code> from the page URL into the hidden referralCode
// field so a converted signup credits the referrer through the existing
// referral spine. No-JS visitors still attribute via the /r/<ref> cookie path.
const referralCaptureScript = `(function(){try{var p=new URLSearchParams(window.location.search);var ref=(p.get('ref')||'').trim();if(!/^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/.test(ref))return;var el=document.getElementById('business-referral-code');if(el)el.value=ref;}catch(e){}})();`

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
      'Usage is framed as clear token-based credits where the paid loop is available. The broader card/Bitcoin-to-credit-to-inference path is still being closed for production, so we scope any paid run with an explicit receipt plan before you fund it. No monthly AI subscription.',
    ],
  )
}

const workspaceInviteView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-4')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.6875rem] uppercase text-white/35',
          ),
        ],
        ['Project invite'],
      ),
      h.h2(
        [Ui.className<Message>('m-0 text-lg font-medium text-[#f1efe8]')],
        ['We prepare the workspace before you open it'],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-base/7 text-white/60')],
        [
          'Your invite opens a named project with seeded notes, starter workflows, and an intro receipt.',
        ],
      ),
    ],
  )
}

const slackOptInView = <Message>(): Html => {
  const h = html<Message>()

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
      h.Id('business-signup'),
      h.Method('post'),
      h.Action(intakeAction),
      h.AriaLabel('Business signup'),
      Ui.className<Message>(
        'grid gap-4 border border-[#222] bg-[#010102] p-5 sm:p-6',
      ),
    ],
    [
      // Inbound referral code (a public referral source ref). Hidden; populated
      // from ?ref= by referralCaptureScript. Server-side validated + bound to
      // the referral attribution spine on a converted signup.
      h.input([
        h.Id('business-referral-code'),
        h.Name('referralCode'),
        h.Type('hidden'),
        h.Value(''),
      ]),
      labelledField<Message>({
        id: 'business-name',
        name: 'businessName',
        label: 'Business name',
        required: true,
        placeholder: 'Acme Co.',
        autocomplete: 'organization',
      }),
      labelledField<Message>({
        id: 'business-email',
        name: 'contactEmail',
        label: 'Work email',
        type: 'email',
        required: true,
        placeholder: 'you@example.com',
        autocomplete: 'email',
        inputmode: 'email',
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
        ['We only use your details to set up your workspace and get in touch.'],
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
            [
              heroView<Message>(),
              offeringsView<Message>(),
              ladderView<Message>(),
              workspaceInviteView<Message>(),
            ],
          ),
          signupFormView<Message>(),
        ],
      ),
      h.script([], [referralCaptureScript]),
    ],
  )
}
