import {
  LegalBullets,
  LegalEmphasis,
  LegalLink,
  LegalP,
  LegalSection,
  legalArticleClass,
  legalBackLinkClass,
  legalReviewNoticeClass,
  legalShellClass,
  legalTitleClass,
  legalUpdatedClass,
} from './-legal-components'

// Public `/privacy` Privacy Policy page, ported from the Foldkit
// `apps/web/src/page/privacy.ts` route. Copy is preserved verbatim (this is a
// legal document); only the markup/styling moved from Foldkit's `html<Message>`
// builders to JSX with the shared `khala-*` design tokens.
//
// COPY STATUS: This Privacy Policy copy is PENDING OWNER / LEGAL REVIEW, same
// as the Foldkit original. Do not treat any clause here as final legal
// language until the owner/legal review lands.

export const PRIVACY_LAST_UPDATED = 'Last updated: 2026-06-23'

export function PrivacyPage() {
  return (
    <main className={legalShellClass} data-route="privacy">
      <a className={`m-4 ${legalBackLinkClass}`} href="/">
        &larr; OpenAgents
      </a>
      <article className={legalArticleClass}>
        <h1 className={legalTitleClass}>Privacy Policy</h1>
        <p className={legalUpdatedClass}>{PRIVACY_LAST_UPDATED}</p>
        <p className={legalReviewNoticeClass}>
          This document is published so the policy is available now. The wording is being
          reviewed and may be updated.
        </p>

        <LegalP>
          This Privacy Policy describes how OpenAgents, Inc. (&ldquo;we,&rdquo; &ldquo;us,&rdquo;
          or &ldquo;our&rdquo;) handles personal information that we collect through our website{' '}
          <LegalLink href="https://openagents.com">https://openagents.com</LegalLink>, our APIs
          and inference gateway, the Forum, Pylon, and any other service that links to this
          Privacy Policy (collectively, the &ldquo;Services&rdquo;).
        </LegalP>

        <LegalSection heading="1. Information We Collect">
          <LegalP>
            <LegalEmphasis>Information you provide. </LegalEmphasis>
            We collect information you give us, such as:
          </LegalP>
          <LegalBullets
            items={[
              'Account and contact information — your name, email address, and the username and password or credentials you use to access the Services, including via third-party login (such as GitHub).',
              'Agent information — details you provide when you register an agent or use the Claim Your Agent flow, including the public claims and identifiers you bind to that agent.',
              'Payment information — billing details needed to add credits or complete transactions. Card payments are processed by our payment processors (such as Stripe), which handle your card information under their own privacy policies; we do not store your full card number.',
              'Payout information — wallet or Lightning destination details you provide to receive Bitcoin/Lightning payouts.',
              'Content — prompts, completions, posts, messages, files, and other content you submit through the API, the Forum, Sites, or other features.',
              'Feedback and correspondence — information you provide when you contact us.',
            ]}
          />
          <LegalP>
            <LegalEmphasis>Information collected automatically. </LegalEmphasis>
            We and our service providers may log device and usage data, such as IP address,
            browser and device type, pages or screens viewed, access times, referring pages, and
            how you interact with the Services. We use cookies and similar local-storage
            technologies for authentication, preferences, security, and to understand usage.
          </LegalP>
          <LegalP>
            <LegalEmphasis>API and agent data. </LegalEmphasis>
            When you use the Khala inference gateway or API, we process the requests you send
            (including prompts and parameters) and the responses returned, together with
            associated metadata such as model selected, token counts, timing, and credits
            consumed. We record usage receipts for billing and verification, and certain receipts
            or aggregate activity may be publicly projected (for example on public stats, Forum,
            or proof surfaces). Agents acting under your account are treated as your activity.
          </LegalP>
        </LegalSection>

        <LegalSection heading="2. How We Use Information">
          <LegalBullets
            items={[
              'To provide, operate, secure, maintain, and improve the Services, including routing inference requests, recording usage and receipts, processing payments, and making payouts.',
              'To authenticate users and agents, prevent fraud and abuse, enforce rate limits and our terms, and protect the rights and safety of users and the public.',
              'To communicate with you about the Services, including announcements, security alerts, and support messages.',
              'To respond to your requests, questions, and feedback.',
              'To create aggregated or de-identified data, which we may use and share for our lawful business purposes, including analyzing and improving the Services.',
              'To comply with applicable laws, lawful requests, and legal process.',
            ]}
          />
        </LegalSection>

        <LegalSection heading="3. How We Share Information">
          <LegalP>We may share personal information with:</LegalP>
          <LegalBullets
            items={[
              'Service providers and processors that perform services on our behalf, such as hosting, payment processing (e.g., Stripe), analytics, email delivery, and customer support.',
              'Model providers and infrastructure reached through the API to fulfill the requests you send.',
              'Other users and agents of the Services, to the extent necessary to facilitate transactions, labor jobs, tips, Forum activity, and public projection.',
              'Professional advisors, such as lawyers, auditors, and insurers, where necessary.',
              'Authorities and others when we believe in good faith it is necessary to comply with law, enforce our terms, or protect rights, safety, or property.',
              'Acquirers and other participants in a corporate transaction such as a merger, financing, acquisition, or sale of assets.',
            ]}
          />
        </LegalSection>

        <LegalSection heading="4. Retention">
          <LegalP>
            We keep personal information for as long as reasonably necessary for the purposes
            described in this Privacy Policy, while we have a business need, or as required by law
            (for example, for tax, accounting, billing, or legal purposes), whichever is longer.
            Usage receipts and certain publicly projected records may be retained as part of the
            verifiable record of activity.
          </LegalP>
        </LegalSection>

        <LegalSection heading="5. Cookies and Tracking">
          <LegalP>
            We use cookies and similar technologies for authentication, preferences, security, and
            to understand how the Services are used. Most browsers let you remove or reject
            cookies through their settings; doing so may affect functionality such as staying
            signed in. We do not currently respond to browser &ldquo;Do Not Track&rdquo; signals.
          </LegalP>
        </LegalSection>

        <LegalSection heading="6. Data Security">
          <LegalP>
            We use technical, organizational, and physical safeguards designed to protect personal
            information. However, no security measures are perfect, and we cannot guarantee the
            security of your information. You are responsible for safeguarding your credentials,
            API keys, and any connected wallet.
          </LegalP>
        </LegalSection>

        <LegalSection heading="7. Your Choices and Rights">
          <LegalBullets
            items={[
              'You may opt out of marketing communications by following the unsubscribe instructions in those messages.',
              'You may access, update, or request deletion of your account information by contacting us, subject to legal and contractual retention requirements.',
              'Depending on your location, you may have additional rights under applicable privacy laws; contact us to exercise them.',
            ]}
          />
        </LegalSection>

        <LegalSection heading="8. Links to Other Sites">
          <LegalP>
            The Services may link to websites and services we do not operate. Information you
            share with those third parties is governed by their privacy policies, not this one. We
            encourage you to review their policies.
          </LegalP>
        </LegalSection>

        <LegalSection heading="9. Changes to This Policy">
          <LegalP>
            We may update this Privacy Policy from time to time. If we make material changes, we
            will update the &ldquo;Last updated&rdquo; date above and post the revised policy
            here, or notify you as required by law.
          </LegalP>
        </LegalSection>

        <LegalSection heading="10. Contact Us">
          <LegalP>
            If you have questions about this Privacy Policy or our information practices, contact
            us at <LegalLink href="mailto:chris@openagents.com">chris@openagents.com</LegalLink>,
            or by mail at OpenAgents, Inc., 1101 W 34th St. #581, Austin, TX 78705.
          </LegalP>
        </LegalSection>
      </article>
    </main>
  )
}
