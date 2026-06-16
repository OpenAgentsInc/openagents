import { clsx } from 'clsx'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

export type Tone =
  | 'neutral'
  | 'accent'
  | 'positive'
  | 'warning'
  | 'negative'
  | 'info'

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger'

export type ButtonSize = 'sm' | 'md'

export type NavItem = Readonly<{
  href: string
  label: string
  meta?: string
  active?: boolean
  tone?: Tone
}>

export type StatItem = Readonly<{
  label: string
  value: string
  tone?: Tone
}>

export type DescriptionItem = Readonly<{
  label: string
  value: string
}>

export type StackedListItem = Readonly<{
  title: string
  detail?: string
  meta?: string
  tone?: Tone
}>

export type MediaRowItem = Readonly<{
  title: string
  detail?: string
  avatarUrl?: string
  fallback?: string
  meta?: string
}>

export type CommerceProductItem = Readonly<{
  title: string
  detail?: string
  price?: string
  href?: string
  imageUrl?: string
  imageAlt?: string
  rating?: number
  reviewCount?: number
  swatches?: ReadonlyArray<Tone>
}>

export type CommerceLineItem = Readonly<{
  title: string
  detail?: string
  price?: string
  quantity?: string
  status?: string
  tone?: Tone
  imageUrl?: string
  imageAlt?: string
}>

export type CommerceSummaryLine = Readonly<{
  label: string
  value: string
  tone?: Tone
  strong?: boolean
}>

export type CommerceFilterGroup = Readonly<{
  label: string
  options: ReadonlyArray<Readonly<{ label: string; count?: string }>>
}>

export type CommerceReview = Readonly<{
  author: string
  body: string
  meta?: string
  rating?: number
}>

export type CommerceIncentive = Readonly<{
  title: string
  body: string
  tone?: Tone
}>

export type MarketingFeature = Readonly<{
  title: string
  body: string
  tone?: Tone
  meta?: string
}>

export type MarketingFaq = Readonly<{
  question: string
  answer: string
}>

export type MarketingPricingTier = Readonly<{
  name: string
  price: string
  description?: string
  features: ReadonlyArray<string>
  highlighted?: boolean
  actionLabel?: string
  actionHref?: string
}>

export type MarketingPost = Readonly<{
  title: string
  excerpt: string
  href?: string
  meta?: string
}>

export type MarketingPerson = Readonly<{
  name: string
  role: string
  handle?: string
  avatarUrl?: string
}>

export type WorkroomStatus = 'queued' | 'running' | 'completed' | 'failed'

export type KeyValueItem = Readonly<{
  label: string
  value: string | Html
}>

export type WorkroomTab = Readonly<{
  label: string
  active?: boolean
}>

export type WorkroomSessionItem = Readonly<{
  title: string
  detail: string
  status: 'active' | 'complete' | 'failed' | 'queued'
  href?: string
  active?: boolean
  attention?: boolean
}>

export type WorkroomSidebarNavSection = Readonly<{
  title: string
  items: ReadonlyArray<NavItem>
}>

export type WorkroomSidebarSessionSection = Readonly<{
  title: string
  items: ReadonlyArray<WorkroomSessionItem>
}>

export type WorkroomAccountMenuItem<Message> = Readonly<{
  label: string
  href?: string
  attrs?: ReadonlyArray<Attribute<Message>>
  tone?: 'normal' | 'danger'
}>

export type WorkroomTimelinePart =
  | Readonly<{
      kind: 'text'
      body: ReadonlyArray<string>
      tone?: 'normal' | 'muted'
    }>
  | Readonly<{
      kind: 'tool'
      title: string
      subtitle: string
      status: 'queued' | 'running' | 'completed' | 'failed'
      detail: ReadonlyArray<string>
      href?: string
      actionHref?: string
      actionLabel?: string
    }>
  | Readonly<{
      kind: 'diff'
      files: ReadonlyArray<
        Readonly<{
          path: string
          added: number
          removed: number
          status: 'modified' | 'added'
        }>
      >
    }>
  | Readonly<{
      kind: 'file'
      path: string
      language: string
      excerpt: ReadonlyArray<string>
    }>

