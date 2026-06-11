/**
 * Marching-orders agent (#4757): read open GitHub issues, propose bounded
 * public-safe work-order candidates, and submit only explicitly approved issue
 * numbers to the Autopilot work API.
 *
 * Dry-run proposal:
 *   bun run scripts/marching-orders-agent.ts --repo OpenAgentsInc/openagents
 *
 * Human-gated submission:
 *   OPENAGENTS_AGENT_TOKEN=... bun run scripts/marching-orders-agent.ts \
 *     --submit --approved-issues 4758,4759,4760 \
 *     --commit $(git rev-parse HEAD) \
 *     --verification-command bun,test \
 *     --agent-id agent.openagents.marching_orders \
 *     --owner-ref owner_ref.openagents_core \
 *     --pylon-id pylon.00bd3fa4f3aca227a496
 */

import {
  buildMarchingOrderSubmission,
  deliveredIssueCommentBody,
  type GitHubIssueForMarchingOrders,
  proposeMarchingOrderIssues,
} from '../src/marching-orders-agent'

const args = process.argv.slice(2)

const flag = (name: string, fallback?: string): string | undefined => {
  const index = args.indexOf(name)
  return index >= 0 ? args[index + 1] : fallback
}

const hasFlag = (name: string): boolean => args.includes(name)

const splitCsv = (value: string | undefined): ReadonlyArray<string> =>
  value === undefined
    ? []
    : value
        .split(',')
        .map(item => item.trim())
        .filter(item => item !== '')

const repo = flag('--repo', 'OpenAgentsInc/openagents')!
const baseUrl = flag('--base-url', 'https://openagents.com')!
const branch = flag('--branch', 'main')!
const commitSha = flag('--commit')
const agentId = flag('--agent-id', 'agent.openagents.marching_orders')!
const agentWalletRef = flag('--agent-wallet-ref')
const ownerRef = flag('--owner-ref', 'owner_ref.openagents_core')!
const pylonId = flag('--pylon-id')
const limit = Number(flag('--limit', '25'))
const submit = hasFlag('--submit')
const poll = hasFlag('--poll')
const commentDelivered = hasFlag('--comment-delivered')
const approvedIssues = new Set(splitCsv(flag('--approved-issues')).map(Number))
const verificationArgs = splitCsv(flag('--verification-command', 'bun,test'))
const verificationCommandRef = flag(
  '--verification-command-ref',
  verificationArgs.join('_') === 'bun_test'
    ? 'command.public.autopilot_coder.bun_test'
    : `command.public.marching_orders.${verificationArgs.join('_')}`,
)!

const githubToken = process.env['GITHUB_TOKEN'] ?? process.env['GH_TOKEN']
const openAgentsToken = process.env['OPENAGENTS_AGENT_TOKEN']

if (!Number.isFinite(limit) || limit <= 0) {
  console.error('--limit must be a positive number')
  process.exit(2)
}

if (submit && (openAgentsToken === undefined || commitSha === undefined)) {
  console.error(
    '--submit requires OPENAGENTS_AGENT_TOKEN and --commit <40-char sha>',
  )
  process.exit(2)
}

const githubHeaders = (): HeadersInit => ({
  Accept: 'application/vnd.github+json',
  'User-Agent': 'openagents-marching-orders-agent',
  ...(githubToken === undefined ? {} : { Authorization: `Bearer ${githubToken}` }),
})

const fetchIssues = async (): Promise<ReadonlyArray<GitHubIssueForMarchingOrders>> => {
  const response = await fetch(
    `https://api.github.com/repos/${repo}/issues?state=open&per_page=100&sort=updated&direction=desc`,
    { headers: githubHeaders() },
  )
  const payload = await response.json()

  if (!response.ok || !Array.isArray(payload)) {
    throw new Error(
      `GitHub issue list failed (${response.status}): ${JSON.stringify(payload)}`,
    )
  }

  return payload as ReadonlyArray<GitHubIssueForMarchingOrders>
}

