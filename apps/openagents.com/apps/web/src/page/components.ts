import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { docsRouter } from '../route'
import { lightBeamsView } from '../scene/lightBeamsElement'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

// Internal design-system workbench.
//
// This page renders the `@openagentsinc/ui` Foldkit component library as a
// living registry of component families, mirroring the deprecated Maud
// `component_workbench.rs` registry (owner / purpose / use-when / avoid-when /
// a11y / tokens). It is rendered in the real Foldkit app shell, NOT Storybook,
// and honors the dark-only / pure-black / compact-mono / thin-border design
// contract documented in `packages/ui/src/README.md`.
//
// Gating note: the app has no hard admin/role gate for these public-style
// shell routes (docs/blog/forum render the same way for logged-out and
// logged-in users). This route is therefore reachable but deliberately kept
// OUT of the public navigation/sitemap and is clearly labeled "Internal" so it
// is not treated as a marketed product surface.

type FamilyMeta = {
  readonly id: string
  readonly title: string
  readonly module: string
  readonly owner: string
  readonly purpose: string
  readonly useWhen: ReadonlyArray<string>
  readonly avoidWhen: ReadonlyArray<string>
  readonly accessibility: ReadonlyArray<string>
  readonly tokens: ReadonlyArray<string>
  // Representative exported symbols from the family, for orientation.
  readonly exports: ReadonlyArray<string>
}

// Whether `@openagentsinc/ui` exports an `ai-elements` family at build time.
// Issue #5083 lands that family in parallel. We must never hard-depend on it,
// so we detect it defensively and render a placeholder when it is absent.
//
// `@openagentsinc/ui` re-exports the family as a namespace
// (`export * as AiElements from './ai-elements'`), so the family surfaces as
// `Ui.AiElements`. We probe a stable base-contract symbol on that namespace
// without importing the (possibly missing) module directly. A flat-name probe
// is kept as a fallback in case the export shape changes.
const aiElementsExported = ((): boolean => {
  const ui = Ui as unknown as Record<string, unknown>
  const namespace = ui['AiElements'] as Record<string, unknown> | undefined
  if (
    namespace !== undefined &&
    (typeof namespace['aiElementBase'] === 'function' ||
      typeof namespace['aiElementModuleCount'] === 'number')
  ) {
    return true
  }
  const probes = ['aiElementBase', 'aiMessage', 'aiPromptInput']
  return probes.some(name => typeof ui[name] === 'function')
})()

