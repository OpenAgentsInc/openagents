import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PylonLocalAgentCommandPacket,
  PylonLocalAgentCommandPacketProjection,
  PylonResourceModeSetupProjection,
  PylonResourceModeSetupUnsafe,
  examplePylonResourceModeSetupPlan,
  projectPylonLocalAgentCommandPacket,
  projectPylonResourceModeSetupPlan,
  pylonLocalAgentCommandPacketProjectionHasPrivateMaterial,
  pylonLocalAgentCommandPacketsFromSetupPlan,
  pylonResourceModeSetupProjectionHasPrivateMaterial,
} from './pylon-resource-mode-setup'

const nowIso = '2026-06-07T00:10:00.000Z'

describe('Pylon resource mode setup', () => {
  test('projects public-safe background, balanced, overnight, and dedicated modes', () => {
    const projection = projectPylonResourceModeSetupPlan(
      examplePylonResourceModeSetupPlan(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(PylonResourceModeSetupProjection)(projection))
      .toEqual(projection)
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(projection.modes.map(mode => [
      mode.mode,
      mode.family,
      mode.envelope.cpuPercentMax,
      mode.envelope.gpuPercentMax,
    ])).toEqual([
      ['background_20', 'background', 20, 0],
      ['balanced', 'balanced', 50, 40],
      ['overnight_full', 'overnight', 90, 90],
      ['dedicated_full_blast', 'dedicated', 100, 100],
    ])
    expect(projection.modes.flatMap(mode => mode.ownerApprovalRefs)).toEqual(
      expect.arrayContaining(['approval.public.owner.local_compute_pylon']),
    )
    expect(projection.commandRecords.every(
      command => command.privateEvidenceRefs.length === 0,
    )).toBe(true)
    expect(pylonResourceModeSetupProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps runtime and readiness command output private for operators', () => {
    const projection = projectPylonResourceModeSetupPlan(
      examplePylonResourceModeSetupPlan(),
      'operator',
      nowIso,
    )

    expect(projection.commandRecords.map(command => command.kind)).toEqual([
      'install_launcher',
      'launch_pylon',
      'set_resource_mode',
      'version_check',
      'runtime_status',
      'training_status',
      'balance_check',
      'history_check',
    ])
    expect(projection.commandRecords.flatMap(command => command.privateEvidenceRefs))
      .toEqual(
        expect.arrayContaining([
          'evidence.private.pylon.runtime_status',
          'evidence.private.pylon.training_status',
          'evidence.private.pylon.balance_check',
          'evidence.private.pylon.history_check',
        ]),
      )
    expect(projection.commandRecords.every(
      command => command.evidenceHandlingRef ===
        'evidence_handling.private_by_default',
    )).toBe(true)
  })

  test('connects modes to work-routing and eligibility caveats without implying eligibility', () => {
    const projection = projectPylonResourceModeSetupPlan(
      examplePylonResourceModeSetupPlan(),
      'public',
      nowIso,
    )
    const background = projection.modes.find(
      mode => mode.mode === 'background_20',
    )
    const dedicated = projection.modes.find(
      mode => mode.mode === 'dedicated_full_blast',
    )

    expect(background?.workRoutingRefs).toContain(
      'routing.public.artanis.background_inference_probe',
    )
    expect(background?.eligibilityCaveatRefs).toContain(
      'caveat.public.online_is_not_eligible',
    )
    expect(dedicated?.eligibilityCaveatRefs).toContain(
      'caveat.public.settlement_not_implied',
    )
  })

  test('generates local-agent command packets for every resource mode', () => {
    const packets = pylonLocalAgentCommandPacketsFromSetupPlan(
      examplePylonResourceModeSetupPlan(),
      nowIso,
    )
    const publicProjections = packets.map(packet =>
      projectPylonLocalAgentCommandPacket(packet, 'public', nowIso),
    )
    const operatorProjections = packets.map(packet =>
      projectPylonLocalAgentCommandPacket(packet, 'operator', nowIso),
    )

    expect(packets.map(packet => packet.mode)).toEqual([
      'background_20',
      'balanced',
      'overnight_full',
      'dedicated_full_blast',
    ])
    expect(S.decodeUnknownSync(PylonLocalAgentCommandPacketProjection)(
      publicProjections[0],
    )).toEqual(publicProjections[0])
    expect(publicProjections.map(packet => packet.state))
      .toEqual([
        'dry_run_ready',
        'dry_run_ready',
        'dry_run_ready',
        'dry_run_ready',
      ])
    expect(publicProjections.every(packet => !packet.localExecutionAllowed))
      .toBe(true)
    expect(publicProjections.every(
      packet => packet.ownerApprovalPromptRef.startsWith(
        'approval_prompt.public.pylon.local_agent.',
      ),
    )).toBe(true)
    expect(publicProjections.every(
      packet => packet.dryRunOutputEvidenceRefs.length === 0,
    )).toBe(true)
    expect(operatorProjections.flatMap(
      packet => packet.dryRunOutputEvidenceRefs,
    )).toEqual(
      expect.arrayContaining([
        'evidence.private.pylon.dry_run.background_20',
        'evidence.private.pylon.dry_run.overnight_full',
        'evidence.private.pylon.dry_run.dedicated_full_blast',
      ]),
    )
    expect(publicProjections[0]?.resourceIntent).toMatchObject({
      cpuPercentMax: 20,
      gpuPercentMax: 0,
      memoryPercentMax: 20,
      networkIntentRef: 'network.public.pylon.low',
      storageIntentRef: 'disk.public.pylon.low_cache',
    })
    expect(publicProjections[2]?.checkpointExpectationRefs).toEqual([
      'checkpoint.public.pylon.resume_after_window',
    ])
    expect(publicProjections[3]?.pauseResumeExpectationRefs).toEqual([
      'pause.public.operator_managed',
    ])
    expect(publicProjections.every(
      packet => pylonLocalAgentCommandPacketProjectionHasPrivateMaterial(packet) === false,
    )).toBe(true)
  })

  test('rejects local-agent packets without owner approval, dry-run evidence, or approved execution state', () => {
    const packet = pylonLocalAgentCommandPacketsFromSetupPlan(
      examplePylonResourceModeSetupPlan(),
      nowIso,
    )[0]!

    expect(() =>
      projectPylonLocalAgentCommandPacket(
        new PylonLocalAgentCommandPacket({
          ...packet,
          ownerApprovalRefs: [],
        }),
        'public',
        nowIso,
      ),
    ).toThrow(PylonResourceModeSetupUnsafe)
    expect(() =>
      projectPylonLocalAgentCommandPacket(
        new PylonLocalAgentCommandPacket({
          ...packet,
          dryRunOutputEvidenceRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonResourceModeSetupUnsafe)
    expect(() =>
      projectPylonLocalAgentCommandPacket(
        new PylonLocalAgentCommandPacket({
          ...packet,
          localExecutionAllowed: true,
          state: 'dry_run_ready',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(PylonResourceModeSetupUnsafe)
  })

  test('rejects local-agent packet secrets, raw outputs, private paths, and unconditional earning claims', () => {
    const packet = pylonLocalAgentCommandPacketsFromSetupPlan(
      examplePylonResourceModeSetupPlan(),
      nowIso,
    )[0]!

    for (
      const unsafePacket of [
        new PylonLocalAgentCommandPacket({
          ...packet,
          dryRunOutputEvidenceRefs: ['/Users/chris/pylon/status.log'],
        }),
        new PylonLocalAgentCommandPacket({
          ...packet,
          dryRunOutputEvidenceRefs: ['wallet.local.seed'],
        }),
        new PylonLocalAgentCommandPacket({
          ...packet,
          dryRunOutputEvidenceRefs: ['provider_credential.local'],
        }),
        new PylonLocalAgentCommandPacket({
          ...packet,
          dryRunOutputEvidenceRefs: ['raw_command_output.pylon.status'],
        }),
        new PylonLocalAgentCommandPacket({
          ...packet,
          earningCaveatRefs: ['caveat.public.run_pylon_and_earn_money'],
        }),
      ]
    ) {
      expect(() =>
        projectPylonLocalAgentCommandPacket(
          unsafePacket,
          'operator',
          nowIso,
        ),
      ).toThrow(PylonResourceModeSetupUnsafe)
    }
  })

  test('rejects setup commands without owner approval or private evidence refs', () => {
    const input = examplePylonResourceModeSetupPlan()

    expect(() =>
      projectPylonResourceModeSetupPlan({
        ...input,
        commandRecords: input.commandRecords.map(command =>
          command.kind === 'runtime_status'
            ? { ...command, ownerApprovalRefs: [] }
            : command
        ),
      }, 'public', nowIso),
    ).toThrow(PylonResourceModeSetupUnsafe)

    expect(() =>
      projectPylonResourceModeSetupPlan({
        ...input,
        commandRecords: input.commandRecords.map(command =>
          command.kind === 'runtime_status'
            ? { ...command, privateEvidenceRefs: [] }
            : command
        ),
      }, 'operator', nowIso),
    ).toThrow(PylonResourceModeSetupUnsafe)
  })

  test('rejects missing modes and command coverage', () => {
    const input = examplePylonResourceModeSetupPlan()

    expect(() =>
      projectPylonResourceModeSetupPlan({
        ...input,
        modes: input.modes.filter(mode => mode.mode !== 'overnight_full'),
      }, 'public', nowIso),
    ).toThrow(PylonResourceModeSetupUnsafe)

    expect(() =>
      projectPylonResourceModeSetupPlan({
        ...input,
        commandRecords: input.commandRecords.filter(
          command => command.kind !== 'history_check',
        ),
      }, 'public', nowIso),
    ).toThrow(PylonResourceModeSetupUnsafe)
  })

  test('rejects raw local paths, wallet material, provider credentials, and raw command output refs', () => {
    const input = examplePylonResourceModeSetupPlan()

    for (
      const unsafeRef of [
        '/Users/chris/.openagents/pylon',
        'wallet.local.seed',
        'provider_credential.local',
        'raw_command_output.pylon.status',
      ]
    ) {
      expect(() =>
        projectPylonResourceModeSetupPlan({
          ...input,
          commandRecords: input.commandRecords.map(command =>
            command.kind === 'runtime_status'
              ? { ...command, privateEvidenceRefs: [unsafeRef] }
              : command
          ),
        }, 'operator', nowIso),
      ).toThrow(PylonResourceModeSetupUnsafe)
    }
  })
})
