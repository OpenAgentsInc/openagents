import {
  BehaviorContractSchemaVersion,
  type BehaviorContract,
  type BehaviorContractRegistryDocument,
  validateBehaviorContractRegistry,
} from '@openagentsinc/behavior-contracts'

import { BUSINESS_OUTREACH_GATED_CLAIM_DENYLIST } from './business-outreach'

export const SITES_TANSTACK_RULES_REF =
  'sites_tanstack_rules.tanstack_start.v1.2026_07_04'
export const SITES_TANSTACK_RULES_VERSION = '2026-07-04.1'
export const SITES_TANSTACK_RULES_DOC_PATH =
  'docs/fable/2026-07-04-ts-5-sites-tanstack-rules-and-contracts.md'
export const SITES_TANSTACK_RULES_METADATA_KEY = 'sitesTanstackRules'

export type SitesTanstackRuleCategory =
  | 'routing'
  | 'server_functions'
  | 'data_auth'
  | 'rendering_modes'
  | 'tokens_design'
  | 'agent_surfaces'
  | 'workers_for_platforms'
  | 'qa_contracts'

export type SitesTanstackRule = Readonly<{
  category: SitesTanstackRuleCategory
  id: string
  instruction: string
  sourceRefs: ReadonlyArray<string>
}>

export type SitesTanstackRulesFeedbackLedgerRow = Readonly<{
  addedAt: string
  feedbackRef: string
  ruleId: string
  summary: string
}>

export type SitesTanstackRulesDocument = Readonly<{
  docPath: string
  feedbackLedger: ReadonlyArray<SitesTanstackRulesFeedbackLedgerRow>
  ref: string
  rules: ReadonlyArray<SitesTanstackRule>
  sourceRefs: ReadonlyArray<string>
  version: string
}>