const families: ReadonlyArray<FamilyMeta> = [
  {
    id: 'primitives',
    title: 'Primitives',
    module: '@openagentsinc/ui/primitives',
    owner: 'UI system',
    purpose:
      'Centralize the shared class-string vocabulary: surfaces, rows, tones, status dots, buttons, links, and the Tailwind v4 kit-family taxonomy.',
    useWhen: [
      'composing a new component from the design contract',
      'needing a tone, surface, or status-dot class instead of a raw literal',
    ],
    avoidWhen: [
      'a higher-level family already renders the element you need',
      'you would inline a raw color literal instead of using a token class',
    ],
    accessibility: [
      'Focus tokens stay visible in the dark theme; never strip focus rings.',
    ],
    tokens: [
      'surfaceClass / surfaceActiveClass',
      'toneTextClass / statusDotClass',
      'buttonClass / textLinkClass',
    ],
    exports: [
      'surfaceClass',
      'toneTextClass',
      'statusDotClass',
      'buttonClass',
      'textLinkClass',
      'kitFamily',
    ],
  },
  {
    id: 'shared',
    title: 'Shared',
    module: '@openagentsinc/ui/shared',
    owner: 'UI system',
    purpose:
      'Render the core interactive atoms: buttons, link buttons, text links, avatars, button groups, dropdown menus, and heading blocks.',
    useWhen: [
      'adding an official button, link, or avatar',
      'needing a consistent heading block across surfaces',
    ],
    avoidWhen: [
      'the action belongs inside a workroom dock (use the workroom family)',
      'you need a v4 chat-styled control (use the v4 family)',
    ],
    accessibility: [
      'Buttons render real <button> elements; link buttons render real <a> elements with hrefs.',
    ],
    tokens: ['buttonClass', 'textLinkClass'],
    exports: [
      'button',
      'linkButton',
      'textLink',
      'avatar',
      'avatarGroup',
      'buttonGroup',
      'dropdownMenu',
      'headingBlock',
    ],
  },
  {
    id: 'forms',
    title: 'Forms',
    module: '@openagentsinc/ui/forms',
    owner: 'UI system',
    purpose:
      'Render labeled inputs, textareas, selects, checkbox/radio groups, comboboxes, toggles, and validated input groups.',
    useWhen: [
      'adding settings or onboarding fields',
      'documenting form spacing and validation slots',
    ],
    avoidWhen: [
      'the field needs validation UI not represented by the current props',
    ],
    accessibility: [
      'Every field exposes a visible label and a stable id/name pair.',
    ],
    tokens: ['inputClass', 'textareaClass', 'selectClass'],
    exports: [
      'inputGroup',
      'validatedInputGroup',
      'textareaGroup',
      'selectMenu',
      'checkboxList',
      'radioGroup',
      'toggleRow',
      'comboboxList',
    ],
  },
  {
    id: 'layout',
    title: 'Layout',
    module: '@openagentsinc/ui/layout',
    owner: 'UI system',
    purpose:
      'Provide the structural shells, containers, sections, cards, dividers, dialogs, drawers, and notification stacks the app composes inside.',
    useWhen: [
      'building a page shell or centered frame',
      'wrapping content in a titled section or card',
    ],
    avoidWhen: [
      'nesting a card inside another card (the contract forbids nested cards)',
    ],
    accessibility: [
      'Modal dialogs and drawers expose dialog semantics and a labeled close affordance.',
    ],
    tokens: ['surfaceClass', 'thin border tokens'],
    exports: [
      'pageShell',
      'stackedApplicationShell',
      'container',
      'section',
      'card',
      'divider',
      'modalDialog',
      'drawerPanel',
      'notificationStack',
    ],
  },
  {
    id: 'navigation',
    title: 'Navigation',
    module: '@openagentsinc/ui/navigation',
    owner: 'UI system',
    purpose:
      'Render tab bars, nav bars, breadcrumbs, pagination, vertical/sidebar navigation, command palettes, and progress lists.',
    useWhen: [
      'adding wayfinding between sections',
      'documenting breadcrumb or pagination spacing',
    ],
    avoidWhen: [
      'the surface is a workroom rail (use the workroom family)',
    ],
    accessibility: [
      'Navigation regions use real nav landmarks and aria-current for the active item.',
    ],
    tokens: ['surfaceActiveClass', 'textLinkClass'],
    exports: [
      'tabBar',
      'navBar',
      'breadcrumbBar',
      'pagination',
      'verticalNavigation',
      'sidebarNavigation',
      'commandPalette',
      'progressList',
    ],
  },
  {
    id: 'data-display',
    title: 'Data display',
    module: '@openagentsinc/ui/data-display',
    owner: 'UI system',
    purpose:
      'Render dense operational state: tables, key/value rows, code blocks, badges, stat grids, description lists, feeds, and commerce grids.',
    useWhen: [
      'showing comparable records or run/evidence rows',
      'showing a short key/value detail list or a code block',
    ],
    avoidWhen: [
      'a single status pill would do (use a badge, not a table)',
    ],
    accessibility: [
      'Tables include a caption, headers, and real cells; code blocks are read-only.',
    ],
    tokens: ['rowClass', 'toneTextClass', 'statusDotClass'],
    exports: [
      'tableList',
      'keyValueRows',
      'codeBlock',
      'badge',
      'statGrid',
      'descriptionList',
      'stackedList',
      'feedList',
      'gridList',
    ],
  },
  {
    id: 'feedback',
    title: 'Feedback',
    module: '@openagentsinc/ui/feedback',
    owner: 'UI system',
    purpose:
      'Render alerts and empty states that communicate tone, status, and next action.',
    useWhen: [
      'surfacing a tone-bearing message (info, positive, warning, negative)',
      'showing an empty list with a clear call to action',
    ],
    avoidWhen: [
      'a transient toast belongs in the notification stack (layout family)',
    ],
    accessibility: [
      'Tone is conveyed by text and a status dot, never color alone.',
    ],
    tokens: ['toneTextClass', 'statusDotClass'],
    exports: ['alert', 'emptyState'],
  },
  {
    id: 'workroom',
    title: 'Workroom',
    module: '@openagentsinc/ui/workroom',
    owner: 'UI system',
    purpose:
      'Render the workroom shell, rails, sidebars, split panes, timelines, composers, docks, panels, and metadata dialogs.',
    useWhen: [
      'building inside the Autopilot workroom surface',
      'rendering a session timeline, composer, or action dock',
    ],
    avoidWhen: [
      'the surface is a public marketing or docs page (use public/layout)',
    ],
    accessibility: [
      'Rails and docks keep focusable controls reachable in keyboard order.',
    ],
    tokens: ['surfaceClass', 'rowClass', 'panel header tokens'],
    exports: [
      'workroomShell',
      'workroomSidebar',
      'workroomSplit',
      'workroomTimeline',
      'workroomComposer',
      'workroomActionDock',
      'workroomPanel',
      'panelHeader',
    ],
  },
  {
    id: 'public',
    title: 'Public',
    module: '@openagentsinc/ui/public',
    owner: 'UI system',
    purpose:
      'Render marketing-surface families: banners, headers, heroes, feature/content/CTA sections, pricing, FAQ, testimonials, team, blog, and footer.',
    useWhen: [
      'building a public landing or marketing section',
      'composing a full marketing landing page',
    ],
    avoidWhen: [
      'introducing a marketing gradient inside a product/command surface',
    ],
    accessibility: [
      'Sections use heading hierarchy and keep link text descriptive.',
    ],
    tokens: ['surfaceClass', 'thin border tokens'],
    exports: [
      'marketingHero',
      'marketingHeader',
      'featureSection',
      'pricingGrid',
      'faqSection',
      'testimonialGrid',
      'blogList',
      'footer',
    ],
  },
  {
    id: 'page-examples',
    title: 'Page examples',
    module: '@openagentsinc/ui/page-examples',
    owner: 'UI system',
    purpose:
      'Render full-page composites assembled from the families: billing/credits, usage telemetry, settings, application home/detail, and commerce pages.',
    useWhen: [
      'documenting how families compose into a complete screen',
      'seeding a new page from an existing composite',
    ],
    avoidWhen: [
      'you only need one family in isolation (compose it directly)',
    ],
    accessibility: [
      'Composites inherit landmark and heading structure from their families.',
    ],
    tokens: ['inherits family tokens'],
    exports: [
      'billingCreditsPage',
      'usageTelemetryPage',
      'settingsScreen',
      'applicationHomeScreen',
      'applicationDetailScreen',
      'commerceCheckoutPage',
    ],
  },
  {
    id: 'v4',
    title: 'V4',
    module: '@openagentsinc/ui/v4',
    owner: 'UI system',
    purpose:
      'Render the v4 chat-styled control set: buttons, badges, agent icons, inputs, modal cards, composer, chat messages, navbar, and sidebar.',
    useWhen: [
      'building the v4 chat/agent surface',
      'needing a v4-styled composer or chat message row',
    ],
    avoidWhen: [
      'the surface uses the standard shared/forms families (do not mix v4 styling)',
    ],
    accessibility: [
      'v4 controls keep real button/input semantics under the chat styling.',
    ],
    tokens: ['v4ButtonClass', 'agent icon tokens'],
    exports: [
      'v4Button',
      'v4LinkButton',
      'v4Badge',
      'v4AgentIcon',
      'v4TextInput',
      'v4Composer',
      'v4ChatMessage',
      'v4Navbar',
      'v4Sidebar',
    ],
  },
]

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
  selectedFamily?: string,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      h.main(
        [
          h.AriaLabel('Component library'),
          Ui.className<Message>(
            'mx-auto grid w-[min(100%,1120px)] gap-8 px-4 py-8 lg:grid-cols-[220px_minmax(0,1fr)]',
          ),
        ],
        [
          sidebarView<Message>(selectedFamily),
          articleView<Message>(selectedFamily),
        ],
      ),
    ],
  )
}

const navLinkClass = (active: boolean): string =>
  active
    ? 'rounded bg-white/[0.07] px-2 py-2 text-base text-[#f1efe8] sm:text-sm'
    : 'rounded px-2 py-2 text-base text-white/50 hover:bg-white/[0.04] hover:text-[#f1efe8] sm:text-sm'

