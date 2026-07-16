import { PublicPageShell } from '@/components/public-page-shell'
import {
  type IntentReporter,
  Link,
  type LinkView,
  Stack,
  type StackView,
  Text,
  type TextView,
  type View,
  makeIntentRegistry,
  makeNavigationIntentHandlers,
  makeViewProgramFromState,
  navigationIntentDefinitions,
  resolveIntentRef,
} from '@effect-native/core'
import {
  Effect,
  Exit,
  Scope,
  SubscriptionRef,
} from '@effect-native/core/effect'
import {
  makeDomNavigationHandler,
  makeDomRenderer,
} from '@effect-native/render-dom'
import { useEffect, useRef } from 'react'

import { stage1EffectNativeTheme } from './-stage1-effect-native-theme'

// EN-4 (#8573) conversion of the interim React `/privacy` page
// (`-privacy-page.tsx`) onto the Effect Native DOM renderer, following the
// EN-1 `/stage1` and EN-4 `/download`/`/khala` patterns: a thin React route
// shell mounts a typed Effect Native `ViewProgram`; all page content is
// authored as typed views, not JSX.
//
// COPY STATUS: This Privacy Policy copy is PENDING OWNER / LEGAL REVIEW, same
// as the Foldkit original and the interim React page. Do not treat any clause
// here as final legal language until the owner/legal review lands.
//
// Copy is preserved verbatim. Effect Native `Text` is flat-content only, so
// rich-inline runs (emphasis labels, in-paragraph links) are structurally
// reworked into one Text/Link view per run under Stack layout — see
// docs/effect-native/2026-07-09-web-absorption-burndown.md.

export const PRIVACY_LAST_UPDATED = 'Last updated: 2026-06-23'

export type PrivacyLandingState = Readonly<Record<string, never>>

export const initialPrivacyLandingState: PrivacyLandingState = {}

const text = (
  key: string,
  content: string,
  variant: TextView['variant'] = 'body',
  color: TextView['color'] = 'textPrimary',
): TextView =>
  Text({
    key,
    content,
    variant,
    color,
    style: { width: 'full' },
  })

const section = (key: string, children: ReadonlyArray<View>): StackView =>
  Stack(
    {
      key,
      direction: 'column',
      gap: '4',
      padding: '6',
      style: { width: 'full', maxWidth: 768, alignSelf: 'center' },
    },
    children,
  )

const legalSection = (
  key: string,
  heading: string,
  children: ReadonlyArray<View>,
): StackView =>
  Stack(
    {
      key: `privacy-section-${key}`,
      direction: 'column',
      gap: '3',
      style: { width: 'full' },
    },
    [
      text(`privacy-heading-${key}`, heading, 'title', 'textPrimary'),
      ...children,
    ],
  )

const bullet = (key: string, content: string): TextView =>
  text(key, `• ${content}`, 'body', 'textMuted')

const bulletList = (key: string, items: ReadonlyArray<string>): StackView =>
  Stack(
    {
      key: `privacy-bullets-${key}`,
      direction: 'column',
      gap: '2',
      style: { width: 'full' },
    },
    items.map((item, index) => bullet(`${key}-${index}`, item)),
  )

const externalLink = (key: string, label: string, href: string): LinkView =>
  Link(
    {
      key,
      destination: { kind: 'url', href },
      style: {
        borderColor: 'border',
        borderWidth: 1,
        borderRadius: 'md',
        paddingTop: '2',
        paddingRight: '3',
        paddingBottom: '2',
        paddingLeft: '3',
      },
    },
    [
      Text({
        key: `${key}-label`,
        content: label,
        variant: 'label',
        color: 'accent',
      }),
    ],
  )

// Paragraph with leading emphasis label as a separate run (flat Text only).
// The non-breaking space belongs to the body run so adjacent inline DOM views
// retain the same readable separation as a normal rich-text paragraph.
const emphasizedParagraph = (
  key: string,
  emphasis: string,
  body: string,
): StackView =>
  Stack(
    {
      key: `privacy-em-p-${key}`,
      direction: 'column',
      gap: '1',
      style: { width: 'full' },
    },
    [
      text(`privacy-em-${key}`, emphasis, 'label', 'textPrimary'),
      text(`privacy-em-body-${key}`, `\u00a0${body}`, 'body', 'textMuted'),
    ],
  )

