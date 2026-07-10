import {
  makeFleetRunAuthorityRepository,
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
import type {
  FleetRunSupervisorDispatchInput,
} from '../../../../pylon/src/orchestration/fleet-run-supervisor'
import { assertPublicProjectionSafe } from '../../../../pylon/src/state'
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

const FIXED_NOW_MS = Date.parse('2026-07-09T23:45:00.000Z')
const COMMIT = '03365073c0d96da42535ac74c6147c38e34368ed'
const OWNER_USER_ID = 'owner.public.c1'
const FOREIGN_OWNER_USER_ID = 'owner.public.foreign'
const AGENT_USER_ID = 'agent.public.c1'
const FOREIGN_AGENT_USER_ID = 'agent.public.foreign'
const PYLON_REF = 'pylon.public.c1'
const OWNER_TOKEN = 'oa_agent_owner_fixture_8637'
const FOREIGN_TOKEN = 'oa_agent_foreign_fixture_8637'

const fleetRequest = {
  objective: 'Implement and verify the bounded public issue.',
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
  workSource: { kind: 'issue_list', issueRefs: ['#8637'] },
  workerPolicy: { workerKind: 'codex', targetPreference: 'owner_local' },
  targetConcurrency: 1,
  idempotencyKey: 'c1-integrated-fixture-0001',
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
  capabilityRefs: ['capability.public.coding.codex'],
  clientProtocolVersion: '1',
  clientVersion: 'openagents.pylon@fixture',
  createdAt: new Date(FIXED_NOW_MS).toISOString(),
  displayName: 'C1 fixture Pylon',
  id: 'registration.public.c1',
  latestHeartbeatAt: new Date(FIXED_NOW_MS).toISOString(),
  latestHeartbeatStatus: 'online',
  latestCapacityRefs: ['capacity.coding.codex.available=1'],
  latestHealthRefs: [],
  latestLoadRefs: ['load.coding.codex.busy=0'],
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
  predicate: () => boolean,
  timeoutMs = 5_000,
): Promise<void> => {
  const deadline = performance.now() + timeoutMs
  while (!predicate()) {
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
      '#8637 composes operator start through durable claim, execution, and safe closeout',
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
        const dispatched: Array<FleetRunSupervisorDispatchInput> = []
        const transportBodies: Array<unknown> = []
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
          }
          const pylonRoutes = makePylonApiRoutes({
            agentStore: () => agentStore,
            fleetRunAuthority: fleetAuthority,
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
            openExecutor: async input => {
              const standing = await openPylonStandingFleetRunExecutor({
                bootstrap: summary,
                now: () => new Date(FIXED_NOW_MS + timeline.now()),
                pylonRef: input.pylonRef,
                runRef: input.runRef,
                startImmediately: false,
                tickIntervalMs: 300_000,
                clock: {
                  now: () => new Date(FIXED_NOW_MS + timeline.now()),
                  sleep: () => new Promise<void>(() => undefined),
                },
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
                      ])
                    },
                  },
                  livenessProbe: () => Promise.resolve('dead' as const),
                  planner: createPylonDurableFleetRunPlanner({ store }),
                  runner: {
                    dispatch: async dispatch => {
                      dispatched.push(dispatch)
                      timeline.advance(1_000)
                      return {
                        assignmentRef: 'assignment.public.c1.fixture',
                        lifecycle: [],
                        status: 'completed' as const,
                        summary: 'Bounded fixture work completed and verified.',
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
          const standing = standingExecutors[0]!
          await waitUntil(() =>
            standing.runtime.store
              .listWorkClaims({ runRef: startOutput.run.runRef })
              .some(
                claim =>
                  claim.assignmentRef === 'assignment.public.c1.fixture' &&
                  claim.state === 'closeout',
              ),
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
                completedAssignments: 1,
                failedAssignments: 0,
              },
              runRef: startOutput.run.runRef,
              state: 'completed',
            },
          })
          expect(dispatched).toHaveLength(1)
          expect(dispatched[0]).toMatchObject({
            accountRef: 'codex-fixture-isolated',
            run: { runRef: startOutput.run.runRef },
            workerKind: 'codex',
            workUnit: {
              baseCommit: COMMIT,
              number: 8637,
              repo: 'OpenAgentsInc/openagents',
            },
          })

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
          expect(runRows).toEqual([{ status: 'claimed_by_pylon', count: 1 }])
          expect(leaseRows).toEqual([{ state: 'accepted', count: 1 }])
          expect(workUnitRows).toEqual([{ count: 1 }])
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
