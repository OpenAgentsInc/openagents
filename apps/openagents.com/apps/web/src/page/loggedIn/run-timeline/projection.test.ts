import { describe, expect, test } from 'vitest'

import {
  ActiveChatRun,
  type ChatRunEvent,
  agentRunExternalRefFromNullable,
  optionFromNullableString,
} from '../model'
import { chatRunTimelineMessages } from './projection'

const event = (
  sequence: number,
  payload: Record<string, unknown>,
  options: Readonly<{
    artifactRefs?: ReadonlyArray<string>
    status?: string | null
    summary?: string
    type?: string
  }> = {},
): ChatRunEvent => ({
  artifactRefs: [...(options.artifactRefs ?? [])],
  createdAt: `2026-06-03T00:00:${String(sequence).padStart(2, '0')}.000Z`,
  externalEventId: optionFromNullableString(`runner-event-${sequence}`),
  id: `event_${sequence}`,
  payloadJson: optionFromNullableString(JSON.stringify(payload)),
  sequence,
  source: 'runner',
  status: optionFromNullableString(options.status),
  summary: options.summary ?? 'Runner event received.',
  tokenModel: optionFromNullableString(undefined),
  tokenProvider: optionFromNullableString(undefined),
  tokenTotal: 0,
  type: options.type ?? 'runner.event',
})

const run = ActiveChatRun({
  events: [
    event(
      1,
      {
        properties: {
          callID: 'shell-call-1',
          command: 'git status --short',
        },
        type: 'session.next.shell.started',
      },
      { status: 'running' },
    ),
    event(
      2,
      {
        properties: {
          part: {
            id: 'assistant-text-1',
            text: "I'll inspect the repo.",
            type: 'text',
          },
        },
        type: 'message.part.updated',
      },
      { type: 'message.part.updated' },
    ),
    event(
      3,
      {
        properties: {
          callID: 'shell-call-1',
          output: '## main...origin/main',
        },
        type: 'session.next.shell.ended',
      },
      { status: 'completed' },
    ),
    event(
      4,
      {
        properties: {
          part: {
            id: 'assistant-text-1',
            text: "I'll inspect the repo.\n\nThe workspace is clean.",
            type: 'text',
          },
        },
        type: 'message.part.updated',
      },
      { type: 'message.part.updated' },
    ),
    event(
      5,
      {
        properties: {
          callID: 'shell-call-2',
          command: 'git branch --show-current',
        },
        type: 'session.next.shell.started',
      },
      { status: 'running' },
    ),
    event(
      6,
      {
        properties: {
          callID: 'shell-call-2',
          output: 'main',
        },
        type: 'session.next.shell.ended',
      },
      { status: 'completed' },
    ),
  ],
  metadata: {
    backend: 'shc_vm',
    createdAt: '2026-06-03T00:00:00.000Z',
    displayRunId: '11111111-1111-4111-8111-111111111111',
    eventCursor: 6,
    externalRunRef: agentRunExternalRefFromNullable(
      'shc:oa-shc-katy-01:11111111-1111-4111-8111-111111111111',
    ),
    goal: 'Summarize your environment with three tool calls max',
    repository: 'OpenAgentsInc/autopilot-omega@main',
    runId: '11111111-1111-4111-8111-111111111111',
    runnerId: 'oa-shc-katy-01',
    runtime: 'opencode_codex',
    status: 'completed',
    statusUrl: '/api/omni/agent-runs/11111111-1111-4111-8111-111111111111',
    streamUrl:
      '/api/omni/agent-runs/11111111-1111-4111-8111-111111111111/events',
    tokenTotal: 0,
    tokenUsageEvents: 0,
    updatedAt: '2026-06-03T00:00:06.000Z',
  },
})