export const privacyLandingView = (
  _state: PrivacyLandingState = initialPrivacyLandingState,
): View =>
  Stack(
    {
      key: 'privacy-root',
      direction: 'column',
      gap: '0',
      style: {
        backgroundColor: 'background',
        minHeight: 'full',
        width: 'full',
      },
    },
    [
      section('privacy-article', [
        text('privacy-title', 'Privacy Policy', 'heading'),
        text('privacy-updated', PRIVACY_LAST_UPDATED, 'caption', 'textMuted'),

        // Intro: link split into separate runs (flat Text / Link).
        Stack(
          {
            key: 'privacy-intro',
            direction: 'column',
            gap: '2',
            style: { width: 'full' },
          },
          [
            text(
              'privacy-intro-before',
              'This Privacy Policy describes how OpenAgents, Inc. (“we,” “us,” or “our”) handles personal information that we collect through our website\u00a0',
              'body',
              'textMuted',
            ),
            externalLink(
              'privacy-intro-site',
              'https://openagents.com',
              'https://openagents.com',
            ),
            text(
              'privacy-intro-after',
              ', our APIs and inference gateway, the Forum, Pylon, and any other service that links to this Privacy Policy (collectively, the “Services”).',
              'body',
              'textMuted',
            ),
          ],
        ),

        legalSection('1-collect', '1. Information We Collect', [
          emphasizedParagraph(
            'you-provide',
            'Information you provide.',
            'We collect information you give us, such as:',
          ),
          bulletList('collect-provided', [
            'Account and contact information — your name, email address, and the username and password or credentials you use to access the Services, including via third-party login (such as GitHub).',
            'Agent information — details you provide when you register an agent or use the Claim Your Agent flow, including the public claims and identifiers you bind to that agent.',
            'Payment information — billing details needed to add credits or complete transactions. Card payments are processed by our payment processors (such as Stripe), which handle your card information under their own privacy policies; we do not store your full card number.',
            'Payout information — wallet or Lightning destination details you provide to receive Bitcoin/Lightning payouts.',
            'Content — prompts, completions, posts, messages, files, and other content you submit through the API, the Forum, Sites, or other features.',
            'Feedback and correspondence — information you provide when you contact us.',
          ]),
          emphasizedParagraph(
            'auto-collect',
            'Information collected automatically.',
            'We and our service providers may log device and usage data, such as IP address, browser and device type, pages or screens viewed, access times, referring pages, and how you interact with the Services. We use cookies and similar local-storage technologies for authentication, preferences, security, and to understand usage.',
          ),
          emphasizedParagraph(
            'api-agent',
            'API and agent data.',
            'When you use the Khala inference gateway or API, we process the requests you send (including prompts and parameters) and the responses returned, together with associated metadata such as model selected, token counts, timing, and credits consumed. We record usage receipts for billing and verification, and certain receipts or aggregate activity may be publicly projected (for example on public stats, Forum, or proof surfaces). Agents acting under your account are treated as your activity.',
          ),
        ]),

        legalSection('2-use', '2. How We Use Information', [
          bulletList('use', [
            'To provide, operate, secure, maintain, and improve the Services, including routing inference requests, recording usage and receipts, processing payments, and making payouts.',
            'To authenticate users and agents, prevent fraud and abuse, enforce rate limits and our terms, and protect the rights and safety of users and the public.',
            'To communicate with you about the Services, including announcements, security alerts, and support messages.',
            'To respond to your requests, questions, and feedback.',
            'To create aggregated or de-identified data, which we may use and share for our lawful business purposes, including analyzing and improving the Services.',
            'To comply with applicable laws, lawful requests, and legal process.',
          ]),
        ]),

        legalSection('3-share', '3. How We Share Information', [
          text(
            'privacy-share-intro',
            'We may share personal information with:',
            'body',
            'textMuted',
          ),
          bulletList('share', [
            'Service providers and processors that perform services on our behalf, such as hosting, payment processing (e.g., Stripe), analytics, email delivery, and customer support.',
            'Model providers and infrastructure reached through the API to fulfill the requests you send.',
            'Other users and agents of the Services, to the extent necessary to facilitate transactions, labor jobs, tips, Forum activity, and public projection.',
            'Professional advisors, such as lawyers, auditors, and insurers, where necessary.',
            'Authorities and others when we believe in good faith it is necessary to comply with law, enforce our terms, or protect rights, safety, or property.',
            'Acquirers and other participants in a corporate transaction such as a merger, financing, acquisition, or sale of assets.',
          ]),
        ]),

        legalSection('4-retention', '4. Retention', [
          text(
            'privacy-retention-body',
            'We keep personal information for as long as reasonably necessary for the purposes described in this Privacy Policy, while we have a business need, or as required by law (for example, for tax, accounting, billing, or legal purposes), whichever is longer. Usage receipts and certain publicly projected records may be retained as part of the verifiable record of activity.',
            'body',
            'textMuted',
          ),
        ]),

        legalSection('5-cookies', '5. Cookies and Tracking', [
          text(
            'privacy-cookies-body',
            'We use cookies and similar technologies for authentication, preferences, security, and to understand how the Services are used. Most browsers let you remove or reject cookies through their settings; doing so may affect functionality such as staying signed in. We do not currently respond to browser “Do Not Track” signals.',
            'body',
            'textMuted',
          ),
        ]),

        legalSection('6-security', '6. Data Security', [
          text(
            'privacy-security-body',
            'We use technical, organizational, and physical safeguards designed to protect personal information. However, no security measures are perfect, and we cannot guarantee the security of your information. You are responsible for safeguarding your credentials, API keys, and any connected wallet.',
            'body',
            'textMuted',
          ),
        ]),

        legalSection('7-choices', '7. Your Choices and Rights', [
          bulletList('choices', [
            'You may opt out of marketing communications by following the unsubscribe instructions in those messages.',
            'You may access, update, or request deletion of your account information by contacting us, subject to legal and contractual retention requirements.',
            'Depending on your location, you may have additional rights under applicable privacy laws; contact us to exercise them.',
          ]),
        ]),

        legalSection('8-links', '8. Links to Other Sites', [
          text(
            'privacy-links-body',
            'The Services may link to websites and services we do not operate. Information you share with those third parties is governed by their privacy policies, not this one. We encourage you to review their policies.',
            'body',
            'textMuted',
          ),
        ]),

        legalSection('9-changes', '9. Changes to This Policy', [
          text(
            'privacy-changes-body',
            'We may update this Privacy Policy from time to time. If we make material changes, we will update the “Last updated” date above and post the revised policy here, or notify you as required by law.',
            'body',
            'textMuted',
          ),
        ]),

        legalSection('10-contact', '10. Contact Us', [
          Stack(
            {
              key: 'privacy-contact-runs',
              direction: 'column',
              gap: '2',
              style: { width: 'full' },
            },
            [
              text(
                'privacy-contact-before',
                'If you have questions about this Privacy Policy or our information practices, contact us at\u00a0',
                'body',
                'textMuted',
              ),
              externalLink(
                'privacy-contact-email',
                'chris@openagents.com',
                'mailto:chris@openagents.com',
              ),
              text(
                'privacy-contact-after',
                ', or by mail at OpenAgents, Inc., 1101 W 34th St. #581, Austin, TX 78705.',
                'body',
                'textMuted',
              ),
            ],
          ),
        ]),
      ]),
    ],
  )