const fetchIssue = async (
  issueNumber: number,
): Promise<GitHubIssueForMarchingOrders> => {
  const response = await fetch(
    `https://api.github.com/repos/${repo}/issues/${issueNumber}`,
    { headers: githubHeaders() },
  )
  const payload = await response.json()

  if (!response.ok) {
    throw new Error(
      `GitHub issue ${issueNumber} failed (${response.status}): ${JSON.stringify(payload)}`,
    )
  }

  return payload as GitHubIssueForMarchingOrders
}

const submitWork = async (
  input: ReturnType<typeof buildMarchingOrderSubmission>,
) => {
  const response = await fetch(`${baseUrl}/api/autopilot/work`, {
    body: JSON.stringify(input.request),
    headers: {
      Authorization: `Bearer ${openAgentsToken}`,
      'Content-Type': 'application/json',
      'Idempotency-Key': input.idempotencyKey,
    },
    method: 'POST',
  })
  const payload = await response.json()

  return {
    issueNumber: input.issueNumber,
    payload,
    status: response.status,
    workOrderRef:
      typeof payload?.work?.workOrderRef === 'string'
        ? payload.work.workOrderRef
        : null,
  }
}

const readWork = async (workOrderRef: string) => {
  const response = await fetch(`${baseUrl}/api/autopilot/work/${workOrderRef}`, {
    headers: {
      Authorization: `Bearer ${openAgentsToken}`,
    },
  })
  const payload = await response.json()

  return { payload, status: response.status }
}

const commentOnIssue = async (
  issueNumber: number,
  body: string,
): Promise<unknown> => {
  const response = await fetch(
    `https://api.github.com/repos/${repo}/issues/${issueNumber}/comments`,
    {
      body: JSON.stringify({ body }),
      headers: {
        ...githubHeaders(),
        'Content-Type': 'application/json',
      },
      method: 'POST',
    },
  )
  const payload = await response.json()

  if (!response.ok) {
    throw new Error(
      `GitHub issue comment failed (${response.status}): ${JSON.stringify(payload)}`,
    )
  }

  return payload
}

const run = async () => {
  const issues = await fetchIssues()
  const proposals = proposeMarchingOrderIssues(issues, { limit })

  if (!submit) {
    console.log(
      JSON.stringify(
        {
          approvedIssuesHint:
            'Rerun with --submit --approved-issues <numbers> --commit <sha> after human approval.',
          proposals,
          schema: 'openagents.marching_orders.proposal.v1',
        },
        null,
        2,
      ),
    )
    return
  }

  const selectedIssues = await Promise.all(
    [...approvedIssues].map(issueNumber => fetchIssue(issueNumber)),
  )
  const submissions = selectedIssues.map(issue =>
    buildMarchingOrderSubmission(issue, {
      agentId,
      agentWalletRef,
      baseUrl,
      branch,
      commitSha: commitSha!,
      ownerRef,
      pylonId,
      repository: repo,
      verificationCommand: {
        args: verificationArgs,
        commandRef: verificationCommandRef,
      },
    }),
  )
  const submitted = await Promise.all(submissions.map(submitWork))
  const polled = poll
    ? await Promise.all(
        submitted
          .filter(result => result.workOrderRef !== null)
          .map(async result => ({
            issueNumber: result.issueNumber,
            status: await readWork(result.workOrderRef!),
            workOrderRef: result.workOrderRef,
          })),
      )
    : []

  if (commentDelivered) {
    for (const result of polled) {
      const work = result.status.payload?.work
      if (work?.state !== 'delivered' || work.executionCloseout === null) {
        continue
      }
      await commentOnIssue(
        result.issueNumber,
        deliveredIssueCommentBody({
          closeoutRefs: work.executionCloseout.closeoutRefs ?? [],
          resultRefs: work.executionCloseout.resultRefs ?? [],
          testRefs: work.executionCloseout.testRefs ?? [],
          workOrderRef: result.workOrderRef!,
        }),
      )
    }
  }

  console.log(
    JSON.stringify(
      {
        polled,
        schema: 'openagents.marching_orders.submission.v1',
        submitted,
      },
      null,
      2,
    ),
  )
}

run().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