const sidebarView = <Message>(selectedFamily?: string): Html => {
  const h = html<Message>()

  const aiElementsNavLabel = aiElementsExported
    ? 'AI Elements'
    : 'AI Elements (coming with #5083)'

  return h.aside(
    [Ui.className<Message>('min-w-0 lg:sticky lg:top-6 lg:self-start')],
    [
      h.a(
        [
          h.Href(docsRouter()),
          Ui.className<Message>(
            'block rounded px-2 py-2 text-base text-white/70 hover:bg-white/[0.04] sm:text-sm',
          ),
        ],
        ['Back to docs'],
      ),
      h.nav(
        [
          h.AriaLabel('Component families'),
          Ui.className<Message>('mt-3 grid gap-1'),
        ],
        [
          h.a(
            [
              h.Href('/components'),
              Ui.className<Message>(navLinkClass(selectedFamily === undefined)),
            ],
            ['All families'],
          ),
          h.a(
            [
              h.Href('/components/login'),
              Ui.className<Message>(navLinkClass(selectedFamily === 'login')),
            ],
            ['Login (rendered)'],
          ),
          ...Array.map(families, family =>
            h.a(
              [
                h.Href(`/components/${family.id}`),
                Ui.className<Message>(
                  navLinkClass(selectedFamily === family.id),
                ),
              ],
              [family.title],
            ),
          ),
          h.a(
            [
              h.Href('/components/ai-elements'),
              Ui.className<Message>(
                navLinkClass(selectedFamily === 'ai-elements'),
              ),
            ],
            [aiElementsNavLabel],
          ),
        ],
      ),
    ],
  )
}

const articleView = <Message>(selectedFamily?: string): Html => {
  const h = html<Message>()

  if (selectedFamily === 'login') {
    return loginShowcaseView<Message>()
  }

  const knownFamily = families.find(family => family.id === selectedFamily)
  const isIndex =
    selectedFamily === undefined ||
    (selectedFamily !== 'ai-elements' && knownFamily === undefined)

  const familySections: ReadonlyArray<Html> = isIndex
    ? [
        ...Array.map(families, family => familyCard<Message>(family)),
        aiElementsCard<Message>(),
      ]
    : selectedFamily === 'ai-elements'
      ? [aiElementsCard<Message>()]
      : knownFamily !== undefined
        ? [familyCard<Message>(knownFamily)]
        : []

  return h.article(
    [
      Ui.className<Message>(
        'min-w-0 border border-[#222] bg-[#010102] p-5 sm:p-6',
      ),
    ],
    [
      h.p(
        [
          Ui.className<Message>(
            'mb-3 font-mono text-base text-white/35 sm:text-sm',
          ),
        ],
        ['Internal - design-system workbench'],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
          ),
        ],
        ['Component library'],
      ),
      h.p(
        [
          Ui.className<Message>(
            'mt-3 max-w-[76ch] text-base/7 text-white/60',
          ),
        ],
        [
          'Every component family in @openagentsinc/ui, rendered in the real Foldkit app shell (not Storybook). This is an internal reference: it is intentionally kept out of the public navigation and honors the dark-only, pure-black, compact-mono, thin-border design contract.',
        ],
      ),
      ...(isIndex ? [liveSamplesView<Message>()] : []),
      h.div([Ui.className<Message>('mt-8 grid gap-4')], familySections),
    ],
  )
}

// A labeled preview frame: caption + export name above the live component on a
// pure-black surface (so the gallery shows the real rendered thing, not prose).
const previewBox = <Message>(
  caption: string,
  exportName: string,
  child: Html,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('border border-[#222] bg-white/[0.02] p-4 sm:p-5')],
    [
      h.div(
        [
          Ui.className<Message>(
            'mb-3 flex flex-wrap items-baseline justify-between gap-2',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/40',
              ),
            ],
            [caption],
          ),
          h.code(
            [Ui.className<Message>('font-mono text-[0.75rem] text-white/40')],
            [exportName],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'rounded border border-[#1a1a1a] bg-[#000] p-5',
          ),
        ],
        [child],
      ),
    ],
  )
}

// A grid of preview boxes, used as the lead "rendered" section of a family
// card (before the contract metadata).
const showcaseGrid = <Message>(boxes: ReadonlyArray<Html>): Html => {
  const h = html<Message>()
  return h.div([Ui.className<Message>('grid gap-3')], boxes)
}