export const SITES_TANSTACK_RULES: SitesTanstackRulesDocument = {
  docPath: SITES_TANSTACK_RULES_DOC_PATH,
  feedbackLedger: [
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.wrangler_jsonc_main_misclassified',
      ruleId: 'sites_tanstack.routing.file_routes_and_worker_main',
      summary:
        'Start projects must keep route files under src/routes and declare the Worker main in wrangler.jsonc so preview classification reads the SSR entry correctly.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.server_fn_mixed_with_client_copy',
      ruleId: 'sites_tanstack.server_functions.boundary',
      summary:
        'Generated pages keep customer copy in route loaders and createServerFn handlers; client components render data and do not own deployment or receipt state.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.binding_policy_not_explicit',
      ruleId: 'sites_tanstack.data_auth.bindings_only',
      summary:
        'Per-site values that vary by deploy target are Worker bindings, not bundled source constants.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.ssr_default_needed_for_agents',
      ruleId: 'sites_tanstack.rendering.ssr_first',
      summary:
        'Landing pages render useful HTML on first response and use client interactivity only where the page actually needs it.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.tokens_missing_from_template',
      ruleId: 'sites_tanstack.design.openagents_tokens',
      summary:
        'Generated Start sites import the OpenAgents React token CSS and stay dark-only unless a later signed design brief says otherwise.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.agent_surfaces_manual_retrofit',
      ruleId: 'sites_tanstack.agent_surfaces.day_one',
      summary:
        'The template owns robots.txt, sitemap.xml, llms.txt, JSON-LD, and /.well-known/openagents.json from the first generated version.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts4.failure.start_site.deploy_gate_namespace',
      ruleId: 'sites_tanstack.wfp.per_site_worker',
      summary:
        'Every generated site gets its own WfP script name and never targets the live openagents.com Worker.',
    },
    {
      addedAt: '2026-07-04T00:00:00.000Z',
      feedbackRef: 'ts5.failure.generated_site_contracts_missing',
      ruleId: 'sites_tanstack.qa.contracts_before_deploy_review',
      summary:
        'A generated site is not review-ready until the starter behavior-contract sweep passes against the preview artifact.',
    },
  ],
  ref: SITES_TANSTACK_RULES_REF,
  rules: [
    {
      category: 'routing',
      id: 'sites_tanstack.routing.file_routes_and_worker_main',
      instruction:
        'Use TanStack file routes under src/routes, keep src/server.ts as the Worker entry, and keep wrangler.jsonc main pointed at that entry.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'docs/fable/2026-07-04-tanstack-start-sites-and-web-app-evaluation.md#3',
      ],
    },
    {
      category: 'server_functions',
      id: 'sites_tanstack.server_functions.boundary',
      instruction:
        'Use createServerFn for server-side data and bounded copy handoff; client components render the result and never own deploy, receipt, or billing authority.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'apps/openagents.com/workers/api/src/sites-start-template.ts',
      ],
    },
    {
      category: 'data_auth',
      id: 'sites_tanstack.data_auth.bindings_only',
      instruction:
        'Treat deploy-target configuration as Worker bindings and metadata refs; generated source must not embed account material, contact data, or mutable operator decisions.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'apps/openagents.com/INVARIANTS.md',
      ],
    },
    {
      category: 'rendering_modes',
      id: 'sites_tanstack.rendering.ssr_first',
      instruction:
        'Default to SSR for route content, prerender only static agent surfaces, and add client hydration only for real interactions.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'docs/fable/2026-07-04-tanstack-start-sites-and-web-app-evaluation.md#3',
      ],
    },
    {
      category: 'tokens_design',
      id: 'sites_tanstack.design.openagents_tokens',
      instruction:
        'Import @openagentsinc/ui/react.css, use Tailwind 4 utilities over one-off CSS, and stay on the OpenAgents dark token family.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'apps/openagents.com/DESIGN.md',
      ],
    },
    {
      category: 'agent_surfaces',
      id: 'sites_tanstack.agent_surfaces.day_one',
      instruction:
        'Ship robots.txt, sitemap.xml, llms.txt, JSON-LD, and /.well-known/openagents.json in the generated site before review.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'apps/openagents.com/workers/api/src/sites-start-template.ts',
      ],
    },
    {
      category: 'workers_for_platforms',
      id: 'sites_tanstack.wfp.per_site_worker',
      instruction:
        'Build to a per-site WfP Worker module with a distinct script name; generated sites never deploy through the main openagents.com Worker.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'apps/openagents.com/workers/api/src/sites-start-template.ts',
      ],
    },
    {
      category: 'qa_contracts',
      id: 'sites_tanstack.qa.contracts_before_deploy_review',
      instruction:
        'Register starter behavior contracts for the generated site and block deploy review until the preview sweep passes.',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8347',
        'docs/fable/2026-07-03-behavior-contracts-and-customer-invariants.md#2',
      ],
    },
  ],
  sourceRefs: [
    'github:OpenAgentsInc/openagents#8347',
    'github:OpenAgentsInc/openagents#8339',
    'docs/fable/2026-07-04-tanstack-start-sites-and-web-app-evaluation.md',
  ],
  version: SITES_TANSTACK_RULES_VERSION,
}

export type SitesTanstackRulesSessionMetadata = Readonly<{
  docPath: string
  feedbackRefs: ReadonlyArray<string>
  injectedAt: string
  ref: string
  ruleRefs: ReadonlyArray<string>
  sessionBrief: ReadonlyArray<string>
  version: string
}>

export const sitesTanstackRulesSessionMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> => ({
  ...(metadata ?? {}),
  [SITES_TANSTACK_RULES_METADATA_KEY]: {
    docPath: SITES_TANSTACK_RULES.docPath,
    feedbackRefs: SITES_TANSTACK_RULES.feedbackLedger.map(row => row.feedbackRef),
    injectedAt: '2026-07-04T00:00:00.000Z',
    ref: SITES_TANSTACK_RULES.ref,
    ruleRefs: SITES_TANSTACK_RULES.rules.map(rule => rule.id),
    sessionBrief: SITES_TANSTACK_RULES.rules.map(rule => rule.instruction),
    version: SITES_TANSTACK_RULES.version,
  } satisfies SitesTanstackRulesSessionMetadata,
})

export type GeneratedSiteContractFile = Readonly<{
  byteSize?: number | undefined
  path: string
  text: string
}>

export type GeneratedSiteBehaviorContractRegistrationInput = Readonly<{
  previewUrl?: string | undefined
  siteId: string
}>

