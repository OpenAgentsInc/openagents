import { describe, expect, it } from 'vitest'

import {
  adapterCapabilityRefs,
  adapterJobKinds,
} from './autopilot-work-adapter-selection'
import {
  CODING_WORK_CLASS_CATALOG,
  CODING_WORK_DEVICE_ADMISSION_GATE_SET,
  CodingAgentSessionWorkClassRef,
  type CodingPylonLadderEvidence,
  MAX_CODING_LADDER_EVIDENCE_RECORDS,
  PylonCodingLadderSchemaVersion,
  PylonCodingWorkRailsSchemaVersion,
  classifyCodingWorkReceiptTier,
  codingJoinLifecycleStateForEvidence,
  codingWorkClassForCapabilityRef,
  codingWorkClassForJobKind,
  exportPylonCodingWorkRailsContract,
  pylonCodingLadderProjection,
} from './pylon-coding-work-rails'
import {
  evaluateDeviceAdmissionGate,
  funnelReasonRefForDeviceAdmissionDecision,
} from './training-device-admission-gates'

const evidence = (
  overrides: Partial<CodingPylonLadderEvidence> = {},
): CodingPylonLadderEvidence => ({
  acceptedCloseoutRefs: [],
  capabilityRefs: ['capability.pylon.local_claude_agent'],
  pylonRef: 'pylon.public.coding_rails.example_1',
  readinessProbeRefs: [],
  smokeVerificationRefs: [],
  ...overrides,
})

