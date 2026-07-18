#!/usr/bin/env node
/** Bounded, idempotent release communication across GitHub issues and Forum. */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  decodeReleasePublicationManifest,
  resolveRepositoryFile,
  validateReleasePublication,
  type ReleasePublicationManifest,
} from "./github-release.js";

export type ReleaseCommunicationPhase = "candidate" | "published" | "rolled_back";

export interface ReleaseCommunicationPort {
  issueHasMarker(issue: number, marker: string): Promise<boolean>;
  commentOnIssue(issue: number, body: string): Promise<void>;
  createForumTopic(
    input: Readonly<{
      forum: "release-candidates";
      title: string;
      body: string;
      idempotencyKey: string;
    }>,
  ): Promise<Readonly<{ topicId: string; url?: string }>>;
  replyToForumTopic(
    input: Readonly<{
      topicId: string;
      body: string;
      idempotencyKey: string;
    }>,
  ): Promise<void>;
}

export type ReleaseCommunicationResult = Readonly<{
  phase: ReleaseCommunicationPhase;
  githubCommentsCreated: number;
  forumTopicId: string;
}>;

export const releaseCommunicationMarker = (
  version: string,
  phase: ReleaseCommunicationPhase,
): string => `<!-- openagents-release-communication:v1:${version}:${phase} -->`;

export const assertOpenAgentsReleaseUrl = (releaseUrl: string): void => {
  const release = new URL(releaseUrl);
  if (
    release.protocol !== "https:" ||
    release.hostname !== "github.com" ||
    !release.pathname.startsWith("/OpenAgentsInc/openagents/releases/")
  )
    throw new Error("release communication URL must be an OpenAgentsInc/openagents release");
};

const assertReleaseCommunicationUrls = (releaseUrl: string, changelogUrl: string): void => {
  assertOpenAgentsReleaseUrl(releaseUrl);
  const changelog = new URL(changelogUrl);
  if (
    changelog.protocol !== "https:" ||
    changelog.hostname !== "openagents.com" ||
    changelog.pathname !== "/changelog"
  )
    throw new Error("release communication changelog URL must be openagents.com/changelog");
};

const testerInstructions = (testers: readonly string[]): string =>
  testers.length === 0
    ? "Candidate feedback is welcome on this issue."
    : `${testers.join(" ")}: please test the candidate and reply here with:\n\n` +
      "```text\nResult: PASS | BLOCKED\nSeverity: P0 | P1 | P2\nObserved: <what happened>\n```";

export const renderReleaseCommunication = (
  input: Readonly<{
    manifest: ReleasePublicationManifest;
    phase: ReleaseCommunicationPhase;
    releaseUrl: string;
    changelogUrl: string;
  }>,
): string => {
  const { manifest, phase } = input;
  const common = [
    releaseCommunicationMarker(manifest.version, phase),
    `OpenAgents Desktop ${manifest.version} — ${phase.replace("_", " ")}`,
    "",
  ];
  if (phase === "candidate") {
    return [
      ...common,
      `${manifest.publicationClass === "desktop_signed_release_set" ? "Signed ReleaseSet candidate" : "Experimental candidate"}: ${input.releaseUrl}`,
      `Changelog: ${input.changelogUrl}`,
      "",
      testerInstructions(manifest.requestedTesters),
      "",
      `Trigger: ${manifest.trigger.kind} — ${manifest.trigger.actor} (${manifest.trigger.ref})`,
      `Authority: ${manifest.authority.profileId} revision ${manifest.authority.profileRevision}; ${manifest.authority.grantRef}`,
      manifest.publicationClass === "desktop_signed_release_set"
        ? "GitHub is the candidate mirror; update admission still depends on the signed OpenAgents feed and platform receipts."
        : `GitHub-only experimental limitations: ${manifest.limitations.join("; ")}`,
    ].join("\n");
  }
  if (phase === "published") {
    return [
      ...common,
      `Release: ${input.releaseUrl}`,
      `Changelog: ${input.changelogUrl}`,
      "",
      "The candidate passed its declared gates and is public. Any follow-up feedback on this issue is automatically eligible for release-feedback intake.",
      `Published under ${manifest.authority.profileId} revision ${manifest.authority.profileRevision}; ${manifest.authority.grantRef}.`,
    ].join("\n");
  }
  return [
    ...common,
    `Affected release: ${input.releaseUrl}`,
    `Status and recovery notes: ${input.changelogUrl}`,
    "",
    "The candidate or service rollout was rolled back through the bounded release path. Installed-client downgrade remains prohibited; the corrective release must use a newer version.",
    `Action authority: ${manifest.authority.profileId} revision ${manifest.authority.profileRevision}; ${manifest.authority.grantRef}.`,
  ].join("\n");
};

