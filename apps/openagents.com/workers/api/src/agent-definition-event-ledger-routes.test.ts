import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { describe, expect, test } from 'vitest'

import type { AgentDefinitionStore } from './agent-definition-routes'
import type {
  AgentDefinitionRunRecord,
  AgentDefinitionRunStore,
} from './agent-definition-run-routes'
import {
  AGENT_TOKEN_PREFIX,
  type AgentCredentialLookup,
  type AgentCredentialRecord,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
  type AgentReissueSelector,
  type AgentReissueTarget,
  sha256Hex,
} from './agent-registration'
import {
  AGENT_DEFINITION_EVENT_LEDGER_HANDLED_STATE_TOOL_REF,
  AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
  handleAgentDefinitionEventLedgerGatewayRequest,
} from './agent-definition-event-ledger-routes'
import type {
  EventLedgerEntry,
  EventLedgerHandledState,
  EventLedgerStore,
} from './event-ledger'

class MemoryAgentRegistrationStore implements AgentRegistrationStore {
  readonly touchedCredentialIds: Array<string> = []

  constructor(
    private readonly lookupsByTokenHash: ReadonlyMap<string, AgentCredentialLookup>,
  ) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(
    tokenHash: string,
    _now: string,
  ): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookupsByTokenHash.get(tokenHash))
  }

  touchAgentCredential(credentialId: string, _lastUsedAt: string): Promise<void> {
    this.touchedCredentialIds.push(credentialId)

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }

  findAgentForReissue(
    _selector: AgentReissueSelector,
  ): Promise<AgentReissueTarget | undefined> {
    return Promise.resolve(undefined)
  }

  addAgentCredential(_record: AgentCredentialRecord): Promise<void> {
    return Promise.resolve()
  }
}

class MemoryDefinitionStore
  implements Pick<AgentDefinitionStore, 'readDefinition'> {
  constructor(
    private readonly definitions: ReadonlyArray<{
      ownerAgentUserId: string
      definition: AgentDefinition
    }>,
  ) {}

  readDefinition(
    ownerAgentUserId: string,
    definitionId: string,
  ): Promise<AgentDefinition | undefined> {
    return Promise.resolve(
      this.definitions.find(row =>
        row.ownerAgentUserId === ownerAgentUserId &&
        row.definition.id === definitionId,
      )?.definition,
    )
  }
}

class MemoryRunStore implements Pick<AgentDefinitionRunStore, 'readRun'> {
  constructor(
    private readonly runs: ReadonlyArray<{
      ownerAgentUserId: string
      run: Pick<AgentDefinitionRunRecord, 'definitionId' | 'runId'>
    }>,
  ) {}

  readRun(
    ownerAgentUserId: string,
    runId: string,
  ): Promise<AgentDefinitionRunRecord | undefined> {
    const run = this.runs.find(row =>
      row.ownerAgentUserId === ownerAgentUserId && row.run.runId === runId,
    )?.run

    return Promise.resolve(run as AgentDefinitionRunRecord | undefined)
  }
}

class MemoryEventLedgerStore
  implements Pick<EventLedgerStore, 'listOwnerEntries' | 'updateHandledState'> {
  constructor(private entries: ReadonlyArray<EventLedgerEntry>) {}

  listOwnerEntries(input: {
    handledStates?: ReadonlyArray<EventLedgerHandledState> | undefined
    limit: number
    ownerAgentUserId: string
    subjectRef?: string | undefined
  }): Promise<ReadonlyArray<EventLedgerEntry>> {
    return Promise.resolve(
      this.entries
        .filter(entry => entry.ownerAgentUserId === input.ownerAgentUserId)
        .filter(entry =>
          input.subjectRef === undefined
            ? true
            : entry.subjectRef === input.subjectRef,
        )
        .filter(entry =>
          input.handledStates === undefined ||
          input.handledStates.length === 0
            ? true
            : input.handledStates.includes(entry.handledState),
        )
        .slice(0, input.limit),
    )
  }

  updateHandledState(input: {
    entryId: string
    handledAt: string
    handledByDefinitionId: string
    handledByRunId: string
    handledReasonRef?: string | undefined
    handledState: EventLedgerHandledState
    ownerAgentUserId: string
  }): Promise<EventLedgerEntry | undefined> {
    let updated: EventLedgerEntry | undefined
    this.entries = this.entries.map(entry => {
      if (
        entry.ownerAgentUserId !== input.ownerAgentUserId ||
        entry.entryId !== input.entryId
      ) {
        return entry
      }

      updated = {
        ...entry,
        handledAt: input.handledAt,
        handledByDefinitionId: input.handledByDefinitionId,
        handledByRunId: input.handledByRunId,
        handledReasonRef: input.handledReasonRef ?? null,
        handledState: input.handledState,
        updatedAt: input.handledAt,
      }

      return updated
    })

    return Promise.resolve(updated)
  }
}

