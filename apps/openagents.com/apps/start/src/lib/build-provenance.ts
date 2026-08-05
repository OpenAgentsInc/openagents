/**
 * Build provenance for key-material-bearing surfaces (SWAP-7, #9322).
 *
 * A user about to commit funds should be able to say which build they
 * trusted. The commit is injected at build time by the Vite `define` in
 * `vite.config.ts` (deploy env `OPENAGENTS_BUILD_COMMIT`, else
 * `git rev-parse HEAD` at build time). When neither is available — dev
 * server without git, unit tests — the surface says "unknown" instead of
 * inventing a version.
 */
import { GITHUB_REPOSITORY_URL } from './public-site'

declare const __OPENAGENTS_BUILD_COMMIT__: string | undefined

export const UNKNOWN_BUILD_COMMIT = 'unknown'

const injectedCommit = (): string => {
  try {
    return typeof __OPENAGENTS_BUILD_COMMIT__ === 'string' &&
      __OPENAGENTS_BUILD_COMMIT__ !== ''
      ? __OPENAGENTS_BUILD_COMMIT__
      : UNKNOWN_BUILD_COMMIT
  } catch {
    return UNKNOWN_BUILD_COMMIT
  }
}

export const buildCommit = (): string => injectedCommit()

export const isKnownBuildCommit = (commit: string): boolean =>
  /^[0-9a-f]{40}$/.test(commit)

export const shortBuildCommit = (commit: string): string =>
  isKnownBuildCommit(commit) ? commit.slice(0, 12) : UNKNOWN_BUILD_COMMIT

export const buildCommitUrl = (commit: string): string | undefined =>
  isKnownBuildCommit(commit)
    ? `${GITHUB_REPOSITORY_URL}/commit/${commit}`
    : undefined
