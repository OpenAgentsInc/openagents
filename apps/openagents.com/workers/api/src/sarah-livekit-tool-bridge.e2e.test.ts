import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_TOOL_PROPOSAL_PATH,
  SARAH_LIVEKIT_TOOL_STATE_PATH,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitToolProposal,
} from '@openagentsinc/audio-contract'
import type { SarahRealtimeVoiceStore } from '@openagentsinc/khala-sync-server'
import { describe, expect, test } from 'vitest'

import { makeSarahLiveKitControlClient } from '../../../../sarah-livekit-agent/src/control-client'
import {
  flushSarahLiveKitToolControl,
  makeSarahRealtimeBridgeData,
  makeSarahRealtimeWebSocketHandlers,
  pollSarahLiveKitToolControl,
} from './cloudrun/sarah-realtime-bridge'
import {
  handleSarahLiveKitWorkerToolProposal,
  handleSarahLiveKitWorkerToolState,
} from './sarah-livekit-worker-routes'

const controlRoot = 'A'.repeat(64)
const dispatch = {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
  sessionRef: 'session:one',
  generation: 1,
  roomRef: 'room:one',
  roomEpoch: 1,
  participantRef: 'owner:one',
  sarahParticipantRef: 'principal.sarah',
  sarahPresenceLeaseRef: 'presence:one',
  capabilityProfile: 'omega_editor',
  roomContext: { kind: 'private' },
} as const

