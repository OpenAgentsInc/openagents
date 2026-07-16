// DIST-14 (#8927): changelog generation — fixture roll, bound enforcement,
// roll-forward idempotence, and route data-module sync.
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { afterEach, describe, expect, test } from "vitest";

import {
  CHANGELOG_DIR,
  EMPTY_UNRELEASED,
  RELEASE_NOTES_MAX_LENGTH,
  ROUTE_DATA_MODULE_PATH,
  UNRELEASED_FILE,
  draftHumanChangelog,
  humanMarkdownToBlocks,
  parseReleaseFile,
  parseUnreleased,
  releaseNotesText,
  renderRouteDataModule,
  rollUnreleased,
  runCheck,
  runRoll,
  runSync,
  sortReleasesNewestFirst,
} from "./changelog.js";

const FIXTURE_UNRELEASED = `# Unreleased

Preamble text that is not an entry.

## Composer keeps drafts across restarts (#1234)

- issues: #1234
- commits: abc1234def
- contracts-specs: none
- invariants: none changed
- evidence: none
- lane: fixture-lane-a

Your unsent draft now survives an app restart, so a quit or crash never
loses what you were about to say.

Engineering detail: drafts persist through the session registry with
per-thread keys.

## Faster diff review (#5678)

- issues: #5678, #5679
- commits: 123abc456d
- contracts-specs: packages/agent-runtime-schema/src/schema.ts
- invariants: none changed
- evidence: docs/qa/fixture-receipt.json
- lane: fixture-lane-b

Reviewing a change is faster: large diffs render progressively instead of
blocking the whole review panel.
`;

describe("changelog parsing", () => {
  test("parses UNRELEASED entries with metadata and summary paragraphs", () => {
    const entries = parseUnreleased(FIXTURE_UNRELEASED);
    expect(entries).toHaveLength(2);
    expect(entries[0]?.title).toBe("Composer keeps drafts across restarts (#1234)");
    expect(entries[0]?.issues).toBe("#1234");
    expect(entries[0]?.lane).toBe("fixture-lane-a");
    expect(entries[0]?.summaryParagraphs).toHaveLength(2);
    expect(entries[0]?.summaryParagraphs[0]).toContain("survives an app restart");
    expect(entries[1]?.contractsSpecs).toBe("packages/agent-runtime-schema/src/schema.ts");
  });

  test("an entry missing required metadata is a typed failure, never silence", () => {
    const broken = `# Unreleased

## Missing metadata (#1)

- issues: #1
- commits: abc

Some summary.
`;
    expect(() => parseUnreleased(broken)).toThrow(
      /missing required metadata: contracts-specs, invariants, evidence, lane/,
    );
  });

  test("the empty accumulator parses to zero entries", () => {
    expect(parseUnreleased(EMPTY_UNRELEASED)).toEqual([]);
  });
});

