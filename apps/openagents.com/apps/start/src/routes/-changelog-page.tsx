import { PublicPageShell } from "@/components/public-page-shell";
import { Badge } from "@/components/ui/badge";

import { CHANGELOG_RELEASES, type ChangelogRelease } from "./-changelog-data.gen";

// The /changelog route is generated from the committed release files in
// docs/changelog/ (via `pnpm changelog sync`). The Start site is built and
// deployed from committed source — exactly how /download ships its release
// constants — so a build-time import is the honest, cache-correct strategy:
// the page can never claim a release the repository has not published, and
// there is no live changelog backend to degrade. The only honest empty state
// is "no releases published yet".

const channelLabel = (channel: string): string =>
  channel === "rc" ? "Release candidate" : channel === "stable" ? "Stable" : channel;

const provenanceLabel = (value: string): string =>
  value
    .split("_")
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");

const isPublicUrl = (value: string): boolean => value.startsWith("https://");

export function ChangelogReleaseList({
  releases,
}: Readonly<{ releases: ReadonlyArray<ChangelogRelease> }>) {
  if (releases.length === 0) {
    return (
      <p
        className="m-0 border border-khala-border bg-khala-surface p-4 text-sm leading-6 text-khala-text-faint"
        data-changelog-empty
      >
        No releases published yet. When a release ships, its changelog appears here.
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-10">
      {releases.map((release) => (
        <article
          className="border border-khala-border bg-khala-surface p-5"
          data-changelog-release={release.version}
          key={release.version}
        >
          <header className="mb-4 flex flex-wrap items-baseline gap-x-3 gap-y-2">
            <h2 className="m-0 font-mono text-lg font-semibold leading-6 text-khala-text">
              {release.version}
            </h2>
            <Badge>{channelLabel(release.channel)}</Badge>
            <time className="text-xs leading-4 text-khala-text-faint" dateTime={release.date}>
              {release.date}
            </time>
          </header>
          <dl
            className="mb-5 grid grid-cols-1 border-y border-khala-border text-xs leading-5 text-khala-text-muted sm:grid-cols-[8rem_1fr]"
            data-changelog-attribution={release.version}
          >
            <dt className="py-2 font-medium text-khala-text-faint sm:border-b sm:border-khala-border">
              Trigger
            </dt>
            <dd className="m-0 pb-2 text-khala-text sm:border-b sm:border-khala-border sm:py-2">
              {provenanceLabel(release.attribution.triggerKind)} · {release.attribution.triggeredBy}
            </dd>
            <dt className="py-2 font-medium text-khala-text-faint sm:border-b sm:border-khala-border">
              Released by
            </dt>
            <dd className="m-0 pb-2 text-khala-text sm:border-b sm:border-khala-border sm:py-2">
              {release.attribution.releaseActor}
            </dd>
            <dt className="py-2 font-medium text-khala-text-faint">Authority</dt>
            <dd className="m-0 pb-2 text-khala-text sm:py-2">{release.attribution.authorityRef}</dd>
          </dl>
          {release.blocks.map((block, index) =>
            block.kind === "paragraph" ? (
              <p className="m-0 mb-3 max-w-prose text-sm leading-6 text-khala-text" key={index}>
                {block.text}
              </p>
            ) : (
              <ul
                className="m-0 mb-3 flex max-w-prose list-disc flex-col gap-2 pl-5 text-sm leading-6 text-khala-text"
                key={index}
              >
                {block.items.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            ),
          )}
          <footer className="flex flex-wrap gap-x-5 gap-y-2 pt-1 text-xs leading-4">
            <a
              className="text-khala-text-faint underline underline-offset-4 hover:text-khala-text"
              href={release.attribution.releaseUrl}
              rel="noreferrer"
              target="_blank"
            >
              Release <span aria-hidden="true">↗</span>
            </a>
            {isPublicUrl(release.attribution.sourceFeedback) ? (
              <a
                className="text-khala-text-faint underline underline-offset-4 hover:text-khala-text"
                href={release.attribution.sourceFeedback}
                rel="noreferrer"
                target="_blank"
              >
                Source feedback <span aria-hidden="true">↗</span>
              </a>
            ) : (
              <span className="text-khala-text-faint">No source feedback recorded</span>
            )}
            <a
              className="text-khala-text-faint underline underline-offset-4 hover:text-khala-text"
              href={release.agentChangelogUrl}
              rel="noreferrer"
              target="_blank"
            >
              Engineering ledger <span aria-hidden="true">↗</span>
            </a>
          </footer>
        </article>
      ))}
    </div>
  );
}

export function ChangelogPage() {
  return (
    <PublicPageShell dataRoute="changelog">
      <main className="mx-auto w-full max-w-3xl px-4 pb-20 pt-12">
        <header className="mb-10">
          <p className="m-0 text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint">
            Releases
          </p>
          <h1 className="m-0 mt-2 text-3xl font-semibold leading-9 text-khala-text">Changelog</h1>
          <p className="m-0 mt-3 max-w-prose text-sm leading-6 text-khala-text-faint">
            What changed, why it shipped, and who authorized it. Every release links its public
            build, source feedback, and engineering ledger.
          </p>
        </header>
        <ChangelogReleaseList releases={CHANGELOG_RELEASES} />
      </main>
    </PublicPageShell>
  );
}