describe('coding-agent work classes on the Pluralis rails (#4861)', () => {
  describe('work-class catalog', () => {
    it('carries the live adapter capability refs and assignment job kinds, not invented ones', () => {
      expect(CODING_WORK_CLASS_CATALOG.map(wc => wc.capabilityRef)).toEqual([
        'capability.pylon.local_claude_agent',
        'capability.pylon.local_codex',
      ])
      expect(CODING_WORK_CLASS_CATALOG.map(wc => wc.jobKind)).toEqual([
        'claude_agent_task',
        'codex_agent_task',
      ])

      for (const workClass of CODING_WORK_CLASS_CATALOG) {
        expect(workClass.capabilityRef).toBe(
          adapterCapabilityRefs[workClass.adapter],
        )
        expect(workClass.jobKind).toBe(adapterJobKinds[workClass.adapter])
      }
    })

    it('cites the live bridge promises and readiness schemas per adapter', () => {
      expect(codingWorkClassForJobKind('claude_agent_task')).toMatchObject({
        promiseRef: 'pylon.local_claude_agent_bridge.v1',
        readinessSchemaRef: 'openagents.pylon.claude_agent_readiness.v0.3',
        workClassRef: 'work_class.coding.claude_agent_task',
      })
      expect(codingWorkClassForJobKind('codex_agent_task')).toMatchObject({
        promiseRef: 'autopilot.codex_probe_pylon_successor.v1',
        readinessSchemaRef: 'openagents.pylon.codex_agent_readiness.v0.3',
        workClassRef: 'work_class.coding.codex_agent_task',
      })
      expect(
        codingWorkClassForCapabilityRef('capability.pylon.local_codex')
          ?.adapter,
      ).toBe('codex')
      expect(
        codingWorkClassForCapabilityRef('capability.pylon.assignment_ready'),
      ).toBeUndefined()
    })
  })

  describe('receipt tiers (reusing classifyReceiptTier)', () => {
    it('classifies a verified accepted closeout from an active Pylon as compute-tier', () => {
      expect(
        classifyCodingWorkReceiptTier({
          evidenceKind: 'accepted_closeout',
          joinLifecycleState: 'active',
          verificationOutcomeRefs: [
            'verification.outcome.coding.example_merged',
          ],
          workClassRef: 'work_class.coding.claude_agent_task',
        }),
      ).toEqual({
        classification: {
          joinLifecycleState: 'active',
          reasonCode: 'receipt_tier.public.compute_merged_verified_closeout',
          tier: 'compute_tier',
          workKind: 'verified_closeout',
        },
        evidenceKind: 'accepted_closeout',
        workClassRef: 'work_class.coding.claude_agent_task',
      })
    })

    it('classifies BYOK readiness probes as presence-tier evidence', () => {
      const classified = classifyCodingWorkReceiptTier({
        evidenceKind: 'byok_readiness_probe',
        joinLifecycleState: 'registered',
        verificationOutcomeRefs: [],
        workClassRef: 'work_class.coding.codex_agent_task',
      })

      expect(classified.classification.tier).toBe('presence_tier')
      expect(classified.classification.workKind).toBe('qualification_probe')
      expect(classified.classification.reasonCode).toBe(
        'receipt_tier.public.presence_qualification_probe',
      )
    })

    it('classifies bounded no-spend smokes as presence-tier by construction', () => {
      const classified = classifyCodingWorkReceiptTier({
        evidenceKind: 'bounded_no_spend_smoke',
        joinLifecycleState: 'warmup',
        verificationOutcomeRefs: ['verification.outcome.coding.smoke_1'],
        workClassRef: 'work_class.coding.claude_agent_task',
      })

      expect(classified.classification.tier).toBe('presence_tier')
      expect(classified.classification.workKind).toBe('shadow_window_work')
      expect(classified.classification.reasonCode).toBe(
        'receipt_tier.public.presence_shadow_window_work',
      )
    })

    it('pays only presence-tier for accepted closeouts below the active rung', () => {
      expect(
        classifyCodingWorkReceiptTier({
          evidenceKind: 'accepted_closeout',
          joinLifecycleState: 'qualified',
          verificationOutcomeRefs: ['verification.outcome.coding.unmerged_1'],
          workClassRef: 'work_class.coding.codex_agent_task',
        }).classification,
      ).toMatchObject({
        reasonCode: 'receipt_tier.public.presence_unmerged_work_not_active',
        tier: 'presence_tier',
      })
    })

    it('refuses an unverified accepted closeout on any tier', () => {
      expect(() =>
        classifyCodingWorkReceiptTier({
          evidenceKind: 'accepted_closeout',
          joinLifecycleState: 'active',
          verificationOutcomeRefs: [],
          workClassRef: 'work_class.coding.claude_agent_task',
        }),
      ).toThrow('unverified work is not payable')
    })
  })

  describe('join ladder evidence mapping', () => {
    it('claims only the rung the refs prove', () => {
      expect(codingJoinLifecycleStateForEvidence(evidence())).toBe('registered')
      expect(
        codingJoinLifecycleStateForEvidence(
          evidence({
            readinessProbeRefs: ['receipt.coding.readiness_probe.example_1'],
          }),
        ),
      ).toBe('qualified')
      expect(
        codingJoinLifecycleStateForEvidence(
          evidence({
            readinessProbeRefs: ['receipt.coding.readiness_probe.example_1'],
            smokeVerificationRefs: ['receipt.coding.no_spend_smoke.example_1'],
          }),
        ),
      ).toBe('warmup')
      expect(
        codingJoinLifecycleStateForEvidence(
          evidence({
            acceptedCloseoutRefs: ['acceptance.coding.closeout.example_1'],
            readinessProbeRefs: ['receipt.coding.readiness_probe.example_1'],
            smokeVerificationRefs: ['receipt.coding.no_spend_smoke.example_1'],
          }),
        ),
      ).toBe('active')
    })

    it('projects coding-capability rungs with per-capability counts', () => {
      const projection = pylonCodingLadderProjection([
        evidence({ pylonRef: 'pylon.public.coding_rails.registered_1' }),
        evidence({
          pylonRef: 'pylon.public.coding_rails.qualified_1',
          readinessProbeRefs: ['receipt.coding.readiness_probe.q1'],
        }),
        evidence({
          capabilityRefs: [
            'capability.pylon.local_claude_agent',
            'capability.pylon.local_codex',
          ],
          pylonRef: 'pylon.public.coding_rails.warmup_1',
          smokeVerificationRefs: ['receipt.coding.no_spend_smoke.w1'],
        }),
        evidence({
          acceptedCloseoutRefs: ['acceptance.coding.closeout.a1'],
          capabilityRefs: ['capability.pylon.local_codex'],
          pylonRef: 'pylon.public.coding_rails.active_1',
        }),
        evidence({
          capabilityRefs: ['capability.pylon.assignment_ready'],
          pylonRef: 'pylon.public.coding_rails.non_coding_1',
        }),
      ])

      expect(projection.schemaVersion).toBe(PylonCodingLadderSchemaVersion)
      expect(projection.contractDefinitionOnly).toBe(true)
      expect(projection.totalCount).toBe(4)
      expect(projection.nonCodingPylonCount).toBe(1)
      expect(projection.byState).toEqual([
        { count: 1, key: 'active' },
        { count: 1, key: 'qualified' },
        { count: 1, key: 'registered' },
        { count: 1, key: 'warmup' },
      ])
      expect(projection.byCapability).toEqual([
        {
          byState: [
            { count: 1, key: 'qualified' },
            { count: 1, key: 'registered' },
            { count: 1, key: 'warmup' },
          ],
          capabilityRef: 'capability.pylon.local_claude_agent',
          totalCount: 3,
        },
        {
          byState: [
            { count: 1, key: 'active' },
            { count: 1, key: 'warmup' },
          ],
          capabilityRef: 'capability.pylon.local_codex',
          totalCount: 2,
        },
      ])
      expect(projection.entries[0]).toMatchObject({
        ladderRank: 4,
        pylonRef: 'pylon.public.coding_rails.active_1',
        state: 'active',
        stateLabel: 'Active',
      })
      expect(Object.isFrozen(projection)).toBe(true)
      expect(Object.isFrozen(projection.byCapability)).toBe(true)
      expect(JSON.parse(JSON.stringify(projection))).toEqual(projection)
    })

    it('enforces bounded lists and the privacy ref scan', () => {
      expect(() =>
        pylonCodingLadderProjection(
          Array.from(
            { length: MAX_CODING_LADDER_EVIDENCE_RECORDS + 1 },
            (_, index) =>
              evidence({ pylonRef: `pylon.public.coding_rails.bulk_${index}` }),
          ),
        ),
      ).toThrow(`at most ${MAX_CODING_LADDER_EVIDENCE_RECORDS}`)
      expect(() =>
        pylonCodingLadderProjection([
          evidence({ pylonRef: 'pylon.wallet.seed_backup' }),
        ]),
      ).toThrow('private host, wallet, payment')
      expect(() =>
        pylonCodingLadderProjection([
          evidence({
            readinessProbeRefs: ['probe-at-2026-06-12T09:00:00Z'],
          }),
        ]),
      ).toThrow('raw timestamp')
    })
  })

  describe('admission gates (definitions only)', () => {
    it('binds all coding gates to the shared session work class with stated reasons', () => {
      expect(CODING_WORK_DEVICE_ADMISSION_GATE_SET).toHaveLength(3)

      for (const gate of CODING_WORK_DEVICE_ADMISSION_GATE_SET) {
        expect(gate.workClassRef).toBe(CodingAgentSessionWorkClassRef)
        expect(gate.rationale.length).toBeGreaterThan(8)
        expect(gate.sourceRefs).toContain('issue.github.openagents.4861')
      }

      expect(
        CODING_WORK_DEVICE_ADMISSION_GATE_SET.map(
          gate => gate.requirement.measurementKind,
        ),
      ).toEqual([
        'node_or_bun_runtime_present',
        'workspace_write_sandbox_supported',
        'host_ram_headroom_gb',
      ])
    })

    it('evaluates every coding gate on both branches through the existing machinery', () => {
      for (const gate of CODING_WORK_DEVICE_ADMISSION_GATE_SET) {
        const margin = Math.max(Math.abs(gate.requirement.threshold), 1)
        const admitted = evaluateDeviceAdmissionGate({
          deviceClassRef: 'device_class.example.coding_capable_host',
          gate,
          measuredValue: gate.requirement.threshold + margin,
        })
        const excluded = evaluateDeviceAdmissionGate({
          deviceClassRef: 'device_class.example.coding_incapable_host',
          gate,
          measuredValue: gate.requirement.threshold - margin,
        })

        expect(admitted.decision).toBe('admitted')
        expect(excluded.decision).toBe('excluded')
        expect(admitted.statedReason).toContain(CodingAgentSessionWorkClassRef)
        expect(
          funnelReasonRefForDeviceAdmissionDecision(excluded).startsWith(
            'device_admission.public.excluded_',
          ),
        ).toBe(true)
      }
    })

    it('excludes a sandboxless host with the workspace-write reason', () => {
      const sandboxGate = CODING_WORK_DEVICE_ADMISSION_GATE_SET.find(
        gate =>
          gate.requirement.measurementKind ===
          'workspace_write_sandbox_supported',
      )!
      const excluded = evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.sandboxless_host',
        gate: sandboxGate,
        measuredValue: 0,
      })

      expect(excluded.reasonCode).toBe(
        'device_admission.public.excluded_workspace_write_sandbox_unsupported',
      )
      expect(excluded.statedReason).toContain('workspace-write sandbox')
    })
  })

  describe('rails contract export', () => {
    it('exports a versioned frozen JSON-able contract with the non-claim posture explicit', () => {
      const contract = exportPylonCodingWorkRailsContract()

      expect(contract.schemaVersion).toBe(PylonCodingWorkRailsSchemaVersion)
      expect(contract.schemaVersion).toBe(
        'openagents.pylon.coding_work_rails.v1',
      )
      expect(contract.contractDefinitionOnly).toBe(true)
      expect(contract.liveAdmissionClaim).toBe(false)
      expect(contract.livePayoutClaim).toBe(false)
      expect(contract.admissionGates.definitionsOnly).toBe(true)
      expect(contract.admissionGates.liveAdmissionClaim).toBe(false)
      expect(contract.admissionGates.gates).toHaveLength(3)
      expect(contract.workClasses).toHaveLength(2)
      expect(contract.promiseRefs).toEqual([
        'autopilot.codex_probe_pylon_successor.v1',
        'pylon.local_claude_agent_bridge.v1',
      ])
      expect(contract.policyRefs).toContain(
        'policy.public.coding_work_rails.accepted_closeouts_pay_compute_tier_only_from_active',
      )
      expect(contract.sourceRefs).toContain('issue.github.openagents.4861')
      expect(contract.sourceRefs).toContain('issue.github.openagents.4862')
      expect(Object.isFrozen(contract)).toBe(true)
      expect(Object.isFrozen(contract.workClasses)).toBe(true)
      expect(Object.isFrozen(contract.admissionGates)).toBe(true)
      expect(JSON.parse(JSON.stringify(contract))).toEqual(contract)
    })
  })
})