export type GeneratedSiteBehaviorContractSweepInput =
  GeneratedSiteBehaviorContractRegistrationInput &
    Readonly<{
      bundleBudgetBytes?: number | undefined
      checkedAt?: string | undefined
      files: ReadonlyArray<GeneratedSiteContractFile>
      knownRoutes?: ReadonlyArray<string> | undefined
      marketingCopy?: string | undefined
    }>

export type GeneratedSiteBehaviorContractFailure = Readonly<{
  blockerRef: string
  contractId: string
  detail: string
  evidenceRefs: ReadonlyArray<string>
}>

export type GeneratedSiteBehaviorContractResult = Readonly<{
  contractId: string
  evidenceRefs: ReadonlyArray<string>
  failures: ReadonlyArray<GeneratedSiteBehaviorContractFailure>
  status: 'pass' | 'fail'
  summary: string
}>

export type GeneratedSiteBehaviorContractSweepReceipt = Readonly<{
  blockerRefs: ReadonlyArray<string>
  checkedAt: string
  readyForDeployReview: boolean
  registry: BehaviorContractRegistryDocument
  registryValid: boolean
  results: ReadonlyArray<GeneratedSiteBehaviorContractResult>
  siteId: string
  status: 'pass' | 'fail'
}>

const starterContractSource = {
  channel: 'issue',
  statedBy: 'owner',
  statedOn: '2026-07-04',
} as const

const contractEvidenceRefs = (
  input: GeneratedSiteBehaviorContractRegistrationInput,
): ReadonlyArray<string> => [
  'github:OpenAgentsInc/openagents#8347',
  'docs/fable/2026-07-03-behavior-contracts-and-customer-invariants.md#2',
  SITES_TANSTACK_RULES_DOC_PATH,
  'apps/openagents.com/workers/api/src/sites-tanstack-rules.test.ts',
  `site:${input.siteId}`,
  ...(input.previewUrl === undefined ? [] : [`preview:${input.previewUrl}`]),
]

const starterContract = (
  input: GeneratedSiteBehaviorContractRegistrationInput,
  contract: Omit<
    BehaviorContract,
    | 'blockerRefs'
    | 'enforcementTier'
    | 'evidenceRefs'
    | 'oracles'
    | 'productArea'
    | 'source'
    | 'state'
    | 'surface'
  > &
    Readonly<{ oracleId: string }>,
): BehaviorContract => ({
  authorityBoundary:
    'This contract blocks generated-site deploy review only. It grants no live deploy, customer-result, public-claim, spend, payout, or settlement authority.',
  blockerRefs: [],
  contractId: contract.contractId,
  enforcementTier: 'test-sweep',
  evidenceRefs: contractEvidenceRefs(input),
  oracles: [
    {
      description: contract.verification,
      id: contract.oracleId,
      kind: 'bun-test',
      mode: 'unit',
      ref: 'apps/openagents.com/workers/api/src/sites-tanstack-rules.test.ts',
    },
  ],
  productArea: 'Autopilot Sites generated site QA',
  source: starterContractSource,
  state: 'enforced',
  statement: contract.statement,
  surface: 'autopilot-sites-generated-start-site',
  verification: contract.verification,
})

