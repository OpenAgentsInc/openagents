import { Effect, Schema as S } from 'effect'

import {
  PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
  publicPylonStatsFromRegistrations,
} from './public-pylon-stats'
import type {
  AgentRegistrationStore,
  LinkedAgentOwnerRecord,
} from './agent-registration'
import type {
  PylonApiRegistrationRecord,
  PylonApiStore,
  PylonCodingServiceCapacityProjection,
} from './pylon-api'
import { pylonCodingServiceCapacityProjection } from './pylon-api'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

const MAX_CONTEXT_PYLONS = 12
const MAX_ANSWER_PYLONS = 8

export type KhalaChatPylonSummary = Readonly<{
  assignmentReadyNow: boolean
  capabilityRefs: ReadonlyArray<string>
  clientVersion: string | null
  codingCapacity: ReadonlyArray<PylonCodingServiceCapacityProjection>
  displayName: string
  latestHeartbeatAgeSeconds: number | null
  latestHeartbeatStatus: string | null
  onlineNow: boolean
  providerMarketRelayRefs: ReadonlyArray<string>
  providerNip90LaneRefs: ReadonlyArray<string>
  providerNostrPubkey: string | null
  pylonRef: string
  resourceMode: PylonApiRegistrationRecord['resourceMode']
  status: PylonApiRegistrationRecord['status']
  walletReady: boolean
  walletReadyNow: boolean
}>

export type KhalaChatPylonContext = Readonly<{
  asOfIso: string
  caveatRefs: ReadonlyArray<string>
  minimumClientVersion: string
  pylons: ReadonlyArray<KhalaChatPylonSummary>
  sourceRefs: ReadonlyArray<string>
  totals: Readonly<{
    activeRegistrations: number
    assignmentReadyNow: number
    onlineNow: number
    registryRowsRead: number
    seen24h: number
    statsRegisteredTotal: number
    walletReadyNow: number
  }>
}>

export type KhalaChatLinkedAgentSummary = Readonly<{
  agentUserId: string
  displayName: string
  linkKind: LinkedAgentOwnerRecord['linkKind']
}>

export type KhalaChatAccountPylonContext = Readonly<{
  asOfIso: string
  caveatRefs: ReadonlyArray<string>
  linkedAgents: ReadonlyArray<KhalaChatLinkedAgentSummary>
  pylons: ReadonlyArray<KhalaChatPylonSummary>
  sourceRefs: ReadonlyArray<string>
  totals: Readonly<{
    assignmentReadyNow: number
    linkedAgents: number
    linkedPylons: number
    onlineNow: number
    walletReadyNow: number
  }>
}>

export type KhalaChatPylonContextEnvelope = Readonly<{
  accountContext?: KhalaChatAccountPylonContext | undefined
  mode: 'anonymous_public' | 'authenticated_account'
  publicContext?: KhalaChatPylonContext | undefined
}>

export type KhalaChatPylonContextStore = Pick<
  PylonApiStore,
  'listRegistrations'
>

export type KhalaChatAccountPylonContextStores = Readonly<{
  agentStore: Pick<AgentRegistrationStore, 'listLinkedAgentsForOpenAuthUser'>
  pylonStore: Pick<
    PylonApiStore,
    'listRegistrations' | 'listRegistrationsForOwnerAgentUserIds'
  >
}>

export type KhalaChatPylonQuestionKind =
  | 'capabilities'
  | 'interaction'
  | 'list_connected'
  | 'ownership'
  | 'register'

export class KhalaChatPylonContextError extends S.TaggedErrorClass<KhalaChatPylonContextError>()(
  'KhalaChatPylonContextError',
  {
    reason: S.String,
  },
) {}

const latestHeartbeatAgeSeconds = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
): number | null => {
  if (registration.latestHeartbeatAt === null) {
    return null
  }

  const heartbeatMs = Date.parse(registration.latestHeartbeatAt)
  if (!Number.isFinite(heartbeatMs)) {
    return null
  }

  return Math.max(0, Math.floor((nowUnixMs - heartbeatMs) / 1000))
}

