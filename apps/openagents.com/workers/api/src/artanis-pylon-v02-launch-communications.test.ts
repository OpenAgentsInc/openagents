import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisPylonV02LaunchCommunicationProjection,
  ArtanisPylonV02LaunchCommunicationRecord,
  ArtanisPylonV02LaunchCommunicationUnsafe,
  exampleArtanisPylonV02LaunchCommunicationRecord,
  projectArtanisPylonV02LaunchCommunication,
} from './artanis-pylon-v02-launch-communications'
import {
  exampleArtanisPylonV02Readiness,
  projectArtanisPylonV02Readiness,
} from './artanis-pylon-v02-readiness'

const nowIso = '2026-06-07T03:05:00.000Z'

const readiness = () =>
  projectArtanisPylonV02Readiness(
    exampleArtanisPylonV02Readiness(),
    'public',
    nowIso,
  )

const launchRecord = (
  overrides: Partial<ArtanisPylonV02LaunchCommunicationRecord> = {},
): ArtanisPylonV02LaunchCommunicationRecord =>
  S.decodeUnknownSync(ArtanisPylonV02LaunchCommunicationRecord)({
    ...exampleArtanisPylonV02LaunchCommunicationRecord(),
    ...overrides,
  })

describe('Artanis Pylon v0.2 launch communications', () => {
  test('projects a public-safe launch package for Forum, docs, /artanis, and social copy', () => {
    const projection = projectArtanisPylonV02LaunchCommunication(
      exampleArtanisPylonV02LaunchCommunicationRecord(),
      readiness(),
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisPylonV02LaunchCommunicationProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      agentRef: 'agent_artanis',
      forumIntentReady: true,
      forumIntentRef: 'intent.public.artanis.pylon_v0_2_launch_communication',
      forumPostTitle: 'Artanis Pylon v0.2 launch readiness update',
      launchPackageRef: 'launch.public.artanis.pylon_v0_2.communication',
      primaryForumTopicRef:
        'topic.public.forum.artanis.pylon_release_work_log',
      primaryForumTopicUrl:
        'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888',
      readinessRef: 'readiness.public.artanis.pylon_v0_2',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.artanisPageRefs).toEqual([
      'https://openagents.com/artanis',
    ])
    expect(projection.capabilityRefs).toEqual([
      'capability.public.pylon.accepted_work_contribution',
      'capability.public.pylon.fine_tuning_training',
      'capability.public.pylon.inference',
      'capability.public.pylon.marketplace_jobs_planned',
      'capability.public.pylon.optimization',
      'capability.public.pylon.validation',
    ])
    expect(projection.stageSummaryRefs).toEqual(
      expect.arrayContaining([
        'stage_summary.public.pylon_v0_2.source_ready.verified',
        'stage_summary.public.pylon_v0_2.release_ready.blocked',
        'stage_summary.public.pylon_v0_2.platform_ready.blocked',
        'stage_summary.public.pylon_v0_2.eligible.planned',
        'stage_summary.public.pylon_v0_2.accepted.prohibited',
        'stage_summary.public.pylon_v0_2.paid.prohibited',
        'stage_summary.public.pylon_v0_2.settled.prohibited',
      ]),
    )
    expect(projection.forumPostBody).toContain('inference')
    expect(projection.forumPostBody).toContain('optimization')
    expect(projection.forumPostBody).toContain('fine-tuning/training')
    expect(projection.forumPostBody).toContain('validation')
    expect(projection.forumPostBody).toContain('accepted-work contribution')
    expect(projection.forumPostBody).toContain('marketplace jobs')
    expect(JSON.stringify(projection)).not.toContain('general availability')
    expect(JSON.stringify(projection)).not.toContain('earn money')
    expect(JSON.stringify(projection)).not.toContain('lnbc')
    expect(JSON.stringify(projection)).not.toContain('preimage')
    expect(JSON.stringify(projection)).not.toContain('private key')
    expect(JSON.stringify(projection)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('rejects launch, earning, settlement, and wallet overclaims', () => {
    expect(() =>
      projectArtanisPylonV02LaunchCommunication(
        launchRecord({
          optionalSocialCopy:
            'Pylon v0.2 is ready for everyone. Run Pylon and earn money.',
        }),
        readiness(),
        nowIso,
      ),
    ).toThrow(ArtanisPylonV02LaunchCommunicationUnsafe)
    expect(() =>
      projectArtanisPylonV02LaunchCommunication(
        launchRecord({
          forumIntent: {
            ...exampleArtanisPylonV02LaunchCommunicationRecord().forumIntent,
            bodyText:
              'Accepted work is already paid and paid work is settled.',
          },
        }),
        readiness(),
        nowIso,
      ),
    ).toThrow(ArtanisPylonV02LaunchCommunicationUnsafe)
  })

  test('requires all capability and readiness-stage refs', () => {
    const base = exampleArtanisPylonV02LaunchCommunicationRecord()

    expect(() =>
      projectArtanisPylonV02LaunchCommunication(
        launchRecord({
          capabilityRefs: base.capabilityRefs.filter(
            ref => ref !== 'capability.public.pylon.validation',
          ),
        }),
        readiness(),
        nowIso,
      ),
    ).toThrow(ArtanisPylonV02LaunchCommunicationUnsafe)
    expect(() =>
      projectArtanisPylonV02LaunchCommunication(
        launchRecord({
          readinessStageRefs: base.readinessStageRefs.filter(
            ref => ref !== 'stage.public.pylon_v0_2.settled',
          ),
        }),
        readiness(),
        nowIso,
      ),
    ).toThrow(ArtanisPylonV02LaunchCommunicationUnsafe)
  })

  test('rejects unsafe refs and private material in launch surfaces', () => {
    for (const unsafe of [
      launchRecord({ docsPageRefs: ['https://github.com/org/private-repo'] }),
      launchRecord({ sourceRefs: ['source.public.2026-06-07T03:00:00Z'] }),
      launchRecord({ ownerSetupRefs: ['wallet.secret.seed'] }),
      launchRecord({
        primaryForumTopicUrl:
          'https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888?token=secret',
      }),
      launchRecord({ briefMarkdown: 'Share a private key before setup.' }),
    ]) {
      expect(() =>
        projectArtanisPylonV02LaunchCommunication(unsafe, readiness(), nowIso),
      ).toThrow(ArtanisPylonV02LaunchCommunicationUnsafe)
    }
  })
})