export const mountPrivacyEffectNativeSurface = (container: HTMLElement) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialPrivacyLandingState)
    const program = makeViewProgramFromState(state, privacyLandingView)
    const registry = yield* makeIntentRegistry(
      navigationIntentDefinitions,
      makeNavigationIntentHandlers(makeDomNavigationHandler()),
    )
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue))
    const surface = yield* makeDomRenderer({
      theme: stage1EffectNativeTheme,
    }).mount(container, program.viewStream, report)

    return { state, unmount: surface.unmount }
  })

export function PrivacyEffectNativePage() {
  const rootRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const root = rootRef.current
    if (root === null) {
      return undefined
    }

    let disposed = false
    let closeScope: (() => void) | undefined

    void Effect.runPromise(Scope.make())
      .then(scope => {
        const close = () => {
          void Effect.runPromise(Scope.close(scope, Exit.void))
        }
        closeScope = close
        if (disposed) {
          close()
          return undefined
        }
        return Effect.runPromise(
          Scope.provide(scope)(mountPrivacyEffectNativeSurface(root)),
        )
      })
      .catch(() => undefined)

    return () => {
      disposed = true
      closeScope?.()
    }
  }, [])

  return (
    <PublicPageShell dataRoute="privacy">
      <main
        aria-label="Privacy Policy"
        className="privacy-effect-native-host"
        data-privacy-effect-native=""
      >
        <div ref={rootRef} data-privacy-effect-native-root="" />
      </main>
    </PublicPageShell>
  )
}