// Live, rendered instances of each family's representative exports. This is the
// primary content of the gallery: real components on a pure-black surface via
// `previewBox`, mirroring the Login showcase. Returns an empty array for any
// family without a rendered showcase (the metadata still renders below).
const familyShowcase = <Message>(familyId: string): ReadonlyArray<Html> => {
  const h = html<Message>()
  const box = previewBox<Message>

  switch (familyId) {
    case 'primitives':
      return [
        box(
          'Tones (badges)',
          'badge x toneTextClass',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.badge<Message>({ label: 'Neutral', tone: 'neutral' }),
              Ui.badge<Message>({ label: 'Accent', tone: 'accent' }),
              Ui.badge<Message>({ label: 'Positive', tone: 'positive' }),
              Ui.badge<Message>({ label: 'Warning', tone: 'warning' }),
              Ui.badge<Message>({ label: 'Negative', tone: 'negative' }),
              Ui.badge<Message>({ label: 'Info', tone: 'info' }),
            ],
          ),
        ),
        box(
          'Buttons',
          'button (buttonClass variants)',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.button<Message>({ label: 'Primary', variant: 'primary' }),
              Ui.button<Message>({ label: 'Secondary', variant: 'secondary' }),
              Ui.button<Message>({ label: 'Ghost', variant: 'ghost' }),
              Ui.button<Message>({ label: 'Danger', variant: 'danger' }),
            ],
          ),
        ),
        box(
          'Text link',
          'textLink (textLinkClass)',
          Ui.textLink<Message>({ href: '#family-primitives', label: 'A text link' }),
        ),
      ]

    case 'shared':
      return [
        box(
          'Heading block',
          'headingBlock',
          Ui.headingBlock<Message>({
            eyebrow: 'Section',
            title: 'Heading block',
            body: 'A consistent heading + body across surfaces.',
            level: 2,
          }),
        ),
        box(
          'Buttons & link button',
          'button / linkButton',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.button<Message>({ label: 'Action', variant: 'primary' }),
              Ui.linkButton<Message>({
                href: '#family-shared',
                label: 'Link button',
                variant: 'secondary',
              }),
            ],
          ),
        ),
        box(
          'Avatar group',
          'avatar / avatarGroup',
          Ui.avatarGroup<Message>([
            { title: 'Agent One' },
            { title: 'Agent Two' },
            { title: 'Agent Three' },
          ]),
        ),
        box(
          'Dropdown menu',
          'dropdownMenu',
          Ui.dropdownMenu<Message>({
            label: 'Menu',
            items: [
              { href: '#', label: 'Open run' },
              { href: '#', label: 'View evidence' },
              { href: '#', label: 'Archive', tone: 'warning' },
            ],
          }),
        ),
      ]

    case 'forms':
      return [
        box(
          'Input group',
          'inputGroup',
          Ui.inputGroup<Message>({
            id: 'showcase-forms-name',
            name: 'name',
            label: 'Display name',
            placeholder: 'Type a name',
            help: 'Shown on public surfaces.',
          }),
        ),
        box(
          'Validated input',
          'validatedInputGroup',
          Ui.validatedInputGroup<Message>({
            id: 'showcase-forms-handle',
            name: 'handle',
            label: 'Handle',
            state: 'invalid',
            value: 'taken-handle',
            error: 'That handle is already in use.',
          }),
        ),
        box(
          'Textarea',
          'textareaGroup',
          Ui.textareaGroup<Message>({
            id: 'showcase-forms-notes',
            name: 'notes',
            label: 'Notes',
            placeholder: 'Add context...',
            rows: 3,
          }),
        ),
        box(
          'Select menu',
          'selectMenu',
          Ui.selectMenu<Message>({
            id: 'showcase-forms-region',
            name: 'region',
            label: 'Region',
            options: [
              { label: 'Auto', value: 'auto' },
              { label: 'US', value: 'us' },
              { label: 'EU', value: 'eu' },
            ],
          }),
        ),
        box(
          'Checkbox list',
          'checkboxList',
          Ui.checkboxList<Message>({
            name: 'capabilities',
            legend: 'Capabilities',
            options: [
              { label: 'Read repository', value: 'read', checked: true },
              { label: 'Open pull requests', value: 'pr' },
              { label: 'Run commands', value: 'run' },
            ],
          }),
        ),
        box(
          'Radio group',
          'radioGroup',
          Ui.radioGroup<Message>({
            name: 'visibility',
            legend: 'Visibility',
            options: [
              { label: 'Private', value: 'private', checked: true },
              { label: 'Public', value: 'public' },
            ],
          }),
        ),
        box(
          'Toggle row',
          'toggleRow',
          Ui.toggleRow<Message>({
            id: 'showcase-forms-autotopup',
            name: 'auto_top_up',
            label: 'Auto top-up',
            detail: 'Refill credits when the balance runs low.',
            checked: true,
          }),
        ),
      ]

    case 'layout':
      return [
        box(
          'Card',
          'card',
          Ui.card<Message>({
            children: [
              Ui.headingBlock<Message>({
                title: 'Card',
                body: 'A titled surface with a thin border.',
                level: 3,
              }),
            ],
          }),
        ),
        box(
          'Section',
          'section',
          Ui.section<Message>([
            h.p(
              [Ui.className<Message>('m-0 text-base/7 text-white/60')],
              ['A structural section wrapper for grouped content.'],
            ),
          ]),
        ),
        box('Divider', 'divider', Ui.divider<Message>('Labeled divider')),
        box(
          'Modal dialog',
          'modalDialog',
          Ui.modalDialog<Message>({
            title: 'Confirm action',
            body: 'This renders the dialog surface inline (no overlay).',
            actions: [
              Ui.button<Message>({ label: 'Confirm', variant: 'primary' }),
              Ui.button<Message>({ label: 'Cancel', variant: 'ghost' }),
            ],
          }),
        ),
        box(
          'Drawer panel',
          'drawerPanel',
          Ui.drawerPanel<Message>({
            title: 'Details',
            children: [
              h.p(
                [Ui.className<Message>('m-0 text-base/7 text-white/60')],
                ['A side drawer panel surface.'],
              ),
            ],
          }),
        ),
        box(
          'Notification stack',
          'notificationStack',
          Ui.notificationStack<Message>([
            { title: 'Run started', body: 'Workroom session is live.', tone: 'info' },
            {
              title: 'Run completed',
              body: 'Evidence attached.',
              tone: 'positive',
            },
          ]),
        ),
      ]

    case 'navigation':
      return [
        box(
          'Tab bar',
          'tabBar',
          Ui.tabBar<Message>([
            { label: 'Overview', active: true },
            { label: 'Activity' },
            { label: 'Settings' },
          ]),
        ),
        box(
          'Nav bar',
          'navBar',
          Ui.navBar<Message>([
            { href: '#', label: 'Home', active: true },
            { href: '#', label: 'Docs' },
            { href: '#', label: 'Forum' },
          ]),
        ),
        box(
          'Breadcrumbs',
          'breadcrumbBar',
          Ui.breadcrumbBar<Message>([
            { href: '#', label: 'Components' },
            { href: '#', label: 'Navigation' },
            { href: '#', label: 'Breadcrumbs', active: true },
          ]),
        ),
        box(
          'Pagination',
          'pagination',
          Ui.pagination<Message>({
            previousHref: '#',
            nextHref: '#',
            pages: [
              { href: '#', label: '1', active: true },
              { href: '#', label: '2' },
              { href: '#', label: '3' },
            ],
          }),
        ),
        box(
          'Vertical navigation',
          'verticalNavigation',
          Ui.verticalNavigation<Message>([
            { href: '#', label: 'Dashboard', active: true },
            { href: '#', label: 'Runs', meta: '12' },
            { href: '#', label: 'Evidence' },
          ]),
        ),
        box(
          'Command palette',
          'commandPalette',
          Ui.commandPalette<Message>({
            title: 'Quick actions',
            placeholder: 'Search actions...',
            items: [
              { href: '#', label: 'Open new run' },
              { href: '#', label: 'View product promises' },
            ],
          }),
        ),
        box(
          'Progress list',
          'progressList',
          Ui.progressList<Message>([
            { label: 'Queued', tone: 'neutral' },
            { label: 'Running', tone: 'info', active: true },
            { label: 'Completed', tone: 'positive' },
          ]),
        ),
      ]

    case 'data-display':
      return [
        box(
          'Table',
          'tableList',
          Ui.tableList<Message>({
            caption: 'Recent runs',
            columns: [
              { key: 'id', label: 'Run' },
              { key: 'status', label: 'Status' },
              { key: 'tokens', label: 'Tokens', align: 'right' },
            ],
            rows: [
              {
                id: 'r1',
                cells: { id: 'run-001', status: 'Completed', tokens: '12,480' },
                tone: 'positive',
              },
              {
                id: 'r2',
                cells: { id: 'run-002', status: 'Running', tokens: '3,210' },
                tone: 'info',
              },
              {
                id: 'r3',
                cells: { id: 'run-003', status: 'Failed', tokens: '980' },
                tone: 'negative',
              },
            ],
          }),
        ),
        box(
          'Key/value rows',
          'keyValueRows',
          Ui.keyValueRows<Message>([
            { label: 'Repository', value: 'openagents/openagents' },
            { label: 'Branch', value: 'main' },
            { label: 'Runner', value: 'oa-node-1' },
          ]),
        ),
        box(
          'Code block',
          'codeBlock',
          Ui.codeBlock<Message>({
            lines: [
              '$ bun run build:web',
              'vite v6 building for production...',
              'built in 4.2s',
            ],
          }),
        ),
        box(
          'Stat grid',
          'statGrid',
          Ui.statGrid<Message>([
            { label: 'Active runs', value: '7', tone: 'info' },
            { label: 'Completed', value: '128', tone: 'positive' },
            { label: 'Failed', value: '3', tone: 'negative' },
          ]),
        ),
        box(
          'Description list',
          'descriptionList',
          Ui.descriptionList<Message>([
            { label: 'Status', value: 'Operational' },
            { label: 'Region', value: 'auto' },
          ]),
        ),
        box(
          'Stacked list',
          'stackedList',
          Ui.stackedList<Message>([
            { title: 'run-001', detail: 'Completed', meta: '2m ago', tone: 'positive' },
            { title: 'run-002', detail: 'Running', meta: 'now', tone: 'info' },
          ]),
        ),
        box(
          'Feed list',
          'feedList',
          Ui.feedList<Message>([
            { title: 'Run queued', meta: '5m ago', tone: 'neutral' },
            { title: 'Evidence attached', meta: '2m ago', tone: 'positive' },
          ]),
        ),
        box(
          'Grid list',
          'gridList',
          Ui.gridList<Message>([
            { title: 'Inference', body: 'Serving and rollout', meta: 'family' },
            { title: 'Training', body: 'Distributed runs', meta: 'family' },
          ]),
        ),
      ]

    case 'feedback':
      return [
        box(
          'Alert (info)',
          'alert',
          Ui.alert<Message>({
            title: 'Heads up',
            body: 'Tone is conveyed by text plus a status dot, never color alone.',
            tone: 'info',
          }),
        ),
        box(
          'Alert (positive)',
          'alert',
          Ui.alert<Message>({
            title: 'Run completed',
            body: 'Evidence is attached and verified.',
            tone: 'positive',
          }),
        ),
        box(
          'Alert (negative)',
          'alert',
          Ui.alert<Message>({
            title: 'Run failed',
            body: 'See the timeline for the failing step.',
            tone: 'negative',
          }),
        ),
        box(
          'Empty state',
          'emptyState',
          Ui.emptyState<Message>({
            title: 'No runs yet',
            body: 'Start a run to see activity here.',
            action: Ui.button<Message>({ label: 'New run', variant: 'primary' }),
          }),
        ),
      ]

    case 'workroom':
      return [
        box(
          'Panel header',
          'panelHeader',
          Ui.panelHeader<Message>({
            title: 'Session',
            detail: 'Live workroom',
            tone: 'info',
          }),
        ),
        box(
          'Panel',
          'workroomPanel',
          Ui.workroomPanel<Message>([
            Ui.panelHeader<Message>({ title: 'Files', detail: '3 changed' }),
            h.p(
              [Ui.className<Message>('m-0 p-3 text-base/7 text-white/60')],
              ['A workroom panel wraps a header and body content.'],
            ),
          ]),
        ),
        box(
          'Checklist',
          'workroomChecklist',
          Ui.workroomChecklist<Message>({
            title: 'Plan',
            meta: '2 of 3',
            items: [
              { label: 'Read the issue', state: 'done' },
              { label: 'Implement the change', state: 'active' },
              { label: 'Open a pull request', state: 'queued' },
            ],
          }),
        ),
        box(
          'Action dock',
          'workroomActionDock',
          Ui.workroomActionDock<Message>({
            title: 'Actions',
            meta: 'Awaiting approval',
            rows: [
              {
                label: 'Approve diff',
                action: Ui.button<Message>({
                  label: 'Approve',
                  variant: 'primary',
                  size: 'sm',
                }),
              },
              {
                label: 'Reject diff',
                action: Ui.button<Message>({
                  label: 'Reject',
                  variant: 'danger',
                  size: 'sm',
                }),
              },
            ],
          }),
        ),
        box(
          'Timeline',
          'workroomTimeline',
          Ui.workroomTimeline<Message>({
            messages: [
              {
                id: 'm1',
                author: 'user',
                label: 'Operator',
                time: 'now',
                parts: [{ kind: 'text', body: ['Implement the gallery showcase.'] }],
              },
              {
                id: 'm2',
                author: 'assistant',
                label: 'Agent',
                time: 'now',
                status: 'complete',
                parts: [
                  {
                    kind: 'text',
                    body: ['On it — rendering live component instances.'],
                  },
                  {
                    kind: 'tool',
                    title: 'edit',
                    subtitle: 'components.ts',
                    status: 'completed',
                    detail: ['Added familyShowcase for every family.'],
                  },
                ],
              },
            ],
          }),
        ),
      ]

    case 'public':
      return [
        box(
          'Marketing hero',
          'marketingHero',
          Ui.marketingHero<Message>({
            eyebrow: 'OpenAgents',
            title: 'A machine-work economy',
            body: 'Render real components, not just prose.',
            primaryAction: Ui.linkButton<Message>({
              href: '#',
              label: 'Get started',
              variant: 'primary',
            }),
            secondaryAction: Ui.linkButton<Message>({
              href: '#',
              label: 'Read the docs',
              variant: 'secondary',
            }),
          }),
        ),
        box(
          'Feature section',
          'featureSection',
          Ui.featureSection<Message>({
            eyebrow: 'Why',
            title: 'Built for agents',
            features: [
              { title: 'Composable', body: 'One family per concern.', tone: 'accent' },
              { title: 'Dark-only', body: 'Pure-black command surfaces.' },
              { title: 'Verifiable', body: 'Evidence over claims.', tone: 'positive' },
            ],
          }),
        ),
        box(
          'Pricing grid',
          'pricingGrid',
          Ui.pricingGrid<Message>([
            {
              name: 'Starter',
              price: 'Free',
              features: ['Community access', 'Public surfaces'],
            },
            {
              name: 'Pro',
              price: '$ usage',
              highlighted: true,
              features: ['Managed runs', 'Evidence receipts'],
              actionLabel: 'Start',
              actionHref: '#',
            },
          ]),
        ),
        box(
          'FAQ section',
          'faqSection',
          Ui.faqSection<Message>([
            { question: 'What is this gallery?', answer: 'A live component reference.' },
            { question: 'Is it public?', answer: 'No — internal, kept out of nav.' },
          ]),
        ),
        box(
          'Blog list',
          'blogList',
          Ui.blogList<Message>([
            {
              title: 'Rendering the component library',
              excerpt: 'Live instances replace metadata-only cards.',
              meta: 'Engineering',
            },
          ]),
        ),
        box(
          'Footer',
          'footer',
          Ui.footer<Message>([
            { href: '#', label: 'Docs' },
            { href: '#', label: 'Forum' },
            { href: '#', label: 'Status' },
          ]),
        ),
      ]

    case 'page-examples':
      return [
        box(
          'Application home',
          'applicationHomeScreen',
          Ui.applicationHomeScreen<Message>({
            eyebrow: 'Welcome',
            title: 'Dashboard',
            body: 'A full-page composite assembled from families.',
            stats: [
              { label: 'Active runs', value: '7', tone: 'info' },
              { label: 'Completed', value: '128', tone: 'positive' },
              { label: 'Credits', value: '$42.10' },
            ],
            steps: [
              { label: 'Connect repo', tone: 'positive' },
              { label: 'Start a run', tone: 'info', active: true },
              { label: 'Review evidence' },
            ],
          }),
        ),
        box(
          'Settings screen',
          'settingsScreen',
          Ui.settingsScreen<Message>({
            title: 'Account settings',
            body: 'A settings composite.',
            details: [
              { label: 'Plan', value: 'Usage-based' },
              { label: 'Region', value: 'Auto' },
            ],
            actions: [
              Ui.button<Message>({ label: 'Save', variant: 'primary' }),
            ],
          }),
        ),
        box(
          'Checkout page',
          'commerceCheckoutPage',
          Ui.commerceCheckoutPage<Message>({
            title: 'Review order',
            fields: [
              { label: 'Plan', value: 'Pro' },
              { label: 'Billing', value: 'Monthly' },
            ],
            lines: [
              { label: 'Subtotal', value: '$20.00' },
              { label: 'Tax', value: '$1.60' },
              { label: 'Total', value: '$21.60', strong: true },
            ],
            action: Ui.button<Message>({
              label: 'Place order',
              variant: 'primary',
              block: true,
            }),
          }),
        ),
      ]

    case 'v4':
      return [
        box(
          'V4 buttons',
          'v4Button',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.v4Button<Message>({ label: 'Primary', variant: 'primary' }),
              Ui.v4Button<Message>({ label: 'Secondary', variant: 'secondary' }),
              Ui.v4Button<Message>({ label: 'Ghost', variant: 'ghost' }),
              Ui.v4Button<Message>({ label: 'Danger', variant: 'danger' }),
            ],
          ),
        ),
        box(
          'V4 badges',
          'v4Badge',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [
              Ui.v4Badge<Message>({ label: 'Neutral', tone: 'neutral' }),
              Ui.v4Badge<Message>({ label: 'Primary', tone: 'primary' }),
              Ui.v4Badge<Message>({ label: 'Success', tone: 'success' }),
              Ui.v4Badge<Message>({ label: 'Warning', tone: 'warning' }),
              Ui.v4Badge<Message>({ label: 'Danger', tone: 'danger' }),
            ],
          ),
        ),
        box(
          'V4 agent icon',
          'v4AgentIcon',
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-3')],
            [
              Ui.v4AgentIcon<Message>({ label: 'Agent', status: 'online' }),
              Ui.v4AgentIcon<Message>({ label: 'Busy', tone: 'warning', status: 'busy' }),
              Ui.v4AgentIcon<Message>({ label: 'Off', status: 'offline' }),
            ],
          ),
        ),
        box(
          'V4 text input',
          'v4TextInput',
          Ui.v4TextInput<Message>({
            id: 'showcase-v4-input',
            name: 'q',
            type: 'search',
            placeholder: 'Search agents...',
          }),
        ),
        box(
          'V4 chat message',
          'v4ChatMessage',
          Ui.v4ChatMessage<Message>({
            author: 'Agent',
            body: 'Rendering the v4 chat-styled control set live.',
            meta: 'now',
          }),
        ),
        box(
          'V4 composer',
          'v4Composer',
          Ui.v4Composer<Message>({
            id: 'showcase-v4-composer',
            name: 'message',
            placeholder: 'Message the agent...',
            caption: 'Enter to send',
            action: Ui.v4Button<Message>({
              label: 'Send',
              variant: 'primary',
              size: 'md',
            }),
          }),
        ),
        box(
          'V4 modal card',
          'v4ModalCard',
          Ui.v4ModalCard<Message>({
            title: 'Connect provider',
            body: 'A v4-styled modal card surface.',
            footer: Ui.v4Button<Message>({ label: 'Connect', variant: 'primary' }),
          }),
        ),
      ]

    default:
      return []
  }
}