export const generatedSiteBehaviorContractRegistry = (
  input: GeneratedSiteBehaviorContractRegistrationInput,
): BehaviorContractRegistryDocument => ({
  contracts: [
    starterContract(input, {
      contractId: 'autopilot_sites.generated.dead_controls.v1',
      oracleId: 'autopilot_sites.generated.dead_controls.sweep',
      statement:
        'Every visible generated-site control must have an observable action; disabled, handlerless, or href-less controls block deploy review.',
      verification:
        'The generated-site preview sweep inspects route source for disabled controls, buttons without a form/action handler, and anchors without hrefs.',
    }),
    starterContract(input, {
      contractId: 'autopilot_sites.generated.navigation_integrity.v1',
      oracleId: 'autopilot_sites.generated.navigation_integrity.sweep',
      statement:
        'Generated-site navigation must not point visitors or agents at broken first-party paths.',
      verification:
        'The generated-site preview sweep extracts hrefs and fails first-party absolute paths that are not in the known route or agent-surface set.',
    }),
    starterContract(input, {
      contractId: 'autopilot_sites.generated.claim_safety.v1',
      oracleId: 'autopilot_sites.generated.claim_safety.sweep',
      statement:
        'Generated marketing copy must pass the LG-4 gated-claim denylist before deploy review.',
      verification:
        'The generated-site preview sweep runs the same gated-claim denylist categories as LG-4 against route text and provided marketing copy.',
    }),
    starterContract(input, {
      contractId: 'autopilot_sites.generated.bundle_budget.v1',
      oracleId: 'autopilot_sites.generated.bundle_budget.sweep',
      statement:
        'Generated sites must stay inside the configured bundle-size budget before deploy review.',
      verification:
        'The generated-site preview sweep totals generated file bytes and fails closed when the configured bundle budget is exceeded.',
    }),
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: '2026-07-04.1',
})

export const validateGeneratedSiteBehaviorContractRegistry = (
  input: GeneratedSiteBehaviorContractRegistrationInput,
) => validateBehaviorContractRegistry(generatedSiteBehaviorContractRegistry(input))

export const lintGeneratedSiteMarketingClaims = (
  text: string,
): ReadonlyArray<string> =>
  BUSINESS_OUTREACH_GATED_CLAIM_DENYLIST.filter(entry =>
    entry.pattern.test(text),
  ).map(entry => entry.claimRef)

const textByteSize = (text: string): number => new TextEncoder().encode(text).length

const totalByteSize = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
): number =>
  files.reduce(
    (total, file) => total + (file.byteSize ?? textByteSize(file.text)),
    0,
  )

const textEvidenceRef = (file: GeneratedSiteContractFile): string =>
  `generated_file:${file.path}`

const fileTexts = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
): string => files.map(file => file.text).join('\n')

const deadControlFailures = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
): ReadonlyArray<GeneratedSiteBehaviorContractFailure> =>
  files.flatMap(file => {
    const disabledControls = [...file.text.matchAll(/<(button|input|select|textarea)\b[^>]*\bdisabled\b[^>]*>/gi)]
    const inertButtons = [...file.text.matchAll(/<button\b([^>]*)>/gi)].filter(
      match => {
        const attributes = match[1] ?? ''
        return (
          !/\bdisabled\b/i.test(attributes) &&
          !/\bon[A-Z][A-Za-z]+\s*=/i.test(attributes) &&
          !/\b(formAction|form)\s*=/i.test(attributes) &&
          !/\btype\s*=\s*["']submit["']/i.test(attributes)
        )
      },
    )
    const hreflessAnchors = [...file.text.matchAll(/<a\b([^>]*)>/gi)].filter(
      match => !/\bhref\s*=/i.test(match[1] ?? ''),
    )
    const findingCount =
      disabledControls.length + inertButtons.length + hreflessAnchors.length

    return findingCount === 0
      ? []
      : [
          {
            blockerRef: 'blocker.sites.generated.dead_controls',
            contractId: 'autopilot_sites.generated.dead_controls.v1',
            detail: `${file.path} has ${findingCount} inert interactive control(s).`,
            evidenceRefs: [textEvidenceRef(file)],
          },
        ]
  })

const hrefPattern = /\bhref\s*=\s*(?:"([^"]*)"|'([^']*)')/gi

const allowedAgentRoutes = [
  '/',
  '/robots.txt',
  '/sitemap.xml',
  '/llms.txt',
  '/.well-known/openagents.json',
  '/.well-known/ai-catalog.json',
]

const normalizeRoute = (href: string): string => {
  const url = new URL(href, 'https://generated-site.openagents.local')
  const path = url.pathname === '' ? '/' : url.pathname
  return path.endsWith('/') && path !== '/' ? path.slice(0, -1) : path
}

const isFirstPartyHref = (href: string): boolean =>
  href.startsWith('/') || href.startsWith('https://generated-site.openagents.local')

