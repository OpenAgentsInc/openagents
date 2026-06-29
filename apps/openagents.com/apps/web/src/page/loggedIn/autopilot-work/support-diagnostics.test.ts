import { describe, expect, test } from 'vitest'

import {
  type ForgeSupportDiagnosticsInput,
  projectForgeSupportDiagnostics,
} from './support-diagnostics'

const baseInput = (
  overrides: Partial<ForgeSupportDiagnosticsInput> = {},
): ForgeSupportDiagnosticsInput => ({
  generatedAt: '2026-06-17T17:00:00.000Z',
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge support diagnostics projection', () => {
  test('reports empty status with a blocker when no evidence is present', () => {
    const view = projectForgeSupportDiagnostics(baseInput())

    expect(view.status).toBe('empty')
    expect(view.doctorChecks).toEqual([])
    expect(view.exportReadiness).toBe('blocked')
    expect(view.blockerRefs).toEqual([
      'forge-support-diagnostics-blocker:work_1:no-support-diagnostics-evidence',
    ])
    expect(view.omittedUnsafeRefCount).toBe(0)
  })

  test('surfaces help, doctor, preflight, and log refs and counts severities', () => {
    const view = projectForgeSupportDiagnostics(
      baseInput({
        diagnosticLogRefs: ['log.public.startup.timing'],
        doctorChecks: [
          {
            category: 'sandbox',
            checkRef: 'doctor.public.sandbox',
            evidenceRefs: ['evidence.public.sandbox.ok'],
            severity: 'ok',
          },
          {
            category: 'settings',
            checkRef: 'doctor.public.settings',
            fixRefs: ['fix.public.settings.repair'],
            severity: 'warning',
          },
        ],
        helpCommandRefs: ['help.public.command.plan'],
        preflightRefs: ['preflight.public.connectivity'],
      }),
    )

    expect(view.status).toBe('attention')
    expect(view.counts).toEqual({ error: 0, info: 0, ok: 1, warning: 1 })
    expect(view.helpCommandRefs).toEqual(['help.public.command.plan'])
    expect(view.preflightRefs).toEqual(['preflight.public.connectivity'])
    expect(view.diagnosticLogRefs).toEqual(['log.public.startup.timing'])
    // warning sorts ahead of ok
    expect(view.doctorChecks.map(check => check.checkRef)).toEqual([
      'doctor.public.settings',
      'doctor.public.sandbox',
    ])
  })

  test('an error-severity doctor check makes the lane fail and blocks export', () => {
    const view = projectForgeSupportDiagnostics(
      baseInput({
        doctorChecks: [
          {
            category: 'install',
            checkRef: 'doctor.public.install',
            severity: 'error',
          },
        ],
        supportBundleSections: [
          { consent: 'consented', sectionRef: 'bundle.public.env' },
        ],
      }),
    )

    expect(view.status).toBe('failing')
    expect(view.counts.error).toBe(1)
    expect(view.exportReadiness).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-support-diagnostics-blocker:work_1:doctor-error:doctor.public.install',
    )
  })

  test('export readiness is consent-gated when doctor is clean', () => {
    const pending = projectForgeSupportDiagnostics(
      baseInput({
        doctorChecks: [{ checkRef: 'doctor.public.ok', severity: 'ok' }],
        supportBundleSections: [
          { consent: 'consented', sectionRef: 'bundle.public.env' },
          { consent: 'pending', sectionRef: 'bundle.public.transcript' },
        ],
      }),
    )
    expect(pending.exportReadiness).toBe('consent_required')

    const ready = projectForgeSupportDiagnostics(
      baseInput({
        doctorChecks: [{ checkRef: 'doctor.public.ok', severity: 'ok' }],
        supportBundleSections: [
          { consent: 'consented', sectionRef: 'bundle.public.env' },
          { consent: 'consented', sectionRef: 'bundle.public.transcript' },
        ],
      }),
    )
    expect(ready.status).toBe('ready')
    expect(ready.exportReadiness).toBe('ready')
  })

  test('redacts unsafe refs across every field and records the omission', () => {
    const view = projectForgeSupportDiagnostics(
      baseInput({
        diagnosticLogRefs: ['/Users/secret/agent.log', 'log.public.ok'],
        doctorChecks: [
          {
            checkRef: 'doctor.public.shell',
            // a raw diff hunk and a bearer token must be dropped
            evidenceRefs: ['@@ -1 +1 @@', 'bearer abc.def', 'evidence.public.shell'],
            fixRefs: ['https://example.com/fix'],
            severity: 'warning',
          },
          // an entirely unsafe checkRef drops the whole check
          { checkRef: 'rm -rf /tmp && echo gone', severity: 'error' },
        ],
        helpCommandRefs: ['help.public.ok', 'ghp_0123456789abcdef'],
        preflightRefs: ['~/private/path', 'preflight.public.ok'],
        supportBundleSections: [
          {
            consent: 'consented',
            evidenceRefs: ['raw-prompt-leak', 'bundle.public.ok'],
            sectionRef: 'bundle.public.env',
          },
        ],
      }),
    )

    // Surviving safe refs only.
    expect(view.helpCommandRefs).toEqual(['help.public.ok'])
    expect(view.preflightRefs).toEqual(['preflight.public.ok'])
    expect(view.diagnosticLogRefs).toEqual(['log.public.ok'])
    expect(view.doctorChecks).toHaveLength(1)
    expect(view.doctorChecks[0]?.checkRef).toBe('doctor.public.shell')
    expect(view.doctorChecks[0]?.evidenceRefs).toEqual(['evidence.public.shell'])
    expect(view.doctorChecks[0]?.fixRefs).toEqual([])
    expect(view.supportBundleSections[0]?.evidenceRefs).toEqual(['bundle.public.ok'])

    // No secret-like or path-like material survived anywhere.
    const allRefs = JSON.stringify(view)
    expect(allRefs).not.toContain('/Users/')
    expect(allRefs).not.toContain('bearer')
    expect(allRefs).not.toContain('ghp_')
    expect(allRefs).not.toContain('@@')
    expect(allRefs).not.toContain('rm -rf')

    expect(view.omittedUnsafeRefCount).toBeGreaterThan(0)
    expect(view.blockerRefs).toContain(
      'forge-support-diagnostics-blocker:work_1:unsafe-support-material-omitted',
    )
    // The export stays blocked because unsafe material was present.
    expect(view.exportReadiness).toBe('blocked')
  })

  test('never claims any runtime authority', () => {
    const view = projectForgeSupportDiagnostics(
      baseInput({
        doctorChecks: [{ checkRef: 'doctor.public.ok', severity: 'ok' }],
      }),
    )

    expect(view.authority).toEqual({
      bundleExportAuthority: false,
      consentGrantAuthority: false,
      credentialReadAuthority: false,
      doctorExecutionAuthority: false,
      preflightExecutionAuthority: false,
      settingsMutationAuthority: false,
    })
  })

  test('deduplicates refs and defaults missing category/severity', () => {
    const view = projectForgeSupportDiagnostics(
      baseInput({
        doctorChecks: [{ checkRef: 'doctor.public.x' }],
        helpCommandRefs: ['help.public.dup', 'help.public.dup'],
      }),
    )

    expect(view.helpCommandRefs).toEqual(['help.public.dup'])
    expect(view.doctorChecks[0]?.category).toBe('install')
    expect(view.doctorChecks[0]?.severity).toBe('info')
    expect(view.status).toBe('ready')
  })
})
