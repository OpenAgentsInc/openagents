#!/usr/bin/env node
// DIST-14 (#8927): release changelog generation.
//
// Every release carries two changelog artifacts (see docs/changelog/README.md):
// a human changelog published at /changelog and embedded (bounded) in the
// signed ReleaseSet payload, and a detailed agent changelog that lives in
// docs/changelog/ as dated per-release files.
//
// Usage (from the repo root, also exposed as `pnpm changelog`):
//   node --import tsx scripts/changelog.ts roll --version 0.1.0-rc.14 --channel rc --date 2026-07-20
//   node --import tsx scripts/changelog.ts sync
//   node --import tsx scripts/changelog.ts check
//   node --import tsx scripts/changelog.ts notes --version 0.1.0-rc.13
//
// `roll` moves the UNRELEASED.md entries into a dated release file, drafts
// the human changelog from each entry's first summary paragraph (the delegated
// release operator MUST review it before commit — the committed artifact is
// reviewed text, not raw generation), resets the accumulator, regenerates the committed
// /changelog data module, and prints the bounded release-notes string.
//
// The /changelog Start route imports the generated data module at build time.
// The site is built from committed source (exactly how /download ships its
// release constants), so a build-time import is the honest, cache-correct
// publication strategy — no live changelog backend exists or is invented.

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

/**
 * Maximum length (in characters) of the release-notes string embedded in the
 * signed ReleaseSet v2 payload (#8915). The ReleaseSet schema should import
 * this constant (or move it into the schema package and re-export it here) so
 * the schema bound and the generator bound can never drift. The full human
 * entry always lives on /changelog; the bounded string is a truncated
 * plain-text projection, never the authority.
 */
export const RELEASE_NOTES_MAX_LENGTH = 2000;

export const CHANGELOG_DIR = "docs/changelog";
export const UNRELEASED_FILE = "UNRELEASED.md";
export const ROUTE_DATA_MODULE_PATH =
  "apps/openagents.com/apps/start/src/routes/-changelog-data.gen.ts";
export const AGENT_CHANGELOG_BASE_URL =
  "https://github.com/OpenAgentsInc/openagents/blob/main/docs/changelog";

const RELEASE_FILE_PATTERN = /^(\d{4}-\d{2}-\d{2})-desktop-(.+)\.md$/;

export const ENTRY_METADATA_KEYS = [
  "issues",
  "commits",
  "contracts-specs",
  "invariants",
  "evidence",
  "lane",
] as const;

export type ChangelogEntry = Readonly<{
  title: string;
  issues: string;
  commits: string;
  contractsSpecs: string;
  invariants: string;
  evidence: string;
  lane: string;
  /** First paragraph is the human-centric summary used for the human draft. */
  summaryParagraphs: readonly string[];
}>;

export type ReleaseChangelog = Readonly<{
  fileName: string;
  title: string;
  version: string;
  channel: string;
  date: string;
  humanMarkdown: string;
  entries: readonly ChangelogEntry[];
  attribution: ReleaseAttribution;
}>;

export type ReleaseAttribution = Readonly<{
  triggerKind: string;
  triggeredBy: string;
  releaseActor: string;
  authorityRef: string;
  releaseUrl: string;
  sourceFeedback: string;
}>;

export type ChangelogHumanBlock =
  | Readonly<{ kind: "paragraph"; text: string }>
  | Readonly<{ kind: "bullets"; items: readonly string[] }>;

export type ChangelogReleaseData = Readonly<{
  version: string;
  channel: string;
  date: string;
  title: string;
  blocks: readonly ChangelogHumanBlock[];
  agentChangelogUrl: string;
  attribution: ReleaseAttribution;
}>;

const splitParagraphs = (text: string): readonly string[] =>
  text
    .split(/\n[ \t]*\n/)
    .map((paragraph) => paragraph.replace(/[ \t]*\n[ \t]*/g, "\n").trim())
    .filter((paragraph) => paragraph.length > 0);

