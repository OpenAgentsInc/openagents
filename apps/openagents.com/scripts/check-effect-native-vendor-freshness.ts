#!/usr/bin/env bun

// Freshness check for the vendored @effect-native/* snapshot.
//
// Compares the pinned commit in
// apps/openagents.com/packages/effect-native-vendor.json against the current
// `origin/main` tip of the sibling upstream checkout
// (OpenAgentsInc/effect-native). Prints a LOUD warning listing how many commits
// behind the vendor is.
//
// This is intentionally NON-FATAL: upstream moves fast, so staleness is a
// warning, not a hard failure. The hard failure (a drifted / partially-bumped
// vendor) is enforced by check-effect-native-vendor.test.ts. If the sibling
// checkout is absent (e.g. CI, a fresh clone), this degrades to a skip message
// and exits 0.
//
// Re-vendoring recipe (when this warns):
//   1. cd <sibling>/effect-native && git fetch origin main
//   2. re-copy each vendored package's upstream src/** over the monorepo copy
//   3. bump `commit` + `catalogVersion` in effect-native-vendor.json
//   4. bump every vendored package.json `effectNativeVendor.commit`
//   5. re-run the consumer typechecks + tests

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { homedir } from 'node:os'

const HERE = dirname(fileURLToPath(import.meta.url))
const APP_ROOT = resolve(HERE, '..') // apps/openagents.com
const REPO_ROOT = resolve(APP_ROOT, '..', '..') // monorepo root
const MANIFEST_PATH = join(APP_ROOT, 'packages', 'effect-native-vendor.json')

const candidateCheckouts = [
  process.env.EFFECT_NATIVE_CHECKOUT,
  resolve(REPO_ROOT, '..', 'effect-native'),
  join(homedir(), 'work', 'effect-native'),
].filter((p): p is string => typeof p === 'string' && p.length > 0)

const gitOrNull = (cwd: string, args: ReadonlyArray<string>): string | null => {
  try {
    return execFileSync('git', ['-C', cwd, ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim()
  } catch {
    return null
  }
}

const main = (): void => {
  const manifest = JSON.parse(readFileSync(MANIFEST_PATH, 'utf8')) as {
    commit: string
    catalogVersion: string
  }

  const checkout = candidateCheckouts.find(
    (p) => existsSync(p) && existsSync(join(p, '.git')),
  )
  if (checkout === undefined) {
    console.log(
      'effect-native vendor freshness: SKIP — no sibling effect-native checkout found ' +
        `(looked at: ${candidateCheckouts.join(', ')}). Set EFFECT_NATIVE_CHECKOUT to override.`,
    )
    return
  }

  // Best-effort refresh so the comparison uses the true remote tip.
  gitOrNull(checkout, ['fetch', 'origin', 'main'])
  const upstreamTip = gitOrNull(checkout, ['rev-parse', 'origin/main'])
  if (upstreamTip === null) {
    console.log(
      `effect-native vendor freshness: SKIP — could not resolve origin/main in ${checkout}.`,
    )
    return
  }

  if (upstreamTip === manifest.commit) {
    console.log(
      `effect-native vendor freshness: OK — vendor is at upstream tip ${manifest.commit.slice(0, 7)} ` +
        `(catalog ${manifest.catalogVersion}).`,
    )
    return
  }

  const behind = gitOrNull(checkout, [
    'rev-list',
    '--count',
    `${manifest.commit}..origin/main`,
  ])
  const behindText =
    behind === null
      ? 'an unknown number of commits (pinned commit not found in the sibling checkout — fetch it)'
      : `${behind} commit(s)`

  const bar = '='.repeat(72)
  console.warn(
    [
      '',
      bar,
      'effect-native vendor freshness: STALE (warning, not a failure)',
      bar,
      `  vendored commit : ${manifest.commit}`,
      `  upstream tip     : ${upstreamTip}`,
      `  behind by        : ${behindText}`,
      `  catalog pinned   : ${manifest.catalogVersion}`,
      '',
      '  Re-vendor when convenient: re-copy each package src/**, then bump the',
      '  commit + catalogVersion in effect-native-vendor.json and every vendored',
      '  package.json effectNativeVendor.commit. See VENDORING.md.',
      bar,
      '',
    ].join('\n'),
  )
}

main()
