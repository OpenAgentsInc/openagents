import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  agentRunExternalRefFromNullable,
  optionFromNullableString,
  runDurationFromNullable,
  threadFileDetailFromDto,
  threadFileOwnershipFromNullableTeamId,
  threadFileOwnershipTeamId,
  threadFileRecordFromDto,
  type ThreadFileApiRecord,
  type ThreadFileDetailApiRecord,
} from './model'

const apiThreadFile = (
  teamId: string | null,
): ThreadFileApiRecord => ({
  contentType: 'text/plain',
  createdAt: '2026-06-04T00:00:00.000Z',
  downloadUrl: '/api/thread-files/file_1/download',
  filename: 'notes.txt',
  id: 'file_1',
  ownerUserId: 'github:14167547',
  scope: teamId === null ? 'personal' : 'team',
  sizeBytes: 12,
  teamId,
  threadId: 'thread_1',
})

describe('logged-in model boundary conversion', () => {
  test('converts agent run external refs into tagged state', () => {
    expect(agentRunExternalRefFromNullable('shc:runner:run_1')).toEqual({
      _tag: 'AgentRunExternalRefPresent',
      value: 'shc:runner:run_1',
    })
    expect(agentRunExternalRefFromNullable(null)).toEqual({
      _tag: 'AgentRunExternalRefMissing',
    })
    expect(agentRunExternalRefFromNullable('')).toEqual({
      _tag: 'AgentRunExternalRefMissing',
    })
  })

  test('converts run durations into known or unknown states', () => {
    expect(runDurationFromNullable(12.6)).toEqual({
      _tag: 'RunDurationKnownSeconds',
      seconds: 13,
    })
    expect(runDurationFromNullable(null)).toEqual({
      _tag: 'RunDurationUnknown',
    })
    expect(runDurationFromNullable(Number.NaN)).toEqual({
      _tag: 'RunDurationUnknown',
    })
  })

  test('converts nullable optional strings into Option', () => {
    expect(Option.getOrUndefined(optionFromNullableString('runner-event-1'))).toBe(
      'runner-event-1',
    )
    expect(Option.isNone(optionFromNullableString(null))).toBe(true)
    expect(Option.isNone(optionFromNullableString(''))).toBe(true)
  })

  test('converts thread-file ownership from DTO team IDs', () => {
    expect(threadFileOwnershipFromNullableTeamId('team_openagents_core')).toEqual(
      {
        _tag: 'ThreadFileOwnershipTeam',
        teamId: 'team_openagents_core',
      },
    )
    expect(threadFileOwnershipFromNullableTeamId(null)).toEqual({
      _tag: 'ThreadFileOwnershipPersonal',
    })
    expect(threadFileOwnershipFromNullableTeamId('')).toEqual({
      _tag: 'ThreadFileOwnershipPersonal',
    })
  })

  test('converts thread-file DTO records into internal ownership state', () => {
    const teamFile = threadFileRecordFromDto(
      apiThreadFile('team_openagents_core'),
    )
    const personalFile = threadFileRecordFromDto(apiThreadFile(null))

    expect(threadFileOwnershipTeamId(teamFile.ownership)).toBe(
      'team_openagents_core',
    )
    expect(threadFileOwnershipTeamId(personalFile.ownership)).toBeUndefined()
  })

  test('converts thread-file details and references into internal ownership state', () => {
    const detail = threadFileDetailFromDto({
      canManage: true,
      file: apiThreadFile('team_openagents_core'),
      references: [
        {
          author: {
            avatarUrl: null,
            githubUsername: null,
            name: 'Christopher David',
            userId: 'github:14167547',
          },
          body: 'Inspect notes.txt',
          createdAt: '2026-06-04T00:00:01.000Z',
          excerpt: 'Inspect notes.txt',
          fileId: 'file_1',
          href: '/teams/openagents-core-team/chat#message-team_chat_1',
          id: 'thread_file_ref_1',
          messageId: 'team_chat_1',
          messageKind: 'message',
          referenceKind: 'message_attachment',
          teamId: 'team_openagents_core',
          threadId: 'thread_1',
        },
      ],
    } satisfies ThreadFileDetailApiRecord)

    expect(threadFileOwnershipTeamId(detail.file.ownership)).toBe(
      'team_openagents_core',
    )
    expect(threadFileOwnershipTeamId(detail.references[0]!.ownership)).toBe(
      'team_openagents_core',
    )
  })
})
