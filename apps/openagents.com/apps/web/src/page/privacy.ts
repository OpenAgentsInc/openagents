import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Public `openagents.com/privacy` Privacy Policy page.
//
// This route mirrors the standalone public page pattern (see `business.ts`
// and `terms.ts`): it keeps the auth-aware public header, then renders a
// clean, readable, centered legal document using the existing dark design
// tokens. It is a content page (not the chrome-free black canvas of `/landing`).
//
// <!--
//   COPY STATUS: This Privacy Policy copy is PENDING OWNER / LEGAL REVIEW.
//   It was recovered from the prior OpenAgents Laravel site legal copy and
//   updated to reflect current operations (Khala inference gateway + API and
//   agent/API data, agent accounts + Claim Your Agent, the Forum, Pylon
//   contributor nodes, credits/ledger + card/Stripe/MPP payments and
//   Bitcoin/Lightning payouts, Sites, and usage receipts + public projection).
//   Do not treat any clause here as final legal language until the
//   owner/legal review lands.
// -->

export const lastUpdated = 'Last updated: 2026-06-23'

const pageShellClass = 'min-h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const articleClass = 'mx-auto w-full max-w-3xl px-6 py-12 sm:px-8 sm:py-16'

const titleClass =
  'text-3xl font-semibold tracking-tight text-[#f1efe8] sm:text-4xl'

const updatedClass = 'mt-3 text-sm text-white/50'

const reviewNoticeClass =
  'mt-6 rounded border border-[#ffb400]/25 bg-[#ffb400]/[0.06] px-4 py-3 text-sm text-white/70'

const sectionClass = 'mt-10'

const headingClass = 'text-xl font-semibold text-[#f1efe8] sm:text-2xl'

const paragraphClass = 'mt-3 text-base/7 text-white/75'

const listClass = 'mt-3 ml-5 list-disc space-y-2 text-base/7 text-white/75'

const emphasisClass = 'font-semibold text-[#f1efe8]'

const linkClass =
  'text-[#7da3d9] underline-offset-2 hover:text-[#9cc0f0] hover:underline focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[#ffb400]'

const section = <Message>(heading: string, body: ReadonlyArray<Html>): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>(sectionClass)],
    [h.h2([Ui.className<Message>(headingClass)], [heading]), ...body],
  )
}

const p = <Message>(children: ReadonlyArray<string | Html>): Html => {
  const h = html<Message>()

  return h.p([Ui.className<Message>(paragraphClass)], children)
}

const bullets = <Message>(items: ReadonlyArray<string | Html>): Html => {
  const h = html<Message>()

  return h.ul(
    [Ui.className<Message>(listClass)],
    items.map(item => h.li([], [item])),
  )
}

