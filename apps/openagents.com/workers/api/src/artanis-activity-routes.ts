import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  optionalInteger,
  parseJsonRecord,
  parseJsonStringArray,
  stringArrayFromUnknown,
} from './json-boundary'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

const DEFAULT_LIMIT = 12
const MAX_LIMIT = 50
const ACTIVE_ASSIGNMENT_STATES = new Set([
  'accepted',
  'assigned',
  'in_progress',
  'leased',
  'proof_ready',
  'queued',
  'running',
])
const TERMINAL_ASSIGNMENT_STATES = new Set([
  'accepted_closeout',
  'cancelled',
  'closeout_submitted',
  'expired',
  'failed',
  'rejected',
])
const PUBLIC_SAFE_REF_PATTERN =
  /^(\/api\/public\/|route:\/api\/public\/|https:\/\/(?:www\.)?(?:github\.com\/OpenAgentsInc\/openagents\/issues\/\d+|openagents\.com\/)|(?:agent|assignment|blocker|closeout|decision|event|issue|proof|repo|route|status|task|trace)\.(?:public|safe)\.|issue[#:_-]?\d+|OpenAgentsInc\/openagents(?:#\d+)?)/
const PRIVATE_MATERIAL_PATTERN =
  /(authorization|bearer|api[_-]?key|secret|token|wallet|email|prompt|diff|\/Users\/|\/home\/|auth\.json|private|credential)/i

type PylonRegistrationRow = Readonly<{
  pylon_ref: string
  status: string
  resource_mode: string | null
  capability_refs_json: string | null
  public_projection_json: string | null
  latest_heartbeat_at: string | null
  updated_at: string
}>

type PylonAssignmentRow = Readonly<{
  assignment_ref: string
  pylon_ref: string
  job_kind: string
  state: string
  task_refs_json: string | null
  acceptance_criteria_refs_json: string | null
  rejection_refs_json: string | null
  proof_refs_json: string | null
  closeout_refs_json: string | null
  public_projection_json: string | null
  updated_at: string
}>

type PylonEventRow = Readonly<{
  event_ref: string
  pylon_ref: string
  event_kind: string
  assignment_ref: string | null
  status: string
  public_projection_json: string | null
  created_at: string
}>

type BurnPaceRow = Readonly<{
  tokens_1h: number | null
  tokens_24h: number | null
  turns_1h: number | null
  turns_24h: number | null
}>

export type PublicArtanisActivityStore = Readonly<{
  listPylons: (limit: number) => Promise<ReadonlyArray<PylonRegistrationRow>>
  listAssignments: (
    limit: number,
  ) => Promise<ReadonlyArray<PylonAssignmentRow>>
  listEvents: (limit: number) => Promise<ReadonlyArray<PylonEventRow>>
  readBurnPace: (nowIso: string) => Promise<BurnPaceRow>
}>

export const PublicArtanisFleetSummary = S.Struct({
  onlineNow: S.Int,
  registeredTotal: S.Int,
  codexReady: S.Int,
  claudeReady: S.Int,
  capacityAvailable: S.Int,
  genericAgents: S.Array(
    S.Struct({
      agentId: S.String,
      family: S.Literals(['codex', 'claude', 'other']),
      onlineNow: S.Boolean,
      capacityAvailable: S.Int,
      status: S.String,
    }),
  ),
})

export const PublicArtanisActiveAssignment = S.Struct({
  assignmentId: S.String,
  agentId: S.String,
  workerFamily: S.Literals(['codex', 'claude', 'other']),
  state: S.String,
  repo: S.NullOr(S.String),
  publicIssue: S.NullOr(S.String),
  sourceRefs: S.Array(S.String),
  updatedAt: S.String,
})

export const PublicArtanisDecision = S.Struct({
  decisionId: S.String,
  kind: S.String,
  status: S.String,
  agentId: S.String,
  assignmentId: S.NullOr(S.String),
  publicIssue: S.NullOr(S.String),
  sourceRefs: S.Array(S.String),
  observedAt: S.String,
})

export const PublicArtanisBurnPace = S.Struct({
  tokensLastHour: S.Int,
  tokensLast24h: S.Int,
  turnsLastHour: S.Int,
  turnsLast24h: S.Int,
  sourceRefs: S.Array(S.String),
})

export const PublicArtanisFailureMode = S.Struct({
  modeRef: S.String,
  count: S.Int,
  exampleSourceRefs: S.Array(S.String),
})

export const PublicArtanisActivityResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_artanis_activity.v1'),
  generatedAt: S.String,
  sourceUrl: S.Literal('https://openagents.com/api/public/artanis/activity'),
  staleness: PublicProjectionStalenessContract,
  fleet: PublicArtanisFleetSummary,
  activeAssignments: S.Array(PublicArtanisActiveAssignment),
  recentDecisions: S.Array(PublicArtanisDecision),
  burnPace: PublicArtanisBurnPace,
  failureModes: S.Array(PublicArtanisFailureMode),
  safety: S.Struct({
    redaction: S.Literal('public_safe_summary_only'),
    excludedFields: S.Array(S.String),
  }),
})
export type PublicArtanisActivityResponse =
  typeof PublicArtanisActivityResponse.Type