describe('chat run timeline projection', () => {
  test('keeps assistant text and tool calls chronological while collapsing tool lifecycle updates', () => {
    const [message] = chatRunTimelineMessages(run)
    const transcript = message?.parts.slice(1) ?? []

    expect(
      transcript.map(part => {
        if (part.kind === 'tool') {
          return part.title
        }

        if (part.kind === 'text') {
          return part.body.join('\n')
        }

        return part.kind
      }),
    ).toEqual([
      'Shell command',
      "I'll inspect the repo.\nThe workspace is clean.",
      'Shell command',
    ])

    expect(transcript.filter(part => part.kind === 'tool')).toHaveLength(2)
    expect(transcript[0]).toMatchObject({
      detail: ['$ git status --short', '## main...origin/main'],
      kind: 'tool',
      status: 'completed',
    })
    expect(transcript[2]).toMatchObject({
      detail: ['$ git branch --show-current', 'main'],
      kind: 'tool',
      status: 'completed',
    })
  })

  test('replaces partial streaming deltas with the final text part', () => {
    const streamingRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            properties: {
              delta: 'Working...',
              part: {
                id: 'assistant-text-stream',
                type: 'text',
              },
            },
            type: 'message.part.delta',
          },
          { type: 'message.part.delta' },
        ),
        event(
          2,
          {
            properties: {
              part: {
                id: 'assistant-text-stream',
                text: 'Working...\n\nFinished.',
                type: 'text',
              },
            },
            type: 'message.part.updated',
          },
          { type: 'message.part.updated' },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 2,
        status: 'running',
      },
    })

    const [message] = chatRunTimelineMessages(streamingRun)
    const text =
      message?.parts
        .filter(part => part.kind === 'text')
        .flatMap(part => part.body) ?? []

    expect(text).toEqual(['Working...', 'Finished.'])
    expect(text).not.toEqual(['Working...'])
    expect(message?.status).toBe('streaming')
  })

  test('shows failure details from runner payloads instead of generic failed-event text', () => {
    const failedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            error: {
              message: 'opencode binary was not found on SHC runner',
            },
            exitCode: 127,
            type: 'runner.dispatch_failed',
          },
          {
            status: 'failed',
            summary: 'Codex reported a failure event.',
            type: 'runner.dispatch_failed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 1,
        status: 'failed',
        tokenTotal: 0,
      },
    })

    const [message] = chatRunTimelineMessages(failedRun)
    const transcript = message?.parts ?? []

    expect(
      transcript.some(
        part =>
          part.kind === 'tool' &&
          part.detail.includes(
            'Failed: opencode binary was not found on SHC runner',
          ),
      ),
    ).toBe(true)
    expect(
      transcript.some(
        part =>
          part.kind === 'tool' &&
          part.detail.includes('Failed: Codex reported a failure event.'),
      ),
    ).toBe(false)
  })

  test('renders OpenCode text events instead of generic completion summaries', () => {
    const completedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_1',
                text: 'I am Autopilot, the OpenAgents coding assistant.',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
        event(
          2,
          {
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: null,
              receiptRefs: [],
            }),
          },
          {
            summary: 'OpenCode/Codex one-shot turn completed.',
            type: 'message.completed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 2,
        status: 'completed',
      },
    })

    const [message] = chatRunTimelineMessages(completedRun)
    const text =
      message?.parts
        .filter(part => part.kind === 'text')
        .flatMap(part => part.body) ?? []

    expect(text).toEqual(['I am Autopilot, the OpenAgents coding assistant.'])
    expect(text).not.toContain('OpenCode/Codex one-shot turn completed.')
  })

  test('renders completed event detail as assistant text', () => {
    const completedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: 'I am Autopilot, the OpenAgents coding assistant.',
              receiptRefs: [],
            }),
          },
          {
            summary: 'Assistant message completed.',
            type: 'message.completed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 1,
        status: 'completed',
      },
    })

    const [message] = chatRunTimelineMessages(completedRun)
    const text =
      message?.parts
        .filter(part => part.kind === 'text')
        .flatMap(part => part.body) ?? []

    expect(text).toEqual(['I am Autopilot, the OpenAgents coding assistant.'])
    expect(text).not.toContain('Assistant message completed.')
  })

  test('uses token usage metadata in the run summary without surfacing usage receipts', () => {
    const completedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            usage: {
              model: 'gpt-5',
              provider: 'openai',
              totalTokens: 123,
            },
          },
          {
            summary: 'Codex run resource usage receipt emitted.',
            type: 'resource.usage.captured',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 1,
        status: 'completed',
        tokenTotal: 123,
        tokenUsageEvents: 1,
      },
    })

    const [message] = chatRunTimelineMessages(completedRun)
    const visible = JSON.stringify(message?.parts ?? [])
    const details = message?.parts.flatMap(part =>
      part.kind === 'tool' ? part.detail : [],
    )

    expect(details).toContain('tokens: 123')
    expect(visible).not.toContain('resource usage')
    expect(visible).not.toContain('gpt-5')
    expect(visible).not.toContain('openai')
  })

  test('keeps OpenCode closeout plumbing out of the visible transcript', () => {
    const completedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            dataJson: JSON.stringify({
              artifactRefs: [],
              detail: 'OpenAgentsInc/autopilot-omega@main',
              receiptRefs: [],
            }),
          },
          {
            summary: 'Repository checkout completed.',
            type: 'repo.checkout.completed',
          },
        ),
        event(
          2,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_1',
                messageID: 'msg_1',
                text: 'I’ll answer directly.\n\nI can inspect files, run tools, and summarize results.',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
        event(
          3,
          {
            dataJson: JSON.stringify({
              part: {
                callID: 'call_status',
                state: {
                  input: {
                    command: 'git status --short --branch',
                  },
                  output: '## main...origin/main\n',
                  status: 'completed',
                },
                tool: 'bash',
                type: 'tool',
              },
              type: 'tool_use',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.tool_use',
          },
        ),
        event(
          4,
          {
            dataJson: JSON.stringify({
              detail: 'result.md',
            }),
          },
          {
            summary: 'Codex artifact captured.',
            type: 'artifact.created',
          },
        ),
        event(
          5,
          {
            dataJson: JSON.stringify({
              detail: 'github-writeback.json',
            }),
          },
          {
            summary: 'Codex artifact captured.',
            type: 'artifact.created',
          },
        ),
        event(
          6,
          {
            dataJson: JSON.stringify({
              detail:
                'Grant codex-auth-grant_grant_ref_4b16c00143bd4c11919e3d37aa23f7e5 resolved for account provider-account_ref_x; provider secret ref stayed server-side.',
            }),
          },
          {
            summary:
              'Vortex ChatGPT/Codex account grant resolved without API-key fallback.',
            type: 'runner.auth_grant_resolved',
          },
        ),
        event(
          7,
          {
            dataJson: JSON.stringify({
              detail:
                'stdout: {"type":"text","part":{"text":"raw JSON should not show"}}',
            }),
          },
          {
            summary: 'Codex VM log captured.',
            type: 'runner.log',
          },
        ),
        event(
          8,
          {
            dataJson: JSON.stringify({
              detail: 'codex workspace removed',
            }),
          },
          {
            summary: 'Codex VM cleanup completed.',
            type: 'runner.cleanup',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 8,
        status: 'completed',
        tokenTotal: 31_556,
      },
    })

    const [message] = chatRunTimelineMessages(completedRun)
    const visible = JSON.stringify(message?.parts ?? [])
    const text =
      message?.parts
        .filter(part => part.kind === 'text')
        .flatMap(part => part.body) ?? []
    const tools = message?.parts.filter(part => part.kind === 'tool') ?? []

    expect(text).toEqual([
      'I’ll answer directly.',
      'I can inspect files, run tools, and summarize results.',
    ])
    expect(
      tools.some(
        part =>
          part.kind === 'tool' &&
          part.title === 'Shell command' &&
          part.detail.includes('$ git status --short --branch') &&
          part.detail.some(detail => detail.includes('## main...origin/main')),
      ),
    ).toBe(true)
    expect(
      tools.some(
        part =>
          part.kind === 'tool' &&
          part.title === 'Artifact captured' &&
          part.subtitle === 'OpenCode artifact' &&
          part.detail.includes('file: github-writeback.json'),
      ),
    ).toBe(true)
    expect(visible).not.toContain('stdout:')
    expect(visible).not.toContain('grant_ref')
    expect(visible).not.toContain('provider secret ref')
    expect(visible).not.toContain('codex workspace removed')
    expect(visible).not.toContain('OpenAgentsInc/autopilot-omega@main')
  })

  test('keeps artifact narration out of the visible assistant transcript', () => {
    const completedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_1',
                text: 'I’ll summarize the provided team files and prepare the required local completion artifacts without touching application code.',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
        event(
          2,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_2',
                text: 'I found only the two provided team-file references in this run: a small test text file and a detailed landing-page spec for `OpenAgents for Lawyers`.',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
        event(
          3,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_3',
                text: 'I’m adding concise local artifacts that record the requested summary and the run outcome.',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
        event(
          4,
          {
            dataJson: JSON.stringify({
              part: {
                id: 'prt_text_4',
                text: 'opencode run completed and closeout manifest submitted',
                type: 'text',
              },
              type: 'text',
            }),
          },
          {
            summary: 'stdout JSON event captured.',
            type: 'runner.text',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 4,
        status: 'completed',
      },
    })

    const [message] = chatRunTimelineMessages(completedRun)
    const visible = JSON.stringify(message?.parts ?? [])
    const text =
      message?.parts
        .filter(part => part.kind === 'text')
        .flatMap(part => part.body) ?? []

    expect(text).toEqual([
      'I found only the two provided team-file references in this run: a small test text file and a detailed landing-page spec for `OpenAgents for Lawyers`.',
    ])
    expect(visible).not.toContain('required local completion artifacts')
    expect(visible).not.toContain('local artifacts')
    expect(visible).not.toContain('closeout manifest')
  })

  test('shows a missing artifact error when that is the primary run failure', () => {
    const failedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            error: 'missing required artifact result.md',
            type: 'artifact_set.failed',
          },
          {
            status: 'failed',
            summary: 'missing required artifact result.md',
            type: 'artifact_set.failed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 1,
        status: 'failed',
        tokenTotal: 0,
      },
    })

    const [message] = chatRunTimelineMessages(failedRun)
    const details = message?.parts.flatMap(part =>
      part.kind === 'tool' ? part.detail : [],
    )

    expect(details).toContain('Failed: missing required artifact result.md')
  })

  test('shows nested provider errors from SHC OpenCode payloads', () => {
    const failedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          {
            dataJson: JSON.stringify({
              error: {
                data: {
                  isRetryable: false,
                  message: 'The requested model is not supported.',
                  metadata: {
                    url: 'https://api.githubcopilot.com/chat/completions',
                  },
                  responseBody: JSON.stringify({
                    error: {
                      code: 'model_not_supported',
                      message: 'The requested model is not supported.',
                      param: 'model',
                      type: 'invalid_request_error',
                    },
                  }),
                  statusCode: 400,
                },
                name: 'APIError',
              },
              sessionID: 'ses_1718d1786ffemQ2fpuushp33Z9',
              type: 'error',
            }),
            source: 'runner',
            summary: 'Codex reported a failure event.',
            type: 'run.failed',
          },
          {
            status: 'failed',
            summary: 'Codex reported a failure event.',
            type: 'run.failed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 1,
        status: 'failed',
        tokenTotal: 0,
      },
    })

    const [message] = chatRunTimelineMessages(failedRun)
    const transcript = message?.parts ?? []

    expect(
      transcript.some(
        part =>
          part.kind === 'tool' &&
          part.detail.includes(
            'Failed: The requested model is not supported. (model_not_supported)',
          ),
      ),
    ).toBe(true)
    expect(
      transcript.some(
        part =>
          part.kind === 'tool' &&
          part.detail.includes('Failed: Codex reported a failure event.'),
      ),
    ).toBe(false)
  })

  test('identifies invalidated ChatGPT Codex account tokens', () => {
    const failedRun = ActiveChatRun({
      ...run,
      events: [
        event(
          1,
          { type: 'run.started' },
          {
            status: 'running',
            summary: 'OpenCode started.',
            type: 'run.started',
          },
        ),
        event(
          2,
          { type: 'repo.checkout.completed' },
          {
            status: 'completed',
            summary: 'Repository checkout completed.',
            type: 'repo.checkout.completed',
          },
        ),
        event(
          3,
          {
            dataJson: JSON.stringify({
              error: {
                data: {
                  isRetryable: false,
                  message:
                    'Your authentication token has been invalidated. Please try signing in again.',
                  metadata: {
                    url: 'https://api.openai.com/v1/responses',
                  },
                  responseBody: JSON.stringify({
                    error: {
                      code: 'token_invalidated',
                      message:
                        'Your authentication token has been invalidated. Please try signing in again.',
                      type: 'invalid_request_error',
                    },
                  }),
                  statusCode: 401,
                },
                name: 'APIError',
              },
              type: 'error',
            }),
            source: 'runner',
            summary: 'Codex reported a failure event.',
            type: 'run.failed',
          },
          {
            status: 'failed',
            summary: 'Codex reported a failure event.',
            type: 'run.failed',
          },
        ),
        event(
          4,
          {
            error: 'missing required artifact result.md',
            type: 'artifact_set.failed',
          },
          {
            status: 'failed',
            summary: 'missing required artifact result.md',
            type: 'artifact_set.failed',
          },
        ),
      ],
      metadata: {
        ...run.metadata,
        eventCursor: 4,
        status: 'failed',
        tokenTotal: 0,
      },
    })

    const [message] = chatRunTimelineMessages(failedRun)
    const parts = message?.parts ?? []
    const details = parts.flatMap(part =>
      part.kind === 'tool' ? part.detail : [],
    )

    expect(parts).toHaveLength(1)
    expect(parts[0]).toMatchObject({
      actionHref: '/settings/connections',
      actionLabel: 'Reconnect ChatGPT',
      kind: 'tool',
      subtitle: 'Reconnect required',
      title: 'ChatGPT not connected',
    })
    expect(details).toEqual([
      'ChatGPT is not connected. Reconnect ChatGPT in Settings before launching Autopilot.',
    ])
    expect(
      details.some(detail =>
        detail.includes('Your authentication token has been invalidated'),
      ),
    ).toBe(false)
    expect(JSON.stringify(parts)).not.toContain('OpenCode started')
    expect(JSON.stringify(parts)).not.toContain('missing required artifact')

    const [connectedMessage] = chatRunTimelineMessages(failedRun, 'connected')
    const [connectedPart] = connectedMessage?.parts ?? []

    expect(connectedPart).toMatchObject({
      kind: 'tool',
      subtitle: 'ready to retry',
      title: 'ChatGPT connected',
    })
    expect(JSON.stringify(connectedPart)).not.toContain('Reconnect required')
  })
})
