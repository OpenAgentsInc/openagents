import { describe, expect, test } from 'vitest'

import {
  buildForgeBrowserDesktopIntegrationInput,
  projectForgeBrowserDesktopIntegration,
} from './browser-desktop-integration'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-18T00:00:00.000Z',
  snapshotRef: 'browser-desktop-integration-snapshot.public.work_1',
  versionRef: 'browser-desktop-integration-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge browser and desktop integration projection', () => {
  test('projects public browser and desktop evidence as refs-only non-authoritative state', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          browserRefs: ['browser.public.chrome'],
          companionRefs: ['companion.public.web'],
          deepLinkRefs: ['deep-link.public.desktop.open_run'],
          desktopAppRefs: ['desktop-app.public.autopilot'],
          extensionRefs: ['extension.public.openagents'],
          freshness: 'fresh',
          installRefs: ['install.public.desktop.autopilot'],
          integrationRef: 'browser-desktop.public.operator',
          notificationRefs: ['notification.public.desktop.review_ready'],
          permissionRefs: ['permission.public.notifications.granted'],
          policyRefs: ['policy.public.browser_desktop.operator_only'],
          state: 'ready',
          statusRefs: ['status.public.browser_desktop.connected'],
          surfaceRefs: ['surface.public.autopilot.desktop'],
          updateRefs: ['update.public.desktop.current'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      connected: 0,
      installed: 0,
      ready: 1,
      total: 1,
      unavailable: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      browserAutomationAuthority: false,
      deepLinkOpenAuthority: false,
      deploymentAuthority: false,
      desktopControlAuthority: false,
      extensionInstallAuthority: false,
      fileReadAuthority: false,
      notificationSendAuthority: false,
      permissionInspectAuthority: false,
      publicClaimAuthority: false,
      sessionInspectAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolExecutionAuthority: false,
      toolRoutingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing browser desktop state as empty', () => {
    const view = projectForgeBrowserDesktopIntegration({
      generatedAt: '2026-06-18T00:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale browser desktop evidence', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          integrationRef: 'browser-desktop.public.stale',
          policyRefs: ['policy.public.browser_desktop.ready'],
          state: 'ready',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:stale-browser-desktop-evidence:browser-desktop.public.stale',
    )
  })

  test('blocks ready surfaces without policy refs', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.no_policy',
          state: 'connected',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:browser-desktop-policy-ref-missing:browser-desktop.public.no_policy',
    )
  })

  test('blocks deep links without policy refs', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          deepLinkRefs: ['deep-link.public.desktop.open_run'],
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.deep_link',
          state: 'unknown',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:deep-link-policy-ref-missing:browser-desktop.public.deep_link',
    )
  })

  test('blocks notifications without permission refs', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.notification',
          notificationRefs: ['notification.public.desktop.review_ready'],
          policyRefs: ['policy.public.browser_desktop.notifications'],
          state: 'unknown',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:notification-permission-ref-missing:browser-desktop.public.notification',
    )
  })

  test('blocks installed surfaces without install or update refs', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.installed',
          policyRefs: ['policy.public.browser_desktop.install'],
          state: 'installed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:install-update-ref-missing:browser-desktop.public.installed',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeBrowserDesktopIntegration({
      generatedAt: '2026-06-18T00:00:00.000Z',
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.no_snapshot',
          policyRefs: ['policy.public.browser_desktop.ready'],
          state: 'ready',
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.no_snapshot:missing-browser-desktop-integration-snapshot-ref',
    )
  })

  test('omits unsafe private browser and desktop material before projection', () => {
    const view = projectForgeBrowserDesktopIntegration({
      ...baseInput,
      blockerRefs: [
        'browser-desktop-blocker.public.safe',
        'raw browser /Users/christopher/profile',
      ],
      entries: [
        {
          browserRefs: ['browser.public.safe', 'raw browser cookie sk-private'],
          companionRefs: ['companion.public.safe'],
          deepLinkRefs: [
            'deep-link.public.safe',
            'openagents://run/Users/christopher/private',
          ],
          desktopAppRefs: ['desktop-app.public.safe', 'desktop app path /Users/christopher/app'],
          extensionRefs: ['extension.public.safe', 'raw extension private'],
          freshness: 'fresh',
          installRefs: ['install.public.safe'],
          integrationRef: 'browser-desktop.public.safe',
          notificationRefs: ['notification.public.safe'],
          permissionRefs: ['permission.public.safe', 'raw permission ~/Library'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          state: 'ready',
          statusRefs: ['status.public.safe'],
          surfaceRefs: ['surface.public.safe', 'private session token'],
          updateRefs: ['update.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.browserRefs).toEqual(['browser.public.safe'])
    expect(view.entries[0]?.deepLinkRefs).toEqual(['deep-link.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-browser-desktop-integration-blocker:work.public.work_1:unsafe-browser-desktop-integration-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw browser')
    expect(payload).not.toContain('openagents://')
    expect(payload).not.toContain('desktop app path')
    expect(payload).not.toContain('raw extension')
    expect(payload).not.toContain('raw permission')
    expect(payload).not.toContain('private session')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      browserDesktopIntegration: {
        entries: [
          {
            freshness: 'fresh',
            integrationRef: 'browser-desktop.public.work_2',
            policyRefs: ['policy.public.work_2'],
            state: 'ready',
          },
        ],
        snapshotRef: 'browser-desktop-integration-snapshot.public.work_2',
        versionRef: 'browser-desktop-integration-version.public.v2',
      },
      generatedAt: '2026-06-18T00:01:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeBrowserDesktopIntegrationInput(work)).toEqual({
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'browser-desktop.public.work_2',
          policyRefs: ['policy.public.work_2'],
          state: 'ready',
        },
      ],
      generatedAt: '2026-06-18T00:01:00.000Z',
      snapshotRef: 'browser-desktop-integration-snapshot.public.work_2',
      versionRef: 'browser-desktop-integration-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