const safeLimit = (value: string | null): number | undefined => {
  if (value === null) {
    return DEFAULT_LIMIT
  }

  const parsed = optionalInteger(value)
  return parsed !== undefined && parsed >= 1 && parsed <= MAX_LIMIT
    ? parsed
    : undefined
}

const publicSafeRefs = (
  refs: ReadonlyArray<string>,
  limit = 6,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()))]
    .filter(ref => ref !== '')
    .filter(ref => PUBLIC_SAFE_REF_PATTERN.test(ref))
    .filter(ref => !PRIVATE_MATERIAL_PATTERN.test(ref))
    .slice(0, limit)

const publicSafeRefsFromProjection = (
  projection: Record<string, unknown> | undefined,
): ReadonlyArray<string> => {
  if (projection === undefined) {
    return []
  }

  return publicSafeRefs([
    ...stringArrayFromUnknown(projection.sourceRefs),
    ...stringArrayFromUnknown(projection.taskRefs),
    ...stringArrayFromUnknown(projection.proofRefs),
    ...stringArrayFromUnknown(projection.publicRefs),
  ])
}

const publicSafeRefsFromAssignment = (
  row: PylonAssignmentRow,
): ReadonlyArray<string> =>
  publicSafeRefs([
    ...parseJsonStringArray(row.task_refs_json),
    ...parseJsonStringArray(row.acceptance_criteria_refs_json),
    ...parseJsonStringArray(row.proof_refs_json),
    ...parseJsonStringArray(row.closeout_refs_json),
    ...publicSafeRefsFromProjection(parseJsonRecord(row.public_projection_json)),
  ])

const familyFromRefs = (
  refs: ReadonlyArray<string>,
): 'codex' | 'claude' | 'other' => {
  const text = refs.join(' ').toLowerCase()
  if (text.includes('codex')) {
    return 'codex'
  }
  if (text.includes('claude')) {
    return 'claude'
  }
  return 'other'
}

const capacityAvailableFromProjection = (
  projection: Record<string, unknown> | undefined,
): number => {
  const refs = stringArrayFromUnknown(projection?.capacityRefs)
  const counted = refs
    .map(ref =>
      ref.match(/^capacity\.coding\.(?:codex|claude)\.available=(\d+)$/),
    )
    .map(match => (match === null ? 0 : Number(match[1])))
    .filter(value => Number.isInteger(value) && value > 0)

  return counted.reduce((sum, value) => sum + value, 0)
}

const pylonOnline = (row: PylonRegistrationRow): boolean => {
  const status = row.status.toLowerCase()
  return status === 'available' || status === 'online' || status === 'ready'
}

