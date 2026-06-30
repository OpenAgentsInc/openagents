import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, test } from 'vitest'

const repoRoot = resolve(new URL('../../..', import.meta.url).pathname)

const commandComposerArtifacts = [
  'docs/adr/0013-adopt-prosemirror-inspired-command-composer.md',
  'docs/khala/2026-06-30-command-composer-acceptance-runbook.md',
  'packages/composer-state/README.md',
  'packages/composer-state/src/index.ts',
  'packages/composer-state/src/index.test.ts',
  'packages/ui/src/ai-elements/command-composer.ts',
  'packages/ui/src/ai-elements/command-composer.css',
  'packages/ui/test/command-composer.test.ts',
  'clients/khala-code-desktop/src/ui/index.html',
  'clients/khala-code-desktop/src/ui/main.ts',
  'clients/khala-code-desktop/src/ui/styles.css',
  'clients/khala-code-desktop/tests/app-shell.test.ts',
  'apps/openagents.com/apps/web/src/main.test.ts',
  'apps/openagents.com/apps/web/src/page/khala-chat/flow.ts',
  'apps/openagents.com/apps/web/src/page/khala-chat/page.ts',
  'apps/openagents.com/apps/web/src/page/loggedOut/update.test.ts',
] as const

const forbiddenPatterns = [
  {
    label: 'OpenAI-style API key',
    pattern: /sk-[A-Za-z0-9_-]{20,}/,
  },
  {
    label: 'OpenRouter-style API key',
    pattern: /sk-or-[A-Za-z0-9_-]{20,}/i,
  },
  {
    label: 'GitHub personal access token',
    pattern: /(ghp|github_pat)_[A-Za-z0-9_]{20,}/,
  },
  {
    label: 'private prompt sentinel',
    pattern:
      /BEGIN PRIVATE PROMPT|END PRIVATE PROMPT|RAW_PRIVATE_PROMPT|UNREDACTED_PROMPT/i,
  },
  {
    label: 'private file sentinel',
    pattern: /RAW_PRIVATE_FILE|PRIVATE_FILE_BYTES|UNREDACTED_FILE/i,
  },
  {
    label: 'private key block',
    pattern: /-----BEGIN [A-Z ]*PRIVATE KEY-----/,
  },
] as const

describe('command composer privacy fixture guard', () => {
  test('keeps the v1 composer artifact list visible to Git', () => {
    const tracked = execFileSync(
      'git',
      [
        'ls-files',
        '--cached',
        '--others',
        '--exclude-standard',
        ...commandComposerArtifacts,
      ],
      {
        cwd: repoRoot,
        encoding: 'utf8',
      },
    )

    expect(tracked.trim().split('\n').sort()).toEqual(
      [...commandComposerArtifacts].sort(),
    )
  })

  test('does not commit raw private prompt, file, or secret sentinels', () => {
    const findings = commandComposerArtifacts.flatMap(path => {
      const text = readFileSync(resolve(repoRoot, path), 'utf8')

      return forbiddenPatterns.flatMap(({ label, pattern }) =>
        pattern.test(text) ? [`${path}: ${label}`] : [],
      )
    })

    expect(findings).toEqual([])
  })
})
