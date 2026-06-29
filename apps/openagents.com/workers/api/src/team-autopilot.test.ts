import { describe, expect, test, vi } from 'vitest'

import {
  selectedTeamFileIdsForAutopilotPrompt,
  teamAdjutantIntentFromBody,
  teamAutopilotAnswerBackDraft,
  teamAutopilotAnswerBackDraftForBundle,
  teamAutopilotChildRunGoal,
  teamAutopilotContextBundle,
  teamAutopilotPromptFromBody,
} from './index'
import type { AgentRunBundle } from './omni-runs'

const teamMessage = (
  id: string,
  body: string,
  createdAt: string,
  name = 'Christopher David',
) => ({
  agentRunId: null,
  author: {
    avatarUrl: null,
    githubUsername: 'AtlantisPleb',
    name,
    userId: `github:${id}`,
  },
  autopilotThreadId: null,
  body,
  createdAt,
  id,
  kind: 'message' as const,
  projectId: null,
  teamId: 'team_openagents_core',
})

const pdfFile = {
  contentType: 'application/pdf',
  createdAt: '2026-06-03T00:02:00.000Z',
  filename: 'deck.pdf',
  id: 'file_pdf_recent',
  sizeBytes: 1024,
}

describe('team Autopilot context', () => {
  test('selects explicit team file ids in hidden dispatch context', () => {
    const bundle = teamAutopilotContextBundle({
      files: [
        pdfFile,
        {
          contentType: 'text/plain',
          createdAt: '2026-06-03T00:03:00.000Z',
          filename: 'notes.txt',
          id: 'file_notes',
          sizeBytes: 32,
        },
      ],
      messages: [
        teamMessage(
          '14167547',
          'We uploaded the customer PDF.',
          '2026-06-03T00:00:00.000Z',
        ),
        teamMessage(
          '99',
          'Please summarize it for launch prep.',
          '2026-06-03T00:01:00.000Z',
          'Teammate',
        ),
      ],
      parentTeamChatMessageId: 'team_chat_parent',
      prompt: 'summarize the selected material',
      requestedFileIds: ['file_pdf_recent'],
      teamId: 'team_openagents_core',
    })
    const goal = teamAutopilotChildRunGoal(bundle)

    expect(bundle.selectedTeamFileIds).toEqual(['file_pdf_recent'])
    expect(bundle.selectedFiles).toMatchObject([
      {
        filename: 'deck.pdf',
        id: 'file_pdf_recent',
      },
    ])
    expect(goal).toContain('parentTeamId: team_openagents_core')
    expect(goal).toContain('parentTeamChatMessageId: team_chat_parent')
    expect(goal).toContain('selectedTeamFileIds: file_pdf_recent')
    expect(goal).toContain('selectedTeamFiles:')
    expect(goal).toContain('deck.pdf')
    expect(goal).toContain('Christopher David (github:14167547)')
  })

  test('does not select files from prompt wording alone', () => {
    expect(
      selectedTeamFileIdsForAutopilotPrompt({
        files: [
          {
            contentType: 'text/markdown',
            createdAt: '2026-06-03T00:04:00.000Z',
            filename: 'landing-page-spec.md',
            id: 'file_markdown_recent',
            sizeBytes: 21348,
          },
          pdfFile,
        ],
        prompt: 'summarize the file i just added',
      }),
    ).toEqual([])
  })

  test('honors explicit team file ids before metadata inference', () => {
    expect(
      selectedTeamFileIdsForAutopilotPrompt({
        files: [pdfFile],
        prompt: 'summarize the attachment',
        requestedFileIds: ['missing', 'file_pdf_recent'],
      }),
    ).toEqual(['file_pdf_recent'])
  })
})

