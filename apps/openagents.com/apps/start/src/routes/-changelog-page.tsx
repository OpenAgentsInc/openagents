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
          <a
            className="text-xs leading-4 text-khala-text-faint underline underline-offset-4 hover:text-khala-text"
            href={release.agentChangelogUrl}
            rel="noreferrer"
            target="_blank"
          >
            Detailed agent changelog for this release <span aria-hidden="true">↗</span>
          </a>
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
            What changed in each OpenAgents release, in plain language. Every release links its
            detailed engineering ledger in the repository.
          </p>
        </header>
        <ChangelogReleaseList releases={CHANGELOG_RELEASES} />
      </main>
    </PublicPageShell>
  );
}