export type WorkroomTimelineMessage = Readonly<{
  id: string
  author: 'user' | 'assistant' | 'system'
  label: string
  time: string
  parts: ReadonlyArray<WorkroomTimelinePart>
  avatarUrl?: string | undefined
  status?: 'complete' | 'streaming'
}>

export type WorkroomChecklistItem = Readonly<{
  label: string
  state: 'done' | 'active' | 'queued'
}>

export type WorkroomFileItem = Readonly<{
  label: string
  meta: string
  depth?: 0 | 1
  active?: boolean
}>

export type BadgeItem = Readonly<{
  label: string
  tone?: Tone
}>

export type ProgressStep = Readonly<{
  label: string
  detail?: string
  tone?: Tone
  active?: boolean
}>

export type CalendarDay = Readonly<{
  label: string
  meta?: string
  tone?: Tone
  active?: boolean
}>

export type TableColumn = Readonly<{
  key: string
  label: string
  align?: 'left' | 'right'
}>

export type TableRow = Readonly<{
  id: string
  cells: Readonly<Record<string, string | Html>>
  tone?: Tone
}>

export type FeedItem = Readonly<{
  title: string
  body?: string
  meta?: string
  tone?: Tone
}>

export type DetailScreenSection = Readonly<{
  title: string
  eyebrow?: string
  details?: ReadonlyArray<DescriptionItem>
  body?: string
  action?: Html
}>

export type GridListItem = Readonly<{
  title: string
  body?: string
  meta?: string
  tone?: Tone
  href?: string
}>

export type FormOption = Readonly<{
  label: string
  value: string
  detail?: string
  checked?: boolean
  disabled?: boolean
}>

export type ValidationState = 'idle' | 'validating' | 'valid' | 'invalid'

export const applicationUiV4Families = [
  'application-shells/sidebar',
  'application-shells/stacked',
  'application-shells/multi-column',
  'data-display/calendars',
  'data-display/stats',
  'data-display/description-lists',
  'elements/avatars',
  'elements/badges',
  'elements/buttons',
  'elements/button-groups',
  'elements/dropdowns',
  'feedback/alerts',
  'feedback/empty-states',
  'forms/action-panels',
  'forms/checkboxes',
  'forms/comboboxes',
  'forms/form-layouts',
  'forms/input-groups',
  'forms/radio-groups',
  'forms/select-menus',
  'forms/sign-in-forms',
  'forms/textareas',
  'forms/toggles',
  'headings/card-headings',
  'headings/page-headings',
  'headings/section-headings',
  'layout/cards',
  'layout/containers',
  'layout/dividers',
  'layout/list-containers',
  'layout/media-objects',
  'lists/feeds',
  'lists/grid-lists',
  'lists/stacked-lists',
  'lists/tables',
  'navigation/breadcrumbs',
  'navigation/command-palettes',
  'navigation/navbars',
  'navigation/pagination',
  'navigation/progress-bars',
  'navigation/sidebar-navigation',
  'navigation/tabs',
  'navigation/vertical-navigation',
  'overlays/drawers',
  'overlays/modal-dialogs',
  'overlays/notifications',
  'page-examples/detail-screens',
  'page-examples/home-screens',
  'page-examples/settings-screens',
] as const

export const ecommerceUiV4Families = [
  'components/category-filters',
  'components/category-previews',
  'components/checkout-forms',
  'components/incentives',
  'components/order-history',
  'components/order-summaries',
  'components/product-features',
  'components/product-lists',
  'components/product-overviews',
  'components/product-quickviews',
  'components/promo-sections',
  'components/reviews',
  'components/shopping-carts',
  'components/store-navigation',
  'page-examples/category-pages',
  'page-examples/checkout-pages',
  'page-examples/order-detail-pages',
  'page-examples/order-history-pages',
  'page-examples/product-pages',
  'page-examples/shopping-cart-pages',
  'page-examples/storefront-pages',
] as const