const parseEntry = (heading: string, body: string): ChangelogEntry => {
  const metadata = new Map<string, string>();
  const bodyLines = body.split("\n");
  let index = 0;
  while (index < bodyLines.length) {
    const line = bodyLines[index] ?? "";
    if (line.trim().length === 0) {
      index += 1;
      continue;
    }
    const match = /^- ([a-z-]+): (.*)$/.exec(line);
    if (match === null) break;
    let value = match[2] ?? "";
    // A metadata value may wrap onto indented continuation lines.
    while (index + 1 < bodyLines.length && /^ {2,}\S/.test(bodyLines[index + 1] ?? "")) {
      value = `${value} ${(bodyLines[index + 1] ?? "").trim()}`;
      index += 1;
    }
    metadata.set(match[1] ?? "", value.trim());
    index += 1;
  }
  const missing = ENTRY_METADATA_KEYS.filter((key) => !metadata.has(key));
  if (missing.length > 0) {
    throw new Error(
      `changelog entry "${heading}" is missing required metadata: ${missing.join(", ")} (see docs/changelog/README.md)`,
    );
  }
  const summaryParagraphs = splitParagraphs(bodyLines.slice(index).join("\n"));
  return {
    commits: metadata.get("commits") ?? "",
    contractsSpecs: metadata.get("contracts-specs") ?? "",
    evidence: metadata.get("evidence") ?? "",
    invariants: metadata.get("invariants") ?? "",
    issues: metadata.get("issues") ?? "",
    lane: metadata.get("lane") ?? "",
    summaryParagraphs,
    title: heading,
  };
};

const splitSections = (
  text: string,
  headingPrefix: string,
): ReadonlyArray<Readonly<{ heading: string; body: string }>> => {
  const sections: Array<{ heading: string; body: string }> = [];
  const lines = text.split("\n");
  let current: { heading: string; body: string[] } | null = null;
  for (const line of lines) {
    if (line.startsWith(headingPrefix)) {
      if (current !== null) {
        sections.push({
          body: current.body.join("\n"),
          heading: current.heading,
        });
      }
      current = { body: [], heading: line.slice(headingPrefix.length).trim() };
    } else if (current !== null) {
      current.body.push(line);
    }
  }
  if (current !== null) {
    sections.push({ body: current.body.join("\n"), heading: current.heading });
  }
  return sections;
};

/** Parse the UNRELEASED.md accumulator into entries (may be empty). */
export const parseUnreleased = (text: string): readonly ChangelogEntry[] =>
  splitSections(text, "## ").map((section) => parseEntry(section.heading, section.body));

/** Parse a dated release file (front metadata + human + agent sections). */
export const parseReleaseFile = (fileName: string, text: string): ReleaseChangelog => {
  const match = RELEASE_FILE_PATTERN.exec(fileName);
  if (match === null) {
    throw new Error(
      `release file name "${fileName}" does not match YYYY-MM-DD-desktop-<version>.md`,
    );
  }
  const titleMatch = /^# (.+)$/m.exec(text);
  const readMeta = (key: string): string => {
    const metaMatch = new RegExp(`^- ${key}: (.+)$`, "m").exec(text);
    if (metaMatch === null) {
      throw new Error(`release file "${fileName}" is missing "- ${key}:"`);
    }
    return (metaMatch[1] ?? "").trim();
  };
  const version = readMeta("version");
  const date = readMeta("date");
  if (match[2] !== version) {
    throw new Error(
      `release file "${fileName}" names version "${match[2]}" but declares "${version}"`,
    );
  }
  if (match[1] !== date) {
    throw new Error(`release file "${fileName}" names date "${match[1]}" but declares "${date}"`);
  }
  const topSections = splitSections(text, "## ");
  const human = topSections.find((section) => section.heading === "Human changelog");
  const agent = topSections.find((section) => section.heading === "Agent changelog");
  if (human === undefined || agent === undefined) {
    throw new Error(
      `release file "${fileName}" must contain "## Human changelog" and "## Agent changelog"`,
    );
  }
  return {
    attribution: {
      authorityRef: readMeta("authority"),
      releaseActor: readMeta("release-actor"),
      releaseUrl: readMeta("release-url"),
      sourceFeedback: readMeta("source-feedback"),
      triggeredBy: readMeta("triggered-by"),
      triggerKind: readMeta("trigger-kind"),
    },
    channel: readMeta("channel"),
    date,
    entries: splitSections(agent.body, "### ").map((section) =>
      parseEntry(section.heading, section.body),
    ),
    fileName,
    humanMarkdown: human.body.trim(),
    title: titleMatch?.[1]?.trim() ?? `OpenAgents Desktop ${version}`,
    version,
  };
};