const summarizeRegistration = (
  registration: PylonApiRegistrationRecord,
  nowUnixMs: number,
  recentByPylonRef: ReadonlyMap<
    string,
    {
      readonly assignmentReadyNow: boolean | null
      readonly onlineNow: boolean | null
      readonly walletReadyNow: boolean | null
    }
  >,
): KhalaChatPylonSummary => {
  const recent = recentByPylonRef.get(registration.pylonRef)
  const onlineNow = recent?.onlineNow === true

  return {
    assignmentReadyNow: recent?.assignmentReadyNow === true,
    capabilityRefs: registration.capabilityRefs,
    clientVersion: registration.clientVersion,
    codingCapacity: pylonCodingServiceCapacityProjection(registration),
    displayName: registration.displayName,
    latestHeartbeatAgeSeconds: latestHeartbeatAgeSeconds(
      registration,
      nowUnixMs,
    ),
    latestHeartbeatStatus: registration.latestHeartbeatStatus,
    onlineNow,
    providerMarketRelayRefs: registration.providerMarketRelayRefs,
    providerNip90LaneRefs: registration.providerNip90LaneRefs,
    providerNostrPubkey: registration.providerNostrPubkey,
    pylonRef: registration.pylonRef,
    resourceMode: registration.latestResourceMode ?? registration.resourceMode,
    status: registration.status,
    walletReady: registration.walletReady,
    walletReadyNow: recent?.walletReadyNow === true,
  }
}

