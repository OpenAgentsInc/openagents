type ComponentFamily = Readonly<{
  id: string
  title: string
  module: string
  purpose: string
  exports: ReadonlyArray<string>
  contract: ReadonlyArray<string>
}>

const families: ReadonlyArray<ComponentFamily> = [
  {
    id: 'primitives',
    title: 'Primitives',
    module: '@openagentsinc/ui/primitives',
    purpose: 'Core surface, tone, status, button, link, and kit-family tokens.',
    exports: ['surfaceClass', 'toneTextClass', 'statusDotClass', 'buttonClass'],
    contract: ['Focus tokens visible', 'No raw color literals'],
  },
  {
    id: 'shared',
    title: 'Shared',
    module: '@openagentsinc/ui/shared',
    purpose: 'Buttons, link buttons, avatars, menu rows, and heading blocks.',
    exports: ['button', 'linkButton', 'avatar', 'headingBlock'],
    contract: ['Real button/link semantics', 'Compact mono dark shell'],
  },
  {
    id: 'forms',
    title: 'Forms',
    module: '@openagentsinc/ui/forms',
    purpose: 'Labeled inputs, textarea groups, select menus, and validation rows.',
    exports: ['inputGroup', 'validatedInputGroup', 'textareaGroup', 'selectMenu'],
    contract: ['Visible labels', 'Stable id/name pairs'],
  },
  {
    id: 'layout',
    title: 'Layout',
    module: '@openagentsinc/ui/layout',
    purpose: 'Page shells, sections, cards, drawers, dialogs, and notifications.',
    exports: ['pageShell', 'section', 'card', 'drawerPanel'],
    contract: ['No nested cards', 'Dialog labels present'],
  },
  {
    id: 'navigation',
    title: 'Navigation',
    module: '@openagentsinc/ui/navigation',
    purpose: 'Tabs, breadcrumbs, sidebars, progress lists, and command palettes.',
    exports: ['tabBar', 'breadcrumbBar', 'sidebarNavigation', 'progressList'],
    contract: ['aria-current for active items', 'Mobile-safe overflow'],
  },
  {
    id: 'data-display',
    title: 'Data display',
    module: '@openagentsinc/ui/data-display',
    purpose: 'Dense operational records: tables, stats, code blocks, and feeds.',
    exports: ['tableList', 'keyValueRows', 'codeBlock', 'statGrid'],
    contract: ['Contract', 'Captions and headers for tables'],
  },
  {
    id: 'feedback',
    title: 'Feedback',
    module: '@openagentsinc/ui/feedback',
    purpose: 'Tone-bearing alerts and empty states with explicit next actions.',
    exports: ['alert', 'emptyState'],
    contract: ['Tone conveyed by text', 'No color-only states'],
  },
  {
    id: 'workroom',
    title: 'Workroom',
    module: '@openagentsinc/ui/workroom',
    purpose: 'Accepted-outcome, evidence, timeline, and review surfaces.',
    exports: ['workroomTimeline', 'evidenceDrawer', 'acceptancePanel'],
    contract: ['Public-safe refs only', 'No raw private patches'],
  },
  {
    id: 'public',
    title: 'Public',
    module: '@openagentsinc/ui/public',
    purpose: 'Marketing and public proof sections for the dark OpenAgents site.',
    exports: ['marketingHero', 'proofStrip', 'agentReadableCallout'],
    contract: ['Registry-bound claims', 'Dark-only StarCraft tokens'],
  },
  {
    id: 'public-theme',
    title: 'Public theme',
    module: '@openagentsinc/ui/public-theme',
    purpose: 'Shell-scoped theme previews for public landing pages.',
    exports: [
      'publicLandingThemeSelector',
      'publicLandingThemeShell mode=light',
      'publicLandingThemeShell mode=dark',
    ],
    contract: ['Shell-scoped theme', 'data-public-landing-shell'],
  },
  {
    id: 'business',
    title: 'Business landing',
    module: '@openagentsinc/ui/business',
    purpose: 'Business funnel cards, offering menus, rate cards, and intake forms.',
    exports: [
      'businessOfferingMenu mode=light',
      'businessIntakeForm',
      'businessPackageGrid',
    ],
    contract: ['data-ui-family business/* markers', 'No self-serve overclaim'],
  },
  {
    id: 'page-examples',
    title: 'Page examples',
    module: '@openagentsinc/ui/page-examples',
    purpose: 'Full-page layout examples that keep app-shell density visible.',
    exports: ['applicationHomeScreen', 'settingsScreen', 'workroomScreen'],
    contract: ['No decorative landing shell for tools', 'Responsive panels'],
  },
  {
    id: 'v4',
    title: 'V4',
    module: '@openagentsinc/ui/v4',
    purpose: 'Composer and chat primitives used by modern Khala surfaces.',
    exports: ['v4Composer', 'v4MessageList', 'v4ToolCallRow'],
    contract: ['No raw prompts in public receipts', 'Keyboard focus visible'],
  },
  {
    id: 'ai-elements',
    title: 'AI Elements',
    module: '@openagentsinc/ui/ai-elements',
    purpose: 'Prompt input, model picker, reasoning controls, and generated UI atoms.',
    exports: ['AiElements.promptInput', 'AiElements.modelPicker', 'AiElements.reasoningControl'],
    contract: ['Typed catalog exports', 'No provider key rendering'],
  },
  {
    id: 'live-samples',
    title: 'Live samples',
    module: '@openagentsinc/ui/live-samples',
    purpose: 'Representative component combinations used as route smoke anchors.',
    exports: ['inputGroup', 'tableList', 'marketingHero', 'AiElements.promptInput'],
    contract: ['Render components, not just prose.', 'Route smoke friendly'],
  },
  {
    id: 'training',
    title: 'Training grammar',
    module: 'oa-training-run / @openagentsinc/three-effect',
    purpose: 'Training-run visual grammar references for replay and verification.',
    exports: [
      'Run field',
      'Contributor node',
      'Replay pair',
      'Verification gate',
      'Receipt burst',
      'Proof drawer',
      'oa-training-grammar-replay-pair',
    ],
    contract: ['three-effect owns visuals', 'No app-local replay renderer'],
  },
]

