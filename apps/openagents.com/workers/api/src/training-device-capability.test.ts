import { describe, expect, it } from 'vitest'

import {
  buildTrainingRunRecord,
  buildTrainingWindowRecord,
} from './training-run-window-authority'
import { buildCs336A2ThermalThrottleMeasurementEvidence } from './cs336-a2-benchmark-workload'
import {
  Cs336A2BenchmarkMeasurements,
  Cs336A2DeviceBenchmarkJobKind,
  Cs336A2HostProbeMeasurements,
  admitCs336A2DeviceBenchmarkEvidence,
  buildCs336A2DeviceBenchmarkPayload,
  publicDeviceCapabilityProjection,
} from './training-device-capability'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
} from './training-verification'

describe('CS336 A2 device capability projection', () => {
  it('keeps the public capability dataset blocked without receipted measurements', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2',
      nowIso: '2026-06-10T12:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a2.benchmark',
      },
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run,
      windows: [],
    })

    expect(projection).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_receipted_benchmark_results',
        'blocker.cs336_a2.requires_replication_across_same_class_devices',
        'blocker.cs336_a2.requires_statistical_cross_check',
      ],
      classDistributions: [],
      jobKind: 'cs336_a2_device_benchmark',
      observedDeviceClassCount: 0,
      observedMeasurementCount: 0,
      requiredSameClassSampleCount: 3,
      schemaVersion: 'openagents.training.device_capability_dataset.v1',
      sameClassReplicationBlockerRefs: [
        'blocker.cs336_a2.requires_replication_across_same_class_devices',
      ],
      sameClassReplicationSignals: [],
      sameClassReplicationStatus: 'missing',
      thermalThrottleBlockerRefs: [
        'blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe',
      ],
      thermalThrottleDetectionStatus: 'missing',
      thermalThrottleSignals: [],
    })
    expect(Cs336A2DeviceBenchmarkJobKind).toBe('cs336_a2_device_benchmark')
    expect(
      buildCs336A2DeviceBenchmarkPayload({
        assignmentRef: 'assignment.cs336.a2.device_benchmark.1',
      }),
    ).toMatchObject({
      benchmarkSuiteRef:
        'benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1',
      jobKind: 'cs336_a2_device_benchmark',
      verificationClass: 'statistical_cross_check',
    })
  })

  it('publishes anonymized same-class distributions with modeled estimates after cross-check', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a2',
      nowIso: '2026-06-10T12:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a2.benchmark',
      },
    })
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        a2DeviceBenchmark: {
          measurements: [
            {
              deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
              earningEstimate: {
                estimateRef: 'estimate.cs336.a2.m3_pro.training_window',
                p50SatsPerHour: 42,
                p90SatsPerHour: 65,
                policyRefs: ['policy.cs336_a2.modeled_from_current_rates'],
                sourceRefs: ['receipt.cs336.a2.estimate.1'],
                workClass: 'small_model_local_training',
              },
              max: 2060,
              measurementRef: 'measurement.cs336.a2.m3_pro.tokens_per_second',
              metric: 'tokens_per_second',
              min: 1710,
              p50: 1900,
              p90: 2025,
              receiptRefs: ['receipt.cs336.a2.measurement.1'],
              sampleCount: 4,
              sourceRefs: ['artifact.cs336.a2.class_distribution.1'],
              unit: 'tokens_per_second',
              verificationRefs: ['challenge.cs336.a2.class_check.1'],
              workClass: 'small_model_local_training',
            },
          ],
        },
      }),
    }
    const window = buildTrainingWindowRecord({
      makeId: () => 'window',
      nowIso: '2026-06-10T12:00:00.000Z',
      request: {
        homeworkKind: 'admin_dispatched_homework',
        trainingRunRef: run.trainingRunRef,
        windowRef: 'training.window.cs336.a2.benchmark.1',
      },
    })
    const challenge = buildTrainingVerificationChallengeRecord({
      makeId: () => 'challenge',
      nowIso: '2026-06-10T12:01:00.000Z',
      request: {
        commitmentRefs: ['commitment.cs336.a2.class_distribution.1'],
        contributionRef: 'measurement.cs336.a2.m3_pro.tokens_per_second',
        homeworkKind: 'admin_dispatched_homework',
        payload: {
          deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
          measurementRef: 'measurement.cs336.a2.m3_pro.tokens_per_second',
        },
        samplingPolicy: 'aggregate',
        trainingRunRef: run.trainingRunRef,
        verificationClass: 'statistical_cross_check',
        windowRef: window.windowRef,
      },
    }).challenge
    const leased = leaseTrainingVerificationChallengeRecord({
      challenge,
      eventId: 'lease',
      nowIso: '2026-06-10T12:02:00.000Z',
      request: { validatorRef: 'validator.cs336.a2' },
    }).challenge
    const verified = finalizeTrainingVerificationChallengeRecord({
      challenge: leased,
      eventId: 'final',
      nowIso: '2026-06-10T12:03:00.000Z',
      request: { receiptRefs: ['receipt.cs336.a2.verdict.1'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.cs336.a2.class_distribution.1'],
      },
    }).challenge
    const projection = publicDeviceCapabilityProjection({
      challenges: [verified],
      leases: [],
      run,
      windows: [window],
    })

    expect(projection.blockerRefs).toEqual([
      'blocker.cs336_a2.requires_cross_machine_same_class_replication',
    ])
    expect(projection.thermalThrottleDetectionStatus).toBe('missing')
    expect(projection.thermalThrottleBlockerRefs).toEqual([
      'blocker.cs336_a2.requires_sustained_vs_burst_thermal_probe',
    ])
    expect(projection.observedDeviceClassCount).toBe(1)
    expect(projection.observedMeasurementCount).toBe(1)
    expect(projection.classDistributions[0]).toMatchObject({
      crossCheckState: 'cross_checked',
      deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
      metric: 'tokens_per_second',
      p50: 1900,
      sameClassReplicationScope: 'cross_process_same_host',
      sameClassReplicationStatus: 'same_host_only',
      sampleCount: 4,
      verified: true,
    })
    expect(projection.sameClassReplicationStatus).toBe('same_host_only')
    expect(projection.sameClassReplicationBlockerRefs).toEqual([
      'blocker.cs336_a2.requires_cross_machine_same_class_replication',
    ])
    expect(projection.sameClassReplicationSignals[0]).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_cross_machine_same_class_replication',
      ],
      reasonCode:
        'device_capability.public.same_class_replication_same_host_only',
      scope: 'cross_process_same_host',
      state: 'same_host_only',
    })
    expect(projection.classDistributions[0]?.earningEstimate).toMatchObject({
      basisLabel: 'modeled_from_measured_benchmark_distribution',
      p50SatsPerHour: 42,
      workClass: 'small_model_local_training',
    })
    expect(JSON.stringify(projection)).not.toMatch(
      /pylonRef|deviceId|mnemonic|paymentHash/i,
    )
  })

  it('rejects device identifiers before building the public projection', () => {
    const runBase = buildTrainingRunRecord({
      makeId: () => 'a2',
      nowIso: '2026-06-10T12:00:00.000Z',
      request: {
        promiseRef: 'pylon.compute_revenue_modes.v1',
        trainingRunRef: 'training.run.cs336.a2.unsafe',
      },
    })
    const run = {
      ...runBase,
      publicProjectionJson: JSON.stringify({
        a2DeviceBenchmark: {
          measurements: [
            {
              deviceClassRef: 'device_class.apple_silicon.m3_pro_18gb',
              deviceId: 'local-machine-123',
              max: 2060,
              metric: 'tokens_per_second',
              min: 1710,
              p50: 1900,
              p90: 2025,
              receiptRefs: ['receipt.cs336.a2.measurement.1'],
              sampleCount: 4,
              unit: 'tokens_per_second',
              verificationRefs: ['challenge.cs336.a2.class_check.1'],
              workClass: 'small_model_local_training',
            },
          ],
        },
      }),
    }

    expect(() =>
      publicDeviceCapabilityProjection({
        challenges: [],
        leases: [],
        run,
        windows: [],
      }),
    ).toThrow('device-identifying')
  })

  it('admits receipted measurement evidence into the run projection and enforces admissibility', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-admit',
      nowIso: '2026-06-11T08:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.demo',
      },
    })
    const measurement = {
      deviceClassRef: 'device_class.apple_silicon_macos.arm64',
      max: 2210,
      metric: 'tokens_per_second' as const,
      min: 1810,
      p50: 1995,
      p90: 2120,
      receiptRefs: ['receipt.cs336.a2.settlement.1'],
      sampleCount: 6,
      unit: 'tokens_per_second',
      verificationRefs: ['verdict.training.statistical_cross_check.1'],
      workClass: 'cs336_a2_device_benchmark',
    }

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-11T08:05:00.000Z',
      request: {
        measurements: [measurement],
        sourceRefs: ['issue.github.openagents.4681'],
      },
      run,
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(admitted.updatedAt).toBe('2026-06-11T08:05:00.000Z')
    expect(projection.classDistributions[0]).toMatchObject({
      crossCheckState: 'cross_checked',
      deviceClassRef: 'device_class.apple_silicon_macos.arm64',
      sampleCount: 6,
      verified: true,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.cs336_a2.requires_cross_machine_same_class_replication',
    ])

    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-11T08:05:00.000Z',
        request: { measurements: [] },
        run,
      }),
    ).toThrow('at least one measurement')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-11T08:05:00.000Z',
        request: { measurements: [{ ...measurement, receiptRefs: [] }] },
        run,
      }),
    ).toThrow('receipt ref')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-11T08:05:00.000Z',
        request: { measurements: [{ ...measurement, p50: 5000 }] },
        run,
      }),
    ).toThrow('min <= p50 <= p90 <= max')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-11T08:05:00.000Z',
        request: {
          measurements: [
            { ...measurement, sourceRefs: ['payment_hash.deadbeef'] },
          ],
        },
        run,
      }),
    ).toThrow('device-identifying or private material')
  })

  it('declares the host-RAM and sustained-vs-burst probe kinds in the qualification payload', () => {
    const payload = buildCs336A2DeviceBenchmarkPayload({
      assignmentRef: 'assignment.cs336.a2.device_benchmark.probe',
    })

    expect(payload.measurementKinds).toEqual([
      ...Cs336A2BenchmarkMeasurements,
      ...Cs336A2HostProbeMeasurements,
    ])
    expect(payload.measurementKinds).toContain('host_ram_headroom_gb')
    expect(payload.measurementKinds).toContain(
      'sustained_vs_burst_throughput_ratio',
    )
    expect(Cs336A2HostProbeMeasurements).toEqual([
      'host_ram_headroom_gb',
      'sustained_vs_burst_throughput_ratio',
    ])
  })

  it('admits host-RAM headroom and sustained-vs-burst evidence through the same admission and projection path', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-host-probe',
      nowIso: '2026-06-12T16:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.host_probe',
      },
    })
    const hostRamMeasurement = {
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      max: 101,
      metric: 'host_ram_headroom_gb' as const,
      min: 88,
      p50: 94,
      p90: 99,
      receiptRefs: ['receipt.cs336.a2.host_ram.1'],
      sameClassReplicationScope: 'cross_machine_same_class' as const,
      sampleCount: 5,
      unit: 'gigabytes',
      verificationRefs: ['verdict.training.statistical_cross_check.2'],
      workClass: 'work_class.example.optimizer_offload_training',
    }
    const sustainedMeasurement = {
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      max: 0.97,
      metric: 'sustained_vs_burst_throughput_ratio' as const,
      min: 0.84,
      p50: 0.91,
      p90: 0.95,
      receiptRefs: ['receipt.cs336.a2.sustained_ratio.1'],
      sameClassReplicationScope: 'cross_machine_same_class' as const,
      sampleCount: 5,
      unit: 'ratio',
      verificationRefs: ['verdict.training.statistical_cross_check.3'],
      workClass: 'work_class.example.sustained_collective_training',
    }

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-12T16:05:00.000Z',
      request: {
        measurements: [hostRamMeasurement, sustainedMeasurement],
        sourceRefs: ['issue.github.openagents.4852'],
      },
      run,
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(projection.observedMeasurementCount).toBe(2)
    expect(projection.thermalThrottleDetectionStatus).toBe(
      'thermal_throttle_not_observed',
    )
    expect(projection.thermalThrottleBlockerRefs).toEqual([])
    expect(projection.sameClassReplicationStatus).toBe(
      'cross_machine_replicated',
    )
    expect(projection.sameClassReplicationBlockerRefs).toEqual([])
    expect(projection.thermalThrottleSignals[0]).toMatchObject({
      deviceClassRef: 'device_class.example.rtx_4090_24gb_96gb_host',
      metric: 'sustained_vs_burst_throughput_ratio',
      p50Ratio: 0.91,
      ratioFloor: 0.8,
      reasonCode:
        'device_capability.public.thermal_throttle_not_observed_sustained_ratio_at_or_above_floor',
      state: 'thermal_throttle_not_observed',
      verified: true,
    })
    expect(projection.classDistributions.map(item => item.metric)).toEqual([
      'host_ram_headroom_gb',
      'sustained_vs_burst_throughput_ratio',
    ])
    expect(projection.classDistributions[0]).toMatchObject({
      crossCheckState: 'cross_checked',
      metric: 'host_ram_headroom_gb',
      p50: 94,
      unit: 'gigabytes',
      verified: true,
    })
    expect(JSON.stringify(projection)).not.toMatch(
      /pylonRef|deviceId|mnemonic|paymentHash/i,
    )

    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-12T16:05:00.000Z',
        request: {
          measurements: [
            { ...hostRamMeasurement, receiptRefs: ['wallet_path.leak'] },
          ],
        },
        run,
      }),
    ).toThrow('device-identifying or private material')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-12T16:05:00.000Z',
        request: {
          measurements: [{ ...sustainedMeasurement, unit: 'percent' }],
        },
        run,
      }),
    ).toThrow('unit ratio')
  })

  it('admits a genuinely measured but unsettled second device class without a settlement receipt or earning estimate', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-measured-unsettled',
      nowIso: '2026-06-20T00:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.x86_64_linux_intel',
      },
    })
    const measurement = {
      deviceClassRef: 'device_class.x86_64_linux.intel',
      digestCommitmentRefs: [
        'commitment.cs336_a2.attention_throughput.sha256_70b508a8a655e0b0',
      ],
      max: 3203.387,
      measurementProvenance: 'measured_unsettled' as const,
      metric: 'attention_throughput' as const,
      min: 174.2336,
      p50: 3109.9212,
      p90: 3169.043,
      receiptRefs: [],
      sampleCount: 24,
      unit: 'megaflops',
      verificationRefs: [],
      workClass: 'cs336_a2_device_benchmark',
    }

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-20T00:01:00.000Z',
      request: {
        measurements: [measurement],
        sourceRefs: ['tailnet.x86_64_linux_intel.self_characterization'],
      },
      run,
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(projection.observedDeviceClassCount).toBe(1)
    expect(projection.observedSettledDeviceClassCount).toBe(0)
    expect(projection.classDistributions[0]).toMatchObject({
      crossCheckState: 'measured_unverified',
      deviceClassRef: 'device_class.x86_64_linux.intel',
      measurementProvenance: 'measured_unsettled',
      sameClassReplicationScope: 'single_observation',
      sameClassReplicationStatus: 'single_observation',
      verified: false,
    })
    expect(projection.sameClassReplicationStatus).toBe('single_observation')
    expect(projection.sameClassReplicationBlockerRefs).toEqual([
      'blocker.cs336_a2.requires_cross_machine_same_class_replication',
    ])
    expect(projection.classDistributions[0]?.earningEstimate).toBeNull()
    expect(projection.classDistributions[0]?.receiptRefs).toEqual([])
    expect(projection.classDistributions[0]?.digestCommitmentRefs).toEqual([
      'commitment.cs336_a2.attention_throughput.sha256_70b508a8a655e0b0',
    ])
  })

  it('classifies measured thermal rows as needing verification until a cross-check verdict exists', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-measured-thermal',
      nowIso: '2026-06-20T00:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.measured_thermal',
      },
    })
    const measurement = {
      deviceClassRef: 'device_class.x86_64_linux.intel',
      digestCommitmentRefs: [
        'commitment.cs336_a2.sustained_ratio.sha256_thermal_probe',
      ],
      max: 0.7,
      measurementProvenance: 'measured_unsettled' as const,
      metric: 'sustained_vs_burst_throughput_ratio' as const,
      min: 0.48,
      p50: 0.62,
      p90: 0.68,
      receiptRefs: [],
      sampleCount: 24,
      unit: 'ratio',
      verificationRefs: [],
      workClass: 'cs336_a2_device_benchmark',
    }

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-20T00:01:00.000Z',
      request: { measurements: [measurement] },
      run,
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(projection.thermalThrottleDetectionStatus).toBe(
      'needs_verified_thermal_probe',
    )
    expect(projection.thermalThrottleBlockerRefs).toEqual([
      'blocker.cs336_a2.requires_verified_sustained_vs_burst_thermal_probe',
    ])
    expect(projection.thermalThrottleSignals).toHaveLength(1)
    expect(projection.thermalThrottleSignals[0]).toMatchObject({
      blockerRefs: [
        'blocker.cs336_a2.requires_verified_sustained_vs_burst_thermal_probe',
      ],
      measurementProvenance: 'measured_unsettled',
      p50Ratio: 0.62,
      reasonCode:
        'device_capability.public.thermal_probe_needs_statistical_cross_check',
      state: 'thermal_probe_needs_verification',
      verified: false,
    })
  })

  it('projects verified continuous thermal-throttle receipts and funnel reason codes', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-verified-thermal',
      nowIso: '2026-06-28T00:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.verified_thermal',
      },
    })
    const measurement = buildCs336A2ThermalThrottleMeasurementEvidence({
      deviceClassRef: 'device_class.example.gpu_24gb',
      digestCommitmentRefs: ['commitment.cs336_a2.thermal.sha256_demo'],
      receiptRefs: ['receipt.cs336_a2.thermal.verified_row.1'],
      samples: [
        { phase: 'burst', throughput: 100 },
        { phase: 'burst', throughput: 100 },
        { phase: 'burst', throughput: 120 },
        { phase: 'sustained', throughput: 70 },
        { phase: 'sustained', throughput: 78 },
        { phase: 'sustained', throughput: 74 },
      ],
      sourceRefs: ['artifact.cs336_a2.thermal_probe.window_samples.1'],
      verificationRefs: ['verdict.training.statistical_cross_check.thermal.1'],
      workClass: 'cs336_a2_device_benchmark',
    })

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-28T00:01:00.000Z',
      request: { measurements: [measurement] },
      run,
    })
    const projection = publicDeviceCapabilityProjection({
      challenges: [],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(projection.thermalThrottleDetectionStatus).toBe(
      'thermal_throttle_observed',
    )
    expect(projection.thermalThrottleBlockerRefs).toEqual([])
    expect(projection.thermalThrottleFunnelReasonCodes).toEqual([
      'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
    ])
    expect(projection.thermalThrottleReceiptRefs).toEqual([
      'receipt.cs336_a2.thermal.verified_row.1',
    ])
    expect(projection.thermalThrottleSignals[0]).toMatchObject({
      p50Ratio: 0.74,
      reasonCode:
        'device_capability.public.thermal_throttle_observed_sustained_ratio_below_floor',
      receiptRefs: ['receipt.cs336_a2.thermal.verified_row.1'],
      state: 'thermal_throttle_observed',
      verified: true,
    })
    expect(JSON.stringify(projection)).not.toMatch(
      /pylonRef|deviceId|mnemonic|paymentHash/i,
    )
  })

  it('does not let a run-level verdict mark a measured_unsettled row verified, and rejects unsettled rows that claim receipts, estimates, or no digest', () => {
    const run = buildTrainingRunRecord({
      makeId: () => 'a2-unsettled-guards',
      nowIso: '2026-06-20T00:00:00.000Z',
      request: {
        promiseRef: 'training.device_capability_dataset.v1',
        trainingRunRef: 'run.cs336.a2.device_capability.unsettled_guards',
      },
    })
    const baseMeasurement = {
      deviceClassRef: 'device_class.x86_64_linux.intel',
      digestCommitmentRefs: [
        'commitment.cs336_a2.memory_bandwidth.sha256_02d2cf92913ee000',
      ],
      max: 13.5337,
      measurementProvenance: 'measured_unsettled' as const,
      metric: 'memory_bandwidth' as const,
      min: 8.2227,
      p50: 10.6705,
      p90: 12.3328,
      receiptRefs: [],
      sampleCount: 24,
      unit: 'gigabytes_per_second',
      verificationRefs: [],
      workClass: 'cs336_a2_device_benchmark',
    }

    const admitted = admitCs336A2DeviceBenchmarkEvidence({
      nowIso: '2026-06-20T00:01:00.000Z',
      request: { measurements: [baseMeasurement] },
      run,
    })
    // A real Verified run-level challenge must NOT leak onto the unsettled row.
    const challenge = finalizeTrainingVerificationChallengeRecord({
      challenge: leaseTrainingVerificationChallengeRecord({
        challenge: buildTrainingVerificationChallengeRecord({
          makeId: () => 'challenge',
          nowIso: '2026-06-20T00:01:00.000Z',
          request: {
            commitmentRefs: ['commitment.cs336_a2.unrelated.1'],
            contributionRef: 'measurement.cs336_a2.unrelated',
            homeworkKind: 'admin_dispatched_homework',
            payload: { deviceClassRef: 'device_class.unrelated' },
            samplingPolicy: 'aggregate',
            trainingRunRef: run.trainingRunRef,
            verificationClass: 'statistical_cross_check',
            windowRef: 'training.window.unrelated.1',
          },
        }).challenge,
        eventId: 'lease',
        nowIso: '2026-06-20T00:02:00.000Z',
        request: { validatorRef: 'validator.unrelated' },
      }).challenge,
      eventId: 'final',
      nowIso: '2026-06-20T00:03:00.000Z',
      request: { receiptRefs: ['receipt.unrelated.verdict.1'] },
      verdict: {
        failureCodes: [],
        state: 'Verified',
        verdictRefs: ['verdict.unrelated.1'],
      },
    }).challenge
    const projection = publicDeviceCapabilityProjection({
      challenges: [challenge],
      leases: [],
      run: admitted,
      windows: [],
    })

    expect(projection.classDistributions[0]?.verified).toBe(false)
    expect(projection.classDistributions[0]?.crossCheckState).toBe(
      'measured_unverified',
    )
    expect(projection.classDistributions[0]?.verificationRefs).toEqual([])

    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-20T00:01:00.000Z',
        request: {
          measurements: [{ ...baseMeasurement, receiptRefs: ['receipt.leak.1'] }],
        },
        run,
      }),
    ).toThrow('must not carry a settlement receipt')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-20T00:01:00.000Z',
        request: {
          measurements: [
            {
              ...baseMeasurement,
              earningEstimate: { workClass: 'cs336_a2_device_benchmark' },
            },
          ],
        },
        run,
      }),
    ).toThrow('must not carry an earning estimate')
    expect(() =>
      admitCs336A2DeviceBenchmarkEvidence({
        nowIso: '2026-06-20T00:01:00.000Z',
        request: {
          measurements: [{ ...baseMeasurement, digestCommitmentRefs: [] }],
        },
        run,
      }),
    ).toThrow('digest-commitment ref')
  })
})
