import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { docsRouter } from '../route'
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

const familyCard = <Message>(family: FamilyMeta): Html => {
  const h = html<Message>()

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

const aiElementsCard = <Message>(): Html => {
  const h = html<Message>()

  if (aiElementsExported) {
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
        h.p(
          [Ui.className<Message>('mt-2 font-mono text-[0.75rem] text-white/45')],
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