const familyById = new Map(families.map(family => [family.id, family]))

const panelClass =
  'grid gap-4 border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

function FamilyCard({ family }: Readonly<{ family: ComponentFamily }>) {
  return (
    <article className={panelClass} data-component-family={family.id}>
      <div className="grid gap-2">
        <p className={eyebrowClass}>{family.module}</p>
        <h2 className="m-0 text-balance text-2xl font-semibold tracking-tight text-white">
          {family.title}
        </h2>
        <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          {family.purpose}
        </p>
      </div>
      <div className="grid gap-2">
        <p className={eyebrowClass}>Exports</p>
        <ul className="grid gap-1 font-mono text-base text-khala-text sm:text-sm" role="list">
          {family.exports.map(exportName => (
            <li key={exportName}>{exportName}</li>
          ))}
        </ul>
      </div>
      <div className="grid gap-2">
        <p className={eyebrowClass}>Contract</p>
        <ul className="grid gap-1 text-base/7 text-khala-text-muted sm:text-sm/6" role="list">
          {family.contract.map(item => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
      <a
        className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
        href={`/components/${family.id}`}
      >
        Open family
      </a>
    </article>
  )
}

function FamilyDetail({ family }: Readonly<{ family: ComponentFamily }>) {
  return (
    <section className="grid gap-5" data-component-family-detail={family.id}>
      <div className="grid gap-2">
        <p className={eyebrowClass}>{family.module}</p>
        <h2 className="m-0 text-balance text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          {family.title}
        </h2>
        <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted">
          {family.purpose}
        </p>
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <div className={panelClass}>
          <p className={eyebrowClass}>Live preview anchors</p>
          <ul className="grid gap-2 font-mono text-base text-white sm:text-sm" role="list">
            {family.exports.map(exportName => (
              <li key={exportName}>{exportName}</li>
            ))}
          </ul>
        </div>
        <div className={panelClass}>
          <p className={eyebrowClass}>Contract</p>
          <ul className="grid gap-2 text-base/7 text-khala-text-muted sm:text-sm/6" role="list">
            {family.contract.map(item => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  )
}

export function ComponentsPage({
  selectedFamily,
}: Readonly<{ selectedFamily?: string }>) {
  const family = selectedFamily === undefined ? undefined : familyById.get(selectedFamily)

  return (
    <main className="min-h-dvh bg-black text-white" data-route="components">
      <div className="mx-auto grid w-full max-w-7xl gap-8 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3">
          <a
            className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
            href="/"
          >
            OpenAgents
          </a>
          <p className={eyebrowClass}>Internal - design-system workbench</p>
          <h1 className="m-0 text-balance text-4xl font-semibold tracking-tight text-white sm:text-5xl">
            Component library
          </h1>
          <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted">
            A Start-rendered inventory of the OpenAgents UI families. This
            route keeps the old workbench's public contract visible while the
            real component registry moves route-by-route out of Foldkit.
          </p>
        </header>
        {family === undefined ? (
          <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            {families.map(item => (
              <FamilyCard family={item} key={item.id} />
            ))}
          </section>
        ) : (
          <FamilyDetail family={family} />
        )}
      </div>
    </main>
  )
}
