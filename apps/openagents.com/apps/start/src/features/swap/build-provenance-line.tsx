import {
  buildCommit,
  buildCommitUrl,
  shortBuildCommit,
} from '@/lib/build-provenance'

/**
 * The one Boltz footer element worth taking outright (#9322): a version line
 * naming the build on every page that holds key material, so a user about to
 * commit funds can say which build they trusted. When the commit is unknown
 * (dev, tests) the line says so instead of inventing one.
 */
export function SwapBuildProvenance() {
  const commit = buildCommit()
  const short = shortBuildCommit(commit)
  const url = buildCommitUrl(commit)
  return (
    <p
      className="m-0 font-mono text-xs text-khala-text-faint"
      data-swap-build-provenance={commit}
    >
      {'Build '}
      {url === undefined ? (
        <span>{short}</span>
      ) : (
        <a
          className="underline decoration-khala-border underline-offset-2 hover:text-khala-text-muted"
          href={url}
          rel="noreferrer"
          target="_blank"
        >
          {short}
        </a>
      )}
      {' — the exact source this page was built from.'}
    </p>
  )
}
