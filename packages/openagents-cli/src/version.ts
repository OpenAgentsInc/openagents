/**
 * The CLI's version, read from `package.json` rather than restated.
 *
 * A hand-edited constant drifted: the published package reported `0.3.0` while
 * the committed constant said `0.1.7` and `package.json` said `0.2.1`, so
 * `openagents --version` named a release that was never published and nothing
 * failed. There is one place a version can live, and it is the manifest npm
 * publishes from.
 *
 * `package.json` sits one directory above the compiled output in both layouts
 * that matter: `dist/version.js` in a local build, and `dist/version.js` beside
 * the manifest in the published tarball. npm always includes `package.json`
 * regardless of the `files` list.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Reported when the manifest cannot be read. It is deliberately not a version:
 * a wrong number is worse than an obvious absence, because a number gets
 * believed and pasted into a bug report.
 */
const UNKNOWN = "unknown";

export const VERSION: string = readVersion();

export function manifestPath(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "..", "package.json");
}

function readVersion(): string {
  try {
    const manifest = JSON.parse(readFileSync(manifestPath(), "utf8")) as Record<string, unknown>;
    const version = manifest["version"];
    return typeof version === "string" && version.length > 0 ? version : UNKNOWN;
  } catch {
    return UNKNOWN;
  }
}
