import type * as React from 'react'

// Shared layout + typography helpers for the standalone legal content pages
// (`/terms`, `/privacy`). These mirror the Foldkit `terms.ts` / `privacy.ts`
// page shell (centered readable article on the dark canvas) with the
// StarCraft `khala-*` design tokens swapped in for the page's raw hex values.

export const legalShellClass = 'min-h-dvh overflow-y-auto bg-khala-void text-khala-text'

export const legalArticleClass = 'mx-auto w-full max-w-3xl px-6 py-12 sm:px-8 sm:py-16'

export const legalTitleClass =
  'text-3xl font-semibold tracking-tight text-khala-text sm:text-4xl'

export const legalUpdatedClass = 'mt-3 text-sm text-khala-text-faint'

export const legalReviewNoticeClass =
  'mt-6 rounded border border-khala-warning/25 bg-khala-warning/[0.06] px-4 py-3 text-sm text-khala-text-muted'

const legalSectionClass = 'mt-10'

const legalHeadingClass = 'text-xl font-semibold text-khala-text sm:text-2xl'

const legalParagraphClass = 'mt-3 text-base/7 text-khala-text-muted'

const legalListClass = 'mt-3 ml-5 list-disc space-y-2 text-base/7 text-khala-text-muted'

export const legalEmphasisClass = 'font-semibold text-khala-text'

export const legalLinkClass =
  'khala-focus text-khala-energy-soft underline-offset-2 hover:text-khala-energy-cyan hover:underline'

export const legalBackLinkClass =
  'khala-focus inline-flex min-h-11 items-center gap-2 font-mono text-sm text-khala-text-muted hover:text-khala-text'

export function LegalSection({
  heading,
  children,
}: Readonly<{ heading: string; children: React.ReactNode }>) {
  return (
    <section className={legalSectionClass}>
      <h2 className={legalHeadingClass}>{heading}</h2>
      {children}
    </section>
  )
}

export function LegalP({ children }: Readonly<{ children: React.ReactNode }>) {
  return <p className={legalParagraphClass}>{children}</p>
}

export function LegalBullets({ items }: Readonly<{ items: ReadonlyArray<React.ReactNode> }>) {
  return (
    <ul className={legalListClass}>
      {items.map((item, index) => (
        // eslint-disable-next-line react/no-array-index-key -- static, order-stable legal copy
        <li key={index}>{item}</li>
      ))}
    </ul>
  )
}

export function LegalEmphasis({ children }: Readonly<{ children: React.ReactNode }>) {
  return <span className={legalEmphasisClass}>{children}</span>
}

export function LegalLink({
  href,
  children,
}: Readonly<{ href: string; children: React.ReactNode }>) {
  return (
    <a className={legalLinkClass} href={href}>
      {children}
    </a>
  )
}