export const communicateRelease = async (
  input: Readonly<{
    manifest: ReleasePublicationManifest;
    phase: ReleaseCommunicationPhase;
    releaseUrl: string;
    changelogUrl: string;
    forumTopicId?: string;
    port: ReleaseCommunicationPort;
  }>,
): Promise<ReleaseCommunicationResult> => {
  assertReleaseCommunicationUrls(input.releaseUrl, input.changelogUrl);
  const body = renderReleaseCommunication(input);
  const marker = releaseCommunicationMarker(input.manifest.version, input.phase);
  let githubCommentsCreated = 0;
  for (const issue of input.manifest.sourceIssues) {
    // Sequential by design: GitHub secondary-rate limiting is stricter than
    // this bounded release list and idempotency is checked before each write.
    // eslint-disable-next-line no-await-in-loop
    if (await input.port.issueHasMarker(issue, marker)) continue;
    // eslint-disable-next-line no-await-in-loop
    await input.port.commentOnIssue(issue, body);
    githubCommentsCreated += 1;
  }

  let forumTopicId = input.forumTopicId;
  if (forumTopicId === undefined) {
    if (input.phase !== "candidate")
      throw new Error(`${input.phase} communication requires the candidate forum topic id`);
    const topic = await input.port.createForumTopic({
      forum: input.manifest.forumSlug,
      title: `OpenAgents Desktop ${input.manifest.version} candidate`,
      body,
      idempotencyKey: `release-${input.manifest.version}-candidate`,
    });
    forumTopicId = topic.topicId;
  } else {
    await input.port.replyToForumTopic({
      topicId: forumTopicId,
      body,
      idempotencyKey: `release-${input.manifest.version}-${input.phase}`,
    });
  }
  return { phase: input.phase, githubCommentsCreated, forumTopicId };
};

const findString = (value: unknown, keys: readonly string[]): string | null => {
  if (value === null || typeof value !== "object") return null;
  if (Array.isArray(value)) {
    for (const child of value) {
      const found = findString(child, keys);
      if (found !== null) return found;
    }
    return null;
  }
  const record = value as Record<string, unknown>;
  for (const key of keys) if (typeof record[key] === "string") return record[key];
  for (const child of Object.values(record)) {
    const found = findString(child, keys);
    if (found !== null) return found;
  }
  return null;
};

const forumCommand = (args: readonly string[]): unknown => {
  const output = execFileSync(
    process.execPath,
    ["apps/openagents.com/scripts/forum.mjs", ...args],
    { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 },
  );
  return JSON.parse(output) as unknown;
};

export const createReleaseCommunicationPort = (
  repo = "OpenAgentsInc/openagents",
): ReleaseCommunicationPort => ({
  issueHasMarker: async (issue, marker) => {
    const output = execFileSync(
      "gh",
      ["issue", "view", String(issue), "--repo", repo, "--comments", "--json", "comments"],
      { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
    );
    const parsed = JSON.parse(output) as { comments?: Array<{ body?: string }> };
    return parsed.comments?.some((comment) => comment.body?.includes(marker)) ?? false;
  },
  commentOnIssue: async (issue, body) => {
    execFileSync("gh", ["issue", "comment", String(issue), "--repo", repo, "--body-file", "-"], {
      input: body,
      stdio: ["pipe", "pipe", "pipe"],
    });
  },
  createForumTopic: async (input) => {
    const response = forumCommand([
      "create-topic",
      "--forum",
      input.forum,
      "--title",
      input.title,
      "--body",
      input.body,
      "--idempotency-key",
      input.idempotencyKey,
    ]);
    const topicId = findString(response, ["topicId", "topic_id", "id"]);
    if (topicId === null) throw new Error("Forum create-topic response has no topic id");
    return { topicId, url: findString(response, ["permalink", "url"]) ?? undefined };
  },
  replyToForumTopic: async (input) => {
    forumCommand([
      "reply",
      "--topic",
      input.topicId,
      "--body",
      input.body,
      "--idempotency-key",
      input.idempotencyKey,
    ]);
  },
});

const argValue = (args: readonly string[], flag: string): string | null => {
  const index = args.indexOf(flag);
  return index >= 0 ? (args[index + 1] ?? null) : null;
};

const main = async (): Promise<void> => {
  const args = process.argv.slice(2);
  const manifestPath = argValue(args, "--manifest");
  const phase = argValue(args, "--phase") as ReleaseCommunicationPhase | null;
  const releaseUrl = argValue(args, "--release-url");
  const changelogUrl = argValue(args, "--changelog-url") ?? "https://openagents.com/changelog";
  const forumTopicId = argValue(args, "--forum-topic") ?? undefined;
  if (
    manifestPath === null ||
    releaseUrl === null ||
    phase === null ||
    !(["candidate", "published", "rolled_back"] as const).includes(phase)
  ) {
    throw new Error(
      "usage: pnpm release:communicate -- --manifest <path> --phase <candidate|published|rolled_back> --release-url <url> [--forum-topic <id>] [--publish]",
    );
  }
  const rootDir = resolve(import.meta.dirname, "..");
  const resolvedManifestPath = resolveRepositoryFile(rootDir, manifestPath, "manifest");
  const manifest = decodeReleasePublicationManifest(
    JSON.parse(readFileSync(resolvedManifestPath, "utf8")),
  );
  validateReleasePublication(rootDir, manifest);
  assertReleaseCommunicationUrls(releaseUrl, changelogUrl);
  const body = renderReleaseCommunication({ manifest, phase, releaseUrl, changelogUrl });
  if (!args.includes("--publish")) {
    process.stdout.write(`${body}\n`);
    return;
  }
  const result = await communicateRelease({
    manifest,
    phase,
    releaseUrl,
    changelogUrl,
    ...(forumTopicId === undefined ? {} : { forumTopicId }),
    port: createReleaseCommunicationPort(),
  });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
};

if (import.meta.main) {
  main().catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