/** One draft bullet per entry, from the entry's first summary paragraph. */
export const draftHumanChangelog = (entries: readonly ChangelogEntry[]): string =>
  entries
    .map((entry) => {
      const summary = entry.summaryParagraphs[0];
      if (summary === undefined) {
        throw new Error(
          `changelog entry "${entry.title}" has no human summary paragraph — the release script drafts the human changelog from it`,
        );
      }
      return `- ${summary.replace(/\s+/g, " ")}`;
    })
    .join("\n");

const renderEntry = (entry: ChangelogEntry, headingPrefix: string): string =>
  [
    `${headingPrefix} ${entry.title}`,
    "",
    `- issues: ${entry.issues}`,
    `- commits: ${entry.commits}`,
    `- contracts-specs: ${entry.contractsSpecs}`,
    `- invariants: ${entry.invariants}`,
    `- evidence: ${entry.evidence}`,
    `- lane: ${entry.lane}`,
    "",
    entry.summaryParagraphs.join("\n\n"),
  ].join("\n");

export const renderReleaseMarkdown = (release: Omit<ReleaseChangelog, "fileName">): string =>
  [
    `# ${release.title}`,
    "",
    `- version: ${release.version}`,
    `- channel: ${release.channel}`,
    `- date: ${release.date}`,
    `- trigger-kind: ${release.attribution.triggerKind}`,
    `- triggered-by: ${release.attribution.triggeredBy}`,
    `- release-actor: ${release.attribution.releaseActor}`,
    `- authority: ${release.attribution.authorityRef}`,
    `- release-url: ${release.attribution.releaseUrl}`,
    `- source-feedback: ${release.attribution.sourceFeedback}`,
    "",
    "## Human changelog",
    "",
    release.humanMarkdown,
    "",
    "## Agent changelog",
    "",
    release.entries.map((entry) => renderEntry(entry, "###")).join("\n\n"),
    "",
  ].join("\n");

export const EMPTY_UNRELEASED = `# Unreleased

Entries accumulate here between releases. Appending an entry when your change
lands on \`main\` is part of the CLAIM-RELEASE protocol — see \`README.md\` in
this directory for the required format. \`pnpm changelog roll\` moves these
entries into the next dated release file.
`;

export type RollInput = Readonly<{
  unreleasedText: string;
  version: string;
  channel: string;
  date: string;
  attribution?: ReleaseAttribution;
}>;

export const defaultAutonomousReleaseAttribution = (version: string): ReleaseAttribution => ({
  triggerKind: "agent_change",
  triggeredBy: "OpenAgents release transaction (no external feedback trigger)",
  releaseActor: "OpenAgents release operator",
  authorityRef:
    "AUTHORITY.md revision 2; program.full_auto_release; grant.autonomous_rc_release_and_communication",
  releaseUrl: `https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v${version}`,
  sourceFeedback: "none recorded",
});

export type RollOutput = Readonly<{
  releaseFileName: string;
  releaseMarkdown: string;
  resetUnreleasedMarkdown: string;
  releaseNotes: string;
}>;

/** Roll the accumulator into a dated release file. Pure; refuses when empty. */
export const rollUnreleased = (input: RollInput): RollOutput => {
  const entries = parseUnreleased(input.unreleasedText);
  if (entries.length === 0) {
    throw new Error("UNRELEASED.md has no entries to roll — nothing to release");
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(input.date)) {
    throw new Error(`--date must be YYYY-MM-DD, got "${input.date}"`);
  }
  const humanMarkdown = draftHumanChangelog(entries);
  const releaseMarkdown = renderReleaseMarkdown({
    attribution: input.attribution ?? defaultAutonomousReleaseAttribution(input.version),
    channel: input.channel,
    date: input.date,
    entries,
    humanMarkdown,
    title: `OpenAgents Desktop ${input.version} — ${input.date}`,
    version: input.version,
  });
  return {
    releaseFileName: `${input.date}-desktop-${input.version}.md`,
    releaseMarkdown,
    releaseNotes: releaseNotesText(humanMarkdown),
    resetUnreleasedMarkdown: EMPTY_UNRELEASED,
  };
};