describe('team Autopilot command parser', () => {
  test('removes exact leading @autopilot command text', () => {
    expect(teamAutopilotPromptFromBody('@autopilot Run the smoke test')).toBe(
      'Run the smoke test',
    )
  })

  test('removes exact trailing @autopilot command text', () => {
    expect(teamAutopilotPromptFromBody('Run the smoke test @autopilot')).toBe(
      'Run the smoke test',
    )
  })

  test('removes a standalone @autopilot command line', () => {
    expect(
      teamAutopilotPromptFromBody(
        'Run the smoke test\n@autopilot\nReport back here',
      ),
    ).toBe('Run the smoke test\nReport back here')
  })

  test('leaves non-command text unchanged', () => {
    expect(teamAutopilotPromptFromBody('email @autopilot@example.com')).toBe(
      'email @autopilot@example.com',
    )
  })
})

describe('team Adjutant command parser', () => {
  test('parses leading @autopilot with explicit software order context', () => {
    expect(
      teamAdjutantIntentFromBody(
        '@autopilot softwareOrderId: software_order_otec Build the Site',
      ),
    ).toEqual({
      schemaVersion: 'openagents.team_chat.adjutant_intent.v1',
      prompt: 'Build the Site',
      softwareOrderId: 'software_order_otec',
    })
  })

  test('keeps standalone @adjutant as a compatibility alias with explicit Site and task packet context', () => {
    expect(
      teamAdjutantIntentFromBody(
        'Adjust the hero section\n@adjutant\nsiteId: site_project_otec taskSpecPath: docs/autopilot-tasks/adjutant-otec.md',
      ),
    ).toEqual({
      schemaVersion: 'openagents.team_chat.adjutant_intent.v1',
      prompt: 'Adjust the hero section',
      siteId: 'site_project_otec',
      taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
    })
  })

  test('leaves non-command text unparsed', () => {
    expect(
      teamAdjutantIntentFromBody('email @adjutant@example.com'),
    ).toBeUndefined()
  })

  test('does not infer context from unbounded prompt wording', () => {
    expect(
      teamAdjutantIntentFromBody(
        '@autopilot Build the Ben OTEC website from the order',
      ),
    ).toEqual({
      schemaVersion: 'openagents.team_chat.adjutant_intent.v1',
      prompt: 'Build the Ben OTEC website from the order',
    })
  })
})

