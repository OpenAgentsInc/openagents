import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_REPO_READ_MAX_BYTES,
  isSafeArtanisRepoPath,
  makeArtanisDispatchCodexTaskTool,
  makeArtanisListRepoDirTool,
  makeArtanisOperatorTools,
  makeArtanisReadRepoFileTool,
} from './artanis-operator-tools'

// A fetch stub that records the URL it was asked for and returns a canned
// Response. This is the only seam the read tools touch the network through.
const stubFetch = (
  handler: (url: string) => Response,
): {
  fetchImpl: typeof fetch
  urls: Array<string>
} => {
  const urls: Array<string> = []
  const fetchImpl = (async (input: RequestInfo | URL) => {
    const url = typeof input === 'string' ? input : input.toString()
    urls.push(url)
    return handler(url)
  }) as typeof fetch
  return { fetchImpl, urls }
}

describe('#6365 read_repo_file (public OpenAgentsInc/openagents only)', () => {
  test('returns the real file contents from raw.githubusercontent', async () => {
    const fileBody =
      '# Khala open issues master roadmap\nFirst priority: the #6316 serving track.'
    const { fetchImpl, urls } = stubFetch(
      () => new Response(fileBody, { status: 200 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })

    const result = await Effect.runPromise(
      tool.execute({
        path: 'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
      }),
    )

    expect(result).toBe(fileBody)
    expect(urls[0]).toBe(
      'https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
    )
  })

  test('a 404 reads as honest "file not found", never invention', async () => {
    const { fetchImpl } = stubFetch(
      () => new Response('Not Found', { status: 404 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ path: 'docs/does-not-exist.md' }),
    )
    expect(result).toContain('file not found')
  })

  test('blocks a secret-bearing path and never fetches it', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response('SHOULD NOT BE READ', { status: 200 }),
    )
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    for (const path of [
      '.secrets/tailnet.env',
      '../../etc/passwd',
      '/etc/passwd',
      'config/wallet-mnemonic.txt',
    ]) {
      const result = await Effect.runPromise(tool.execute({ path }))
      expect(result).toContain('blocked')
    }
    expect(urls).toHaveLength(0)
  })

  test('truncates an oversized file at the byte cap', async () => {
    const big = 'a'.repeat(ARTANIS_REPO_READ_MAX_BYTES + 10)
    const { fetchImpl } = stubFetch(() => new Response(big, { status: 200 }))
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(
      tool.execute({ path: 'docs/big.md' }),
    )
    expect(result).toContain('truncated')
    expect(result.length).toBeLessThan(big.length + 200)
  })

  test('invalid arguments return an honest message, not a throw', async () => {
    const { fetchImpl } = stubFetch(() => new Response('', { status: 200 }))
    const tool = makeArtanisReadRepoFileTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ notPath: 1 }))
    expect(result).toContain('invalid arguments')
  })
})

describe('#6365 list_repo_dir', () => {
  test('lists directory entries from the GitHub contents API', async () => {
    const body = JSON.stringify([
      { name: 'README.md', type: 'file' },
      { name: 'khala', type: 'dir' },
    ])
    const { fetchImpl, urls } = stubFetch(
      () => new Response(body, { status: 200 }),
    )
    const tool = makeArtanisListRepoDirTool({ fetchImpl })
    const result = await Effect.runPromise(tool.execute({ path: 'docs' }))
    expect(result).toContain('README.md (file)')
    expect(result).toContain('khala (dir)')
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/contents/docs?ref=main',
    )
  })

  test('root listing accepts "" and "."', async () => {
    const { fetchImpl, urls } = stubFetch(
      () => new Response(JSON.stringify([{ name: 'docs', type: 'dir' }]), {
        status: 200,
      }),
    )
    const tool = makeArtanisListRepoDirTool({ fetchImpl })
    await Effect.runPromise(tool.execute({ path: '.' }))
    expect(urls[0]).toBe(
      'https://api.github.com/repos/OpenAgentsInc/openagents/contents/?ref=main',
    )
  })
})

describe('isSafeArtanisRepoPath', () => {
  test('accepts normal repo paths', () => {
    expect(
      isSafeArtanisRepoPath(
        'docs/khala/2026-06-26-khala-open-issues-master-roadmap.md',
      ),
    ).toBe(true)
    expect(isSafeArtanisRepoPath('apps/openagents.com/workers/api/src/x.ts')).toBe(
      true,
    )
  })
  test('rejects traversal, absolute, secret, and node_modules paths', () => {
    expect(isSafeArtanisRepoPath('../secrets')).toBe(false)
    expect(isSafeArtanisRepoPath('/etc/passwd')).toBe(false)
    expect(isSafeArtanisRepoPath('.secrets/tailnet.env')).toBe(false)
    expect(isSafeArtanisRepoPath('node_modules/x/index.js')).toBe(false)
    expect(isSafeArtanisRepoPath('')).toBe(false)
  })
})

describe('#6366 dispatch_codex_task (risky, plan-only)', () => {
  const tool = makeArtanisDispatchCodexTaskTool()

  test('is a risky pylon_job_dispatch tool (never executes)', () => {
    expect(tool.kind).toBe('risky')
    expect(tool.riskyActionKind).toBe('pylon_job_dispatch')
    // The risky tool exposes plan(), not execute() — the boundary is structural.
    expect('execute' in tool).toBe(false)
  })

  test('plan returns the exact public-safe Khala -> Pylon -> Codex dispatch', async () => {
    const plan = await Effect.runPromise(
      tool.plan({
        branch: 'main',
        filePaths: ['apps/openagents.com/workers/api/src/foo.ts'],
        issue: 6320,
        objective: 'Improve serving throughput per the roadmap.',
        verify:
          'bun run --cwd apps/openagents.com/workers/api test -- src/foo.test.ts',
      }),
    )
    expect(plan).toContain('pylon khala request')
    expect(plan).toContain('--workflow codex_agent_task')
    expect(plan).toContain('--repo OpenAgentsInc/openagents')
    expect(plan).toContain('run-no-spend')
    expect(plan).toContain('#6320')
    expect(plan).toContain('src/foo.test.ts')
  })

  test('blocks a dispatch field carrying non-public-safe material', async () => {
    const plan = await Effect.runPromise(
      tool.plan({
        objective: 'use the bearer token sk-abc123 to pay the payout',
      }),
    )
    expect(plan).toContain('blocked')
  })

  test('requires a public-safe objective', async () => {
    const plan = await Effect.runPromise(tool.plan({}))
    expect(plan).toContain('invalid arguments')
  })
})

describe('makeArtanisOperatorTools default table', () => {
  test('includes the two repo-read tools and the dispatch tool', () => {
    const tools = makeArtanisOperatorTools()
    const names = tools.map(tool => tool.definition.name).sort()
    expect(names).toEqual([
      'dispatch_codex_task',
      'list_repo_dir',
      'read_repo_file',
    ])
  })
})