describe('Sarah LiveKit private tool bridge', () => {
  test('waits for an authenticated Omega decision and outcome before returning success', async () => {
    let nowMs = 2_000_000_000_000
    let proposal: SarahLiveKitToolProposal | undefined
    const proposalAttempts: Array<{
      proposalDigest: string
      expiresAt: string
    }> = []
    let state:
      | { state: 'waiting_decision' }
      | { state: 'declined' }
      | { state: 'execute_sent' }
      | { state: 'outcome'; outcomeRef: string; ok: boolean; summary: string } =
      {
        state: 'waiting_decision',
      }
    const store = {
      proposeLiveKitTool: async (input: {
        proposalRef: string
        proposalDigest: string
        command: SarahLiveKitToolProposal['command']
        expiresAt: string
      }) => {
        proposalAttempts.push({
          proposalDigest: input.proposalDigest,
          expiresAt: input.expiresAt,
        })
        if (proposal !== undefined) return proposal
        proposal = {
          proposalRef: input.proposalRef,
          proposalDigest: input.proposalDigest,
          command: input.command,
          confirmationRequired: true,
          expiresAtMs: Date.parse(input.expiresAt),
        }
        return proposal
      },
      readLiveKitToolProposals: async () =>
        proposal !== undefined && state.state === 'waiting_decision'
          ? [proposal]
          : [],
      decideLiveKitTool: async (input: { decision: 'confirm' | 'decline' }) => {
        if (proposal === undefined || state.state !== 'waiting_decision') {
          throw new Error('proposal unavailable')
        }
        state =
          input.decision === 'confirm'
            ? { state: 'execute_sent' }
            : { state: 'declined' }
        return input.decision === 'confirm' ? proposal : undefined
      },
      recordLiveKitToolOutcome: async (input: {
        outcomeRef: string
        ok: boolean
        summary: string
      }) => {
        if (state.state !== 'execute_sent')
          throw new Error('execution was not approved')
        state = {
          state: 'outcome',
          outcomeRef: input.outcomeRef,
          ok: input.ok,
          summary: input.summary,
        }
      },
      readLiveKitToolState: async () => state,
    } as unknown as SarahRealtimeVoiceStore
    const dependencies = {
      controlRoot: () => controlRoot,
      creditMsatPerMillionTokens: () => 100_000,
      now: () => nowMs,
      openStore: async () => ({ store, close: async () => undefined }),
      cleanup: async () => undefined,
    }
    const fetcher: typeof fetch = async (input, init) => {
      const request = new Request(input, init)
      const path = new URL(request.url).pathname
      if (path === SARAH_LIVEKIT_TOOL_PROPOSAL_PATH) {
        return handleSarahLiveKitWorkerToolProposal(dependencies, request, {})
      }
      if (path === SARAH_LIVEKIT_TOOL_STATE_PATH) {
        return handleSarahLiveKitWorkerToolState(dependencies, request, {})
      }
      return Response.json({ error: 'not_found' }, { status: 404 })
    }
    const worker = makeSarahLiveKitControlClient(
      {
        baseUrl: 'https://openagents.com',
        workerRef: 'worker:one',
        controlRoot,
      },
      fetcher,
    )
    const created = await worker.proposeTool(dispatch, {
      sessionRef: dispatch.sessionRef,
      generation: dispatch.generation,
      jobRef: 'job:one',
      eventRef: 'tool:event:one',
      providerCallRef: 'call:one',
      command: {
        _tag: 'start_agent_thread',
        message: 'Inspect the current test failure.',
        presentation: 'foreground',
      },
    })
    nowMs += 5_000
    const responseLossReplay = await worker.proposeTool(dispatch, {
      sessionRef: dispatch.sessionRef,
      generation: dispatch.generation,
      jobRef: 'job:one',
      eventRef: 'tool:event:one',
      providerCallRef: 'call:one',
      command: {
        _tag: 'start_agent_thread',
        message: 'Inspect the current test failure.',
        presentation: 'foreground',
      },
    })
    expect(responseLossReplay).toEqual(created)
    expect(proposalAttempts).toHaveLength(2)
    expect(proposalAttempts[1]).not.toEqual(proposalAttempts[0])

    expect(
      await worker.readToolState(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: 'job:one',
        proposalRef: created.proposalRef,
        proposalDigest: created.proposalDigest,
      }),
    ).toMatchObject({ state: 'waiting_decision' })

    const session = {
      sessionRef: dispatch.sessionRef,
      ownerUserId: 'owner:one',
      ownerActorRef: 'agent:owner:one',
      deviceRef: 'device:one',
      threadRef: 'thread:one',
      generation: dispatch.generation,
      disclosureRef: 'disclosure:one',
      clientProfile: 'omega_editor' as const,
      transportKind: 'livekit_room_v1' as const,
      creditMode: 'metered' as const,
      entitlementRef: null,
      admissionCohortRef: 'sarah_voice_cohort:alpha_v1',
      state: 'connected' as const,
      reservedMsat: 1_000,
      chargedMsat: 0,
      ticketExpiresAt: '2033-05-18T03:34:00.000Z',
      sessionExpiresAt: '2033-05-18T03:35:00.000Z',
      settlementReceiptRef: null,
    }
    const data = makeSarahRealtimeBridgeData({
      session,
      apiKey: 'unused',
      safetyIdentifier: 'unused',
      creditMsatPerMillionTokens: 100_000,
      store,
      closeStore: async () => undefined,
      tasks: {} as never,
    })
    data.helloReceived = true
    const controls: Array<Record<string, unknown>> = []
    const socket = {
      data,
      send: (message: string) =>
        controls.push(JSON.parse(message) as Record<string, unknown>),
      close: () => undefined,
    }
    const identity = {
      ownerRef: session.ownerUserId,
      deviceRef: session.deviceRef,
      threadRef: session.threadRef,
      sessionRef: session.sessionRef,
      generation: session.generation,
    }
    await pollSarahLiveKitToolControl(socket as never)
    expect(controls.at(-1)).toMatchObject({
      _tag: 'tool_proposal',
      proposalRef: created.proposalRef,
      command: { _tag: 'start_agent_thread' },
    })

    const handlers = makeSarahRealtimeWebSocketHandlers()
    handlers.message(
      socket as never,
      JSON.stringify({
        schema: 'openagents.sarah.voice.v1',
        _tag: 'tool_decision',
        identity,
        sequence: 0,
        proposalRef: created.proposalRef,
        proposalDigest: created.proposalDigest,
        decision: 'confirm',
      }),
    )
    await flushSarahLiveKitToolControl(socket as never)
    expect(controls.at(-1)).toMatchObject({
      _tag: 'tool_execute',
      command: { _tag: 'start_agent_thread' },
    })
    expect(
      await worker.readToolState(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: 'job:one',
        proposalRef: created.proposalRef,
        proposalDigest: created.proposalDigest,
      }),
    ).toMatchObject({ state: 'execute_sent' })

    handlers.message(
      socket as never,
      JSON.stringify({
        schema: 'openagents.sarah.voice.v1',
        _tag: 'tool_outcome',
        identity,
        sequence: 1,
        proposalRef: created.proposalRef,
        proposalDigest: created.proposalDigest,
        outcomeRef: 'outcome:one',
        ok: true,
        summary: 'Omega accepted the new agent thread.',
      }),
    )
    await flushSarahLiveKitToolControl(socket as never)
    const finalState = await worker.readToolState(dispatch, {
      sessionRef: dispatch.sessionRef,
      generation: dispatch.generation,
      jobRef: 'job:one',
      proposalRef: created.proposalRef,
      proposalDigest: created.proposalDigest,
    })
    expect(finalState).toEqual({
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      state: 'outcome',
      outcomeRef: 'outcome:one',
      ok: true,
      summary: 'Omega accepted the new agent thread.',
    })
    expect(JSON.stringify({ proposal, state })).not.toMatch(
      /transcript|audio/iu,
    )
  })
})