export const loadKhalaChatPylonContext = (
  store: KhalaChatPylonContextStore,
  nowUnixMs: number = currentEpochMillis(),
): Effect.Effect<KhalaChatPylonContext, KhalaChatPylonContextError> =>
  Effect.tryPromise({
    catch: error =>
      new KhalaChatPylonContextError({
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () => store.listRegistrations(100),
  }).pipe(
    Effect.map(registrations => {
      const stats = publicPylonStatsFromRegistrations(registrations, nowUnixMs)
      const recentByPylonRef = new Map(
        stats.recentPylons
          .filter(pylon => pylon.pylonRef !== null)
          .map(pylon => [
            pylon.pylonRef!,
            {
              assignmentReadyNow: pylon.assignmentReadyNow,
              onlineNow: pylon.onlineNow,
              walletReadyNow: pylon.walletReadyNow,
            },
          ]),
      )
      const pylons = registrations
        .map(registration =>
          summarizeRegistration(registration, nowUnixMs, recentByPylonRef),
        )
        .sort((left, right) => {
          if (left.onlineNow !== right.onlineNow) {
            return left.onlineNow ? -1 : 1
          }
          return (
            (left.latestHeartbeatAgeSeconds ?? Number.MAX_SAFE_INTEGER) -
            (right.latestHeartbeatAgeSeconds ?? Number.MAX_SAFE_INTEGER)
          )
        })
        .slice(0, MAX_CONTEXT_PYLONS)

      return {
        asOfIso: epochMillisToIsoTimestamp(nowUnixMs),
        caveatRefs: [
          'caveat.public.pylon_chat_reads_public_registry_projection_only',
          'caveat.public.pylon_online_is_not_paid_work',
          'caveat.public.assignment_ready_is_not_payout_evidence',
        ],
        minimumClientVersion: PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
        pylons,
        sourceRefs: [
          'route:/api/khala/chat',
          'route:/api/pylons',
          'route:/api/public/pylon-stats',
          'openagents.public.pylon_api.registrations',
        ],
        totals: {
          activeRegistrations: registrations.filter(
            registration => registration.status === 'active',
          ).length,
          assignmentReadyNow: stats.pylonsAssignmentReadyNow,
          onlineNow: stats.pylonsOnlineNow,
          registryRowsRead: registrations.length,
          seen24h: stats.pylonsSeen24h,
          statsRegisteredTotal: stats.pylonsRegisteredTotal,
          walletReadyNow: stats.pylonsWalletReadyNow,
        },
      }
    }),
  )

export const loadKhalaChatAccountPylonContext = (
  stores: KhalaChatAccountPylonContextStores,
  openauthUserId: string,
  nowUnixMs: number = currentEpochMillis(),
): Effect.Effect<KhalaChatAccountPylonContext, KhalaChatPylonContextError> =>
  Effect.gen(function* () {
    const linkedAgents = yield* Effect.tryPromise({
      catch: error =>
        new KhalaChatPylonContextError({
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        stores.agentStore.listLinkedAgentsForOpenAuthUser === undefined
          ? Promise.resolve([])
          : stores.agentStore.listLinkedAgentsForOpenAuthUser(
              openauthUserId,
              100,
            ),
    })

    const ownerIds = [...new Set(linkedAgents.map(agent => agent.agentUserId))]
    const registrations = yield* Effect.tryPromise({
      catch: error =>
        new KhalaChatPylonContextError({
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        stores.pylonStore.listRegistrationsForOwnerAgentUserIds === undefined
          ? stores.pylonStore
              .listRegistrations(200)
              .then(rows =>
                rows.filter(registration =>
                  ownerIds.includes(registration.ownerAgentUserId),
                ),
              )
          : stores.pylonStore.listRegistrationsForOwnerAgentUserIds(
              ownerIds,
              200,
            ),
    })

    const stats = publicPylonStatsFromRegistrations(registrations, nowUnixMs)
    const recentByPylonRef = new Map(
      stats.recentPylons
        .filter(pylon => pylon.pylonRef !== null)
        .map(pylon => [
          pylon.pylonRef!,
          {
            assignmentReadyNow: pylon.assignmentReadyNow,
            onlineNow: pylon.onlineNow,
            walletReadyNow: pylon.walletReadyNow,
          },
        ]),
    )
    const pylons = registrations
      .map(registration =>
        summarizeRegistration(registration, nowUnixMs, recentByPylonRef),
      )
      .sort((left, right) => {
        if (left.onlineNow !== right.onlineNow) {
          return left.onlineNow ? -1 : 1
        }
        return (
          (left.latestHeartbeatAgeSeconds ?? Number.MAX_SAFE_INTEGER) -
          (right.latestHeartbeatAgeSeconds ?? Number.MAX_SAFE_INTEGER)
        )
      })
      .slice(0, MAX_CONTEXT_PYLONS)

    return {
      asOfIso: epochMillisToIsoTimestamp(nowUnixMs),
      caveatRefs: [
        'caveat.account.pylon_chat_reads_authenticated_owner_links_only',
        'caveat.account.pylon_context_excludes_tokens_wallets_raw_traces',
        'caveat.public.assignment_ready_is_not_payout_evidence',
      ],
      linkedAgents: linkedAgents.slice(0, 20).map(agent => ({
        agentUserId: agent.agentUserId,
        displayName: agent.displayName,
        linkKind: agent.linkKind,
      })),
      pylons,
      sourceRefs: [
        'route:/api/khala/chat',
        'route:/api/account/pylons',
        'openagents.account.pylon_openauth_links',
      ],
      totals: {
        assignmentReadyNow: pylons.filter(pylon => pylon.assignmentReadyNow)
          .length,
        linkedAgents: linkedAgents.length,
        linkedPylons: registrations.length,
        onlineNow: pylons.filter(pylon => pylon.onlineNow).length,
        walletReadyNow: pylons.filter(pylon => pylon.walletReadyNow).length,
      },
    }
  })

const ageLabel = (seconds: number | null): string =>
  seconds === null
    ? 'no heartbeat'
    : seconds < 60
      ? `${seconds}s ago`
      : seconds < 3600
        ? `${Math.floor(seconds / 60)}m ago`
        : `${Math.floor(seconds / 3600)}h ago`

const commaList = (
  values: ReadonlyArray<string>,
  empty = 'none',
  limit = 4,
): string => {
  const unique = [...new Set(values.map(value => value.trim()).filter(Boolean))]
  if (unique.length === 0) {
    return empty
  }
  const shown = unique.slice(0, limit)
  const suffix =
    unique.length > shown.length
      ? `, +${unique.length - shown.length} more`
      : ''
  return `${shown.join(', ')}${suffix}`
}

const codingCapacityLabel = (
  codingCapacity: ReadonlyArray<PylonCodingServiceCapacityProjection>,
): string => {
  if (codingCapacity.length === 0) {
    return 'no coding slots advertised'
  }

  return codingCapacity
    .map(
      capacity =>
        `${capacity.service}: ${capacity.available}/${capacity.ready} available`,
    )
    .join('; ')
}

const pylonStatusLine = (pylon: KhalaChatPylonSummary): string =>
  `- ${pylon.pylonRef}${pylon.displayName === pylon.pylonRef ? '' : ` (${pylon.displayName})`}: ${pylon.onlineNow ? 'online' : 'not online now'}, heartbeat ${ageLabel(pylon.latestHeartbeatAgeSeconds)}, status ${pylon.latestHeartbeatStatus ?? 'unknown'}, version ${pylon.clientVersion ?? 'unknown'}, mode ${pylon.resourceMode}, wallet ${pylon.walletReadyNow ? 'ready' : pylon.walletReady ? 'reported ready, not online-now ready' : 'not ready'}, assignment ${pylon.assignmentReadyNow ? 'ready' : 'not ready'}`

const contextHeader = (context: KhalaChatPylonContext): string =>
  [
    `OpenAgents Pylon registry as of ${context.asOfIso}:`,
    `- ${context.totals.onlineNow} online now`,
    `- ${context.totals.seen24h} seen in 24h`,
    `- ${context.totals.statsRegisteredTotal} registered on the v${context.minimumClientVersion}+ public stats floor`,
    `- ${context.totals.activeRegistrations} active registration rows read`,
    `- ${context.totals.walletReadyNow} wallet-ready now`,
    `- ${context.totals.assignmentReadyNow} assignment-ready now`,
  ].join('\n')

const accountContextHeader = (context: KhalaChatAccountPylonContext): string =>
  [
    `Authenticated OpenAgents account Pylon context as of ${context.asOfIso}:`,
    `- ${context.totals.linkedAgents} linked agent${context.totals.linkedAgents === 1 ? '' : 's'}`,
    `- ${context.totals.linkedPylons} linked Pylon${context.totals.linkedPylons === 1 ? '' : 's'}`,
    `- ${context.totals.onlineNow} of your linked Pylons online now`,
    `- ${context.totals.walletReadyNow} wallet-ready now`,
    `- ${context.totals.assignmentReadyNow} assignment-ready now`,
  ].join('\n')

export const renderKhalaChatPylonContextForPrompt = (
  context: KhalaChatPylonContext,
): string =>
  [
    'OpenAgents Pylons are registered OpenAgents compute/agent nodes. They are not StarCraft or electrical-grid pylons.',
    'Use this public registry projection when answering Pylon questions. Do not invent private state. Do not claim public chat can mutate Pylons.',
    contextHeader(context),
    'Recent/online Pylon rows:',
    ...(context.pylons.length === 0
      ? ['- none returned by the public registry projection']
      : context.pylons.slice(0, MAX_CONTEXT_PYLONS).map(pylonStatusLine)),
    `Source refs: ${context.sourceRefs.join(', ')}`,
    `Caveat refs: ${context.caveatRefs.join(', ')}`,
  ].join('\n')

export const renderKhalaChatPylonContextEnvelopeForPrompt = (
  envelope: KhalaChatPylonContextEnvelope,
): string =>
  [
    ...(envelope.publicContext === undefined
      ? []
      : [renderKhalaChatPylonContextForPrompt(envelope.publicContext)]),
    ...(envelope.accountContext === undefined
      ? []
      : [
          [
            'The request has a verified OpenAuth browser session. Use the following account-owned Pylon context only for this authenticated request.',
            'Do not expose bearer tokens, token prefixes, wallet material, raw prompts, private traces, local paths, or another account owner link.',
            accountContextHeader(envelope.accountContext),
            'Linked agents:',
            ...(envelope.accountContext.linkedAgents.length === 0
              ? ['- none linked to this OpenAuth account']
              : envelope.accountContext.linkedAgents.map(
                  agent =>
                    `- ${agent.agentUserId} (${agent.displayName}); link ${agent.linkKind}`,
                )),
            'Your linked Pylon rows:',
            ...(envelope.accountContext.pylons.length === 0
              ? ['- none linked to this OpenAuth account']
              : envelope.accountContext.pylons
                  .slice(0, MAX_CONTEXT_PYLONS)
                  .map(pylonStatusLine)),
            `Source refs: ${envelope.accountContext.sourceRefs.join(', ')}`,
            `Caveat refs: ${envelope.accountContext.caveatRefs.join(', ')}`,
          ].join('\n'),
        ]),
  ].join('\n\n')

const normalizeQuestion = (input: string): string =>
  input
    .trim()
    .toLowerCase()
    .replace(/[?!.,;:]+/g, ' ')
    .replace(/\s+/g, ' ')

// Bounded parser for public Pylon operational questions after the request is
// already in Khala chat. This is not the product's general intent router.
export const classifyKhalaChatPylonQuestion = (
  content: string,
): KhalaChatPylonQuestionKind | null => {
  const question = normalizeQuestion(content)
  if (!/\bpylons?\b/.test(question)) {
    return null
  }

  if (
    /\b(my|mine|owned|owner|account|linked)\b/.test(question) ||
    /\b(use my|show my|which pylons are mine|are those mine|are these mine)\b/.test(
      question,
    )
  ) {
    return 'ownership'
  }

  if (
    /\b(register|connect|start|add|join|setup|set up|install)\b/.test(question)
  ) {
    return 'register'
  }

  if (
    /\b(operator|interact|assign|dispatch|send work|job|task|control|command|mutate)\b/.test(
      question,
    )
  ) {
    return 'interaction'
  }

  if (
    /\b(capabilities|capability|capacity|tools|can do|ready)\b/.test(question)
  ) {
    return 'capabilities'
  }

  if (
    /\b(what|which|list|show|see|how many|status|online|connected|registered|running|available)\b/.test(
      question,
    )
  ) {
    return 'list_connected'
  }

  return null
}

const connectedPylonAnswer = (
  context: KhalaChatPylonContextEnvelope | undefined,
): string => {
  if (context?.publicContext === undefined) {
    return [
      'I cannot read the live Pylon registry from this chat turn right now.',
      'The public read surfaces are GET /api/pylons and GET /api/public/pylon-stats.',
    ].join('\n\n')
  }

  const publicContext = context.publicContext
  const connected = publicContext.pylons.filter(pylon => pylon.onlineNow)
  const rows = connected.length === 0 ? publicContext.pylons : connected

  return [
    contextHeader(publicContext),
    '',
    connected.length === 0
      ? 'No Pylons are online in the public stats window right now. The most recent registry rows I can see are:'
      : 'Connected now:',
    ...(rows.length === 0
      ? ['- none']
      : rows.slice(0, MAX_ANSWER_PYLONS).map(pylonStatusLine)),
    '',
    'Source: public Pylon registration and heartbeat projection. Online/assignment-ready is not payment or settlement evidence.',
  ].join('\n')
}

const capabilityAnswer = (
  context: KhalaChatPylonContextEnvelope | undefined,
): string => {
  if (context?.publicContext === undefined) {
    return [
      'I cannot read live Pylon capabilities from this chat turn right now.',
      'Use GET /api/pylons to inspect public capabilityRefs, codingCapacity, NIP-90 lane refs, and heartbeat fields.',
    ].join('\n\n')
  }

  const publicContext = context.publicContext

  return [
    contextHeader(publicContext),
    '',
    'Advertised Pylon capabilities:',
    ...(publicContext.pylons.length === 0
      ? ['- none returned by the registry projection']
      : publicContext.pylons
          .slice(0, MAX_ANSWER_PYLONS)
          .map(pylon =>
            [
              `- ${pylon.pylonRef}: ${commaList(pylon.capabilityRefs)}`,
              `; coding capacity: ${codingCapacityLabel(pylon.codingCapacity)}`,
              pylon.providerNostrPubkey === null
                ? ''
                : `; provider pubkey: ${pylon.providerNostrPubkey}`,
              pylon.providerNip90LaneRefs.length === 0
                ? ''
                : `; lanes: ${commaList(pylon.providerNip90LaneRefs, 'none', 3)}`,
            ].join(''),
          )),
  ].join('\n')
}

const registerAnswer = (): string =>
  [
    'To connect a Pylon to OpenAgents:',
    '',
    '1. Create or use an OpenAgents agent token (`oa_agent_...`). Keep it private.',
    '2. Run the Pylon launcher with registration enabled, for example:',
    '',
    '```bash',
    'export OPENAGENTS_AGENT_TOKEN="oa_agent_..."',
    'npx @openagentsinc/pylon@latest --register-openagents --openagents-api https://openagents.com --resource-mode background_20 --json',
    '```',
    '',
    'Programmatic writes use `Authorization: Bearer <agent token>` plus `Idempotency-Key`:',
    '- `POST /api/pylons/register`',
    '- `POST /api/pylons/{pylonRef}/heartbeat`',
    '- `POST /api/pylons/{pylonRef}/wallet-readiness`',
    '',
    'Public reads are `GET /api/pylons`, `GET /api/pylons/{pylonRef}`, and `GET /api/public/pylon-stats`.',
  ].join('\n')

const interactionAnswer = (): string =>
  [
    'Pylon interaction is split by authority:',
    '',
    '- Public chat can read and explain public Pylon registry/status projections.',
    '- A Pylon owner agent can register, heartbeat, report wallet readiness, and report assignment progress with its bearer token.',
    '- Operator-only assignment tools are not available to anonymous web chat sessions.',
    '- Authenticated Khala coding delegation can route caller-owned coding workflows to linked local Codex/Claude-capable Pylons through the OpenAI-compatible API path.',
    '',
    'This public chat route does not dispatch paid work, approve payout targets, spend bitcoin, settle providers, or mutate Pylon state by itself.',
  ].join('\n')

const ownershipAnswer = (
  context: KhalaChatPylonContextEnvelope | undefined,
): string => {
  if (context?.accountContext === undefined) {
    if (context?.mode === 'authenticated_account') {
      return [
        'Your browser session is signed in, but I cannot read your account Pylon links for this chat turn right now.',
        'I will not infer ownership from the public registry. Try again, or use the account Pylon page/API for the owner-scoped view.',
      ].join('\n\n')
    }

    return [
      'I cannot verify Pylon ownership from the public registry alone.',
      'Sign in at /login?returnTo=/chat, then link an OpenAgents agent token through the account Pylon flow so I can answer from your OpenAuth-owned Pylons.',
    ].join('\n\n')
  }

  const accountContext = context.accountContext
  if (accountContext.totals.linkedPylons === 0) {
    return [
      accountContextHeader(accountContext),
      '',
      'No Pylons are linked to your OpenAuth account yet.',
      'Link an OpenAgents agent token in the account Pylon flow, then heartbeat/register the Pylon from that linked agent.',
    ].join('\n')
  }

  const onlineRows = accountContext.pylons.filter(pylon => pylon.onlineNow)
  const rows = onlineRows.length === 0 ? accountContext.pylons : onlineRows

  return [
    accountContextHeader(accountContext),
    '',
    'Your linked Pylons:',
    ...(rows.length === 0
      ? ['- none']
      : rows.slice(0, MAX_ANSWER_PYLONS).map(pylonStatusLine)),
    '',
    'This is owner-scoped to your verified OpenAuth session. Public registry rows may include other Pylons, but I am not treating those as yours.',
  ].join('\n')
}

export const answerKhalaChatPylonQuestion = (
  latestUserContent: string,
  context: KhalaChatPylonContextEnvelope | undefined,
): string | null => {
  const kind = classifyKhalaChatPylonQuestion(latestUserContent)
  switch (kind) {
    case 'capabilities':
      return capabilityAnswer(context)
    case 'interaction':
      return interactionAnswer()
    case 'list_connected':
      return connectedPylonAnswer(context)
    case 'ownership':
      return ownershipAnswer(context)
    case 'register':
      return registerAnswer()
    case null:
      return null
  }
}