// Live, rendered instances of the AI Elements family via the `Ui.AiElements.*`
// namespace. Probed defensively so a missing/renamed export never breaks the
// build: any export that is not a function is skipped gracefully.
const aiElementsShowcase = <Message>(): ReadonlyArray<Html> => {
  const ns = (Ui as unknown as Record<string, unknown>)['AiElements'] as
    | Record<string, unknown>
    | undefined
  if (ns === undefined) {
    return []
  }

  const box = previewBox<Message>
  const boxes: Array<Html> = []
  const has = (name: string): boolean => typeof ns[name] === 'function'
  const call = <T>(name: string, arg: T): Html =>
    (ns[name] as (input: T) => Html)(arg)

  if (has('promptInput')) {
    boxes.push(
      box(
        'Prompt input',
        'AiElements.promptInput',
        call('promptInput', {
          props: {
            name: 'prompt',
            placeholder: 'Ask the agent to do something...',
            status: 'ready',
            submitLabel: 'Send',
            rows: 3,
          },
        }),
      ),
    )
  }

  if (has('message')) {
    boxes.push(
      box(
        'Message (assistant)',
        'AiElements.message',
        call('message', {
          props: {
            role: 'assistant',
            author: 'Agent',
            time: 'now',
            body: 'Here is a rendered AI message bubble with role-based styling.',
          },
        }),
      ),
    )
  }

  if (has('codeBlock')) {
    boxes.push(
      box(
        'Code block',
        'AiElements.codeBlock',
        call('codeBlock', {
          props: {
            filename: 'example.ts',
            language: 'typescript',
            code: "export const greet = (name: string): string =>\n  `Hello, ${name}`",
          },
          result: {
            status: 'passed',
            summary: 'Compiled and ran',
            duration: '0.4s',
          },
        }),
      ),
    )
  }

  if (has('task')) {
    boxes.push(
      box(
        'Task',
        'AiElements.task',
        call('task', {
          props: {
            title: 'Implement the gallery',
            open: true,
            items: [
              { label: 'Read the issue', status: 'done' },
              { label: 'Render components', status: 'active' },
              { label: 'Open a PR', status: 'queued' },
            ],
          },
        }),
      ),
    )
  }

  if (has('sources')) {
    boxes.push(
      box(
        'Sources',
        'AiElements.sources',
        call('sources', {
          props: {
            open: true,
            label: 'Sources',
            sources: [
              { title: 'Component contract', href: '#' },
              { title: 'Design tokens', href: '#' },
            ],
          },
        }),
      ),
    )
  }

  if (has('tool')) {
    boxes.push(
      box(
        'Tool (awaiting approval)',
        'AiElements.tool',
        call('tool', {
          props: {
            name: 'edit_file',
            state: 'awaiting-approval',
            input: '{ "path": "components.ts" }',
            open: true,
          },
        }),
      ),
    )
  }

  if (has('confirmation')) {
    boxes.push(
      box(
        'Confirmation',
        'AiElements.confirmation',
        call('confirmation', {
          props: {
            title: 'Apply 3 file edits?',
            state: 'requested',
            detail: 'The agent wants to write changes to the working tree.',
          },
        }),
      ),
    )
  }

  if (has('reasoning')) {
    boxes.push(
      box(
        'Reasoning',
        'AiElements.reasoning',
        call('reasoning', {
          props: {
            text: 'First read the issue, then render each family with realistic sample props.',
            open: true,
            duration: 4,
          },
        }),
      ),
    )
  }

  if (has('webPreview')) {
    boxes.push(
      box(
        'Web preview',
        'AiElements.webPreview',
        call('webPreview', {
          props: {
            url: 'https://openagents.com/components',
            title: 'Component library',
            console: ['ready', 'rendered 12 families'],
          },
        }),
      ),
    )
  }

  return boxes
}

