import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotDesktopCleanMacProofInput,
  AutopilotDesktopCleanMacProofProjection,
  autopilotDesktopCleanMacProofHasPrivateMaterial,
  planAutopilotDesktopCleanMacProof,
} from './autopilot-desktop-clean-mac-proof'

const ciInput = (
  overrides: Partial<AutopilotDesktopCleanMacProofInput> = {},
): AutopilotDesktopCleanMacProofInput =>
  new AutopilotDesktopCleanMacProofInput({
    desktopRuntimeWiringRefs: [],
    installerSignatureRefs: [],
    meteredComputeSessionRefs: [],
    mode: 'ci_contract_only',
    nowIso: '2026-06-29T12:00:00.000Z',
    ownerSignoffRefs: [],
    packagedComputeReadinessRefs: [],
    productionPresenceRefs: [],
    renderedWindowRefs: [],
    settledBitcoinReceiptRefs: [],
    ...overrides,
  })

const liveInput = (
  overrides: Partial<AutopilotDesktopCleanMacProofInput> = {},
): AutopilotDesktopCleanMacProofInput =>
  new AutopilotDesktopCleanMacProofInput({
    desktopRuntimeWiringRefs: [
      'runtime.autopilot_desktop.pdf_preview_ingest_browser.live.ref',
    ],
    installerSignatureRefs: ['installer.autopilot_desktop.dmg.notarized.sha256.ref'],
    meteredComputeSessionRefs: [
      'metering.builtin_compute.from_install.token_usage_event.ref',
    ],
    mode: 'owner_clean_mac_from_dmg',
    nowIso: '2026-06-29T12:00:00.000Z',
    ownerSignoffRefs: ['promise_transition.owner_signed.autopilot_desktop.ref'],
    packagedComputeReadinessRefs: [
      'builtin_compute.packaged_entitlement.secret_ref_only.ready.ref',
    ],
    productionPresenceRefs: ['production.pylon_stats.clean_mac_presence.ref'],
    renderedWindowRefs: ['screenshot.clean_mac.rendered_window.public.ref'],
    settledBitcoinReceiptRefs: ['receipt.tassadar.settled_bitcoin.public.ref'],
    ...overrides,
  })

describe('planAutopilotDesktopCleanMacProof — ci_contract_only', () => {
  test('produces a schema-valid CI contract projection without live refs', () => {
    const projection = planAutopilotDesktopCleanMacProof(ciInput())

    expect(
      S.decodeUnknownSync(AutopilotDesktopCleanMacProofProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.status).toBe('ci_contract_ready')
    expect(projection.cleanMacProofVerified).toBe(false)
    expect(projection.blockerRefs).toHaveLength(0)
    expect(projection.proofBundleRefs).toHaveLength(0)
  })

  test('keeps every step present and non-blocked in CI mode', () => {
    const projection = planAutopilotDesktopCleanMacProof(ciInput())
    expect(projection.steps.map(step => step.kind)).toEqual([
      'installer_signed',
      'rendered_window_captured',
      'production_presence_observed',
      'desktop_runtime_wiring_observed',
      'packaged_compute_ready',
      'metered_compute_session_recorded',
      'settled_bitcoin_receipt_captured',
      'owner_signoff_recorded',
    ])
    for (const step of projection.steps) {
      expect(step.state).toBe('planned_no_live_sessions')
    }
  })
})

describe('planAutopilotDesktopCleanMacProof — owner_clean_mac_from_dmg', () => {
  test('verifies the live clean-Mac proof when all public refs exist', () => {
    const projection = planAutopilotDesktopCleanMacProof(liveInput())

    expect(projection.status).toBe('clean_mac_proof_verified')
    expect(projection.cleanMacProofVerified).toBe(true)
    expect(projection.blockerRefs).toHaveLength(0)
    for (const step of projection.steps) {
      expect(step.state).toBe('passed')
    }
  })

  test('keeps desktop GUI and packaged compute refs separated', () => {
    const projection = planAutopilotDesktopCleanMacProof(liveInput())

    expect(projection.desktopGuiProofRefs).toContain(
      'screenshot.clean_mac.rendered_window.public.ref',
    )
    expect(projection.desktopGuiProofRefs).toContain(
      'production.pylon_stats.clean_mac_presence.ref',
    )
    expect(projection.desktopGuiProofRefs).not.toContain(
      'metering.builtin_compute.from_install.token_usage_event.ref',
    )

    expect(projection.packagedComputeProofRefs).toContain(
      'builtin_compute.packaged_entitlement.secret_ref_only.ready.ref',
    )
    expect(projection.packagedComputeProofRefs).toContain(
      'metering.builtin_compute.from_install.token_usage_event.ref',
    )
    expect(projection.packagedComputeProofRefs).not.toContain(
      'screenshot.clean_mac.rendered_window.public.ref',
    )
  })

  test('blocks when the clean-Mac rendered-window proof is missing', () => {
    const projection = planAutopilotDesktopCleanMacProof(
      liveInput({ renderedWindowRefs: [] }),
    )

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.autopilot_desktop_clean_mac_proof.rendered_window_missing',
    )
    expect(
      projection.steps.find(step => step.kind === 'rendered_window_captured')
        ?.state,
    ).toBe('blocked')
  })

  test('blocks when packaged compute readiness is missing', () => {
    const projection = planAutopilotDesktopCleanMacProof(
      liveInput({ packagedComputeReadinessRefs: [] }),
    )

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.autopilot_desktop_clean_mac_proof.packaged_compute_missing',
    )
  })

  test('blocks when owner signoff is missing', () => {
    const projection = planAutopilotDesktopCleanMacProof(
      liveInput({ ownerSignoffRefs: [] }),
    )

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.autopilot_desktop_clean_mac_proof.owner_signoff_missing',
    )
  })
})

describe('autopilotDesktopCleanMacProofHasPrivateMaterial', () => {
  test('allows public-safe refs', () => {
    expect(
      autopilotDesktopCleanMacProofHasPrivateMaterial({
        ref: 'receipt.public.clean_mac.ref',
      }),
    ).toBe(false)
  })

  test('rejects raw local paths and bearer material', () => {
    expect(
      autopilotDesktopCleanMacProofHasPrivateMaterial({
        log: '/Users/operator/Library/Application Support/OpenAgents/auth.json',
      }),
    ).toBe(true)
    expect(
      autopilotDesktopCleanMacProofHasPrivateMaterial({
        authorization: 'Bearer eyJhbGciOiJIUzI1NiJ9',
      }),
    ).toBe(true)
  })
})