/**
 * Plain-text projection of a human changelog, bounded for the signed
 * ReleaseSet payload. Never exceeds RELEASE_NOTES_MAX_LENGTH.
 */
export const releaseNotesText = (humanMarkdown: string): string => {
  const plain = humanMarkdown
    .split("\n")
    .map((line) => line.trim())
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
  if (plain.length <= RELEASE_NOTES_MAX_LENGTH) return plain;
  const slice = plain.slice(0, RELEASE_NOTES_MAX_LENGTH - 1);
  const lastBreak = Math.max(slice.lastIndexOf(" "), slice.lastIndexOf("\n"));
  const cut = lastBreak > RELEASE_NOTES_MAX_LENGTH / 2 ? slice.slice(0, lastBreak) : slice;
  return `${cut.trimEnd()}…`;
};

/** Human markdown -> renderable blocks (paragraphs and bullet lists). */
export const humanMarkdownToBlocks = (humanMarkdown: string): readonly ChangelogHumanBlock[] => {
  const blocks: ChangelogHumanBlock[] = [];
  let bullets: string[] = [];
  let paragraph: string[] = [];
  const flushBullets = (): void => {
    if (bullets.length > 0) {
      blocks.push({ items: bullets, kind: "bullets" });
      bullets = [];
    }
  };
  const flushParagraph = (): void => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
  };
  for (const rawLine of humanMarkdown.split("\n")) {
    const line = rawLine.trim();
    if (line.length === 0) {
      flushBullets();
      flushParagraph();
    } else if (line.startsWith("- ")) {
      flushParagraph();
      bullets.push(line.slice(2).trim());
    } else if (bullets.length > 0) {
      // Continuation of a wrapped bullet line.
      bullets[bullets.length - 1] = `${bullets[bullets.length - 1]} ${line}`;
    } else {
      paragraph.push(line);
    }
  }
  flushBullets();
  flushParagraph();
  return blocks;
};

export const toReleaseData = (release: ReleaseChangelog): ChangelogReleaseData => ({
  agentChangelogUrl: `${AGENT_CHANGELOG_BASE_URL}/${release.fileName}`,
  blocks: humanMarkdownToBlocks(release.humanMarkdown),
  channel: release.channel,
  date: release.date,
  title: release.title,
  version: release.version,
  attribution: release.attribution,
});

/** Newest first: date desc, then file name desc for same-day releases. */
export const sortReleasesNewestFirst = (
  releases: readonly ReleaseChangelog[],
): readonly ReleaseChangelog[] =>
  [...releases].sort((a, b) =>
    a.date === b.date ? b.fileName.localeCompare(a.fileName) : b.date.localeCompare(a.date),
  );

/** Render the committed data module the /changelog Start route imports. */
export const renderRouteDataModule = (releases: readonly ReleaseChangelog[]): string => {
  const data = sortReleasesNewestFirst(releases).map(toReleaseData);
  return [
    "// GENERATED by scripts/changelog.ts — DO NOT EDIT BY HAND.",
    "// Source of truth: the dated release files in docs/changelog/.",
    "// Regenerate with: pnpm changelog sync",
    "",
    "export type ChangelogHumanBlock =",
    '  | Readonly<{ kind: "paragraph"; text: string }>',
    '  | Readonly<{ kind: "bullets"; items: ReadonlyArray<string> }>;',
    "",
    "export type ChangelogRelease = Readonly<{",
    "  version: string;",
    "  channel: string;",
    "  date: string;",
    "  title: string;",
    "  blocks: ReadonlyArray<ChangelogHumanBlock>;",
    "  agentChangelogUrl: string;",
    "  attribution: Readonly<{",
    "    triggerKind: string;",
    "    triggeredBy: string;",
    "    releaseActor: string;",
    "    authorityRef: string;",
    "    releaseUrl: string;",
    "    sourceFeedback: string;",
    "  }>;",
    "}>;",
    "",
    "/** Newest release first. */",
    "export const CHANGELOG_RELEASES: ReadonlyArray<ChangelogRelease> =",
    `  ${JSON.stringify(data, null, 2).split("\n").join("\n  ")};`,
    "",
  ].join("\n");
};