// The Login showcase: the real loginScreen/loginForm from @openagentsinc/ui,
// rendered live, plus the smaller components it is composed from.
const loginShowcaseView = <Message>(): Html => {
  const h = html<Message>()
  const githubHref = '/login/github'

  return h.article(
    [
      Ui.className<Message>(
        'min-w-0 border border-[#222] bg-[#010102] p-5 sm:p-6',
      ),
    ],
    [
      h.p(
        [
          Ui.className<Message>(
            'mb-3 font-mono text-base text-white/35 sm:text-sm',
          ),
        ],
        ['Internal - design-system workbench'],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-balance text-3xl font-medium tracking-normal text-[#f1efe8] sm:text-4xl',
          ),
        ],
        ['Login'],
      ),
      h.p(
        [Ui.className<Message>('mt-3 max-w-[76ch] text-base/7 text-white/60')],
        [
          'The login screen and form rendered live from @openagentsinc/ui (loginScreen / loginForm), composed from the input-group and button primitives shown below. This is the real component, before it ships on the /login page.',
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-8 grid gap-4')],
        [
          previewBox<Message>(
            'Light beams (background)',
            'lightBeamsView / oa-light-beams',
            h.div(
              [
                Ui.className<Message>(
                  'relative h-[260px] overflow-hidden bg-[#000]',
                ),
              ],
              [lightBeamsView<Message>()],
            ),
          ),
          previewBox<Message>(
            'Login screen',
            'loginForm + light beams',
            h.div(
              [
                Ui.className<Message>(
                  'relative grid h-[460px] place-items-center overflow-hidden bg-[#000] px-4',
                ),
              ],
              [
                lightBeamsView<Message>(),
                h.div(
                  [
                    Ui.className<Message>(
                      'relative z-10 grid w-full max-w-[360px] gap-10',
                    ),
                  ],
                  [
                    h.div(
                      [
                        Ui.className<Message>(
                          'text-center text-2xl font-medium tracking-tight text-[#f1efe8]',
                        ),
                      ],
                      ['OpenAgents'],
                    ),
                    Ui.loginForm<Message>({ githubHref }),
                  ],
                ),
              ],
            ),
          ),
          previewBox<Message>(
            'Login form',
            'loginForm',
            Ui.loginForm<Message>({ githubHref }),
          ),
          previewBox<Message>(
            'Email field',
            'inputGroup',
            Ui.inputGroup<Message>({
              id: 'showcase-login-email',
              name: 'email',
              label: 'Email address',
              type: 'email',
              placeholder: 'you@example.com',
            }),
          ),
          previewBox<Message>(
            'Submit button',
            'button (primary, block)',
            Ui.button<Message>({
              label: 'Send sign-in link',
              variant: 'primary',
              block: true,
            }),
          ),
          previewBox<Message>(
            'GitHub button',
            'linkButton (secondary, block)',
            Ui.linkButton<Message>({
              href: githubHref,
              label: 'Continue with GitHub',
              variant: 'secondary',
              block: true,
            }),
          ),
        ],
      ),
    ],
  )
}