const navigationFailures = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
  knownRoutes: ReadonlyArray<string>,
): ReadonlyArray<GeneratedSiteBehaviorContractFailure> => {
  const allowedRoutes = new Set(
    [...allowedAgentRoutes, ...knownRoutes].map(route => normalizeRoute(route)),
  )

  return files.flatMap(file => {
    const broken = [...file.text.matchAll(hrefPattern)]
      .map(match => match[1] ?? match[2] ?? '')
      .filter(href => href.trim() !== '')
      .filter(href => isFirstPartyHref(href))
      .map(href => ({ href, route: normalizeRoute(href) }))
      .filter(link => !allowedRoutes.has(link.route))

    return broken.length === 0
      ? []
      : [
          {
            blockerRef: 'blocker.sites.generated.navigation_integrity',
            contractId: 'autopilot_sites.generated.navigation_integrity.v1',
            detail: `${file.path} links to unknown first-party route(s): ${broken.map(link => link.href).join(', ')}.`,
            evidenceRefs: [textEvidenceRef(file)],
          },
        ]
  })
}

const claimFailures = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
  marketingCopy: string | undefined,
): ReadonlyArray<GeneratedSiteBehaviorContractFailure> => {
  const text = [fileTexts(files), marketingCopy ?? ''].join('\n')
  const lintRefs = lintGeneratedSiteMarketingClaims(text)

  return lintRefs.length === 0
    ? []
    : [
        {
          blockerRef: 'blocker.sites.generated.claim_safety',
          contractId: 'autopilot_sites.generated.claim_safety.v1',
          detail: `Generated marketing copy tripped gated claim lint: ${lintRefs.join(', ')}.`,
          evidenceRefs: lintRefs,
        },
      ]
}

const bundleBudgetFailures = (
  files: ReadonlyArray<GeneratedSiteContractFile>,
  budgetBytes: number,
): ReadonlyArray<GeneratedSiteBehaviorContractFailure> => {
  const actualBytes = totalByteSize(files)

  return actualBytes <= budgetBytes
    ? []
    : [
        {
          blockerRef: 'blocker.sites.generated.bundle_budget',
          contractId: 'autopilot_sites.generated.bundle_budget.v1',
          detail: `Generated source is ${actualBytes} bytes, above the ${budgetBytes} byte budget.`,
          evidenceRefs: ['budget.sites.generated.bundle_bytes'],
        },
      ]
}

const resultFor = (
  contractId: string,
  failures: ReadonlyArray<GeneratedSiteBehaviorContractFailure>,
): GeneratedSiteBehaviorContractResult => ({
  contractId,
  evidenceRefs: [...new Set(failures.flatMap(failure => failure.evidenceRefs))],
  failures,
  status: failures.length === 0 ? 'pass' : 'fail',
  summary:
    failures.length === 0
      ? 'Generated-site contract passed.'
      : failures.map(failure => failure.detail).join(' '),
})

export const runGeneratedSiteBehaviorContractSweep = (
  input: GeneratedSiteBehaviorContractSweepInput,
): GeneratedSiteBehaviorContractSweepReceipt => {
  const registry = generatedSiteBehaviorContractRegistry(input)
  const registryValidation = validateBehaviorContractRegistry(registry)
  const deadControls = deadControlFailures(input.files)
  const navigation = navigationFailures(input.files, input.knownRoutes ?? ['/'])
  const claims = claimFailures(input.files, input.marketingCopy)
  const bundle = bundleBudgetFailures(
    input.files,
    input.bundleBudgetBytes ?? 250_000,
  )
  const results = [
    resultFor('autopilot_sites.generated.dead_controls.v1', deadControls),
    resultFor('autopilot_sites.generated.navigation_integrity.v1', navigation),
    resultFor('autopilot_sites.generated.claim_safety.v1', claims),
    resultFor('autopilot_sites.generated.bundle_budget.v1', bundle),
  ]
  const blockerRefs = [
    ...registryValidation.issues.map(
      issue =>
        `blocker.sites.generated.registry.${issue.kind}.${issue.contractId ?? 'document'}`,
    ),
    ...results.flatMap(result =>
      result.failures.map(failure => failure.blockerRef),
    ),
  ]
  const uniqueBlockerRefs = [...new Set(blockerRefs)].sort()

  return {
    blockerRefs: uniqueBlockerRefs,
    checkedAt: input.checkedAt ?? '2026-07-04T00:00:00.000Z',
    readyForDeployReview: uniqueBlockerRefs.length === 0,
    registry,
    registryValid: registryValidation.ok,
    results,
    siteId: input.siteId,
    status: uniqueBlockerRefs.length === 0 ? 'pass' : 'fail',
  }
}
