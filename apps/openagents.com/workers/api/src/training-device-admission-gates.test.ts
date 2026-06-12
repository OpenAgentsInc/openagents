import { describe, expect, it } from 'vitest'

import { projectPylonCapacityFunnelRecord } from './pylon-capacity-funnel'
import {
  EXAMPLE_DEVICE_ADMISSION_GATE_SET,
  assertAdmissibleDeviceAdmissionDecision,
  type DeviceAdmissionDecisionRecord,
  type DeviceAdmissionGateDefinition,
  DeviceAdmissionGateContractSchemaVersion,
  evaluateDeviceAdmissionGate,
  exportDeviceAdmissionGateContract,
  funnelReasonRefForDeviceAdmissionDecision,
} from './training-device-admission-gates'

const hostRamGate = EXAMPLE_DEVICE_ADMISSION_GATE_SET.find(
  gate => gate.requirement.measurementKind === 'host_ram_headroom_gb',
)!
const sustainedGate = EXAMPLE_DEVICE_ADMISSION_GATE_SET.find(
  gate =>
    gate.requirement.measurementKind === 'sustained_vs_burst_throughput_ratio',
)!
const bf16Gate = EXAMPLE_DEVICE_ADMISSION_GATE_SET.find(
  gate => gate.requirement.measurementKind === 'attention_throughput',
)!

