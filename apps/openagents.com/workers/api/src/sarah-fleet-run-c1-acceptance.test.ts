import { canonicalJson, fleetRunScope } from '@openagentsinc/khala-sync'
import {
  makeFleetSteeringExchangeRepository,
  makeFleetRunAuthorityRepository,
  readScopeOwner,
  resolveScopeRead,
  type FleetRunAuthorityRepositoryShape,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { Effect } from 'effect'
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import postgres from 'postgres'
import { describe, expect, test } from 'vitest'

import { handleSarahRequest } from '../../../../sarah/src/server'
import {
  createBootstrapSummary,
  parseBootstrapArgs,
} from '../../../../pylon/src/bootstrap'
import { hashPylonAccountRef } from '../../../../pylon/src/account-registry'
import {
  openPylonNodeFleetRunActivationService,
} from '../../../../pylon/src/node/fleet-run-activation'
import {
  openPylonFleetRunIntakePoller,
} from '../../../../pylon/src/node/fleet-run-intake-poller'
import {
  createPylonDurableFleetRunPlanner,
} from '../../../../pylon/src/orchestration/fleet-run-durable-planner'
import {
  makePylonFleetRunExecutionHttpPort,
} from '../../../../pylon/src/orchestration/fleet-run-execution-reporter'
import {
  makePylonFleetRunHttpIntake,
} from '../../../../pylon/src/orchestration/fleet-run-http-intake'
import {
  openPylonFleetRunRemoteIntakeService,
} from '../../../../pylon/src/orchestration/fleet-run-remote-intake'
import {
  openPylonFleetRunRuntime,
} from '../../../../pylon/src/orchestration/fleet-run-runtime'
import {
  openPylonStandingFleetRunExecutor,
  type PylonStandingFleetRunExecutor,
} from '../../../../pylon/src/orchestration/fleet-run-standing-executor'
import {
  makePylonFleetRunSteeringHttpTransport,
  openPylonFleetRunSteeringConsumer,
  type PylonFleetRunSteeringConsumer,
} from '../../../../pylon/src/orchestration/fleet-run-steering-consumer'
import {
  openPylonFleetRunSteeringFollowUpDispatcher,
  type PylonFleetRunAttemptControl,
  type PylonFleetRunSteeringFollowUpDispatcher,
} from '../../../../pylon/src/orchestration/fleet-run-steering-follow-up-dispatcher'
import type {
  FleetRunSupervisorDispatchInput,
} from '../../../../pylon/src/orchestration/fleet-run-supervisor'
import { assertPublicProjectionSafe } from '../../../../pylon/src/state'
import {
  makeSarahFleetBrowserCommands,
  parseSarahFleetBrowserConfig,
} from '../../../../sarah/src/services/fleet-browser-host'
import { makeSarahFleetSyncClient } from '../../../../sarah/src/services/fleet-sync-client'
import {
  makeSarahFleetProjectionStore,
  type SarahFleetProjectionOpenResult,
  type SarahFleetProjectionState,
} from '../../../../sarah/src/services/fleet-sync-projection-store'
import { materializeHttpResult } from './http/responses'
import {
  type AgentCredentialLookup,
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  type PylonApiRegistrationRecord,
  type PylonApiStore,
} from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'
import {
  makeSarahFleetRunRoutes,
  SARAH_FLEET_RUNS_PATH,
} from './sarah-fleet-run-routes'
import { handleKhalaSyncBootstrap } from './khala-sync-bootstrap-routes'
import { handleKhalaSyncLog } from './khala-sync-log-routes'
import { makeKhalaSyncWorkerMutatorRegistry } from './khala-sync-mutators'
import { handleKhalaSyncPush } from './khala-sync-push-routes'

const FIXED_NOW_MS = Date.parse('2026-07-09T23:45:00.000Z')
const COMMIT = '03365073c0d96da42535ac74c6147c38e34368ed'
const OWNER_USER_ID = 'owner.public.c1'
const FOREIGN_OWNER_USER_ID = 'owner.public.foreign'
const AGENT_USER_ID = 'agent.public.c1'
const FOREIGN_AGENT_USER_ID = 'agent.public.foreign'
const PYLON_REF = 'pylon.public.c1'
const OWNER_TOKEN = 'oa_agent_owner_fixture_8639'
const FOREIGN_TOKEN = 'oa_agent_foreign_fixture_8639'

const fleetRequest = {
  objective: 'Run three bounded public work streams and verify each closeout.',
  repository: {
    owner: 'OpenAgentsInc',
    name: 'openagents',
    branch: 'main',
    commit: COMMIT,
  },
  verifier: {
    kind: 'command',
    command:
      'bun test apps/openagents.com/workers/api/src/sarah-fleet-run-c1-acceptance.test.ts',
  },
  workSource: {
    kind: 'issue_list',
    issueRefs: ['#8637', '#8633', '#8639'],
  },
  workerPolicy: { workerKind: 'auto', targetPreference: 'owner_local' },
  targetConcurrency: 3,
  idempotencyKey: 'c1-integrated-fixture-0002',
} as const

type PublicStartEnvelope = Readonly<{
  duplicate: boolean
  ok: true
  run: Readonly<{
    runRef: string
    scope: string
    status: string
    privateMaterialExcluded: true
  }>
}>

type SarahTurnEnvelope = Readonly<{
  personaPreview: string
  reply: string
  toolResults: ReadonlyArray<
    Readonly<{
      ok: boolean
      output: unknown
      toolName: string
    }>
  >
}>

type DeterministicTimeline = {
  advance: (milliseconds: number) => void
  now: () => number
  acknowledgementAt: number | null
  firstCapacityAt: number | null
  firstClaimAt: number | null
}

const makeTimeline = (): DeterministicTimeline => {
  let now = 0
  return {
    advance: milliseconds => {
      now += milliseconds
    },
    now: () => now,
    acknowledgementAt: null,
    firstCapacityAt: null,
    firstClaimAt: null,
  }
}

const migrate = async (sql: ReturnType<typeof postgres>): Promise<void> => {
  const directory = join(
    process.cwd(),
    '../../../../packages/khala-sync-server/migrations',
  )
  const filenames = (await readdir(directory))
    .filter(filename => /^\d{4}_[a-z0-9_]+\.sql$/u.test(filename))
    .sort()
  for (const filename of filenames) {
    await sql.unsafe(await readFile(join(directory, filename), 'utf8'))
  }
}

const seedClaimablePylon = async (
  sql: ReturnType<typeof postgres>,
): Promise<void> => {
  const nowIso = new Date(FIXED_NOW_MS).toISOString()
  await sql`
    INSERT INTO pylon_registrations
      (id, pylon_ref, owner_agent_user_id, owner_agent_credential_id,
       owner_agent_token_prefix, display_name, status, resource_mode,
       capability_refs_json, wallet_ready, latest_heartbeat_at,
       latest_heartbeat_status, latest_health_refs_json,
       latest_load_refs_json, latest_capacity_refs_json,
       provider_market_relay_refs_json, provider_nip90_lane_refs_json,
       public_projection_json, created_at, updated_at)
    VALUES
      ('registration.public.c1', ${PYLON_REF}, ${AGENT_USER_ID},
       'credential.public.c1', 'oa_agent_owner', 'C1 fixture Pylon',
       'active', 'balanced', '[]', 0, ${nowIso}, 'online', '[]',
       '[]', '[]', '[]', '[]', '{}', ${nowIso}, ${nowIso})
  `
  await sql`
    INSERT INTO openauth_agent_links
      (id, openauth_user_id, agent_user_id, agent_credential_id,
       link_kind, status, created_at, updated_at, revoked_at)
    VALUES
      ('link.public.c1', ${OWNER_USER_ID}, ${AGENT_USER_ID},
       'credential.public.c1', 'credential_anchor', 'active', ${nowIso},
       ${nowIso}, NULL)
  `
}

const registration = (): PylonApiRegistrationRecord => ({
  capabilityRefs: [
    'capability.public.coding.codex',
    'capability.public.coding.claude',
    'capability.public.coding.grok',
  ],
  clientProtocolVersion: '1',
  clientVersion: 'openagents.pylon@fixture',
  createdAt: new Date(FIXED_NOW_MS).toISOString(),
  displayName: 'C1 fixture Pylon',
  id: 'registration.public.c1',
  latestHeartbeatAt: new Date(FIXED_NOW_MS).toISOString(),
  latestHeartbeatStatus: 'online',
  latestCapacityRefs: [
    'capacity.coding.codex.available=1',
    'capacity.coding.claude.available=1',
    'capacity.coding.grok.available=1',
  ],
  latestHealthRefs: [],
  latestLoadRefs: [
    'load.coding.codex.busy=0',
    'load.coding.claude.busy=0',
    'load.coding.grok.busy=0',
  ],
  latestResourceMode: 'balanced',
  ownerAgentCredentialId: 'credential.public.c1',
  ownerAgentTokenPrefix: 'oa_agent_owner',
  ownerAgentUserId: AGENT_USER_ID,
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: PYLON_REF,
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: new Date(FIXED_NOW_MS).toISOString(),
  walletReady: false,
  walletRef: null,
})

const agentLookup = (
  agentUserId: string,
  ownerUserId: string,
): AgentCredentialLookup => ({
  credentialId: `credential.${agentUserId}`,
  openauthUserId: ownerUserId,
  profileMetadataJson: '{}',
  tokenPrefix: 'oa_agent_fixture',
  user: {
    id: agentUserId,
    kind: 'agent',
    displayName: `Agent ${agentUserId}`,
    primaryEmail: null,
    avatarUrl: null,
    status: 'active',
    createdAt: new Date(FIXED_NOW_MS).toISOString(),
    updatedAt: new Date(FIXED_NOW_MS).toISOString(),
  },
})

const makeAgentStore = async (): Promise<AgentRegistrationStore> => {
  const lookups = new Map([
    [await sha256Hex(OWNER_TOKEN), agentLookup(AGENT_USER_ID, OWNER_USER_ID)],
    [
      await sha256Hex(FOREIGN_TOKEN),
      agentLookup(FOREIGN_AGENT_USER_ID, FOREIGN_OWNER_USER_ID),
    ],
  ])
  return {
    createAgentRegistration: () => Promise.resolve(),
    findAgentByTokenHash: tokenHash => Promise.resolve(lookups.get(tokenHash)),
    listLinkedAgentsForOpenAuthUser: openauthUserId =>
      Promise.resolve(
        openauthUserId === OWNER_USER_ID
          ? [
              {
                agentUserId: AGENT_USER_ID,
                credentialId: 'credential.public.c1',
                displayName: 'C1 fixture agent',
                linkKind: 'credential_anchor',
                openauthUserId: OWNER_USER_ID,
                tokenPrefix: 'oa_agent_owner',
              },
            ]
          : [],
      ),
    touchAgentCredential: () => Promise.resolve(),
    updateAgentDisplayName: () => Promise.resolve(0),
  }
}

const waitUntil = async (
  predicate: () => boolean | Promise<boolean>,
  timeoutMs = 15_000,
): Promise<void> => {
  const deadline = performance.now() + timeoutMs
  while (!(await predicate())) {
    if (performance.now() >= deadline) {
      throw new Error('timed out waiting for the C1 fixture closeout')
    }
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}

describe.skipIf(!hasLocalPostgres())(
  'Sarah FleetRun C1 integrated acceptance',
  () => {
    test(
      '#8639 composes three-stream owner control through reconnect-safe closeout',
      async () => {
        const pg = await startLocalPostgres()
        const root = await mkdtemp(join(tmpdir(), 'sarah-c1-acceptance-'))
        const sql = postgres(pg.urlFor('postgres'), {
          max: 8,
          prepare: false,
        })
        const timeline = makeTimeline()
        const summary = createBootstrapSummary(
          parseBootstrapArgs(['--json']),
          { PYLON_HOME: join(root, 'pylon-home') },
        )
        const standingExecutors: Array<PylonStandingFleetRunExecutor> = []
        const steeringConsumers: Array<PylonFleetRunSteeringConsumer> = []
        const steeringDispatchers: Array<PylonFleetRunSteeringFollowUpDispatcher> = []
        const dispatched: Array<FleetRunSupervisorDispatchInput> = []
        const transportBodies: Array<unknown> = []
        const executionTransportBodies: Array<unknown> = []
        const executionTransportFailures: Array<unknown> = []
        const steeringTransportFailures: Array<unknown> = []
        const steeringTransportBodies: Array<unknown> = []
        const appliedSteers: Array<Parameters<PylonFleetRunAttemptControl['applySteer']>[0]> = []
        const appliedApprovals: Array<Parameters<PylonFleetRunAttemptControl['applyApproval']>[0]> = []
        const approvalCallbackFailures: Array<string> = []
        const PRIVATE_STEER_BODY =
          'PRIVATE C1 steer: inspect the exact named attempt before closeout.'
        let approvalRequestObserved = false
        let releaseApprovedCodex!: () => void
        const approvedCodex = new Promise<void>(resolve => {
          releaseApprovedCodex = resolve
        })
        let authorityClaimCalls = 0
        let authorityAcceptCalls = 0
        let importedBeforeAccept = false
        const sessionIndexFilename = `c1-acceptance-${process.pid}.json`
        const sessionIndexPath = join(
          process.cwd(),
          '.sarah',
          sessionIndexFilename,
        )
        const savedAccountLinkTestMode =
          process.env.SARAH_ACCOUNT_LINK_TEST_MODE
        const savedSessionIndexPath = process.env.SARAH_SESSION_INDEX_PATH
        let activation: Awaited<
          ReturnType<typeof openPylonNodeFleetRunActivationService>
        > | undefined
        let poller: ReturnType<typeof openPylonFleetRunIntakePoller> | undefined

        try {
          process.env.SARAH_ACCOUNT_LINK_TEST_MODE = '1'
          process.env.SARAH_SESSION_INDEX_PATH = sessionIndexFilename
          await migrate(sql)
          await seedClaimablePylon(sql)
          const authority = makeFleetRunAuthorityRepository({
            sql: sql as unknown as SyncSql,
            now: Effect.sync(() => FIXED_NOW_MS + timeline.now()),
          })
          const steeringExchange = makeFleetSteeringExchangeRepository({
            sql: sql as unknown as SyncSql,
            now: Effect.sync(() => FIXED_NOW_MS + timeline.now()),
          })
          let startBudgetOpen = true
          const sarahRoutes = makeSarahFleetRunRoutes({
            authenticateOwner: request => {
              if (startBudgetOpen) {
                timeline.advance(400)
              }
              const cookie = request.headers.get('cookie') ?? ''
              const userId =
                cookie.includes('oa_session=owner')
                  ? OWNER_USER_ID
                  : cookie.includes('oa_session=prospect')
                    ? 'owner.public.prospect'
                    : cookie.includes('oa_session=foreign')
                      ? FOREIGN_OWNER_USER_ID
                      : undefined
              return Promise.resolve(
                userId === undefined
                  ? undefined
                  : {
                      userId,
                      email: `${userId}@example.com`,
                      decorateResponseHeaders: headers => {
                        headers.append(
                          'set-cookie',
                          'oa_session=refreshed; Secure; HttpOnly',
                        )
                      },
                    },
              )
            },
            resolveRelationshipMode: owner => {
              if (startBudgetOpen) {
                timeline.advance(300)
              }
              return Promise.resolve(
                owner.userId === 'owner.public.prospect'
                  ? ('prospect' as const)
                  : ('operator' as const),
              )
            },
            makeSqlClient: () => {
              if (startBudgetOpen) {
                timeline.advance(500)
              }
              return Promise.resolve({
                sql: sql as unknown as SyncSql,
                end: () => Promise.resolve(),
              })
            },
            makeRepository: () => authority,
          })
          const sarahFetch = (request: Request): Promise<Response> =>
            Effect.runPromise(
              sarahRoutes.handle(
                request,
                { KHALA_SYNC_DB: { connectionString: pg.url } },
                {} as ExecutionContext,
              ),
            ).then(materializeHttpResult)

          const operatorSystems: Array<string> = []
          const authorityBodies: Array<unknown> = []
          const fleetAuthorityFetch = async (
            request: Request,
          ): Promise<Response> => {
            authorityBodies.push(await request.clone().json())
            return sarahFetch(request)
          }
          const startConversation = (): Promise<Response> =>
            handleSarahRequest(
              new Request('https://openagents.com/sarah/api/eve/turn', {
                method: 'POST',
                headers: {
                  cookie: 'oa_session=owner; oa_access=fixture-current',
                  'content-type': 'application/json',
                  'x-sarah-test-oa-session': JSON.stringify({
                    userId: OWNER_USER_ID,
                    email: 'operator@example.com',
                    teams: [
                      {
                        id: 'team_openagents_core',
                        name: 'OpenAgents Core Team',
                        slug: 'openagents-core-team',
                      },
                    ],
                    isAdmin: false,
                  }),
                },
                body: JSON.stringify({
                  message: 'Start the bounded coding fleet now.',
                  prospectRef: 'prospect-public-c1',
                  relationshipMode: 'prospect',
                }),
              }),
              {
                fleetAuthorityFetch,
                generateOwnedReply: ({ system }) => {
                  operatorSystems.push(system)
                  timeline.advance(300)
                  return Promise.resolve({
                    ok: true as const,
                    reply: JSON.stringify({
                      sarah_tool: 'coding_fleet_start',
                      args: fleetRequest,
                    }),
                    model: 'fixture-gemma',
                    usage: {
                      promptTokens: 2,
                      outputTokens: 2,
                      thoughtTokens: 0,
                      totalTokens: 4,
                    },
                  })
                },
              },
            )

          const wallStartedAt = performance.now()
          const started = await startConversation()
          timeline.advance(300)
          timeline.acknowledgementAt = timeline.now()
          startBudgetOpen = false
          const acknowledgementWallMs = performance.now() - wallStartedAt
          const startedTurn = (await started.json()) as SarahTurnEnvelope
          const startedTool = startedTurn.toolResults[0]
          const startOutput = startedTool?.output as PublicStartEnvelope
          expect(started.ok).toBe(true)
          expect(startedTool).toMatchObject({
            ok: true,
            toolName: 'coding_fleet_start',
          })
          expect(startOutput).toMatchObject({
            duplicate: false,
            ok: true,
            run: {
              privateMaterialExcluded: true,
              status: 'pending_executor',
            },
          })
          expect(startOutput.run.runRef).toMatch(
            /^fleet_run\.sarah\.[0-9a-f]{20}$/u,
          )
          expect(authorityBodies).toEqual([fleetRequest])
          expect(operatorSystems).toHaveLength(1)
          expect(operatorSystems[0]).toContain('coding_fleet_start')
          expect(operatorSystems[0]).toContain(
            "owner's AI coding-fleet operator",
          )
          expect(operatorSystems[0]).not.toContain('AI sales employee')
          expect(startedTurn.personaPreview).toContain(
            'AI coding-fleet operator',
          )
          expect(started.headers.getSetCookie()).toEqual([
            'oa_session=refreshed; Secure; HttpOnly',
          ])
          expect(timeline.acknowledgementAt).toBeLessThanOrEqual(5_000)
          expect(acknowledgementWallMs).toBeLessThan(5_000)

          const duplicate = await startConversation()
          const duplicateTurn = (await duplicate.json()) as SarahTurnEnvelope
          expect(duplicateTurn.toolResults[0]?.output).toMatchObject({
            duplicate: true,
            run: { runRef: startOutput.run.runRef },
          })

          let prospectAuthorityCalls = 0
          const prospectSystems: Array<string> = []
          const prospect = await handleSarahRequest(
            new Request('https://openagents.com/sarah/api/eve/turn', {
              method: 'POST',
              headers: { 'content-type': 'application/json' },
              body: JSON.stringify({
                message: 'Start my coding fleet.',
                prospectRef: 'prospect-public-c1-refusal',
                relationshipMode: 'administrator',
              }),
            }),
            {
              generateOwnedReply: ({ system }) => {
                prospectSystems.push(system)
                return Promise.resolve({
                  ok: true as const,
                  reply: JSON.stringify({
                    sarah_tool: 'coding_fleet_start',
                    args: fleetRequest,
                  }),
                  model: 'fixture-gemma',
                  usage: {
                    promptTokens: 1,
                    outputTokens: 1,
                    thoughtTokens: 0,
                    totalTokens: 2,
                  },
                })
              },
              fleetAuthorityFetch: () => {
                prospectAuthorityCalls += 1
                return Promise.resolve(new Response())
              },
            },
          )
          const prospectTurn = (await prospect.json()) as SarahTurnEnvelope
          expect(prospectTurn).toMatchObject({
            reply:
              'Coding fleet commands are available only in authenticated owner-operator mode.',
            toolResults: [],
          })
          expect(prospectSystems).toHaveLength(1)
          expect(prospectSystems[0]).not.toContain('coding_fleet_start')
          expect(prospectAuthorityCalls).toBe(0)

          const foreignObservation = materializeHttpResult(
            await Effect.runPromise(
              sarahRoutes.handle(
                new Request(
                  `https://openagents.com${SARAH_FLEET_RUNS_PATH}?runRef=${startOutput.run.runRef}`,
                  { headers: { cookie: 'oa_session=foreign' } },
                ),
                { KHALA_SYNC_DB: { connectionString: pg.url } },
                {} as ExecutionContext,
              ),
            ),
          )
          expect(foreignObservation.status).toBe(404)
          expect(await foreignObservation.json()).toMatchObject({
            error: { code: 'run_not_found' },
          })

          const pylonStore = {
            readRegistration: (pylonRef: string) =>
              Promise.resolve(
                pylonRef === PYLON_REF ? registration() : undefined,
              ),
          } as unknown as PylonApiStore
          const agentStore = await makeAgentStore()
          const fleetAuthority = {
            claim: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<FleetRunAuthorityRepositoryShape['claim']>[0],
            ) =>
              Effect.gen(function* () {
                authorityClaimCalls += 1
                const result = yield* authority.claim(input)
                if (timeline.firstClaimAt === null) {
                  timeline.firstClaimAt = timeline.now()
                }
                return result
              }),
            acceptClaim: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<
                FleetRunAuthorityRepositoryShape['acceptClaim']
              >[0],
            ) =>
              Effect.sync(() => {
                authorityAcceptCalls += 1
              }).pipe(Effect.andThen(authority.acceptClaim(input))),
            appendExecutionEvents: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<
                NonNullable<
                  FleetRunAuthorityRepositoryShape['appendExecutionEvents']
                >
              >[0],
            ) => authority.appendExecutionEvents(input),
          }
          const fleetSteeringExchange = {
            readPage: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<typeof steeringExchange.readPage>[0],
            ) => steeringExchange.readPage(input),
            appendOutcomes: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<typeof steeringExchange.appendOutcomes>[0],
            ) => steeringExchange.appendOutcomes(input),
            appendCompletions: (
              _env: Readonly<Record<string, unknown>>,
              input: Parameters<typeof steeringExchange.appendCompletions>[0],
            ) => steeringExchange.appendCompletions(input),
          }
          const pylonRoutes = makePylonApiRoutes({
            agentStore: () => agentStore,
            fleetRunAuthority: fleetAuthority,
            fleetSteeringExchange,
            makeStore: () => pylonStore,
            nowIso: () =>
              new Date(FIXED_NOW_MS + timeline.now()).toISOString(),
          })
          const routePylon = (request: Request): Promise<Response> => {
            const route = pylonRoutes.routePylonApiRequest(
              request,
              {},
              {} as ExecutionContext,
            )
            if (route === undefined) {
              throw new Error(`unmatched Pylon route: ${request.url}`)
            }
            return Effect.runPromise(route)
          }
          const steeringTransport = makePylonFleetRunSteeringHttpTransport({
            agentToken: OWNER_TOKEN,
            baseUrl: 'https://openagents.test',
            fetchImpl: Object.assign(
              async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
                const request = new Request(input, init)
                const requestBody = request.method === 'GET'
                  ? null
                  : await request.clone().json()
                const response = await routePylon(request)
                steeringTransportBodies.push({
                  method: request.method,
                  url: request.url,
                  request: requestBody,
                  response: await response.clone().json(),
                  status: response.status,
                })
                if (!response.ok) {
                  steeringTransportFailures.push({
                    method: request.method,
                    url: request.url,
                    request: requestBody,
                    response: await response.clone().json(),
                    status: response.status,
                  })
                }
                return response
              },
              { preconnect: fetch.preconnect },
            ),
          })
          const steeringControl: PylonFleetRunAttemptControl = {
            applyApproval: input => {
              appliedApprovals.push(input)
              return Promise.resolve({ state: 'applied' })
            },
            applySteer: input => {
              appliedSteers.push(input)
              return Promise.resolve({ state: 'applied' })
            },
            observeStop: () =>
              Promise.resolve({
                state: 'failed',
                failureRef: 'blocker.public.c1.stop_not_exercised',
              }),
          }
          const syncScope = fleetRunScope(startOutput.run.runRef)
          const makeSyncSqlClient = () =>
            Promise.resolve({
              sql: sql as unknown as SyncSql,
              end: () => Promise.resolve(),
            })
          const resolveSyncScope = (userId: string | undefined, scope: string) =>
            resolveScopeRead(
              {
                canReadAgentRun: () => Promise.resolve(false),
                canReadThread: () => Promise.resolve(false),
                isTeamMember: () => Promise.resolve(false),
                readFleetScopeOwner: requestedScope =>
                  readScopeOwner(
                    sql as unknown as SyncSql,
                    requestedScope,
                  ),
              },
              userId,
              scope as Parameters<typeof resolveScopeRead>[2],
            )
          const syncRegistry = makeKhalaSyncWorkerMutatorRegistry()
          const syncFetch = async (
            path: string,
            init?: RequestInit,
          ): Promise<Response> => {
            const request = new Request(
              new URL(path, 'https://openagents.test'),
              init,
            )
            if (request.url.includes('/api/sync/push')) {
              return Effect.runPromise(
                handleKhalaSyncPush(request, {
                  authenticate: () =>
                    Promise.resolve({ userId: OWNER_USER_ID }),
                  binding: { connectionString: pg.url },
                  makeSqlClient: makeSyncSqlClient,
                  registry: syncRegistry,
                }),
              )
            }
            if (request.url.includes('/api/sync/bootstrap')) {
              return Effect.runPromise(
                handleKhalaSyncBootstrap(request, {
                  authenticate: () =>
                    Promise.resolve({ userId: OWNER_USER_ID }),
                  binding: { connectionString: pg.url },
                  makeSqlClient: makeSyncSqlClient,
                  resolveScopeRead: resolveSyncScope,
                }),
              )
            }
            if (request.url.includes('/api/sync/log')) {
              return Effect.runPromise(
                handleKhalaSyncLog(request, {
                  authenticate: () =>
                    Promise.resolve({ userId: OWNER_USER_ID }),
                  binding: { connectionString: pg.url },
                  hubNamespace: undefined,
                  makeSqlClient: makeSyncSqlClient,
                  resolveScopeRead: resolveSyncScope,
                }),
              )
            }
            throw new Error(`unmatched Sync route: ${request.url}`)
          }
          let persistedProjectionJson: string | null = null
          const projectionPersistence = {
            load: () =>
              Promise.resolve(
                persistedProjectionJson === null
                  ? null
                  : (JSON.parse(persistedProjectionJson) as unknown),
              ),
            save: (state: SarahFleetProjectionState) => {
              persistedProjectionJson = JSON.stringify(state)
              return Promise.resolve()
            },
          }
          let browserSerial = 0
          const makeBrowserClient = (clientId: string) =>
            makeSarahFleetSyncClient({
              fetch: syncFetch,
              clientGroupId: 'sarah.web.c1',
              clientId,
            })
          const openBrowserProjection = async (
            client: ReturnType<typeof makeBrowserClient>,
          ): Promise<SarahFleetProjectionOpenResult> =>
            makeSarahFleetProjectionStore({
              client,
              persistence: projectionPersistence,
              now: () => FIXED_NOW_MS + timeline.now(),
            }).open(syncScope)

          const foreignClaim = await routePylon(
            new Request(
              `https://openagents.test/api/pylons/${PYLON_REF}/fleet-runs/claim`,
              {
                method: 'POST',
                headers: {
                  authorization: `Bearer ${FOREIGN_TOKEN}`,
                  'content-type': 'application/json',
                  'idempotency-key': 'foreign-owner-claim-0001',
                },
                body: JSON.stringify({
                  schema: 'openagents.pylon.fleet_run_claim.request.v1',
                  runRef: startOutput.run.runRef,
                }),
              },
            ),
          )
          expect(foreignClaim.status).toBe(403)
          expect(authorityClaimCalls).toBe(0)

          const injectedOwner = await routePylon(
            new Request(
              `https://openagents.test/api/pylons/${PYLON_REF}/fleet-runs/claim`,
              {
                method: 'POST',
                headers: {
                  authorization: `Bearer ${OWNER_TOKEN}`,
                  'content-type': 'application/json',
                  'idempotency-key': 'body-owner-injection-0001',
                },
                body: JSON.stringify({
                  schema: 'openagents.pylon.fleet_run_claim.request.v1',
                  ownerUserId: FOREIGN_OWNER_USER_ID,
                }),
              },
            ),
          )
          expect(injectedOwner.status).toBe(400)
          expect(authorityClaimCalls).toBe(0)

          activation = await openPylonNodeFleetRunActivationService({
            summary,
            pylonRef: PYLON_REF,
            baseUrl: 'https://openagents.test',
            agentToken: OWNER_TOKEN,
            executionRemote: makePylonFleetRunExecutionHttpPort({
              agentToken: OWNER_TOKEN,
              baseUrl: 'https://openagents.test',
              fetchImpl: Object.assign(
                async (
                  input: Parameters<typeof fetch>[0],
                  init?: Parameters<typeof fetch>[1],
                ) => {
                  const request = new Request(input, init)
                  const requestBody = await request.clone().json()
                  executionTransportBodies.push(requestBody)
                  const response = await routePylon(request)
                  if (!response.ok) {
                    executionTransportFailures.push({
                      request: requestBody,
                      response: await response.clone().json(),
                      status: response.status,
                    })
                  }
                  return response
                },
                { preconnect: fetch.preconnect },
              ),
            }),
            openExecutor: async input => {
              const standing = await openPylonStandingFleetRunExecutor({
                bootstrap: summary,
                now: () => new Date(FIXED_NOW_MS + timeline.now()),
                pylonRef: input.pylonRef,
                runRef: input.runRef,
                startImmediately: false,
                tickIntervalMs: 5,
                clock: {
                  now: () => new Date(FIXED_NOW_MS + timeline.now()),
                  sleep: milliseconds =>
                    new Promise(resolve => setTimeout(resolve, milliseconds)),
                },
                steeringConsumerFactory: input => {
                  const consumer = openPylonFleetRunSteeringConsumer({
                    ...input,
                    transport: steeringTransport,
                    now: () => new Date(FIXED_NOW_MS + timeline.now()),
                    startImmediately: false,
                    intervalMs: 60_000,
                  })
                  steeringConsumers.push(consumer)
                  return consumer
                },
                steeringFollowUpDispatcherFactory: input => {
                  const dispatcher =
                    openPylonFleetRunSteeringFollowUpDispatcher({
                      ...input,
                      control: steeringControl,
                      now: () => new Date(FIXED_NOW_MS + timeline.now()),
                      startImmediately: false,
                      intervalMs: 60_000,
                      onCompletion: async completion => {
                        await steeringTransport.postCompletions({
                          ...input,
                          completions: [
                            {
                              seq: completion.seq,
                              intentId: completion.intentId,
                              state: completion.state,
                              completionRef: completion.completionRef,
                              completedAt: completion.completedAt,
                            },
                          ],
                        })
                        const approval = appliedApprovals.find(
                          candidate =>
                            candidate.intent.intentId === completion.intentId,
                        )
                        if (approval !== undefined) {
                          const binding =
                            input.store.getFleetRunSteeringApprovalBinding(
                              approval.approvalRef,
                            )
                          expect(binding).toMatchObject({
                            state: 'resolved',
                            decision: 'allow',
                          })
                          releaseApprovedCodex()
                        }
                      },
                    })
                  steeringDispatchers.push(dispatcher)
                  return dispatcher
                },
                ...(input.onLifecycle === undefined
                  ? {}
                  : { onLifecycle: input.onLifecycle }),
                adapterFactory: ({ store }) => ({
                  capacity: {
                    accounts: () => {
                      timeline.advance(2_000)
                      if (timeline.firstCapacityAt === null) {
                        timeline.firstCapacityAt = timeline.now()
                      }
                      return Promise.resolve([
                        {
                          accountRef: 'codex-fixture-isolated',
                          advertisedCapacity: 1,
                          marginalCostClass: 'subscription' as const,
                          workerKind: 'codex' as const,
                        },
                        {
                          accountRef: 'claude-fixture-isolated',
                          advertisedCapacity: 1,
                          marginalCostClass: 'subscription' as const,
                          workerKind: 'claude' as const,
                        },
                        {
                          accountRef: 'grok-fixture-isolated',
                          advertisedCapacity: 1,
                          marginalCostClass: 'not_measured' as const,
                          workerKind: 'grok' as const,
                        },
                      ])
                    },
                  },
                  livenessProbe: () => Promise.resolve('dead' as const),
                  planner: createPylonDurableFleetRunPlanner({ store }),
                  runner: {
                    dispatch: async dispatch => {
                      dispatched.push(dispatch)
                      timeline.advance(1_000)
                      const assignmentRef =
                        `assignment.public.c1.${dispatch.workerKind}`
                      const accountProvider =
                        dispatch.workerKind === 'claude'
                          ? 'claude_agent'
                          : dispatch.workerKind
                      const accountRefHash = hashPylonAccountRef(
                        accountProvider,
                        dispatch.accountRef,
                      )
                      const usageEvidence =
                        dispatch.workerKind === 'grok'
                          ? {
                              schema:
                                'openagents.pylon.fleet_run_usage_evidence.v1' as const,
                              truth: 'not_measured' as const,
                              harnessKind: 'grok' as const,
                              evidenceRef: 'evidence.public.c1.grok',
                              assignmentRef,
                              receiptRef: 'receipt.public.c1.grok',
                              tokenUsageRefs: [],
                              caveatRefs: [
                                'caveat.pylon.fleet_run.grok_usage_not_measured',
                              ],
                            }
                          : {
                              schema:
                                'openagents.pylon.fleet_run_usage_evidence.v1' as const,
                              truth: 'exact' as const,
                              harnessKind: dispatch.workerKind,
                              evidenceRef:
                                `evidence.public.c1.${dispatch.workerKind}`,
                              assignmentRef,
                              pylonRef: PYLON_REF,
                              provider:
                                dispatch.workerKind === 'codex'
                                  ? ('pylon-codex-own-capacity' as const)
                                  : ('pylon-claude-own-capacity' as const),
                              model:
                                dispatch.workerKind === 'codex'
                                  ? ('openagents/pylon-codex' as const)
                                  : ('openagents/pylon-claude' as const),
                              demandKind: 'own_capacity' as const,
                              demandSource: 'khala_coding_delegation' as const,
                              inputTokens: 8,
                              outputTokens: 4,
                              reasoningTokens: 1,
                              cacheReadTokens: 2,
                              totalTokens: 12,
                              tokenRows: 1,
                              tokenUsageRefs: [
                                `token_usage_event.public.c1.${dispatch.workerKind}`,
                              ],
                              proofRefs: [
                                `proof.public.c1.${dispatch.workerKind}`,
                              ],
                              closeoutChecklistRefs: [
                                `check.public.c1.closeout.${dispatch.workerKind}`,
                              ],
                              proofChecklistRefs: [
                                `check.public.c1.proof.${dispatch.workerKind}`,
                              ],
                            }
                      if (dispatch.workerKind === 'codex') {
                        try {
                          await dispatch.onApprovalRequested?.({
                            approvalRef: 'approval.public.c1.write_file',
                            assignmentRef,
                            toolClass: 'write_file',
                          })
                        } catch (error) {
                          approvalCallbackFailures.push(
                            error instanceof Error
                              ? (error.stack ?? error.message)
                              : String(error),
                          )
                          throw error
                        }
                        approvalRequestObserved = true
                        await approvedCodex
                      }
                      return {
                        assignmentRef,
                        accountRefHash,
                        artifactRefs: [
                          `artifact.public.c1.${dispatch.workerKind}`,
                        ],
                        authorityReceiptRefs: [
                          `receipt.public.c1.authority.${dispatch.workerKind}`,
                        ],
                        closeoutRef:
                          `closeout.public.c1.${dispatch.workerKind}`,
                        proofRefs: [
                          `proof.public.c1.${dispatch.workerKind}`,
                        ],
                        usageEvidence,
                        lifecycle: [],
                        marginalCostClass:
                          dispatch.workerKind === 'grok'
                            ? ('not_measured' as const)
                            : ('subscription' as const),
                        status: 'completed' as const,
                        summary: `Bounded ${dispatch.workerKind} fixture work completed and verified.`,
                        verification: {
                          truth: 'passed' as const,
                          verifierRef:
                            `verifier.public.c1.${dispatch.workerKind}`,
                          evidenceRefs: [
                            `verification.public.c1.${dispatch.workerKind}`,
                          ],
                        },
                      }
                    },
                  },
                }),
              })
              standingExecutors.push(standing)
              return standing
            },
          })

          const remote = makePylonFleetRunHttpIntake({
            agentToken: OWNER_TOKEN,
            baseUrl: 'https://openagents.test',
            makeId: () => 'c1-standing-claim-0001',
            fetchImpl: Object.assign(
              async (
                input: Parameters<typeof fetch>[0],
                init?: Parameters<typeof fetch>[1],
              ): Promise<Response> => {
                const request = new Request(input, init)
                const body = await request.clone().json()
                transportBodies.push(body)
                if (request.url.endsWith('/claim')) {
                  timeline.advance(4_000)
                } else {
                  const imported = await openPylonFleetRunRuntime({
                    bootstrap: summary,
                  })
                  try {
                    importedBeforeAccept =
                      imported.store.getFleetRun(startOutput.run.runRef)
                        ?.authorityBinding?.phase === 'imported'
                  } finally {
                    await imported.close()
                  }
                  timeline.advance(500)
                }
                return routePylon(request)
              },
              { preconnect: fetch.preconnect },
            ),
          })
          const intake = await openPylonFleetRunRemoteIntakeService({
            activation,
            bootstrap: summary,
            now: () => new Date(FIXED_NOW_MS + timeline.now()),
            pylonRef: PYLON_REF,
            remote,
          })
          poller = openPylonFleetRunIntakePoller({
            intake,
            intervalMs: 300_000,
            startImmediately: false,
          })

          const [firstProjection, coalescedProjection] = await Promise.all([
            poller.runNow(),
            poller.runNow(),
          ])
          const firstClaimWallMs = performance.now() - wallStartedAt
          expect(firstProjection).toEqual(coalescedProjection)
          expect(firstProjection).toMatchObject({
            blockerRefs: [],
            pylonRef: PYLON_REF,
            runRef: startOutput.run.runRef,
            state: 'active',
          })
          expect(importedBeforeAccept).toBe(true)
          expect(authorityClaimCalls).toBe(1)
          expect(authorityAcceptCalls).toBe(1)
          expect(timeline.firstClaimAt).not.toBeNull()
          expect(timeline.firstCapacityAt).not.toBeNull()
          expect(timeline.firstClaimAt!).toBeLessThanOrEqual(15_000)
          expect(timeline.firstCapacityAt!).toBeLessThanOrEqual(15_000)
          expect(firstClaimWallMs).toBeLessThan(15_000)

          expect(standingExecutors).toHaveLength(1)
          expect(steeringConsumers).toHaveLength(1)
          expect(steeringDispatchers).toHaveLength(1)
          const standing = standingExecutors[0]!
          const steeringConsumer = steeringConsumers[0]!
          const steeringDispatcher = steeringDispatchers[0]!
          await waitUntil(
            () => approvalRequestObserved || executionTransportFailures.length > 0,
            30_000,
          )
          if (executionTransportFailures.length > 0) {
            throw new Error(
              JSON.stringify({ approvalCallbackFailures, executionTransportBodies, executionTransportFailures }),
            )
          }
          expect(approvalRequestObserved).toBe(true)
          await waitUntil(async () => {
            const rows = await sql<Array<{ count: number }>>`
              SELECT count(*)::int AS count
              FROM khala_sync_changelog
              WHERE scope = ${syncScope}
                AND entity_type IN ('fleet_approval', 'fleet_worker', 'fleet_assignment')
            `
            return (rows[0]?.count ?? 0) >= 3
          })

          const browserConfig = parseSarahFleetBrowserConfig(
            `https://openagents.com/sarah?fleet_run=${startOutput.run.runRef}`,
          )!
          const browserClient = makeBrowserClient('sarah.web.c1.controls')
          let browser = await openBrowserProjection(browserClient)
          expect(browser.source).toBe('bootstrap')
          const pendingApproval = browser.projection.approvals.find(
            approval => approval.status === 'pending',
          )
          expect(pendingApproval).toMatchObject({
            approvalRef: 'approval.public.c1.write_file',
            bindingStatus: 'exact',
            availableDecisions: ['allow', 'deny'],
          })
          expect(pendingApproval?.attemptRef).not.toBeNull()
          const exactAttemptRef = pendingApproval!.attemptRef!
          const commandReceipts: Array<unknown> = []
          const commands = makeSarahFleetBrowserCommands({
            config: browserConfig,
            client: browserClient,
            cursor: () => Number(browser.state.cursor),
            projection: () => browser.projection,
            now: () => new Date(FIXED_NOW_MS + timeline.now()).toISOString(),
            randomId: () => `c1browsercommand${++browserSerial}`,
          })

          commandReceipts.push(
            await commands.runControl({
              runRef: startOutput.run.runRef,
              action: 'pause',
            }),
          )
          const pauseTick = await steeringConsumer.tick()
          if (!pauseTick.ok) {
            throw new Error(
              JSON.stringify({ pauseTick, steeringTransportBodies, steeringTransportFailures }),
            )
          }
          expect(pauseTick).toMatchObject({
            ok: true,
            applied: 1,
            acknowledged: 1,
          })
          browser = await openBrowserProjection(browserClient)
          expect(browser.source).toBe('resume')
          expect(standing.runtime.store.getFleetRun(startOutput.run.runRef)?.state)
            .toBe('paused')
          expect(browser.projection.run.status).toBe('paused')
          expect(browser.projection.commandOutcomes).toContainEqual(
            expect.objectContaining({
              kind: 'fleet_run_control',
              deliveryOutcome: 'applied',
              effectiveOutcome: 'paused',
            }),
          )

          commandReceipts.push(
            await commands.runControl({
              runRef: startOutput.run.runRef,
              action: 'resume',
            }),
          )
          expect(await steeringConsumer.tick()).toMatchObject({
            ok: true,
            applied: 1,
            acknowledged: 1,
          })
          browser = await openBrowserProjection(browserClient)
          expect(standing.runtime.store.getFleetRun(startOutput.run.runRef)?.state)
            .toBe('running')
          expect(browser.projection.run.status).toBe('running')
          expect(browser.projection.commandOutcomes).toContainEqual(
            expect.objectContaining({
              kind: 'fleet_run_control',
              deliveryOutcome: 'applied',
              effectiveOutcome: 'running',
            }),
          )

          commandReceipts.push(
            await commands.steer({
              runRef: startOutput.run.runRef,
              targetRef: exactAttemptRef,
              body: PRIVATE_STEER_BODY,
            }),
          )
          expect(await steeringConsumer.tick()).toMatchObject({
            ok: true,
            applied: 1,
            acknowledged: 1,
          })
          const steerDispatchTick = await steeringDispatcher.tick()
          if (!steerDispatchTick.ok) {
            throw new Error(
              JSON.stringify({
                steerDispatchTick,
                steeringTransportBodies,
                steeringTransportFailures,
              }),
            )
          }
          expect(steerDispatchTick).toMatchObject({
            ok: true,
            dispatched: 1,
            completionsDelivered: 1,
          })
          expect(appliedSteers).toHaveLength(1)
          expect(appliedSteers[0]).toMatchObject({
            runRef: startOutput.run.runRef,
            workClaimRef: exactAttemptRef,
            body: PRIVATE_STEER_BODY,
            bodyRef: null,
          })
          browser = await openBrowserProjection(browserClient)
          expect(browser.projection.commandOutcomes).toContainEqual(
            expect.objectContaining({
              kind: 'steer_message',
              targetRef: exactAttemptRef,
              deliveryOutcome: 'queued_follow_up',
              completionOutcome: 'applied',
              effectiveOutcome: 'steer_delivered',
            }),
          )

          commandReceipts.push(
            await commands.approvalDecision({
              runRef: startOutput.run.runRef,
              approvalRef: pendingApproval!.approvalRef,
              decision: 'allow',
            }),
          )
          expect(await steeringConsumer.tick()).toMatchObject({
            ok: true,
            applied: 1,
            acknowledged: 1,
          })
          expect(await steeringDispatcher.tick()).toMatchObject({
            ok: true,
            dispatched: 1,
            completionsDelivered: 1,
          })
          expect(appliedApprovals).toHaveLength(1)
          expect(appliedApprovals[0]).toMatchObject({
            runRef: startOutput.run.runRef,
            workClaimRef: exactAttemptRef,
            approvalRef: pendingApproval!.approvalRef,
            decision: 'allow',
          })
          await approvedCodex

          await waitUntil(() =>
            standing.runtime.store
              .listWorkClaims({ runRef: startOutput.run.runRef })
              .filter(claim => claim.state === 'closeout').length === 3,
          )
          await waitUntil(async () => {
            if (executionTransportFailures.length > 0) return true
            const rows = await sql<Array<{ execution_state: string }>>`
              SELECT execution_state
              FROM sarah_fleet_run_requests
              WHERE run_ref = ${startOutput.run.runRef}
            `
            return rows[0]?.execution_state === 'completed'
          }, 30_000)
          browser = await openBrowserProjection(browserClient)
          const reconnectClient = makeBrowserClient('sarah.web.c1.reconnect')
          const reconnected = await openBrowserProjection(reconnectClient)
          expect(reconnected.source).toBe('resume')
          expect(reconnected.projection.run.status).toBe('completed')
          expect(reconnected.projection.approvals).toContainEqual(
            expect.objectContaining({
              approvalRef: pendingApproval!.approvalRef,
              status: 'allowed',
              availableDecisions: [],
            }),
          )
          expect(reconnected.projection.commandOutcomes).toHaveLength(4)
          expect(
            reconnected.projection.commandOutcomes?.map(outcome =>
              outcome.effectiveOutcome,
            ),
          ).toEqual(['paused', 'running', 'steer_delivered', 'allowed'])
          expect(canonicalJson(commandReceipts)).not.toContain(PRIVATE_STEER_BODY)
          expect(persistedProjectionJson).not.toContain(PRIVATE_STEER_BODY)
          expect(canonicalJson(reconnected.projection)).not.toContain(
            PRIVATE_STEER_BODY,
          )
          const closed = await standing.runtime.manager.status(
            startOutput.run.runRef,
          )
          expect(Array.isArray(closed)).toBe(false)
          expect(closed).toMatchObject({
            active: false,
            run: {
              counters: {
                activeAssignments: 0,
                completedAssignments: 3,
                failedAssignments: 0,
              },
              runRef: startOutput.run.runRef,
              state: 'completed',
            },
          })
          expect(dispatched).toHaveLength(3)
          expect(executionTransportFailures).toEqual([])
          expect(
            dispatched
              .map(dispatch => ({
                accountRef: dispatch.accountRef,
                issue: dispatch.workUnit.number,
                runRef: dispatch.run.runRef,
                workerKind: dispatch.workerKind,
              }))
              .sort((left, right) => left.workerKind.localeCompare(right.workerKind)),
          ).toEqual([
            {
              accountRef: 'claude-fixture-isolated',
              issue: 8633,
              runRef: startOutput.run.runRef,
              workerKind: 'claude',
            },
            {
              accountRef: 'codex-fixture-isolated',
              issue: 8637,
              runRef: startOutput.run.runRef,
              workerKind: 'codex',
            },
            {
              accountRef: 'grok-fixture-isolated',
              issue: 8639,
              runRef: startOutput.run.runRef,
              workerKind: 'grok',
            },
          ])

          const publicProjection = {
            latency: {
              acknowledgementMs: timeline.acknowledgementAt,
              firstCapacityMs: timeline.firstCapacityAt,
              firstClaimMs: timeline.firstClaimAt,
            },
            poller: poller.status(),
            runtime: standing.runtime.store.publicSnapshot(),
          }
          expect(() => assertPublicProjectionSafe(publicProjection)).not.toThrow()
          const publicJson = JSON.stringify(publicProjection)
          expect(publicJson).not.toContain(OWNER_TOKEN)
          expect(publicJson).not.toContain(OWNER_USER_ID)
          expect(publicJson).not.toContain(root)
          expect(publicJson).not.toContain('oa_session')

          expect(transportBodies).toEqual([
            { schema: 'openagents.pylon.fleet_run_claim.request.v1' },
            {
              schema: 'openagents.pylon.fleet_run_accept.request.v1',
              runRef: startOutput.run.runRef,
              claimRef: expect.stringMatching(
                /^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u,
              ),
            },
          ])
          expect(JSON.stringify(transportBodies)).not.toContain('ownerUserId')
          expect(JSON.stringify(transportBodies)).not.toContain('pylonRef')
          expect(JSON.stringify(transportBodies)).not.toContain(OWNER_TOKEN)

          const observed = materializeHttpResult(
            await Effect.runPromise(
              sarahRoutes.handle(
                new Request(
                  `https://openagents.com${SARAH_FLEET_RUNS_PATH}?runRef=${startOutput.run.runRef}`,
                  { headers: { cookie: 'oa_session=owner' } },
                ),
                { KHALA_SYNC_DB: { connectionString: pg.url } },
                {} as ExecutionContext,
              ),
            ),
          )
          expect(observed.status).toBe(200)
          const observedBody = await observed.json()
          expect(observedBody).toMatchObject({
            ok: true,
            run: {
              execution: {
                state: 'completed',
                counters: {
                  acceptedAssignments: 3,
                  activeAssignments: 0,
                  failedAssignments: 0,
                  staleAssignments: 0,
                  workUnitsTotal: 3,
                },
                closeouts: [
                  { terminalState: 'accepted' },
                  { terminalState: 'accepted' },
                  { terminalState: 'accepted' },
                ],
              },
              privateMaterialExcluded: true,
              runRef: startOutput.run.runRef,
              status: 'claimed_by_pylon',
            },
          })
          expect(observedBody).not.toHaveProperty('run.ownerUserId')
          expect(observedBody).not.toHaveProperty('run.requestFingerprint')
          expect(observedBody).not.toHaveProperty('run.idempotencyKey')

          const runRows = await sql<
            Array<{ status: string; count: number }>
          >`
            SELECT min(status) AS status, count(*)::int AS count
            FROM sarah_fleet_run_requests
            WHERE run_ref = ${startOutput.run.runRef}
          `
          const leaseRows = await sql<
            Array<{ state: string; count: number }>
          >`
            SELECT min(state) AS state, count(*)::int AS count
            FROM sarah_fleet_run_intake_leases
            WHERE run_ref = ${startOutput.run.runRef}
          `
          const workUnitRows = await sql<Array<{ count: number }>>`
            SELECT count(*)::int AS count
            FROM sarah_fleet_run_work_units
            WHERE run_ref = ${startOutput.run.runRef}
          `
          const eventRows = await sql<
            Array<{
              event_kind: string
              event_ref: string
              sequence: number
            }>
          >`
            SELECT event_kind, event_ref, sequence::int AS sequence
            FROM sarah_fleet_run_execution_events
            WHERE run_ref = ${startOutput.run.runRef}
            ORDER BY sequence
          `
          const changelogRows = await sql<
            Array<{ post_image_json: unknown }>
          >`
            SELECT post_image_json
            FROM khala_sync_changelog
            WHERE scope = ${syncScope}
            ORDER BY version
          `
          const attemptRows = await sql<
            Array<{
              state: string
              worker_kind: string
              usage_truth: string
              evidence_complete: boolean
            }>
          >`
            SELECT state, worker_kind, usage_truth,
                   jsonb_array_length(artifact_refs_json::jsonb) > 0
                     AND jsonb_array_length(proof_refs_json::jsonb) > 0
                     AND jsonb_array_length(authority_receipt_refs_json::jsonb) > 0
                     AND jsonb_array_length(
                       verification_json::jsonb -> 'evidenceRefs'
                     ) > 0 AS evidence_complete
            FROM sarah_fleet_run_attempts
            WHERE run_ref = ${startOutput.run.runRef}
            ORDER BY worker_kind
          `
          expect(runRows).toEqual([{ status: 'claimed_by_pylon', count: 1 }])
          expect(leaseRows).toEqual([{ state: 'accepted', count: 1 }])
          expect(workUnitRows).toEqual([{ count: 3 }])
          expect(eventRows.map(row => row.sequence)).toEqual(
            Array.from({ length: eventRows.length }, (_, index) => index + 1),
          )
          expect(new Set(eventRows.map(row => row.event_ref)).size).toBe(
            eventRows.length,
          )
          expect(
            eventRows.reduce<Record<string, number>>((counts, row) => {
              counts[row.event_kind] = (counts[row.event_kind] ?? 0) + 1
              return counts
            }, {}),
          ).toEqual({
            approval_requested: 1,
            run_started: 1,
            run_terminal: 1,
            work_progress: 3,
            work_terminal: 3,
          })
          expect(approvalCallbackFailures).toEqual([])
          expect(steeringTransportFailures).toEqual([])
          expect(
            canonicalJson(
              steeringTransportBodies.filter(
                value =>
                  typeof value === 'object' &&
                  value !== null &&
                  (value as { method?: unknown }).method === 'POST',
              ),
            ),
          ).not.toContain(PRIVATE_STEER_BODY)
          expect(canonicalJson(changelogRows)).not.toContain(PRIVATE_STEER_BODY)
          expect(attemptRows).toEqual([
            {
              evidence_complete: true,
              state: 'succeeded',
              usage_truth: 'exact',
              worker_kind: 'claude',
            },
            {
              evidence_complete: true,
              state: 'succeeded',
              usage_truth: 'exact',
              worker_kind: 'codex',
            },
            {
              evidence_complete: true,
              state: 'succeeded',
              usage_truth: 'not_measured',
              worker_kind: 'grok',
            },
          ])
        } finally {
          await poller?.close().catch(() => undefined)
          await activation?.close().catch(() => undefined)
          await sql.end({ timeout: 5 }).catch(() => undefined)
          await pg.stop().catch(() => undefined)
          await rm(sessionIndexPath, { force: true })
          await rm(root, { force: true, recursive: true })
          if (savedAccountLinkTestMode === undefined) {
            delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
          } else {
            process.env.SARAH_ACCOUNT_LINK_TEST_MODE =
              savedAccountLinkTestMode
          }
          if (savedSessionIndexPath === undefined) {
            delete process.env.SARAH_SESSION_INDEX_PATH
          } else {
            process.env.SARAH_SESSION_INDEX_PATH = savedSessionIndexPath
          }
        }
      },
      120_000,
    )
  },
)
