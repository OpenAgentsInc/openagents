#!/usr/bin/env node

import { lstatSync, readdirSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import {
  PRIVACY_SCOPE_EXPORT_SCHEMA,
  PRIVACY_SCOPE_MANIFEST,
  PRIVACY_SCOPES,
} from "./livekit-privacy-scan-lib.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-privacy-export.mjs \\
    --scope <privacy-scope> \\
    --source-base-revision <40-hex-git-revision> \\
    --started-at <ISO-8601> --completed-at <ISO-8601> \\
    --input <private-read-only-export-directory> --apply

Seals one already-complete read-only export for the LiveKit privacy scanner.
The command never prints object names or contents. It refuses symbolic links,
special files, empty exports, an existing manifest, and capture windows over
two hours.
`);
};

const parseArgs = (args) => {
  const parsed = { apply: false };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    if (argument === "--scope") parsed.scope = value;
    else if (argument === "--source-base-revision") parsed.sourceBaseRevision = value;
    else if (argument === "--started-at") parsed.startedAt = value;
    else if (argument === "--completed-at") parsed.completedAt = value;
    else if (argument === "--input") parsed.input = value;
    else throw new Error(`unsupported argument ${argument}`);
    index += 1;
  }
  if (!parsed.apply) throw new Error("privacy export sealing requires --apply");
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }
  for (const key of ["scope", "sourceBaseRevision", "startedAt", "completedAt", "input"]) {
    if (!parsed[key]) throw new Error(`required privacy export argument is missing: ${key}`);
  }
  if (!PRIVACY_SCOPES.includes(parsed.scope)) throw new Error("privacy export scope is unsupported");
  if (!/^[0-9a-f]{40}$/u.test(parsed.sourceBaseRevision)) {
    throw new Error("source-base-revision must be a full Git revision");
  }
  const startedAt = Date.parse(parsed.startedAt);
  const completedAt = Date.parse(parsed.completedAt);
  if (
    !Number.isFinite(startedAt) ||
    !Number.isFinite(completedAt) ||
    completedAt < startedAt ||
    completedAt - startedAt > 2 * 60 * 60_000 ||
    completedAt > Date.now() + 5 * 60_000
  ) {
    throw new Error("privacy export capture window is invalid");
  }
  return parsed;
};

const inventory = (input) => {
  const root = resolve(input);
  const rootStat = lstatSync(root);
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
    throw new Error("privacy export input must be a real directory");
  }
  const canonicalRoot = realpathSync(root);
  const objects = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (directory === canonicalRoot && entry.name === PRIVACY_SCOPE_MANIFEST) {
        throw new Error("privacy export manifest already exists");
      }
      const path = join(directory, entry.name);
      const entryStat = lstatSync(path);
      if (entryStat.isSymbolicLink()) throw new Error("privacy export contains a symbolic link");
      if (entryStat.isDirectory()) {
        visit(path);
        continue;
      }
      if (!entryStat.isFile()) throw new Error("privacy export contains a special file");
      const canonicalPath = realpathSync(path);
      if (canonicalPath !== canonicalRoot && !canonicalPath.startsWith(`${canonicalRoot}${sep}`)) {
        throw new Error("privacy export object escapes its root");
      }
      objects.push(statSync(canonicalPath).size);
    }
  };
  visit(canonicalRoot);
  if (objects.length === 0) throw new Error("privacy export is empty");
  const byteCount = objects.reduce((total, size) => total + size, 0);
  if (byteCount === 0) throw new Error("privacy export contains no evidence bytes");
  return { byteCount, objectCount: objects.length, root: canonicalRoot };
};

try {
  const parsed = parseArgs(process.argv.slice(2));
  const { byteCount, objectCount, root } = inventory(parsed.input);
  const manifest = {
    schemaVersion: PRIVACY_SCOPE_EXPORT_SCHEMA,
    scope: parsed.scope,
    sourceBaseRevision: parsed.sourceBaseRevision,
    collectionMode: "read_only",
    complete: true,
    startedAt: new Date(parsed.startedAt).toISOString(),
    completedAt: new Date(parsed.completedAt).toISOString(),
    objectCount,
    byteCount,
  };
  writeFileSync(join(root, PRIVACY_SCOPE_MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({
      schemaVersion: manifest.schemaVersion,
      scope: manifest.scope,
      complete: manifest.complete,
      objectCount,
      byteCount,
    })}\n`,
  );
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
}