describe('reasoned device admission gates (Pluralis roadmap P1.4)', () => {
  it('admits with a stated, measured reason', () => {
    const decision = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      gate: hostRamGate,
      measuredValue: 96,
    })

    expect(decision).toMatchObject({
      decision: 'admitted',
      gateRef: 'gate.device_admission.example.host_ram_headroom_floor.v1',
      measuredValue: 96,
      measurementKind: 'host_ram_headroom_gb',
      reasonCode:
        'device_admission.public.admitted_host_ram_headroom_at_or_above_floor',
      threshold: 80,
      workClassRef: 'work_class.example.optimizer_offload_training',
    })
    expect(decision.statedReason).toContain('measured host_ram_headroom_gb 96')
    expect(decision.statedReason).toContain('at or above the floor of 80')
    expect(decision.statedReason).toContain('Adam moments in host RAM')
  })

  it('excludes with a stated, measured reason on the same gate', () => {
    const decision = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_32gb_host',
      gate: hostRamGate,
      measuredValue: 32,
    })

    expect(decision).toMatchObject({
      decision: 'excluded',
      measuredValue: 32,
      reasonCode:
        'device_admission.public.excluded_host_ram_headroom_below_floor',
    })
    expect(decision.statedReason).toContain('measured host_ram_headroom_gb 32')
    expect(decision.statedReason).toContain('below the floor of 80')
  })

  it('carries the Pluralis T4/V100 bf16 reason and the thermal collapse reason in the seeded definitions', () => {
    expect(bf16Gate.rationale).toContain('emulate BF16 slower than FP32')
    expect(sustainedGate.rationale).toContain('14-node')
    expect(
      evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.thermally_limited_laptop',
        gate: sustainedGate,
        measuredValue: 0.55,
      }),
    ).toMatchObject({
      decision: 'excluded',
      reasonCode:
        'device_admission.public.excluded_sustained_throughput_ratio_below_floor',
    })
  })

  it('supports at_most ceilings with reasoned branches', () => {
    const ceilingGate: DeviceAdmissionGateDefinition = {
      admittedReasonCode:
        'device_admission.public.admitted_step_time_at_or_below_ceiling',
      excludedReasonCode:
        'device_admission.public.excluded_step_time_above_ceiling',
      gateRef: 'gate.device_admission.example.step_time_ceiling.v1',
      rationale:
        'pipeline stages stall behind the slowest member, so a bounded reference step must complete inside the window budget.',
      requirement: {
        comparison: 'at_most',
        measurementKind: 'step_time_ms',
        threshold: 250,
        unit: 'milliseconds',
      },
      sourceRefs: ['issue.github.openagents.4852'],
      workClassRef: 'work_class.example.pipeline_stage_training',
    }

    expect(
      evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.fast',
        gate: ceilingGate,
        measuredValue: 120,
      }),
    ).toMatchObject({ decision: 'admitted' })
    const excluded = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.slow',
      gate: ceilingGate,
      measuredValue: 900,
    })

    expect(excluded.decision).toBe('excluded')
    expect(excluded.statedReason).toContain('above the ceiling of 250')
  })

  it('rejects reasonless and self-contradicting decisions', () => {
    const decision = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      gate: hostRamGate,
      measuredValue: 96,
    })

    expect(() =>
      assertAdmissibleDeviceAdmissionDecision({
        ...decision,
        statedReason: '   ',
      }),
    ).toThrow('stated, measured reason')
    expect(() =>
      assertAdmissibleDeviceAdmissionDecision({
        ...decision,
        reasonCode:
          'device_admission.public.excluded_host_ram_headroom_below_floor',
      }),
    ).toThrow('admitted_')
    expect(() =>
      assertAdmissibleDeviceAdmissionDecision({
        ...decision,
        decision: 'excluded',
        reasonCode:
          'device_admission.public.excluded_host_ram_headroom_below_floor',
      }),
    ).toThrow('contradicts its own measured value')
    expect(() =>
      evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.broken',
        gate: hostRamGate,
        measuredValue: Number.NaN,
      }),
    ).toThrow('finite number')
  })

  it('rejects private material in stated reasons and rationales', () => {
    const decision = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      gate: hostRamGate,
      measuredValue: 96,
    })

    expect(() =>
      assertAdmissibleDeviceAdmissionDecision({
        ...decision,
        statedReason: 'admitted because wallet balance was positive',
      }),
    ).toThrow('private host, wallet, payment')
    expect(() =>
      evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
        gate: {
          ...hostRamGate,
          rationale: 'measured at 2026-06-12T09:00:00Z on /Users/operator',
        },
        measuredValue: 96,
      }),
    ).toThrow('raw timestamp')
  })

  it('exports a versioned frozen JSON-able contract marked definitions-only', () => {
    const contract = exportDeviceAdmissionGateContract()

    expect(contract.schemaVersion).toBe(
      DeviceAdmissionGateContractSchemaVersion,
    )
    expect(contract.schemaVersion).toBe(
      'openagents.training.device_admission_gates.v1',
    )
    expect(contract.definitionsOnly).toBe(true)
    expect(contract.liveAdmissionClaim).toBe(false)
    expect(contract.gates).toHaveLength(3)
    expect(contract.policyRefs).toContain(
      'policy.public.device_admission.every_decision_carries_stated_measured_reason',
    )
    expect(contract.policyRefs).toContain(
      'policy.public.device_admission.psionic_preflight_consumes_this_contract',
    )
    expect(Object.isFrozen(contract)).toBe(true)
    expect(Object.isFrozen(contract.gates)).toBe(true)
    expect(Object.isFrozen(contract.gates[0])).toBe(true)
    expect(Object.isFrozen(contract.gates[0]?.requirement)).toBe(true)
    expect(JSON.parse(JSON.stringify(contract))).toEqual(contract)
  })

  it('surfaces exclusion reason codes through the capacity funnel without tripping the privacy scanner', () => {
    const decision = evaluateDeviceAdmissionGate({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_32gb_host',
      gate: hostRamGate,
      measuredValue: 32,
    })
    const reasonRef = funnelReasonRefForDeviceAdmissionDecision(decision)

    expect(reasonRef).toBe(
      'device_admission.public.excluded_host_ram_headroom_below_floor',
    )

    const projection = projectPylonCapacityFunnelRecord(
      {
        acceptanceRefs: [],
        artifactRefs: [],
        assignmentRefs: [],
        benchmarkRefs: ['benchmark.capacity.admission_demo_1'],
        capacityRef: 'capacity.pylon_admission_demo_1',
        caveatRefs: [
          'caveat.capacity.device_admission_definitions_not_live_policy',
        ],
        darkCapacityReasonRefs: [reasonRef],
        eligibilityRefs: [],
        evidenceRefs: [decision.gateRef],
        id: 'capacity_funnel_admission_demo_1',
        nodeRef: 'node.public_admission_demo_1',
        nodeVisibility: 'public',
        providerRef: 'provider.public_admission_demo_1',
        providerVisibility: 'public',
        rewardRefs: [],
        runRefs: [],
        settlementRefs: [],
        stage: 'dark',
        updatedAtIso: '2026-06-12T17:00:00.000Z',
        workClassRefs: [decision.workClassRef],
      },
      'public',
      '2026-06-12T17:05:00.000Z',
    )

    expect(projection.darkCapacityReasonRefs).toEqual([reasonRef])
    expect(projection.workClassRefs).toEqual([
      'work_class.example.optimizer_offload_training',
    ])
  })

  it('keeps every seeded gate evaluable on both branches', () => {
    for (const gate of EXAMPLE_DEVICE_ADMISSION_GATE_SET) {
      const margin = Math.max(Math.abs(gate.requirement.threshold), 1)
      const passing = evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.passing',
        gate,
        measuredValue:
          gate.requirement.comparison === 'at_least'
            ? gate.requirement.threshold + margin
            : gate.requirement.threshold - margin,
      })
      const failing = evaluateDeviceAdmissionGate({
        deviceClassRef: 'device_class.example.failing',
        gate,
        measuredValue:
          gate.requirement.comparison === 'at_least'
            ? gate.requirement.threshold - margin
            : gate.requirement.threshold + margin,
      })
      const decisions: ReadonlyArray<DeviceAdmissionDecisionRecord> = [
        passing,
        failing,
      ]

      expect(passing.decision).toBe('admitted')
      expect(failing.decision).toBe('excluded')

      for (const decision of decisions) {
        expect(decision.statedReason.length).toBeGreaterThan(8)
        expect(decision.reasonCode.startsWith('device_admission.public.')).toBe(
          true,
        )
      }
    }
  })
})