const lookup = (input: {
  readonly credentialId: string
  readonly tokenPrefix: string
  readonly userId: string
}): AgentCredentialLookup => ({
  user: {
    id: input.userId,
    kind: 'agent',
    displayName: `Agent ${input.userId}`,
    primaryEmail: null,
    avatarUrl: null,
    status: 'active',
    createdAt: '2026-07-04T00:00:00.000Z',
    updatedAt: '2026-07-04T00:00:00.000Z',
  },
  credentialId: input.credentialId,
  openauthUserId: null,
  profileMetadataJson: '{}',
  tokenPrefix: input.tokenPrefix,
})

const definition = (input: {
  allow?: ReadonlyArray<string>
  ownerAgentUserId?: string
  secretPolicy?: 'none' | 'owner_scoped_refs_only'
} = {}): AgentDefinition =>
  decodeAgentDefinition({
    schema: 'openagents.agent_definition.v1',
    id: 'agent_definition.public.event_ledger',
    ownerRef: `agent:${input.ownerAgentUserId ?? 'agent_user_owner'}`,
    name: 'Event Ledger Reader',
    slug: 'event-ledger-reader',
    goal: 'Read the private event ledger through the redacting gateway.',
    harness: { kind: 'codex' },
    toolset: {
      allow: input.allow ?? [
        AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
        AGENT_DEFINITION_EVENT_LEDGER_HANDLED_STATE_TOOL_REF,
      ],
      deny: [],
      ask: [],
      networkPolicy: 'owner_scoped',
      secretPolicy: input.secretPolicy ?? 'owner_scoped_refs_only',
    },
    triggers: [{ kind: 'manual', triggerRef: 'trigger.public.event_ledger.manual' }],
    lane: 'own_pylon',
    budget: { maxRunSeconds: 900, maxRunsPerDay: 3, maxCreditsPerDay: 0 },
    escalation: {
      channel: 'operator',
      askPolicy: {
        mode: 'operator_required',
        policyRef: 'policy.public.agent_definition.operator_required.v1',
      },
    },
    sourceRefs: ['github.issue.8213'],
    createdAt: '2026-07-04T00:00:00.000Z',
    updatedAt: '2026-07-04T00:00:00.000Z',
  })

const ledgerEntry = (
  input: Partial<EventLedgerEntry> = {},
): EventLedgerEntry => ({
  actorRef: 'github.user.AtlantisPleb',
  contentRef: 'github.comment.OpenAgentsInc/openagents.1001',
  createdAt: '2026-07-04T00:06:00.000Z',
  entryId: 'event_ledger.github.agent_user_owner.000000000001',
  eventType: 'issue_comment.created.mention',
  externalRef: 'github.delivery.delivery-8213',
  handledAt: null,
  handledByDefinitionId: null,
  handledByRunId: null,
  handledReasonRef: null,
  handledState: 'open',
  occurredAt: '2026-07-04T00:05:00.000Z',
  orderingKey: 'github:github.delivery.delivery-8213',
  orderingSequence: 1,
  ownerAgentUserId: 'agent_user_owner',
  ownerRef: 'agent:agent_user_owner',
  payloadSummary: {
    repositoryFullName: 'OpenAgentsInc/openagents',
    secret: 'Secret-ish payload summary must not leave the gateway.',
  },
  receivedAt: '2026-07-04T00:05:00.000Z',
  source: 'github',
  sourceRefs: [
    'github.delivery.delivery-8213',
    'github.comment.OpenAgentsInc/openagents.1001',
  ],
  subjectRef: 'github.repository.OpenAgentsInc/openagents.issue.8213',
  trainingConsent: false,
  updatedAt: '2026-07-04T00:06:00.000Z',
  ...input,
})

const makeDependencies = async (input: {
  definition?: AgentDefinition
  entries?: ReadonlyArray<EventLedgerEntry>
  runs?: ReadonlyArray<{
    ownerAgentUserId: string
    run: Pick<AgentDefinitionRunRecord, 'definitionId' | 'runId'>
  }>
} = {}) => {
  const ownerToken = `${AGENT_TOKEN_PREFIX}owner_gateway_test`
  const otherToken = `${AGENT_TOKEN_PREFIX}other_gateway_test`
  const agentStore = new MemoryAgentRegistrationStore(
    new Map([
      [
        await sha256Hex(ownerToken),
        lookup({
          credentialId: 'credential_owner',
          tokenPrefix: `${AGENT_TOKEN_PREFIX}owner`,
          userId: 'agent_user_owner',
        }),
      ],
      [
        await sha256Hex(otherToken),
        lookup({
          credentialId: 'credential_other',
          tokenPrefix: `${AGENT_TOKEN_PREFIX}other`,
          userId: 'agent_user_other',
        }),
      ],
    ]),
  )
  const activeDefinition = input.definition ?? definition()

  return {
    dependencies: {
      agentStore,
      definitionStore: new MemoryDefinitionStore([
        {
          definition: activeDefinition,
          ownerAgentUserId: 'agent_user_owner',
        },
      ]),
      eventLedgerStore: new MemoryEventLedgerStore(
        input.entries ?? [ledgerEntry()],
      ),
      nowIso: () => '2026-07-04T00:10:00.000Z',
      runStore: new MemoryRunStore(
        input.runs ?? [
          {
            ownerAgentUserId: 'agent_user_owner',
            run: {
              definitionId: activeDefinition.id,
              runId: 'agent_definition_run.public.touch_1',
            },
          },
        ],
      ),
    },
    otherToken,
    ownerToken,
  }
}