describe('team Autopilot answer-back', () => {
  test('uses the latest assistant text event as the team answer', () => {
    const bundle = {
      events: [
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:01.000Z',
          externalEventId: null,
          id: 'event_1',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            properties: {
              part: {
                id: 'text_1',
                text: 'I am reviewing the PDF.',
                type: 'text',
              },
            },
            type: 'message.part.updated',
          }),
          sequence: 1,
          source: 'runner',
          status: 'running',
          summary: 'stdout JSON event captured.',
          type: 'message.part.updated',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:02.000Z',
          externalEventId: null,
          id: 'event_2',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            properties: {
              part: {
                id: 'text_1',
                text: 'The PDF says revenue increased and churn fell.',
                type: 'text',
              },
            },
            type: 'message.part.updated',
          }),
          sequence: 2,
          source: 'runner',
          status: 'completed',
          summary: 'stdout JSON event captured.',
          type: 'message.part.updated',
        },
      ],
    }

    expect(teamAutopilotAnswerBackDraft(bundle)).toEqual({
      body: 'The PDF says revenue increased and churn fell.',
      sourceEventId: 'event_2',
    })
  })

  test('uses OpenCode text events instead of lifecycle completion summaries', () => {
    const bundle = {
      events: [
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:01.000Z',
          externalEventId: null,
          id: 'event_1',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              part: {
                id: 'text_1',
                text: 'I am Autopilot, the OpenAgents coding assistant.',
                type: 'text',
              },
              type: 'text',
            }),
          }),
          sequence: 1,
          source: 'runner',
          status: null,
          summary: 'stdout JSON event captured.',
          type: 'runner.text',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:02.000Z',
          externalEventId: null,
          id: 'event_2',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: null,
              receiptRefs: [],
            }),
          }),
          sequence: 2,
          source: 'runner',
          status: null,
          summary: 'OpenCode/Codex one-shot turn completed.',
          type: 'message.completed',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:03.000Z',
          externalEventId: null,
          id: 'event_3',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: 'codex workspace removed',
              receiptRefs: [],
            }),
          }),
          sequence: 3,
          source: 'runner',
          status: null,
          summary: 'Codex VM cleanup completed.',
          type: 'runner.cleanup',
        },
      ],
    }

    expect(teamAutopilotAnswerBackDraft(bundle)).toEqual({
      body: 'I am Autopilot, the OpenAgents coding assistant.',
      sourceEventId: 'event_1',
    })
  })

  test('keeps artifact bookkeeping out of the team answer-back', () => {
    const bundle = {
      events: [
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:01.000Z',
          externalEventId: null,
          id: 'event_1',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              part: {
                id: 'text_1',
                text: 'I’ll summarize the provided team files and prepare the required local completion artifacts without touching application code.',
                type: 'text',
              },
              type: 'text',
            }),
          }),
          sequence: 1,
          source: 'runner',
          status: null,
          summary: 'stdout JSON event captured.',
          type: 'runner.text',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:02.000Z',
          externalEventId: null,
          id: 'event_2',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              part: {
                id: 'text_2',
                text: 'I found only the two provided team-file references in this run: a small test text file and a detailed landing-page spec for `OpenAgents for Lawyers`.',
                type: 'text',
              },
              type: 'text',
            }),
          }),
          sequence: 2,
          source: 'runner',
          status: null,
          summary: 'stdout JSON event captured.',
          type: 'runner.text',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:03.000Z',
          externalEventId: null,
          id: 'event_3',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              part: {
                id: 'text_3',
                text: 'I’m adding concise local artifacts that record the requested summary and the run outcome.',
                type: 'text',
              },
              type: 'text',
            }),
          }),
          sequence: 3,
          source: 'runner',
          status: null,
          summary: 'stdout JSON event captured.',
          type: 'runner.text',
        },
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:04.000Z',
          externalEventId: null,
          id: 'event_4',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              part: {
                id: 'text_4',
                text: 'opencode run completed and closeout manifest submitted',
                type: 'text',
              },
              type: 'text',
            }),
          }),
          sequence: 4,
          source: 'runner',
          status: null,
          summary: 'stdout JSON event captured.',
          type: 'runner.text',
        },
      ],
    }

    expect(teamAutopilotAnswerBackDraft(bundle)).toEqual({
      body: 'I found only the two provided team-file references in this run: a small test text file and a detailed landing-page spec for `OpenAgents for Lawyers`.',
      sourceEventId: 'event_2',
    })
  })

  test('uses completed event detail when SHC promotes assistant text', () => {
    const bundle = {
      events: [
        {
          artifactRefs: [],
          createdAt: '2026-06-03T00:00:01.000Z',
          externalEventId: null,
          id: 'event_1',
          parentId: 'run_1',
          payloadJson: JSON.stringify({
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: 'I am Autopilot, the OpenAgents coding assistant.',
              receiptRefs: [],
            }),
          }),
          sequence: 1,
          source: 'runner',
          status: null,
          summary: 'Assistant message completed.',
          type: 'message.completed',
        },
      ],
    }

    expect(teamAutopilotAnswerBackDraft(bundle)).toEqual({
      body: 'I am Autopilot, the OpenAgents coding assistant.',
      sourceEventId: 'event_1',
    })
  })

  test('uses result artifact text before progress assistant text', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (url, init) => {
        const href =
          typeof url === 'string'
            ? url
            : url instanceof URL
              ? url.toString()
              : url.url

        expect(href).toBe(
          'https://api.github.com/repos/OpenAgentsInc/autopilot-omega/contents/result.md?ref=openagents%2Fartanis-answerback-smoke',
        )
        expect(init?.headers).toMatchObject({
          authorization: 'Bearer github-token',
        })

        return Response.json({
          content: btoa(
            'Artanis project chat answer-back is using assistant text, not cleanup logs.',
          ),
          encoding: 'base64',
        })
      })

    try {
      const bundle = {
        events: [
          {
            artifactRefs: [],
            createdAt: '2026-06-03T00:00:01.000Z',
            externalEventId: null,
            id: 'event_1',
            parentId: 'run_1',
            payloadJson: JSON.stringify({
              dataJson: JSON.stringify({
                part: {
                  id: 'text_1',
                  text: 'The worktree is clean on `main`; I am creating the requested result artifact.',
                  type: 'text',
                },
                type: 'text',
              }),
            }),
            sequence: 1,
            source: 'runner',
            status: null,
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
          {
            artifactRefs: ['sha256:result'],
            createdAt: '2026-06-03T00:00:02.000Z',
            externalEventId: null,
            id: 'event_2',
            parentId: 'run_1',
            payloadJson: JSON.stringify({
              dataJson: JSON.stringify({
                artifactRefs: ['sha256:result'],
                detail: 'result.md',
                receiptRefs: [],
              }),
            }),
            sequence: 2,
            source: 'runner',
            status: null,
            summary: 'Codex artifact captured.',
            type: 'artifact.created',
          },
        ],
        run: {
          archivedAt: null,
          assignment: {
            artifactPolicy: 'metadata_only',
            assignmentKind: 'workroom_agent',
            backend: 'shc_vm',
            callback: {
              tokenRef: 'callback-token-ref',
              url: 'https://nexus.openagents.com/callback',
            },
            goal: 'Answer the team chat',
            modelProfile: {
              kind: 'codex',
              model: 'gpt-5',
              provider: 'openai',
            },
            repository: {
              owner: 'OpenAgentsInc',
              provider: 'github',
              ref: 'main',
              repo: 'autopilot-omega',
            },
            retentionMode: 'openagents_durable',
            runId: 'run_1',
            runtime: 'opencode_codex',
            sandbox: {
              mode: 'workspace_write',
              network: 'enabled',
              timeoutMs: 600000,
            },
            schemaVersion: 'openagents.agent_run_assignment.v1',
            trainingUse: 'denied',
            githubWorkOrder: {
              baseRef: 'main',
              branchName: 'openagents/artanis-answerback-smoke',
              commitMessage: 'Answer team chat',
              provider: 'github',
              repository: {
                owner: 'OpenAgentsInc',
                provider: 'github',
                ref: 'main',
                repo: 'autopilot-omega',
              },
              writeback: {
                commentOnIssue: false,
                openPullRequest: false,
                pushBranch: false,
              },
            },
          },
          assignmentKind: 'workroom_agent',
          authGrantRef: null,
          backend: 'shc_vm',
          canceledAt: null,
          completedAt: '2026-06-03T00:00:03.000Z',
          createdAt: '2026-06-03T00:00:00.000Z',
          eventCursor: 2,
          externalRunId: null,
          failedAt: null,
          goal: 'Answer the team chat',
          goalId: null,
          id: 'run_1',
          projectId: 'project_artanis',
          providerAccountRef: null,
          repository: {
            owner: 'OpenAgentsInc',
            provider: 'github',
            ref: 'main',
            repo: 'autopilot-omega',
          },
          runnerId: 'runner_1',
          runtime: 'opencode_codex',
          startedAt: '2026-06-03T00:00:00.000Z',
          status: 'completed',
          teamId: 'team_openagents_core',
          updatedAt: '2026-06-03T00:00:03.000Z',
          userId: 'github:14167547',
        },
      } satisfies AgentRunBundle

      await expect(
        teamAutopilotAnswerBackDraftForBundle(bundle, {
          githubAccessToken: 'github-token',
        }),
      ).resolves.toEqual({
        body: 'Artanis project chat answer-back is using assistant text, not cleanup logs.',
        sourceEventId: 'event_2',
      })
      expect(fetchMock).toHaveBeenCalledTimes(1)
    } finally {
      fetchMock.mockRestore()
    }
  })
})
