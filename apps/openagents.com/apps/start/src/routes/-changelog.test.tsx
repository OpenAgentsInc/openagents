import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";

import { CHANGELOG_RELEASES } from "./-changelog-data.gen";
import { ChangelogPage, ChangelogReleaseList } from "./-changelog-page";

describe("Start /changelog route", () => {
  test("server-renders the route contract and page chrome", () => {
    const html = renderToStaticMarkup(<ChangelogPage />);

    expect(html).toContain('data-route="changelog"');
    expect(html).toContain('aria-label="Primary navigation"');
    expect(html).toContain("© 2026 OpenAgents, Inc.");
    expect(html).toContain("Changelog");
    expect(html).toContain("What changed, why it shipped, and who authorized it.");
  });

  test("the committed data lists releases newest-first with version, channel, and date", () => {
    expect(CHANGELOG_RELEASES.length).toBeGreaterThan(0);
    const dates = CHANGELOG_RELEASES.map((release) => release.date);
    expect([...dates].sort().reverse()).toEqual(dates);
    for (const release of CHANGELOG_RELEASES) {
      expect(release.version.length).toBeGreaterThan(0);
      expect(release.channel.length).toBeGreaterThan(0);
      expect(release.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(release.blocks.length).toBeGreaterThan(0);
      expect(release.agentChangelogUrl).toMatch(
        /^https:\/\/github\.com\/OpenAgentsInc\/openagents\/blob\/main\/docs\/changelog\//,
      );
      expect(release.attribution.triggerKind.length).toBeGreaterThan(0);
      expect(release.attribution.triggeredBy.length).toBeGreaterThan(0);
      expect(release.attribution.releaseActor.length).toBeGreaterThan(0);
      expect(release.attribution.authorityRef.length).toBeGreaterThan(0);
      expect(release.attribution.releaseUrl).toMatch(/^https:\/\//);
    }
  });

  test("renders every committed release with its agent-changelog link", () => {
    const html = renderToStaticMarkup(<ChangelogPage />);

    for (const release of CHANGELOG_RELEASES) {
      expect(html).toContain(`data-changelog-release="${release.version}"`);
      expect(html.toLowerCase()).toContain(`datetime="${release.date}"`);
      expect(html).toContain(release.agentChangelogUrl);
    }
    expect(html).toContain("Engineering ledger");
    expect(html).toContain("Source feedback");
    expect(html).toContain('data-changelog-attribution="0.1.0-rc.20"');
    expect(html).not.toContain("data-changelog-empty");
  });

  test("the backfilled first release entry stays honest about its boundary", () => {
    const html = renderToStaticMarkup(<ChangelogPage />);

    expect(html).toContain("0.1.0-rc.13");
    expect(html).toContain("Release candidate");
    expect(html).toContain("Apple silicon Macs");
    expect(html).toContain("Intel Macs, Windows, and Linux are not supported yet.");
  });

  test("recent releases expose their real feedback and historical authority boundaries", () => {
    const html = renderToStaticMarkup(<ChangelogPage />);

    expect(html).toContain("0.1.0-rc.20");
    expect(html).toContain("Tester Feedback");
    expect(html).toContain("@lathe-agent-oa");
    expect(html).toContain("before AUTHORITY.md revision 2");
    expect(html).toContain("OpenAgents release agent (historical)");
  });

  test("the empty state is honest — no fabricated releases", () => {
    const html = renderToStaticMarkup(<ChangelogReleaseList releases={[]} />);

    expect(html).toContain("data-changelog-empty");
    expect(html).toContain("No releases published yet.");
    expect(html).not.toContain("data-changelog-release=");
  });

  test("human copy carries no commit-hash soup or internal-only vocabulary", () => {
    for (const release of CHANGELOG_RELEASES) {
      for (const block of release.blocks) {
        const texts = block.kind === "paragraph" ? [block.text] : block.items;
        for (const text of texts) {
          expect(text).not.toMatch(/\b[0-9a-f]{10,40}\b/);
          expect(text.toLowerCase()).not.toContain("ontology");
        }
      }
    }
  });
});