export const marketingUiV4Families = [
  'elements/banners',
  'elements/flyout-menus',
  'elements/headers',
  'feedback/404-pages',
  'page-examples/about-pages',
  'page-examples/landing-pages',
  'page-examples/pricing-pages',
  'sections/bento-grids',
  'sections/blog-sections',
  'sections/contact-sections',
  'sections/content-sections',
  'sections/cta-sections',
  'sections/faq-sections',
  'sections/feature-sections',
  'sections/footers',
  'sections/header',
  'sections/heroes',
  'sections/logo-clouds',
  'sections/newsletter-sections',
  'sections/pricing',
  'sections/stats-sections',
  'sections/team-sections',
  'sections/testimonials',
] as const

export const surfaceClass = 'border border-[#222] bg-[#010102] text-[#f1efe8]'
export const surfaceActiveClass =
  'border border-[#333] bg-[#141414] text-[#f1efe8]'
export const rowClass = 'flex min-w-0 items-center gap-2.5'
export const eyebrowClass =
  'text-[0.6875rem] font-semibold uppercase leading-[1.2] tracking-[0.08em] text-white/35'
export const titleClass =
  'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-[0.8125rem] font-medium text-white/90'
export const metaClass =
  'm-0 overflow-hidden text-ellipsis whitespace-nowrap text-[0.75rem] text-white/35'
export const motionStatusMorphAnimationClass = 'oa-status-morph'
export const motionStatusMorphClass = 'oa-status-morph inline-block'
export const motionOdometerClass =
  'oa-odometer-number inline-block tabular-nums'
export const motionTextRevealAnimationClass = 'oa-text-reveal'
export const motionTextRevealClass = 'oa-text-reveal inline-block'
export const motionPaneOpenClass = 'oa-pane-open'
export const motionRowHoverClass =
  'transition-[border-color,background-color,color,transform] duration-150 hover:-translate-y-px motion-reduce:transition-none motion-reduce:hover:translate-y-0'

export const toneTextClass = (tone: Tone = 'neutral'): string =>
  clsx({
    'text-white/60': tone === 'neutral',
    'text-[#ffb400]': tone === 'accent',
    'text-[#00c853]': tone === 'positive',
    'text-[#ff6f00]': tone === 'warning',
    'text-[#d32f2f]': tone === 'negative',
    'text-[#2979ff]': tone === 'info',
  })

export const statusDotClass = (tone: Tone = 'neutral'): string =>
  clsx('h-2 w-2 flex-none border', motionStatusMorphAnimationClass, {
    'border-white/30 bg-white/30': tone === 'neutral',
    'border-[#ffb400] bg-[#ffb400]': tone === 'accent',
    'border-[#00c853] bg-[#00c853]': tone === 'positive',
    'border-[#ff6f00] bg-[#ff6f00]': tone === 'warning',
    'border-[#d32f2f] bg-[#d32f2f]': tone === 'negative',
    'border-[#2979ff] bg-[#2979ff]': tone === 'info',
  })

export const buttonClass = (
  variant: ButtonVariant = 'primary',
  size: ButtonSize = 'md',
): string =>
  clsx(
    'inline-grid place-items-center border text-center font-medium no-underline transition-colors disabled:cursor-not-allowed disabled:opacity-45',
    {
      'min-h-11 px-4 text-sm': size === 'md',
      'min-h-9 px-3 text-xs': size === 'sm',
      'border-[#f1efe8] bg-[#f1efe8] text-[#000] hover:border-[#ffb400]':
        variant === 'primary',
      'border-[#222] bg-transparent text-[#f1efe8] hover:border-[#ffb400]':
        variant === 'secondary',
      'border-transparent bg-transparent text-white/60 hover:border-[#333] hover:bg-[#080808] hover:text-[#f1efe8]':
        variant === 'ghost',
      'border-[#d32f2f] bg-[#d32f2f] text-white hover:border-[#ff6f00]':
        variant === 'danger',
    },
  )

export const textLinkClass =
  'text-[#f1efe8] underline underline-offset-[3px] hover:text-[#ffb400]'

export const kitFamily = <Message>(family: string): Attribute<Message> =>
  html<Message>().DataAttribute('ui-family', family)