const request = (
  path: string,
  token: string,
  init: RequestInit = {},
): Request =>
  new Request(`https://openagents.test${path}`, {
    ...init,
    headers: {
      authorization: `Bearer ${token}`,
      ...(init.headers ?? {}),
    },
  })

describe('agent definition event-ledger gateway routes', () => {
  // background_agents.inbox.event_ledger_handled_gateway_redacted.v1
  test('reads owner-scoped ledger entries through the refs-only gateway', async () => {
    const { dependencies, ownerToken, otherToken } = await makeDependencies()
    const response = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger?state=open',
        ownerToken,
      ),
      dependencies,
    )
    const body = await response!.json() as Record<string, unknown>

    expect(response?.status).toBe(200)
    expect(body).toMatchObject({
      count: 1,
      definitionId: 'agent_definition.public.event_ledger',
      redaction: {
        policy: 'owner_scoped_refs_only',
        redactionClass: 'owner_scoped_refs',
      },
      toolRef: AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
    })
    expect(JSON.stringify(body)).toContain(
      'github.comment.OpenAgentsInc/openagents.1001',
    )
    expect(JSON.stringify(body)).not.toContain('Secret-ish')
    expect(JSON.stringify(body)).not.toContain('payloadSummary')

    const crossOwner = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger',
        otherToken,
      ),
      dependencies,
    )

    expect(crossOwner?.status).toBe(404)
  })

  test('redacts to state-only when a definition has no secret policy', async () => {
    const { dependencies, ownerToken } = await makeDependencies({
      definition: definition({ secretPolicy: 'none' }),
    })
    const response = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger',
        ownerToken,
      ),
      dependencies,
    )
    const body = await response!.json() as { entries: Array<Record<string, unknown>> }

    expect(response?.status).toBe(200)
    expect(body.entries[0]).toMatchObject({
      eventType: 'issue_comment.created.mention',
      handledState: 'open',
      redactionClass: 'state_only',
      source: 'github',
    })
    expect(body.entries[0]).not.toHaveProperty('actorRef')
    expect(body.entries[0]).not.toHaveProperty('contentRef')
    expect(body.entries[0]).not.toHaveProperty('externalRef')
    expect(body.entries[0]).not.toHaveProperty('sourceRefs')
    expect(body.entries[0]).not.toHaveProperty('subjectRef')
  })

  test('refuses reads when the definition toolset does not allow the gateway tool', async () => {
    const { dependencies, ownerToken } = await makeDependencies({
      definition: definition({ allow: ['tool.openagents.crm.read'] }),
    })
    const response = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger',
        ownerToken,
      ),
      dependencies,
    )
    const body = await response!.json() as Record<string, unknown>

    expect(response?.status).toBe(403)
    expect(body).toMatchObject({
      error: 'agent_definition_tool_not_authorized',
      reasonRef: 'reason.agent_definition.tool_not_in_allowlist',
      toolRef: AGENT_DEFINITION_EVENT_LEDGER_READ_TOOL_REF,
    })
  })

  test('updates handled-state only for a run from the same definition', async () => {
    const { dependencies, ownerToken } = await makeDependencies()
    const response = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger/handled-state',
        ownerToken,
        {
          body: JSON.stringify({
            entryId: 'event_ledger.github.agent_user_owner.000000000001',
            handledState: 'responded',
            reasonRef: 'reason.agent_definition.event_ledger.responded',
            runId: 'agent_definition_run.public.touch_1',
          }),
          method: 'POST',
        },
      ),
      dependencies,
    )
    const body = await response!.json() as { entries: Array<Record<string, unknown>> }

    expect(response?.status).toBe(200)
    expect(body.entries[0]).toMatchObject({
      handledAt: '2026-07-04T00:10:00.000Z',
      handledByDefinitionId: 'agent_definition.public.event_ledger',
      handledByRunId: 'agent_definition_run.public.touch_1',
      handledReasonRef: 'reason.agent_definition.event_ledger.responded',
      handledState: 'responded',
    })

    const wrongDefinition = await makeDependencies({
      runs: [
        {
          ownerAgentUserId: 'agent_user_owner',
          run: {
            definitionId: 'agent_definition.public.other',
            runId: 'agent_definition_run.public.other',
          },
        },
      ],
    })
    const rejected = await handleAgentDefinitionEventLedgerGatewayRequest(
      request(
        '/v1/agent-definitions/agent_definition.public.event_ledger/event-ledger/handled-state',
        wrongDefinition.ownerToken,
        {
          body: JSON.stringify({
            entryId: 'event_ledger.github.agent_user_owner.000000000001',
            handledState: 'ignored',
            runId: 'agent_definition_run.public.other',
          }),
          method: 'POST',
        },
      ),
      wrongDefinition.dependencies,
    )

    expect(rejected?.status).toBe(404)
  })
})
