import { describe, expect, test } from 'vitest'

const setupCheck = await import('./private-workspace-setup-check.mjs')

describe('private workspace setup checker', () => {
  test('parses setup flags and redacts transcript-unsafe values', () => {
    const parsed = setupCheck.parseSetupArgs([
      '--team-id',
      'team_private_partner',
      '--project-id',
      'project_private_partner',
      '--invite-id',
      'team_workspace_invite_123',
      '--session-ready',
      'yes',
      '--live-d1',
      '--remote',
    ])

    expect(parsed.flags.get('team-id')).toBe('team_private_partner')
    expect(parsed.flags.get('project-id')).toBe('project_private_partner')
    expect(parsed.flags.get('invite-id')).toBe('team_workspace_invite_123')
    expect(parsed.flags.get('session-ready')).toBe('yes')
    expect(parsed.flags.get('live-d1')).toBe(true)
    expect(parsed.flags.get('remote')).toBe(true)

    const redacted = setupCheck.redactTranscriptUnsafeText(
      [
        'Authorization: Bearer secret-token',
        'OPENAGENTS_ADMIN_API_TOKEN=secret',
        'email teammate@example.com',
        'https://openagents.com/api/team-workspace-invites/accept?token=oa_team_invite_secret',
        'team_private_partner',
      ].join('\n'),
      ['team_private_partner'],
    )

    expect(redacted).not.toContain('secret-token')
    expect(redacted).not.toContain('teammate@example.com')
    expect(redacted).not.toContain('oa_team_invite_secret')
    expect(redacted).not.toContain('team_private_partner')
    expect(redacted).toContain('<email:redacted>')
    expect(redacted).toContain('<redacted:value>')
  })

  test('summarizes a ready preflight without leaking ids or email addresses', async () => {
    const parsed = setupCheck.parseSetupArgs([
      '--team-id',
      'team_private_partner',
      '--project-id',
      'project_private_partner',
      '--invite-id',
      'team_workspace_invite_123',
      '--session-ready',
      'yes',
      '--live-d1',
      '--live-config',
    ])
    const calls: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = []
    const runCommand = (
      command: string,
      commandArgs: ReadonlyArray<string>,
    ) => {
      ;(calls as Array<readonly [string, ReadonlyArray<string>]>).push([
        command,
        commandArgs,
      ])
      const commandText = commandArgs.join(' ')

      if (commandText.includes('secret list')) {
        return JSON.stringify([
          { name: 'RESEND_API_KEY' },
          { name: 'RESEND_FROM_EMAIL' },
        ])
      }

      if (commandText.includes('FROM teams')) {
        return JSON.stringify([{ results: [{ count: 1 }] }])
      }

      if (commandText.includes('FROM team_projects')) {
        return JSON.stringify([{ results: [{ count: 1 }] }])
      }

      if (commandText.includes('FROM team_workspace_invites')) {
        return JSON.stringify([
          {
            results: [
              {
                email_message_id: 'email_message_1',
                has_email_message_id: 1,
                send_count: 1,
                status: 'pending',
              },
            ],
          },
        ])
      }

      if (commandText.includes('FROM email_messages')) {
        return JSON.stringify([
          {
            results: [
              {
                accepted_delivery_count: 1,
                delivery_count: 1,
                failed_delivery_count: 0,
                has_provider_message_id: 1,
                message_status: 'accepted',
                provider: 'resend',
              },
            ],
          },
        ])
      }

      throw new Error(`unexpected command: ${commandText}`)
    }
    const readTextFile = () => '"RESEND_REPLY_TO_EMAIL": "ops@example.invalid"'

    const observation = await setupCheck.collectWorkspaceSetupObservation(
      parsed,
      {
        env: { OPENAGENTS_ADMIN_API_TOKEN: 'secret' },
        readTextFile,
        runCommand,
      },
    )
    const summary = setupCheck.summarizeWorkspaceSetupPreflight(observation)
    const serialized = JSON.stringify(summary)

    expect(summary.state).toBe('ready')
    expect(summary.blockers).toEqual([])
    expect(summary.authority).toMatchObject({
      d1MutationAllowed: false,
      inviteTokenPrinted: false,
      rawEmailAddressPrinted: false,
    })
    expect(serialized).not.toContain('team_private_partner')
    expect(serialized).not.toContain('project_private_partner')
    expect(serialized).not.toContain('team_workspace_invite_123')
    expect(serialized).not.toContain('ops@example.invalid')
    expect(calls.some(([, args]) => args.includes('execute'))).toBe(true)
  })

  test('surfaces manual fallback blockers when email delivery is not ready', () => {
    const summary = setupCheck.summarizeWorkspaceSetupPreflight({
      adminTokenPresent: true,
      emailConfig: {
        apiKey: 'missing',
        fromEmail: 'missing',
        replyToEmail: 'missing',
        status: 'missing',
      },
      emailLedger: {
        checked: true,
        rows: [
          {
            accepted_delivery_count: 0,
            delivery_count: 1,
            failed_delivery_count: 1,
            message_status: 'failed',
            provider: 'resend',
          },
        ],
      },
      invite: {
        checked: true,
        rows: [
          {
            has_email_message_id: 1,
            send_count: 1,
            status: 'pending',
          },
        ],
      },
      project: { checked: true, rows: [{ count: 1 }] },
      requested: {
        hasEmailMessageId: true,
        hasInviteId: true,
        hasProjectId: true,
        hasTeamId: true,
        liveConfig: true,
        liveD1: true,
      },
      sessionState: 'missing',
      team: { checked: true, rows: [{ count: 1 }] },
    })

    expect(summary.state).toBe('blocked')
    expect(summary.blockers).toEqual(
      expect.arrayContaining([
        'browser_session',
        'email_config',
        'email_ledger',
      ]),
    )
    expect(summary.nextActions).toEqual(
      expect.arrayContaining([
        'Resend config is missing RESEND_API_KEY or RESEND_FROM_EMAIL.',
      ]),
    )
  })
})