// A small set of safe, self-contained live samples so the gallery shows real
// rendered primitives, not just documentation prose.
const liveSamplesView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('mt-6 grid gap-3')],
    [
      sectionHeading<Message>('Live samples'),
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-3')],
        [
          Ui.button<Message>({ label: 'Primary', variant: 'primary' }),
          Ui.button<Message>({ label: 'Secondary', variant: 'secondary' }),
          Ui.textLink<Message>({ href: '#family-shared', label: 'Text link' }),
          Ui.badge<Message>({ label: 'Live', tone: 'positive' }),
          Ui.badge<Message>({ label: 'Gated', tone: 'warning' }),
          Ui.badge<Message>({ label: 'Blocked', tone: 'negative' }),
        ],
      ),
      Ui.alert<Message>({
        title: 'Dark-only contract',
        body: 'Tone is conveyed by text plus a status dot, never by color alone.',
        tone: 'info',
      }),
    ],
  )
}

const sectionHeading = <Message>(text: string): Html => {
  const h = html<Message>()

  return h.h2(
    [
      Ui.className<Message>(
        'm-0 text-xl font-medium tracking-normal text-[#f1efe8]',
      ),
    ],
    [text],
  )
}

const metaList = <Message>(
  label: string,
  items: ReadonlyArray<string>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/40',
          ),
        ],
        [label],
      ),
      h.ul(
        [Ui.className<Message>('m-0 grid list-disc gap-1 pl-5')],
        Array.map(items, item =>
          h.li(
            [Ui.className<Message>('text-base/7 text-white/60')],
            [item],
          ),
        ),
      ),
    ],
  )
}