// --- Filesystem layer (root-relative so tests can run against fixtures) ---

export const readReleases = (rootDir: string): readonly ReleaseChangelog[] => {
  const dir = join(rootDir, CHANGELOG_DIR);
  return readdirSync(dir)
    .filter((name) => RELEASE_FILE_PATTERN.test(name))
    .map((name) => parseReleaseFile(name, readFileSync(join(dir, name), "utf8")));
};

export const runSync = (rootDir: string): string => {
  const modulePath = join(rootDir, ROUTE_DATA_MODULE_PATH);
  const rendered = renderRouteDataModule(readReleases(rootDir));
  writeFileSync(modulePath, rendered);
  return modulePath;
};

export const runCheck = (rootDir: string): void => {
  const modulePath = join(rootDir, ROUTE_DATA_MODULE_PATH);
  const committed = existsSync(modulePath) ? readFileSync(modulePath, "utf8") : "";
  const rendered = renderRouteDataModule(readReleases(rootDir));
  if (committed !== rendered) {
    throw new Error(
      `${ROUTE_DATA_MODULE_PATH} is stale — run \`pnpm changelog sync\` and commit the result`,
    );
  }
};

export const runRoll = (rootDir: string, input: Omit<RollInput, "unreleasedText">): RollOutput => {
  const unreleasedPath = join(rootDir, CHANGELOG_DIR, UNRELEASED_FILE);
  const rolled = rollUnreleased({
    ...input,
    unreleasedText: readFileSync(unreleasedPath, "utf8"),
  });
  const releasePath = join(rootDir, CHANGELOG_DIR, rolled.releaseFileName);
  if (existsSync(releasePath)) {
    throw new Error(
      `${rolled.releaseFileName} already exists — refusing to overwrite a released changelog`,
    );
  }
  writeFileSync(releasePath, rolled.releaseMarkdown);
  writeFileSync(unreleasedPath, rolled.resetUnreleasedMarkdown);
  runSync(rootDir);
  return rolled;
};

// --- CLI ---

const argValue = (args: readonly string[], flag: string): string => {
  const index = args.indexOf(flag);
  const value = args[index + 1];
  if (index === -1 || value === undefined) {
    throw new Error(`missing required ${flag} <value>`);
  }
  return value;
};

const main = (): void => {
  const rootDir = resolve(import.meta.dirname, "..");
  const [command, ...args] = process.argv.slice(2);
  switch (command) {
    case "roll": {
      const rolled = runRoll(rootDir, {
        channel: argValue(args, "--channel"),
        date: argValue(args, "--date"),
        version: argValue(args, "--version"),
      });
      console.log(`wrote ${CHANGELOG_DIR}/${rolled.releaseFileName}`);
      console.log(`reset ${CHANGELOG_DIR}/${UNRELEASED_FILE}`);
      console.log(`regenerated ${ROUTE_DATA_MODULE_PATH}`);
      console.log("\nRELEASE-OPERATOR REVIEW: the human changelog section is a draft. Edit it");
      console.log(
        "for clarity and attribution before committing — the committed artifact is reviewed",
      );
      console.log("text, not raw generation.\n");
      console.log(
        `release-notes string (${rolled.releaseNotes.length}/${RELEASE_NOTES_MAX_LENGTH} chars):\n`,
      );
      console.log(rolled.releaseNotes);
      return;
    }
    case "sync": {
      console.log(`regenerated ${runSync(rootDir)}`);
      return;
    }
    case "check": {
      runCheck(rootDir);
      console.log(`${ROUTE_DATA_MODULE_PATH} matches docs/changelog/`);
      return;
    }
    case "notes": {
      const version = argValue(args, "--version");
      const release = readReleases(rootDir).find((candidate) => candidate.version === version);
      if (release === undefined) {
        throw new Error(`no release file found for version ${version}`);
      }
      console.log(releaseNotesText(release.humanMarkdown));
      return;
    }
    default:
      throw new Error(`usage: changelog <roll|sync|check|notes> — got "${command ?? ""}"`);
  }
};

if (import.meta.main) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
