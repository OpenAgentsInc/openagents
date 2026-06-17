import { describe, expect, test } from 'vitest'

import {
  buildForgeRemoteSessionBridgeInput,
  projectForgeRemoteSessionBridge,
} from './remote-session-bridge'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-18T00:20:00.000Z',
  snapshotRef: 'remote-session-bridge-snapshot.public.work_1',
  versionRef: 'remote-session-bridge-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge remote session bridge projection', () => {
  test('projects public remote bridge evidence as refs-only non-authoritative state', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      entries: [
        {
          bridgeRef: 'remote-bridge.public.autopilot_control',
          controllerRefs: ['controller.public.browser'],
          freshness: 'fresh',
          heartbeatRefs: ['heartbeat.public.remote_session.ok'],
          permissionRefs: ['permission.public.remote_control.read_only'],
          policyRefs: ['policy.public.remote_bridge.read_only'],
          protocolRefs: ['protocol.public.autopilot_control.v1'],
          sessionRefs: ['remote-session.public.work_1'],
          state: 'ready',
          transportRefs: ['transport.public.websocket.bridge'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      connected: 0,
      ready: 1,
      reconnecting: 0,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      logStreamingAuthority: false,
      publicClaimAuthority: false,
      remoteCommandAuthority: false,
      remoteHostInspectAuthority: false,
      remoteSessionControlAuthority: false,
      remoteSessionOpenAuthority: false,
      remoteSessionReconnectAuthority: false,
      remoteSessionTerminateAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing remote bridge state as empty', () => {
    const view = projectForgeRemoteSessionBridge({
      generatedAt: '2026-06-18T00:20:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale bridge evidence', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      entries: [
        {
          bridgeRef: 'remote-bridge.public.stale',
          freshness: 'stale',
          policyRefs: ['policy.public.remote_bridge.ready'],
          protocolRefs: ['protocol.public.remote_bridge'],
          state: 'ready',
          transportRefs: ['transport.public.remote_bridge'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.work_1:stale-remote-bridge-evidence:remote-bridge.public.stale',
    )
  })

  test('blocks ready bridge state without transport protocol and policy refs', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      entries: [
        {
          bridgeRef: 'remote-bridge.public.no_readiness',
          freshness: 'fresh',
          state: 'ready',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.work_1:remote-bridge-readiness-missing:remote-bridge.public.no_readiness',
    )
  })

  test('blocks reconnecting state without reconnect refs', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      entries: [
        {
          bridgeRef: 'remote-bridge.public.reconnecting',
          freshness: 'fresh',
          state: 'reconnecting',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.work_1:reconnect-ref-missing:remote-bridge.public.reconnecting',
    )
  })

  test('blocks controller refs without permission refs', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      entries: [
        {
          bridgeRef: 'remote-bridge.public.controller',
          controllerRefs: ['controller.public.browser'],
          freshness: 'fresh',
          policyRefs: ['policy.public.remote_bridge.read_only'],
          protocolRefs: ['protocol.public.remote_bridge'],
          state: 'ready',
          transportRefs: ['transport.public.remote_bridge'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.work_1:controller-permission-ref-missing:remote-bridge.public.controller',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeRemoteSessionBridge({
      generatedAt: '2026-06-18T00:20:00.000Z',
      entries: [
        {
          bridgeRef: 'remote-bridge.public.no_snapshot',
          freshness: 'fresh',
          policyRefs: ['policy.public.remote_bridge.ready'],
          protocolRefs: ['protocol.public.remote_bridge'],
          state: 'ready',
          transportRefs: ['transport.public.remote_bridge'],
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.no_snapshot:missing-remote-session-bridge-snapshot-ref',
    )
  })

  test('omits unsafe private remote bridge material before projection', () => {
    const view = projectForgeRemoteSessionBridge({
      ...baseInput,
      blockerRefs: [
        'remote-bridge-blocker.public.safe',
        'raw remote /Users/christopher/remote.log',
      ],
      entries: [
        {
          bridgeRef: 'remote-bridge.public.safe',
          controllerRefs: ['controller.public.safe', 'raw command sk-private'],
          freshness: 'fresh',
          heartbeatRefs: ['heartbeat.public.safe', 'remote log /Users/christopher/log'],
          permissionRefs: ['permission.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          protocolRefs: ['protocol.public.safe'],
          reconnectRefs: ['reconnect.public.safe'],
          sessionRefs: ['remote-session.public.safe', 'ssh://private-host/session'],
          state: 'ready',
          transportRefs: ['transport.public.safe', 'raw transport /Users/christopher/socket'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.controllerRefs).toEqual(['controller.public.safe'])
    expect(view.entries[0]?.transportRefs).toEqual(['transport.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-remote-session-bridge-blocker:work.public.work_1:unsafe-remote-session-bridge-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw remote')
    expect(payload).not.toContain('raw command')
    expect(payload).not.toContain('remote log')
    expect(payload).not.toContain('ssh://')
    expect(payload).not.toContain('raw transport')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T00:21:00.000Z',
      remoteSessionBridge: {
        entries: [
          {
            bridgeRef: 'remote-bridge.public.work_2',
            freshness: 'fresh',
            policyRefs: ['policy.public.work_2'],
            protocolRefs: ['protocol.public.work_2'],
            state: 'ready',
            transportRefs: ['transport.public.work_2'],
          },
        ],
        snapshotRef: 'remote-session-bridge-snapshot.public.work_2',
        versionRef: 'remote-session-bridge-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeRemoteSessionBridgeInput(work)).toEqual({
      entries: [
        {
          bridgeRef: 'remote-bridge.public.work_2',
          freshness: 'fresh',
          policyRefs: ['policy.public.work_2'],
          protocolRefs: ['protocol.public.work_2'],
          state: 'ready',
          transportRefs: ['transport.public.work_2'],
        },
      ],
      generatedAt: '2026-06-18T00:21:00.000Z',
      snapshotRef: 'remote-session-bridge-snapshot.public.work_2',
      versionRef: 'remote-session-bridge-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