// The secondary contract-metadata block (use/avoid/a11y/tokens/exports),
// rendered below the live components.
const familyContractView = <Message>(family: FamilyMeta): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('mt-5 border-t border-[#1a1a1a] pt-4')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/40',
          ),
        ],
        ['Contract'],
      ),
      h.p(
        [
          Ui.className<Message>(
            'mt-2 font-mono text-[0.75rem] text-white/45',
          ),
        ],
        [`Owner: ${family.owner}`],
      ),
      h.p(
        [Ui.className<Message>('mt-2 max-w-[76ch] text-base/7 text-white/60')],
        [family.purpose],
      ),
      h.div(
        [Ui.className<Message>('mt-3 grid gap-3 md:grid-cols-2')],
        [
          metaList<Message>('Use when', family.useWhen),
          metaList<Message>('Avoid when', family.avoidWhen),
          metaList<Message>('Accessibility', family.accessibility),
          metaList<Message>('Tokens', family.tokens),
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-3')],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/40',
              ),
            ],
            ['Exports'],
          ),
          h.div(
            [Ui.className<Message>('mt-1 flex flex-wrap gap-1.5')],
            Array.map(family.exports, name =>
              h.code(
                [
                  Ui.className<Message>(
                    'border border-[#333] px-1.5 py-0.5 font-mono text-[0.75rem] text-white/55',
                  ),
                ],
                [name],
              ),
            ),
          ),
        ],
      ),
    ],
  )
}

const familyCard = <Message>(family: FamilyMeta): Html => {
  const h = html<Message>()

  const showcase = familyShowcase<Message>(family.id)

  return h.section(
    [
      h.Id(`family-${family.id}`),
      Ui.className<Message>(
        'scroll-mt-6 border border-[#222] bg-white/[0.02] p-4 sm:p-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-baseline justify-between gap-2',
          ),
        ],
        [
          sectionHeading<Message>(family.title),
          h.code(
            [Ui.className<Message>('font-mono text-[0.75rem] text-white/40')],
            [family.module],
          ),
        ],
      ),
      // Lead with the live, rendered components (the primary content).
      ...(showcase.length > 0
        ? [h.div([Ui.className<Message>('mt-4')], [showcaseGrid<Message>(showcase)])]
        : []),
      // Contract metadata as a secondary section below the rendered thing.
      familyContractView<Message>(family),
    ],
  )
}

const aiElementsCard = <Message>(): Html => {
  const h = html<Message>()

  if (aiElementsExported) {
    const showcase = aiElementsShowcase<Message>()

    return h.section(
      [
        h.Id('family-ai-elements'),
        Ui.className<Message>(
          'scroll-mt-6 border border-[#222] bg-white/[0.02] p-4 sm:p-5',
        ),
      ],
      [
        h.div(
          [
            Ui.className<Message>(
              'flex flex-wrap items-baseline justify-between gap-2',
            ),
          ],
          [
            sectionHeading<Message>('AI Elements'),
            h.code(
              [Ui.className<Message>('font-mono text-[0.75rem] text-white/40')],
              ['@openagentsinc/ui/ai-elements'],
            ),
          ],
        ),
        // Lead with the live, rendered AI elements.
        ...(showcase.length > 0
          ? [h.div([Ui.className<Message>('mt-4')], [showcaseGrid<Message>(showcase)])]
          : []),
        // Contract metadata as a secondary section.
        h.div(
          [Ui.className<Message>('mt-5 border-t border-[#1a1a1a] pt-4')],
          [
            h.p(
              [
                Ui.className<Message>(
                  'm-0 font-mono text-[0.75rem] uppercase tracking-[0.08em] text-white/40',
                ),
              ],
              ['Contract'],
            ),
            h.p(
              [
                Ui.className<Message>(
                  'mt-2 font-mono text-[0.75rem] text-white/45',
                ),
              ],
              ['Owner: UI system'],
            ),
            h.p(
              [
                Ui.className<Message>(
                  'mt-2 max-w-[76ch] text-base/7 text-white/60',
                ),
              ],
              [
                'AI-native element family: agent, prompt-input, message, conversation, response, reasoning, tool/agent status, sources, code-block-with-run, confirmation, and web-preview. Live in @openagentsinc/ui via issue #5083.',
              ],
            ),
            h.div(
              [Ui.className<Message>('mt-3 grid gap-3 md:grid-cols-2')],
              [
                metaList<Message>('Use when', [
                  'rendering agent prompt input, messages, or responses',
                  'showing reasoning, tool status, sources, or a runnable code block',
                ]),
                metaList<Message>('Avoid when', [
                  'a plain text/forms control already covers the need',
                ]),
                metaList<Message>('Accessibility', [
                  'Prompt inputs and messages keep real input/region semantics.',
                ]),
                metaList<Message>('Tokens', ['surfaceClass', 'toneTextClass']),
              ],
            ),
          ],
        ),
      ],
    )
  }

  return h.section(
    [
      h.Id('family-ai-elements'),
      Ui.className<Message>(
        'scroll-mt-6 border border-dashed border-[#333] bg-white/[0.01] p-4 sm:p-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-baseline justify-between gap-2',
          ),
        ],
        [
          sectionHeading<Message>('AI Elements'),
          Ui.badge<Message>({ label: 'coming with #5083', tone: 'warning' }),
        ],
      ),
      h.p(
        [Ui.className<Message>('mt-2 max-w-[76ch] text-base/7 text-white/55')],
        [
          'The AI-native element family (agent, prompt-input, message, conversation, response, reasoning, tool/agent status, sources, code-block-with-run, confirmation, web-preview) is not yet exported by @openagentsinc/ui. It lands with issue #5083. This placeholder keeps the gallery building regardless of that work’s timing; once the family exports, this section renders its contract automatically.',
        ],
      ),
    ],
  )
}