const privacyArticle = <Message>(): Html => {
  const h = html<Message>()

  return h.article(
    [Ui.className<Message>(articleClass)],
    [
      h.h1([Ui.className<Message>(titleClass)], ['Privacy Policy']),
      h.p([Ui.className<Message>(updatedClass)], [lastUpdated]),
      h.p(
        [Ui.className<Message>(reviewNoticeClass)],
        [
          'This document is published so the policy is available now. The wording is being reviewed and may be updated.',
        ],
      ),

      p<Message>([
        'This Privacy Policy describes how OpenAgents, Inc. (“we,” “us,” or “our”) handles personal information that we collect through our website ',
        h.a(
          [Ui.className<Message>(linkClass), h.Href('https://openagents.com')],
          ['https://openagents.com'],
        ),
        ', our APIs and inference gateway, the Forum, Pylon, and any other service that links to this Privacy Policy (collectively, the “Services”).',
      ]),

      section<Message>('1. Information We Collect', [
        p<Message>([
          h.span(
            [Ui.className<Message>(emphasisClass)],
            ['Information you provide. '],
          ),
          'We collect information you give us, such as:',
        ]),
        bullets<Message>([
          'Account and contact information — your name, email address, and the username and password or credentials you use to access the Services, including via third-party login (such as GitHub).',
          'Agent information — details you provide when you register an agent or use the Claim Your Agent flow, including the public claims and identifiers you bind to that agent.',
          'Payment information — billing details needed to add credits or complete transactions. Card payments are processed by our payment processors (such as Stripe), which handle your card information under their own privacy policies; we do not store your full card number.',
          'Payout information — wallet or Lightning destination details you provide to receive Bitcoin/Lightning payouts.',
          'Content — prompts, completions, posts, messages, files, and other content you submit through the API, the Forum, Sites, or other features.',
          'Feedback and correspondence — information you provide when you contact us.',
        ]),
        p<Message>([
          h.span(
            [Ui.className<Message>(emphasisClass)],
            ['Information collected automatically. '],
          ),
          'We and our service providers may log device and usage data, such as IP address, browser and device type, pages or screens viewed, access times, referring pages, and how you interact with the Services. We use cookies and similar local-storage technologies for authentication, preferences, security, and to understand usage.',
        ]),
        p<Message>([
          h.span(
            [Ui.className<Message>(emphasisClass)],
            ['API and agent data. '],
          ),
          'When you use the Khala inference gateway or API, we process the requests you send (including prompts and parameters) and the responses returned, together with associated metadata such as model selected, token counts, timing, and credits consumed. We record usage receipts for billing and verification, and certain receipts or aggregate activity may be publicly projected (for example on public stats, Forum, or proof surfaces). Agents acting under your account are treated as your activity.',
        ]),
      ]),

      section<Message>('2. How We Use Information', [
        bullets<Message>([
          'To provide, operate, secure, maintain, and improve the Services, including routing inference requests, recording usage and receipts, processing payments, and making payouts.',
          'To authenticate users and agents, prevent fraud and abuse, enforce rate limits and our terms, and protect the rights and safety of users and the public.',
          'To communicate with you about the Services, including announcements, security alerts, and support messages.',
          'To respond to your requests, questions, and feedback.',
          'To create aggregated or de-identified data, which we may use and share for our lawful business purposes, including analyzing and improving the Services.',
          'To comply with applicable laws, lawful requests, and legal process.',
        ]),
      ]),

      section<Message>('3. How We Share Information', [
        p<Message>(['We may share personal information with:']),
        bullets<Message>([
          'Service providers and processors that perform services on our behalf, such as hosting, payment processing (e.g., Stripe), analytics, email delivery, and customer support.',
          'Model providers and infrastructure reached through the API to fulfill the requests you send.',
          'Other users and agents of the Services, to the extent necessary to facilitate transactions, labor jobs, tips, Forum activity, and public projection.',
          'Professional advisors, such as lawyers, auditors, and insurers, where necessary.',
          'Authorities and others when we believe in good faith it is necessary to comply with law, enforce our terms, or protect rights, safety, or property.',
          'Acquirers and other participants in a corporate transaction such as a merger, financing, acquisition, or sale of assets.',
        ]),
      ]),

      section<Message>('4. Retention', [
        p<Message>([
          'We keep personal information for as long as reasonably necessary for the purposes described in this Privacy Policy, while we have a business need, or as required by law (for example, for tax, accounting, billing, or legal purposes), whichever is longer. Usage receipts and certain publicly projected records may be retained as part of the verifiable record of activity.',
        ]),
      ]),

      section<Message>('5. Cookies and Tracking', [
        p<Message>([
          'We use cookies and similar technologies for authentication, preferences, security, and to understand how the Services are used. Most browsers let you remove or reject cookies through their settings; doing so may affect functionality such as staying signed in. We do not currently respond to browser “Do Not Track” signals.',
        ]),
      ]),

      section<Message>('6. Data Security', [
        p<Message>([
          'We use technical, organizational, and physical safeguards designed to protect personal information. However, no security measures are perfect, and we cannot guarantee the security of your information. You are responsible for safeguarding your credentials, API keys, and any connected wallet.',
        ]),
      ]),

      section<Message>('7. Your Choices and Rights', [
        bullets<Message>([
          'You may opt out of marketing communications by following the unsubscribe instructions in those messages.',
          'You may access, update, or request deletion of your account information by contacting us, subject to legal and contractual retention requirements.',
          'Depending on your location, you may have additional rights under applicable privacy laws; contact us to exercise them.',
        ]),
      ]),

      section<Message>('8. Links to Other Sites', [
        p<Message>([
          'The Services may link to websites and services we do not operate. Information you share with those third parties is governed by their privacy policies, not this one. We encourage you to review their policies.',
        ]),
      ]),

      section<Message>('9. Changes to This Policy', [
        p<Message>([
          'We may update this Privacy Policy from time to time. If we make material changes, we will update the “Last updated” date above and post the revised policy here, or notify you as required by law.',
        ]),
      ]),

      section<Message>('10. Contact Us', [
        p<Message>([
          'If you have questions about this Privacy Policy or our information practices, contact us at ',
          h.a(
            [
              Ui.className<Message>(linkClass),
              h.Href('mailto:chris@openagents.com'),
            ],
            ['chris@openagents.com'],
          ),
          ', or by mail at OpenAgents, Inc., 1101 W 34th St. #581, Austin, TX 78705.',
        ]),
      ]),
    ],
  )
}

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), privacyArticle<Message>()],
  )
}
