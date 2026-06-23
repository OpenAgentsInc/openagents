import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'

const scriptPath = join(import.meta.dirname, 'check-contract-drift.mjs')
const repoRoot = join(import.meta.dirname, '..', '..', '..')

const runGuard = () => {
  try {
    const stdout = execFileSync('bun', [scriptPath], {
      cwd: join(repoRoot, 'apps/openagents.com'),
      encoding: 'utf8',
    })
    return { code: 0, output: stdout }
  } catch (error) {
    const err = error as { status?: number; stdout?: string; stderr?: string }
    return {
      code: err.status ?? 1,
      output: `${err.stdout ?? ''}${err.stderr ?? ''}`,
    }
  }
}

const createdPaths: string[] = []

afterEach(() => {
  for (const path of createdPaths.splice(0)) {
    rmSync(path, { recursive: true, force: true })
  }
})

describe('security-contract drift guard', () => {
  test('passes on the current consolidated tree (one authority, no drift)', () => {
    const result = runGuard()
    expect(result.code).toBe(0)
    expect(result.output).toContain('one authority, no drift')
  })

  test('FAILS when a NEW duplicate ProviderSecretRef authority is reintroduced', () => {
    // Drop a forbidden duplicate definition somewhere in the scanned tree.
    const dir = join(
      repoRoot,
      'packages/probe/packages/runtime/src/__drift_guard_probe__',
    )
    mkdirSync(dir, { recursive: true })
    createdPaths.push(dir)
    const file = join(dir, 'duplicate-authority.ts')
    writeFileSync(
      file,
      'export const ProviderSecretRef = "reforked" as never\n',
      'utf8',
    )

    const result = runGuard()
    expect(result.code).toBe(1)
    expect(result.output).toContain('duplicate contract authority')
    expect(result.output).toContain('ProviderSecretRef')
  })

  test('FAILS when a NEW duplicate IsPrivateDataSafe authority is reintroduced', () => {
    const dir = join(
      repoRoot,
      'packages/probe/packages/runtime/src/__drift_guard_blueprint__',
    )
    mkdirSync(dir, { recursive: true })
    createdPaths.push(dir)
    const file = join(dir, 'duplicate-predicate.ts')
    writeFileSync(
      file,
      'export function isBlueprintProjectionPrivateDataSafe() { return true }\n',
      'utf8',
    )

    const result = runGuard()
    expect(result.code).toBe(1)
    expect(result.output).toContain('duplicate contract authority')
    expect(result.output).toContain('isBlueprintProjectionPrivateDataSafe')
  })

  test('ignores duplicate-looking definitions in ignored local worktree mirrors', () => {
    for (const root of [
      '.claude/worktrees/__drift_guard_ignored__',
      '.worktrees/__drift_guard_ignored__',
      '.pylon-local/cache/multi-session-worktrees/__drift_guard_ignored__',
    ]) {
      const dir = join(
        repoRoot,
        root,
        'apps/openagents.com/packages/provider-account-schema/src',
      )
      mkdirSync(dir, { recursive: true })
      createdPaths.push(join(repoRoot, root))
      writeFileSync(
        join(dir, 'index.ts'),
        'export const ProviderSecretRef = "ignored-local-mirror" as never\n',
        'utf8',
      )
    }

    const result = runGuard()
    expect(result.code).toBe(0)
    expect(result.output).toContain('one authority, no drift')
  })

  test('FAILS on residual drift when a former-copy shim regains a local definition', () => {
    // Temporarily turn a former-copy shim back into an authority. We use the
    // mtime-safe approach of appending a definition then restoring.
    const shim = join(
      repoRoot,
      'apps/pylon/packages/runtime/src/contracts/provider-account.ts',
    )
    const fs = require('node:fs') as typeof import('node:fs')
    const original = fs.readFileSync(shim, 'utf8')
    fs.writeFileSync(
      shim,
      `${original}\nexport function containsSecretMaterial() { return false }\n`,
      'utf8',
    )
    try {
      const result = runGuard()
      expect(result.code).toBe(1)
      expect(result.output).toContain('residual contract drift')
    } finally {
      fs.writeFileSync(shim, original, 'utf8')
    }
  })
})