describe("rollUnreleased", () => {
  const rolled = rollUnreleased({
    channel: "rc",
    date: "2026-08-01",
    unreleasedText: FIXTURE_UNRELEASED,
    version: "0.1.0-rc.99",
  });

  test("produces a dated release file with human draft and agent entries", () => {
    expect(rolled.releaseFileName).toBe("2026-08-01-desktop-0.1.0-rc.99.md");
    expect(rolled.releaseMarkdown).toContain("# OpenAgents Desktop 0.1.0-rc.99 — 2026-08-01");
    expect(rolled.releaseMarkdown).toContain("- version: 0.1.0-rc.99");
    expect(rolled.releaseMarkdown).toContain("- channel: rc");
    expect(rolled.releaseMarkdown).toContain("## Human changelog");
    expect(rolled.releaseMarkdown).toContain("## Agent changelog");
    expect(rolled.releaseMarkdown).toContain("### Composer keeps drafts across restarts (#1234)");
    expect(rolled.releaseMarkdown).toContain("- lane: fixture-lane-b");
    // The human draft carries one bullet per entry, from the first paragraph.
    expect(rolled.releaseMarkdown).toContain("- Your unsent draft now survives an app restart");
    expect(rolled.releaseMarkdown).toContain("- Reviewing a change is faster:");
    // Engineering detail stays in the agent section, not the human draft.
    const humanSection = rolled.releaseMarkdown.split("## Agent changelog")[0] as string;
    expect(humanSection).not.toContain("session registry");
  });

  test("the rolled release file round-trips through the parser", () => {
    const parsed = parseReleaseFile(rolled.releaseFileName, rolled.releaseMarkdown);
    expect(parsed.version).toBe("0.1.0-rc.99");
    expect(parsed.channel).toBe("rc");
    expect(parsed.date).toBe("2026-08-01");
    expect(parsed.entries).toHaveLength(2);
    expect(parsed.entries[1]?.evidence).toBe("docs/qa/fixture-receipt.json");
  });

  test("resets the accumulator and refuses to roll it again (idempotence)", () => {
    expect(parseUnreleased(rolled.resetUnreleasedMarkdown)).toEqual([]);
    expect(() =>
      rollUnreleased({
        channel: "rc",
        date: "2026-08-01",
        unreleasedText: rolled.resetUnreleasedMarkdown,
        version: "0.1.0-rc.100",
      }),
    ).toThrow(/no entries to roll/);
  });

  test("an entry without a human summary paragraph refuses to draft", () => {
    expect(() =>
      draftHumanChangelog([
        {
          commits: "abc",
          contractsSpecs: "none",
          evidence: "none",
          invariants: "none changed",
          issues: "#1",
          lane: "lane",
          summaryParagraphs: [],
          title: "No summary (#1)",
        },
      ]),
    ).toThrow(/no human summary paragraph/);
  });
});

describe("release-notes bound", () => {
  test("short human text passes through unmodified", () => {
    const text = "- One change.\n- Another change.";
    expect(releaseNotesText(text)).toBe(text);
  });

  test("long human text is truncated to the exported bound", () => {
    const long = Array.from(
      { length: 200 },
      (_, index) => `- Bullet number ${index} with some descriptive words.`,
    ).join("\n");
    expect(long.length).toBeGreaterThan(RELEASE_NOTES_MAX_LENGTH);
    const bounded = releaseNotesText(long);
    expect(bounded.length).toBeLessThanOrEqual(RELEASE_NOTES_MAX_LENGTH);
    expect(bounded.endsWith("…")).toBe(true);
    // Truncation happens at a whitespace boundary, not mid-word.
    expect(/\S…$/.test(bounded)).toBe(true);
    expect(bounded.slice(0, -1)).toBe(bounded.slice(0, -1).trimEnd());
  });

  test("the bound constant is the documented 2000 characters", () => {
    expect(RELEASE_NOTES_MAX_LENGTH).toBe(2000);
  });
});

describe("route data module", () => {
  test("blocks parsing folds wrapped bullets and paragraphs", () => {
    const blocks = humanMarkdownToBlocks(
      "Intro paragraph\nspanning two lines.\n\n- First bullet\n  wrapping onward.\n- Second bullet.\n\nClosing paragraph.",
    );
    expect(blocks).toEqual([
      { kind: "paragraph", text: "Intro paragraph spanning two lines." },
      {
        items: ["First bullet wrapping onward.", "Second bullet."],
        kind: "bullets",
      },
      { kind: "paragraph", text: "Closing paragraph." },
    ]);
  });

  test("releases sort newest-first and render deterministically", () => {
    const older = parseReleaseFile(
      "2026-08-01-desktop-0.1.0-rc.99.md",
      rollUnreleased({
        channel: "rc",
        date: "2026-08-01",
        unreleasedText: FIXTURE_UNRELEASED,
        version: "0.1.0-rc.99",
      }).releaseMarkdown,
    );
    const newer = parseReleaseFile(
      "2026-08-02-desktop-0.1.0-rc.100.md",
      rollUnreleased({
        channel: "rc",
        date: "2026-08-02",
        unreleasedText: FIXTURE_UNRELEASED,
        version: "0.1.0-rc.100",
      }).releaseMarkdown,
    );
    const sorted = sortReleasesNewestFirst([older, newer]);
    expect(sorted.map((release) => release.version)).toEqual(["0.1.0-rc.100", "0.1.0-rc.99"]);
    const rendered = renderRouteDataModule([older, newer]);
    expect(rendered).toBe(renderRouteDataModule([newer, older]));
    expect(rendered.indexOf("0.1.0-rc.100")).toBeLessThan(rendered.indexOf("0.1.0-rc.99"));
    expect(rendered).toContain("GENERATED by scripts/changelog.ts");
  });
});

