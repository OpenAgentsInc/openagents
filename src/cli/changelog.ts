import { existsSync, readFileSync } from "fs";
import { join, resolve } from "path";
import { SettingsManager } from "./settings-manager.js";

export interface ChangelogEntry {
  version: string;
  date?: string;
  body: string;
}

const DEFAULT_CHANGELOG_PATH = resolve(import.meta.dir, "../..", "CHANGELOG.md");

const headingRegex = /^##\s+(.*)$/;
const versionRegex = /v?\d+\.\d+(?:\.\d+)?(?:[-\w.]*)?/;
const trailingDateRegex = /\(([^)]+)\)\s*$/;

export function parseChangelog(markdown: string): ChangelogEntry[] {
  const lines = markdown.split(/\r?\n/);
  const entries: Array<{ version: string; date?: string; bodyLines: string[] }> = [];
  let current: { version: string; date?: string; bodyLines: string[] } | null = null;

  for (const line of lines) {
    const headingMatch = line.match(headingRegex);
    if (headingMatch) {
      if (current) {
        entries.push(
          current.date
            ? { version: current.version, date: current.date, bodyLines: current.bodyLines }
            : { version: current.version, bodyLines: current.bodyLines },
        );
      }

      const headingText = headingMatch[1].trim();
      const versionMatch = headingText.match(versionRegex);
      const dateMatch = headingText.match(trailingDateRegex);
      const date = dateMatch?.[1]?.trim();

      current = {
        version: versionMatch ? versionMatch[0] : headingText,
        bodyLines: [],
      };
      if (date) {
        current.date = date;
      }
      continue;
    }

    if (current) {
      current.bodyLines.push(line);
    }
  }

  if (current) {
    entries.push(
      current.date
        ? { version: current.version, date: current.date, bodyLines: current.bodyLines }
        : { version: current.version, bodyLines: current.bodyLines },
    );
  }

  return entries
    .filter((entry) => entry.version.length > 0)
    .map((entry) => {
      const body = entry.bodyLines.join("\n").trim();
      if (entry.date) {
        return { version: entry.version, date: entry.date, body };
      }
      return { version: entry.version, body };
    });
}

export function formatChangelogEntries(entries: ChangelogEntry[]): string {
  return entries
    .map((entry) => {
      const heading = entry.date ? `${entry.version} (${entry.date})` : entry.version;
      const body = entry.body.trim();
      return [`## ${heading}`, body].filter(Boolean).join("\n");
    })
    .join("\n\n");
}

export function getUnseenEntries(entries: ChangelogEntry[], lastSeenVersion?: string): ChangelogEntry[] {
  if (!entries.length) return [];
  if (!lastSeenVersion) return entries;

  const lastSeenIndex = entries.findIndex((entry) => entry.version === lastSeenVersion);
  return lastSeenIndex === -1 ? entries : entries.slice(0, lastSeenIndex);
}

export interface ShowChangelogOptions {
  settingsManager: SettingsManager;
  changelogPath?: string;
  log?: (line: string) => void;
}

export interface ShowChangelogResult {
  displayed: ChangelogEntry[];
  latestVersion?: string;
}

export function showChangelogOnStart(options: ShowChangelogOptions): ShowChangelogResult {
  const changelogPath = options.changelogPath ?? DEFAULT_CHANGELOG_PATH;
  const logger = options.log ?? console.log;

  if (!existsSync(changelogPath)) {
    return { displayed: [] };
  }

  const content = readFileSync(changelogPath, "utf8");
  const entries = parseChangelog(content);
  if (!entries.length) {
    return { displayed: [] };
  }

  const lastSeen = options.settingsManager.getLastSeenChangelogVersion();
  const unseen = getUnseenEntries(entries, lastSeen);

  if (!unseen.length) {
    return { displayed: [], latestVersion: entries[0]?.version };
  }

  logger("=== OpenAgents Changelog ===");
  logger(formatChangelogEntries(unseen));
  logger("");

  options.settingsManager.setLastSeenChangelogVersion(entries[0].version);
  return { displayed: unseen, latestVersion: entries[0].version };
}

export function resolveDefaultChangelogPath(baseDir?: string): string {
  if (baseDir) {
    return join(baseDir, "CHANGELOG.md");
  }
  return DEFAULT_CHANGELOG_PATH;
}
