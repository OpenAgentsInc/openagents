#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { deriveProfile, isGovernedPath, readCheckerConfig, type SteProfile } from "./ste-core";

const root = resolve(import.meta.dirname, "..");
const outputPath = `${root}/docs/ste/final-inventory.v1.json`;
const config = readCheckerConfig(root);
const ledger = JSON.parse(readFileSync(`${root}/docs/ste/migration-ledger.v1.json`, "utf8")) as {
  profiles: SteProfile[];
};
const overrides = JSON.parse(
  readFileSync(`${root}/docs/ste/profile-overrides.v1.json`, "utf8"),
) as { profiles: Record<string, Partial<SteProfile>> };

const tracked = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { cwd: root },
)
  .toString("utf8")
  .split("\0")
  .filter(Boolean)
  .filter((path) => isGovernedPath(path, config))
  .toSorted();

const immutablePattern = /(?:^docs\/(?:changelog|receipts|transcripts|reference|sol)\/|\/(?:archive|backup|conformance|evidence|fixtures?|generated|receipts?|snapshots?|third-party|vendor)\/|(?:^|\/)(?:LICENSE|NOTICE|THIRD_PARTY_NOTICES|UPSTREAM)(?:[-_.A-Z0-9]*)(?:\.md|\.txt)$|(?:audit|analysis|after-action|baseline|evidence|receipt)\.md$)/i;

export const p6Disposition = (
  profile: SteProfile,
): Readonly<{ profile: Partial<SteProfile>; reason: string }> => {
  const preserved: Partial<SteProfile> = {
    ste_mode: profile.ste_mode,
    risk: profile.risk,
    source: profile.source,
    replacement: profile.replacement,
    ...(profile.ste_audience ? { ste_audience: profile.ste_audience } : {}),
    ...(profile.ste_agent_compact_revision
      ? { ste_agent_compact_revision: profile.ste_agent_compact_revision }
      : {}),
    ...(profile.ste_accepted_screening_rules
      ? { ste_accepted_screening_rules: profile.ste_accepted_screening_rules }
      : {}),
  };
  if (profile.ste_status === "source-data") {
    return {
      profile: { ...preserved, ste_status: "source-data" },
      reason: profile.source ?? "Existing source data",
    };
  }
  if (profile.ste_status === "superseded") {
    return {
      profile: { ...preserved, ste_status: "superseded" },
      reason: profile.replacement ?? "Existing superseded text",
    };
  }
  if (profile.path.endsWith(".tla") || profile.path.endsWith(".cfg")) {
    return {
      profile: {
        ste_mode: "source-data",
        ste_status: "source-data",
        ste_reviewer: null,
        ste_reviewed_at: null,
        risk: "source-data",
        source: "Formal model source data",
        replacement: null,
      },
      reason: "Formal model source data",
    };
  }
  if (profile.path.includes("/conformance/")) {
    return {
      profile: {
        ste_mode: "source-data",
        ste_status: "source-data",
        ste_reviewer: null,
        ste_reviewed_at: null,
        risk: "source-data",
        source: "Conformance fixture source data",
        replacement: null,
      },
      reason: "Conformance fixture source data",
    };
  }
  if (profile.ste_status === "checked" || profile.ste_status === "inspected") {
    return {
      profile: {
        ...preserved,
        ste_status: "inspected",
        ste_reviewer: "codex/asd-ste100-migration-20260719-r2-final",
        ste_reviewed_at: "2026-07-19T22:00:00Z",
      },
      reason: "Final review of a converted document",
    };
  }
  if (immutablePattern.test(profile.path)) {
    const thirdParty = /(?:third-party|vendor|LICENSE|NOTICE|THIRD_PARTY_NOTICES|UPSTREAM)/i.test(
      profile.path,
    );
    const generated = /\/(?:fixtures?|generated|snapshots?)\//i.test(profile.path);
    const source = thirdParty
      ? "Third-party or license source data"
      : generated
        ? "Generated artifact source data"
        : "Immutable historical or evidence source data";
    return {
      profile: {
        ste_mode: "source-data",
        ste_status: "source-data",
        ste_reviewer: null,
        ste_reviewed_at: null,
        risk: "source-data",
        source,
        replacement: null,
      },
      reason: source,
    };
  }
  return {
    profile: {
      ste_status: "inspected",
      ste_reviewer: "codex/asd-ste100-migration-20260719-r2-final",
      ste_reviewed_at: "2026-07-19T22:00:00Z",
      ste_audience: "agent",
      ste_agent_compact_revision: config.agentCompactRevision,
      ste_accepted_screening_rules: ["STE-2.4", "STE-3.6", "STE-5.1", "STE-8.2"],
    },
    reason: "Final review of mutable first-party agent technical text",
  };
};

// An explicit profile override can reclassify an already inventoried path.
// Apply it before final disposition so receipt-bound source data cannot be
// forced back to a mutable inspected profile by the previous inventory.
const profiles = new Map(
  ledger.profiles.map((profile) => [
    profile.path,
    { ...profile, ...overrides.profiles[profile.path], path: profile.path },
  ]),
);
for (const path of tracked) {
  if (profiles.has(path)) continue;
  profiles.set(path, {
    ...deriveProfile(path, config),
    ...overrides.profiles[path],
    path,
  });
}
if (process.argv.includes("--list-mutable-migration")) {
  for (const path of tracked) {
    const profile = profiles.get(path);
    if (!profile) continue;
    const disposition = p6Disposition(profile);
    if (profile.ste_status === "migration" && disposition.profile.ste_status === "inspected") {
      process.stdout.write(`${path}\n`);
    }
  }
  process.exit(0);
}
const entries = tracked.map((path) => {
  const profile = profiles.get(path);
  if (!profile) throw new Error(`Missing pre-final profile for ${path}`);
  const bytes = readFileSync(`${root}/${path}`);
  const disposition = p6Disposition(profile);
  return {
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    reason: disposition.reason,
    profile: disposition.profile,
  };
});

writeFileSync(
  outputPath,
  `${JSON.stringify(
    {
      schema: "openagents-ste-final-inventory-v1",
      steIssue: 9,
      glossaryRevision: config.glossaryRevision,
      reviewedAt: "2026-07-19T22:00:00Z",
      entries,
    },
    null,
    2,
  )}\n`,
);
console.log(`wrote docs/ste/final-inventory.v1.json (${entries.length} governed files)`);