describe("filesystem roll + sync (fixture root)", () => {
  let fixtureRoot: string | null = null;

  afterEach(() => {
    if (fixtureRoot !== null) rmSync(fixtureRoot, { recursive: true });
    fixtureRoot = null;
  });

  const makeFixtureRoot = (): string => {
    const root = mkdtempSync(join(tmpdir(), "changelog-fixture-"));
    mkdirSync(join(root, CHANGELOG_DIR), { recursive: true });
    mkdirSync(dirname(join(root, ROUTE_DATA_MODULE_PATH)), { recursive: true });
    writeFileSync(join(root, CHANGELOG_DIR, UNRELEASED_FILE), FIXTURE_UNRELEASED);
    return root;
  };

  test("runRoll writes the release file, resets UNRELEASED, and syncs the module", () => {
    fixtureRoot = makeFixtureRoot();
    const rolled = runRoll(fixtureRoot, {
      channel: "rc",
      date: "2026-08-01",
      version: "0.1.0-rc.99",
    });
    const releaseText = readFileSync(
      join(fixtureRoot, CHANGELOG_DIR, rolled.releaseFileName),
      "utf8",
    );
    expect(releaseText).toContain("## Human changelog");
    expect(
      parseUnreleased(readFileSync(join(fixtureRoot, CHANGELOG_DIR, UNRELEASED_FILE), "utf8")),
    ).toEqual([]);
    const moduleText = readFileSync(join(fixtureRoot, ROUTE_DATA_MODULE_PATH), "utf8");
    expect(moduleText).toContain("0.1.0-rc.99");
    // The committed module now matches regeneration.
    expect(() => runCheck(fixtureRoot as string)).not.toThrow();
    // Sync is idempotent: running again produces byte-identical output.
    const first = readFileSync(join(fixtureRoot, ROUTE_DATA_MODULE_PATH), "utf8");
    runSync(fixtureRoot);
    expect(readFileSync(join(fixtureRoot, ROUTE_DATA_MODULE_PATH), "utf8")).toBe(first);
    // Rolling again refuses: the accumulator is empty and the file exists.
    expect(() =>
      runRoll(fixtureRoot as string, {
        channel: "rc",
        date: "2026-08-01",
        version: "0.1.0-rc.99",
      }),
    ).toThrow(/no entries to roll/);
  });

  test("runCheck fails when the committed module is stale", () => {
    fixtureRoot = makeFixtureRoot();
    runRoll(fixtureRoot, {
      channel: "rc",
      date: "2026-08-01",
      version: "0.1.0-rc.99",
    });
    writeFileSync(join(fixtureRoot, ROUTE_DATA_MODULE_PATH), "// stale\n");
    expect(() => runCheck(fixtureRoot as string)).toThrow(/is stale/);
  });
});

describe("the committed repository changelog", () => {
  const repoRoot = resolve(import.meta.dirname, "..");

  test("docs/changelog parses and the committed data module is in sync", () => {
    expect(() => runCheck(repoRoot)).not.toThrow();
  });

  test("the committed UNRELEASED.md parses under the required entry format", () => {
    const entries = parseUnreleased(
      readFileSync(join(repoRoot, CHANGELOG_DIR, UNRELEASED_FILE), "utf8"),
    );
    for (const entry of entries) {
      expect(entry.summaryParagraphs.length).toBeGreaterThan(0);
      expect(entry.lane.length).toBeGreaterThan(0);
    }
  });

  test("every committed release yields a bounded release-notes string", () => {
    const files = readFileSync(
      join(repoRoot, CHANGELOG_DIR, "2026-07-16-desktop-0.1.0-rc.13.md"),
      "utf8",
    );
    const release = parseReleaseFile("2026-07-16-desktop-0.1.0-rc.13.md", files);
    const notes = releaseNotesText(release.humanMarkdown);
    expect(notes.length).toBeGreaterThan(0);
    expect(notes.length).toBeLessThanOrEqual(RELEASE_NOTES_MAX_LENGTH);
  });
});