const repoFromRefs = (refs: ReadonlyArray<string>): string | null =>
  refs.some(ref => ref.includes('OpenAgentsInc/openagents'))
    ? 'OpenAgentsInc/openagents'
    : null

const issueFromRefs = (refs: ReadonlyArray<string>): string | null => {
  const text = refs.join(' ')
  const match =
    text.match(/github\.com\/OpenAgentsInc\/openagents\/issues\/(\d+)/) ??
    text.match(/OpenAgentsInc\/openagents#(\d+)/) ??
    text.match(/issue[#:_-]?(\d+)/i)

  return match === null ? null : `#${match[1]}`
}

const safeStatus = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .slice(0, 64) || 'unknown'

const agentIdsForPylons = (
  rows: ReadonlyArray<PylonRegistrationRow>,
): Map<string, string> => {
  const counts = { claude: 0, codex: 0, other: 0 }
  return new Map(
    rows.map(row => {
      const projection = parseJsonRecord(row.public_projection_json)
      const family = familyFromRefs([
        ...parseJsonStringArray(row.capability_refs_json),
        ...stringArrayFromUnknown(projection?.capacityRefs),
      ])
      counts[family] += 1
      return [
        row.pylon_ref,
        `${family === 'other' ? 'Agent' : family[0]!.toUpperCase() + family.slice(1)}-${counts[family]}`,
      ]
    }),
  )
}

const assignmentId = (index: number): string => `assignment-${index + 1}`

const decisionId = (index: number): string => `decision-${index + 1}`

const maybeAssignmentId = (
  assignments: ReadonlyArray<PylonAssignmentRow>,
  assignmentRef: string | null,
): string | null => {
  if (assignmentRef === null) {
    return null
  }

  const index = assignments.findIndex(
    assignment => assignment.assignment_ref === assignmentRef,
  )
  return index < 0 ? null : assignmentId(index)
}

const isoHoursBefore = (nowIso: string, hours: number): string => {
  const nowMs = Date.parse(nowIso)
  if (Number.isNaN(nowMs)) {
    return nowIso
  }

  return epochMillisToIsoTimestamp(nowMs - hours * 60 * 60 * 1000)
}

const buildFailureModes = (
  assignments: ReadonlyArray<PylonAssignmentRow>,
  events: ReadonlyArray<PylonEventRow>,
): ReadonlyArray<typeof PublicArtanisFailureMode.Type> => {
  const counts = new Map<string, { count: number; refs: ReadonlyArray<string> }>()
  const add = (modeRef: string, refs: ReadonlyArray<string>) => {
    const current = counts.get(modeRef)
    counts.set(modeRef, {
      count: (current?.count ?? 0) + 1,
      refs: publicSafeRefs([...(current?.refs ?? []), ...refs], 3),
    })
  }

  assignments.forEach(row => {
    if (row.rejection_refs_json !== null) {
      parseJsonStringArray(row.rejection_refs_json).forEach(ref => {
        const safe = publicSafeRefs([ref])
        if (safe.length > 0) {
          add(safe[0]!, publicSafeRefsFromAssignment(row))
        }
      })
    }
    if (TERMINAL_ASSIGNMENT_STATES.has(row.state) && row.state !== 'closeout_submitted') {
      add(`status.public.artanis.assignment.${safeStatus(row.state)}`, publicSafeRefsFromAssignment(row))
    }
  })

  events
    .filter(row => row.status.toLowerCase().includes('fail'))
    .forEach(row => {
      add(`status.public.artanis.event.${safeStatus(row.event_kind)}`, publicSafeRefsFromProjection(parseJsonRecord(row.public_projection_json)))
    })

  return [...counts.entries()]
    .map(([modeRef, value]) => ({
      count: value.count,
      exampleSourceRefs: value.refs,
      modeRef,
    }))
    .sort((left, right) => right.count - left.count || left.modeRef.localeCompare(right.modeRef))
    .slice(0, 8)
}

export const buildPublicArtanisActivity = async (
  store: PublicArtanisActivityStore,
  input: Readonly<{ limit: number; nowIso: string }>,
): Promise<PublicArtanisActivityResponse> => {
  const [pylons, assignments, events, burnPace] = await Promise.all([
    store.listPylons(input.limit),
    store.listAssignments(input.limit),
    store.listEvents(input.limit),
    store.readBurnPace(input.nowIso),
  ])
  const agentIds = agentIdsForPylons(pylons)
  const familyByPylon = new Map(
    pylons.map(row => {
      const projection = parseJsonRecord(row.public_projection_json)
      return [
        row.pylon_ref,
        familyFromRefs([
          ...parseJsonStringArray(row.capability_refs_json),
          ...stringArrayFromUnknown(projection?.capacityRefs),
        ]),
      ] as const
    }),
  )
  const activeAssignments = assignments
    .filter(row => ACTIVE_ASSIGNMENT_STATES.has(row.state))
    .map((row, index) => {
      const refs = publicSafeRefsFromAssignment(row)
      const family = familyByPylon.get(row.pylon_ref) ?? 'other'
      return {
        agentId: agentIds.get(row.pylon_ref) ?? 'Agent-1',
        assignmentId: assignmentId(index),
        publicIssue: issueFromRefs(refs),
        repo: repoFromRefs(refs),
        sourceRefs: refs,
        state: safeStatus(row.state),
        updatedAt: row.updated_at,
        workerFamily: family,
      }
    })

  return {
    activeAssignments,
    burnPace: {
      sourceRefs: ['route:/api/public/khala-tokens-served'],
      tokensLast24h: burnPace.tokens_24h ?? 0,
      tokensLastHour: burnPace.tokens_1h ?? 0,
      turnsLast24h: burnPace.turns_24h ?? 0,
      turnsLastHour: burnPace.turns_1h ?? 0,
    },
    failureModes: buildFailureModes(assignments, events),
    fleet: {
      capacityAvailable: pylons.reduce(
        (sum, row) =>
          sum +
          capacityAvailableFromProjection(parseJsonRecord(row.public_projection_json)),
        0,
      ),
      claudeReady: pylons.filter(
        row => familyByPylon.get(row.pylon_ref) === 'claude' && pylonOnline(row),
      ).length,
      codexReady: pylons.filter(
        row => familyByPylon.get(row.pylon_ref) === 'codex' && pylonOnline(row),
      ).length,
      genericAgents: pylons.map(row => {
        const projection = parseJsonRecord(row.public_projection_json)
        return {
          agentId: agentIds.get(row.pylon_ref) ?? 'Agent-1',
          capacityAvailable: capacityAvailableFromProjection(projection),
          family: familyByPylon.get(row.pylon_ref) ?? 'other',
          onlineNow: pylonOnline(row),
          status: safeStatus(row.status),
        }
      }),
      onlineNow: pylons.filter(pylonOnline).length,
      registeredTotal: pylons.length,
    },
    generatedAt: input.nowIso,
    recentDecisions: events.map((row, index) => {
      const refs = publicSafeRefsFromProjection(
        parseJsonRecord(row.public_projection_json),
      )
      return {
        agentId: agentIds.get(row.pylon_ref) ?? 'Agent-1',
        assignmentId: maybeAssignmentId(assignments, row.assignment_ref),
        decisionId: decisionId(index),
        kind: safeStatus(row.event_kind),
        observedAt: row.created_at,
        publicIssue: issueFromRefs(refs),
        sourceRefs: refs,
        status: safeStatus(row.status),
      }
    }),
    safety: {
      excludedFields: [
        'owner_agent_user_id',
        'owner_agent_credential_id',
        'owner_agent_token_prefix',
        'event_body_json',
        'raw_prompts',
        'diffs',
        'local_paths',
        'emails',
        'wallet_material',
      ],
      redaction: 'public_safe_summary_only',
    },
    schemaVersion: 'openagents.public_artanis_activity.v1',
    sourceUrl: 'https://openagents.com/api/public/artanis/activity',
    staleness: liveAtReadStaleness([
      'pylon_api_registrations',
      'pylon_api_assignments',
      'pylon_api_events',
      'token_usage_events',
      'agent_traces',
    ]),
  }
}

export const makeD1PublicArtanisActivityStore = (
  db: D1Database,
): PublicArtanisActivityStore => ({
  listAssignments: async limit => {
    const rows = await db
      .prepare(
        `SELECT assignment_ref, pylon_ref, job_kind, state, task_refs_json,
                acceptance_criteria_refs_json, rejection_refs_json, proof_refs_json,
                closeout_refs_json, public_projection_json, updated_at
           FROM pylon_api_assignments
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<PylonAssignmentRow>()
    return rows.results
  },
  listEvents: async limit => {
    const rows = await db
      .prepare(
        `SELECT event_ref, pylon_ref, event_kind, assignment_ref, status,
                public_projection_json, created_at
           FROM pylon_api_events
          WHERE archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<PylonEventRow>()
    return rows.results
  },
  listPylons: async limit => {
    const rows = await db
      .prepare(
        `SELECT pylon_ref, status, resource_mode, capability_refs_json,
                public_projection_json, latest_heartbeat_at, updated_at
           FROM pylon_api_registrations
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<PylonRegistrationRow>()
    return rows.results
  },
  readBurnPace: async nowIso => {
    const oneHourAgoIso = isoHoursBefore(nowIso, 1)
    const oneDayAgoIso = isoHoursBefore(nowIso, 24)
    const row = await db
      .prepare(
        `SELECT
            COALESCE(SUM(CASE WHEN observed_at >= ? THEN total_tokens ELSE 0 END), 0) AS tokens_1h,
            COALESCE(SUM(CASE WHEN observed_at >= ? THEN total_tokens ELSE 0 END), 0) AS tokens_24h,
            COALESCE(SUM(CASE WHEN observed_at >= ? THEN 1 ELSE 0 END), 0) AS turns_1h,
            COALESCE(SUM(CASE WHEN observed_at >= ? THEN 1 ELSE 0 END), 0) AS turns_24h
           FROM token_usage_events
          WHERE usage_truth = 'exact'
            AND (
              demand_kind IN ('internal', 'own_capacity')
              OR account_ref = 'agent:artanis'
              OR demand_source IN ('khala_coding_delegation', 'glm-pool-heartbeat', 'heartbeat')
            )`,
      )
      .bind(oneHourAgoIso, oneDayAgoIso, oneHourAgoIso, oneDayAgoIso)
      .first<BurnPaceRow>()

    return (
      row ?? {
        tokens_1h: 0,
        tokens_24h: 0,
        turns_1h: 0,
        turns_24h: 0,
      }
    )
  },
})

type PublicArtanisActivityRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  nowIso?: () => string
  store?: PublicArtanisActivityStore
}>

const routeError = () =>
  noStoreJsonResponse({ error: 'internal_server_error' }, { status: 500 })

export const handlePublicArtanisActivityApi = (
  request: Request,
  input: PublicArtanisActivityRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const limit = safeLimit(new URL(request.url).searchParams.get('limit'))
  if (limit === undefined) {
    return Effect.succeed(
      noStoreJsonResponse(
        { error: 'invalid_limit', min: 1, max: MAX_LIMIT },
        { status: 400 },
      ),
    )
  }

  const store =
    input.store ??
    (input.OPENAGENTS_DB === undefined
      ? undefined
      : makeD1PublicArtanisActivityStore(input.OPENAGENTS_DB))

  if (store === undefined) {
    return Effect.succeed(routeError())
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp

  return Effect.tryPromise(() =>
    buildPublicArtanisActivity(store, { limit, nowIso: nowIso() }),
  ).pipe(
    Effect.map(payload => noStoreJsonResponse(payload)),
    Effect.catch(() => Effect.succeed(routeError())),
  )
}
