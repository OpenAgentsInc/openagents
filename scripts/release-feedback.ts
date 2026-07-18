#!/usr/bin/env node
/**
 * Release tester-feedback intake.
 *
 * Candidate communication chooses the release-feedback route first. Parsing
 * below is therefore limited to bounded fields (PASS/BLOCKED, severity), as
 * required by the workspace semantic-routing invariant. Unstructured replies
 * are not guessed at: they become an explicit triage issue.
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  decodeReleasePublicationManifest,
  resolveRepositoryFile,
  validateReleasePublication,
  type ReleasePublicationManifest,
} from "./github-release.js";
import {
  assertOpenAgentsReleaseUrl,
  releaseCommunicationMarker,
} from "./release-communications.js";

export type ParsedTesterFeedback = Readonly<{
  result: "pass" | "blocked" | "unstructured";
  severity: "P0" | "P1" | "P2" | "unclassified";
  observed: string;
}>;

export type ReleaseIssueComment = Readonly<{
  id: string;
  author: string;
  body: string;
  url: string;
  createdAt: string;
}>;

export interface ReleaseFeedbackPort {
  comments(issue: number): Promise<readonly ReleaseIssueComment[]>;
  forumComments?(topicId: string): Promise<readonly ReleaseIssueComment[]>;
  findIssueByMarker(marker: string): Promise<Readonly<{ number: number; url: string }> | null>;
  createIssue(
    input: Readonly<{
      title: string;
      body: string;
      labels: readonly string[];
    }>,
  ): Promise<Readonly<{ number: number; url: string }>>;
  commentOnIssue(issue: number, body: string): Promise<void>;
  replyToForumTopic?(topicId: string, body: string, idempotencyKey: string): Promise<void>;
}

export type ReleaseFeedbackIntakeResult = Readonly<{
  inspected: number;
  passesAcknowledged: number;
  followupIssuesCreated: number;
  alreadyIngested: number;
}>;

const field = (body: string, name: string): string | null =>
  new RegExp(`^${name}:\\s*(.+?)\\s*$`, "im").exec(body)?.[1]?.trim() ?? null;

export const parseTesterFeedback = (body: string): ParsedTesterFeedback => {
  const resultField = field(body, "Result")?.toUpperCase();
  const severityField = field(body, "Severity")?.toUpperCase();
  const observed = (field(body, "Observed") ?? body.trim()).replace(/\s+/g, " ").slice(0, 500);
  return {
    result:
      resultField === "PASS" ? "pass" : resultField === "BLOCKED" ? "blocked" : "unstructured",
    severity:
      severityField === "P0" || severityField === "P1" || severityField === "P2"
        ? severityField
        : "unclassified",
    observed,
  };
};

export const releaseFeedbackMarker = (commentId: string): string =>
  `<!-- openagents-release-feedback:v1:${commentId} -->`;

const normalizedTester = (value: string): string => value.replace(/^@/, "").toLowerCase();

/**
 * A tester identity and a later timestamp do not bind a comment to a release.
 * The generated candidate request supplies this exact field so both structured
 * and unstructured replies can be correlated without guessing from prose.
 */
const isFeedbackForCandidate = (body: string, version: string): boolean =>
  field(body, "Candidate-Version") === version;

const labelsFor = (feedback: ParsedTesterFeedback): readonly string[] => {
  const labels = ["area:release", "area:desktop"];
  if (feedback.severity === "P0") labels.push("priority:P0");
  if (feedback.severity === "P1") labels.push("priority:P1-parallel");
  return labels;
};

const feedbackIssueTitle = (
  manifest: ReleasePublicationManifest,
  comment: ReleaseIssueComment,
  feedback: ParsedTesterFeedback,
): string => {
  const summary = feedback.observed.replace(/[`#<>]/g, "").slice(0, 100) || "unstructured result";
  return `[${manifest.version}] Candidate feedback from @${comment.author}: ${summary}`;
};

const feedbackIssueBody = (
  input: Readonly<{
    manifest: ReleasePublicationManifest;
    sourceIssue: number;
    releaseUrl: string;
    comment: ReleaseIssueComment;
    feedback: ParsedTesterFeedback;
  }>,
): string => {
  const { manifest, comment, feedback } = input;
  return [
    releaseFeedbackMarker(comment.id),
    `# Release feedback — OpenAgents Desktop ${manifest.version}`,
    "",
    `- source issue: #${input.sourceIssue}`,
    `- source comment: ${comment.url}`,
    `- release: ${input.releaseUrl}`,
    `- tester: @${comment.author}`,
    `- parsed result: ${feedback.result}`,
    `- parsed severity: ${feedback.severity}`,
    `- trigger: ${manifest.trigger.kind} — ${manifest.trigger.ref}`,
    `- intake authority: ${manifest.authority.profileId} revision ${manifest.authority.profileRevision}; ${manifest.authority.grantRef}`,
    "",
    "## Tester report",
    "",
    comment.body.trim().slice(0, 3_000),
    "",
    "## Automation disposition",
    "",
    feedback.result === "unstructured"
      ? "The reply did not use the bounded PASS/BLOCKED fields, so no semantic severity was guessed. Full Auto should reproduce and triage it."
      : "Full Auto should reproduce this blocked candidate report, patch it, run the affected release gates, and publish a strictly newer candidate.",
  ].join("\n");
};

export const ingestReleaseFeedback = async (
  input: Readonly<{
    manifest: ReleasePublicationManifest;
    releaseUrl: string;
    forumTopicId?: string;
    port: ReleaseFeedbackPort;
  }>,
): Promise<ReleaseFeedbackIntakeResult> => {
  assertOpenAgentsReleaseUrl(input.releaseUrl);
  const requested = new Set(input.manifest.requestedTesters.map(normalizedTester));
  let inspected = 0;
  let passesAcknowledged = 0;
  let followupIssuesCreated = 0;
  let alreadyIngested = 0;

  for (const issue of input.manifest.sourceIssues) {
    // eslint-disable-next-line no-await-in-loop -- bounded API sequence with per-comment idempotency.
    const comments = await input.port.comments(issue);
    const candidateIndex = comments.findIndex((comment) =>
      comment.body.includes(releaseCommunicationMarker(input.manifest.version, "candidate")),
    );
    if (candidateIndex < 0) continue;
    const feedbackComments = comments
      .slice(candidateIndex + 1)
      .filter(
        (comment) =>
          requested.has(normalizedTester(comment.author)) &&
          isFeedbackForCandidate(comment.body, input.manifest.version),
      );
    for (const comment of feedbackComments) {
      inspected += 1;
      const marker = releaseFeedbackMarker(comment.id);
      // The marker can live in either an acknowledgement comment or a created issue.
      // eslint-disable-next-line no-await-in-loop
      const existing = await input.port.findIssueByMarker(marker);
      if (existing !== null || comments.some((row) => row.body.includes(marker))) {
        alreadyIngested += 1;
        continue;
      }
      const feedback = parseTesterFeedback(comment.body);
      if (feedback.result === "pass") {
        // eslint-disable-next-line no-await-in-loop
        await input.port.commentOnIssue(
          issue,
          [
            marker,
            `Candidate PASS recorded from @${comment.author} for ${input.manifest.version}.`,
            `Source: ${comment.url}`,
            `Authority: ${input.manifest.authority.profileId} revision ${input.manifest.authority.profileRevision}; ${input.manifest.authority.grantRef}.`,
          ].join("\n\n"),
        );
        passesAcknowledged += 1;
        continue;
      }
      // eslint-disable-next-line no-await-in-loop
      const created = await input.port.createIssue({
        title: feedbackIssueTitle(input.manifest, comment, feedback),
        body: feedbackIssueBody({
          manifest: input.manifest,
          sourceIssue: issue,
          releaseUrl: input.releaseUrl,
          comment,
          feedback,
        }),
        labels: labelsFor(feedback),
      });
      // eslint-disable-next-line no-await-in-loop
      await input.port.commentOnIssue(
        issue,
        [
          marker,
          `Candidate feedback from @${comment.author} was ingested as #${created.number}: ${created.url}`,
          "Full Auto may claim the follow-up without a separate owner handoff.",
        ].join("\n\n"),
      );
      followupIssuesCreated += 1;
    }
  }

  if (input.forumTopicId !== undefined) {
    if (input.port.forumComments === undefined || input.port.replyToForumTopic === undefined)
      throw new Error("Forum feedback intake requires Forum comment and reply ports");
    const comments = await input.port.forumComments(input.forumTopicId);
    const candidateIndex = comments.findIndex((comment) =>
      comment.body.includes(releaseCommunicationMarker(input.manifest.version, "candidate")),
    );
    const feedbackComments =
      candidateIndex < 0
        ? []
        : comments
            .slice(candidateIndex + 1)
            .filter(
              (comment) =>
                requested.has(normalizedTester(comment.author)) &&
                isFeedbackForCandidate(comment.body, input.manifest.version),
            );
    for (const comment of feedbackComments) {
      inspected += 1;
      const marker = releaseFeedbackMarker(comment.id);
      // eslint-disable-next-line no-await-in-loop
      const existing = await input.port.findIssueByMarker(marker);
      if (existing !== null || comments.some((row) => row.body.includes(marker))) {
        alreadyIngested += 1;
        continue;
      }
      const feedback = parseTesterFeedback(comment.body);
      if (feedback.result === "pass") {
        // eslint-disable-next-line no-await-in-loop
        await input.port.replyToForumTopic(
          input.forumTopicId,
          [
            marker,
            `Candidate PASS recorded from @${comment.author} for ${input.manifest.version}.`,
            `Source: ${comment.url}`,
            `Authority: ${input.manifest.authority.profileId} revision ${input.manifest.authority.profileRevision}; ${input.manifest.authority.grantRef}.`,
          ].join("\n\n"),
          `release-feedback-${comment.id}`,
        );
        passesAcknowledged += 1;
        continue;
      }
      const sourceIssue = input.manifest.sourceIssues[0];
      if (sourceIssue === undefined)
        throw new Error("Forum feedback intake requires one linked source issue");
      // eslint-disable-next-line no-await-in-loop
      const created = await input.port.createIssue({
        title: feedbackIssueTitle(input.manifest, comment, feedback),
        body: feedbackIssueBody({
          manifest: input.manifest,
          sourceIssue,
          releaseUrl: input.releaseUrl,
          comment,
          feedback,
        }),
        labels: labelsFor(feedback),
      });
      // eslint-disable-next-line no-await-in-loop
      await input.port.replyToForumTopic(
        input.forumTopicId,
        [
          marker,
          `Candidate feedback from @${comment.author} was ingested as #${created.number}: ${created.url}`,
          "Full Auto may claim the follow-up without a separate owner handoff.",
        ].join("\n\n"),
        `release-feedback-${comment.id}`,
      );
      followupIssuesCreated += 1;
    }
  }
  return { inspected, passesAcknowledged, followupIssuesCreated, alreadyIngested };
};

type GhComment = Readonly<{
  id?: string;
  body?: string;
  url?: string;
  createdAt?: string;
  author?: Readonly<{ login?: string }>;
}>;

type ForumTopicResponse = Readonly<{
  posts?: ReadonlyArray<
    Readonly<{
      postId?: string;
      bodyText?: string;
      permalink?: string;
      createdAt?: string;
      author?: Readonly<{ slug?: string }>;
      authorProfile?: Readonly<{ slug?: string }>;
    }>
  >;
}>;

export const createReleaseFeedbackPort = (
  repo = "OpenAgentsInc/openagents",
): ReleaseFeedbackPort => ({
  comments: async (issue) => {
    const output = execFileSync(
      "gh",
      ["issue", "view", String(issue), "--repo", repo, "--comments", "--json", "comments"],
      { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
    );
    const parsed = JSON.parse(output) as { comments?: GhComment[] };
    return (parsed.comments ?? []).map((comment, index) => ({
      id: comment.id ?? `issue-${issue}-comment-${index}`,
      author: comment.author?.login ?? "unknown",
      body: comment.body ?? "",
      url: comment.url ?? `https://github.com/${repo}/issues/${issue}`,
      createdAt: comment.createdAt ?? "1970-01-01T00:00:00Z",
    }));
  },
  forumComments: async (topicId) => {
    const output = execFileSync(
      process.execPath,
      ["apps/openagents.com/scripts/forum.mjs", "topic", "--topic", topicId],
      { encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
    );
    const parsed = JSON.parse(output) as ForumTopicResponse;
    return (parsed.posts ?? []).map((post, index) => ({
      id: post.postId ?? `forum-${topicId}-post-${index}`,
      author: post.author?.slug ?? post.authorProfile?.slug ?? "unknown",
      body: post.bodyText ?? "",
      url: post.permalink ?? `https://openagents.com/forum/t/${topicId}`,
      createdAt: post.createdAt ?? "1970-01-01T00:00:00Z",
    }));
  },
  findIssueByMarker: async (marker) => {
    const output = execFileSync(
      "gh",
      [
        "search",
        "issues",
        marker,
        "--repo",
        repo,
        "--match",
        "body",
        "--limit",
        "1",
        "--json",
        "number,url",
      ],
      { encoding: "utf8", maxBuffer: 2 * 1024 * 1024 },
    );
    const rows = JSON.parse(output) as Array<{ number?: number; url?: string }>;
    const row = rows[0];
    return row?.number === undefined || row.url === undefined
      ? null
      : { number: row.number, url: row.url };
  },
  createIssue: async (input) => {
    const args = ["issue", "create", "--repo", repo, "--title", input.title, "--body-file", "-"];
    for (const label of input.labels) args.push("--label", label);
    const url = execFileSync("gh", args, {
      input: input.body,
      encoding: "utf8",
      maxBuffer: 2 * 1024 * 1024,
    }).trim();
    const number = Number(/\/issues\/(\d+)$/.exec(url)?.[1]);
    if (!Number.isSafeInteger(number)) throw new Error("GitHub issue create returned no issue id");
    return { number, url };
  },
  commentOnIssue: async (issue, body) => {
    execFileSync("gh", ["issue", "comment", String(issue), "--repo", repo, "--body-file", "-"], {
      input: body,
      stdio: ["pipe", "pipe", "pipe"],
    });
  },
  replyToForumTopic: async (topicId, body, idempotencyKey) => {
    execFileSync(
      process.execPath,
      [
        "apps/openagents.com/scripts/forum.mjs",
        "reply",
        "--topic",
        topicId,
        "--body",
        body,
        "--idempotency-key",
        idempotencyKey,
      ],
      { stdio: ["ignore", "pipe", "pipe"] },
    );
  },
});

const argValue = (args: readonly string[], flag: string): string | null => {
  const index = args.indexOf(flag);
  return index >= 0 ? (args[index + 1] ?? null) : null;
};

const main = async (): Promise<void> => {
  const args = process.argv.slice(2);
  const manifestPath = argValue(args, "--manifest");
  const releaseUrl = argValue(args, "--release-url");
  const forumTopicId = argValue(args, "--forum-topic") ?? undefined;
  if (manifestPath === null || releaseUrl === null)
    throw new Error(
      "usage: pnpm release:feedback -- --manifest <path> --release-url <url> [--forum-topic <id>] --publish",
    );
  const rootDir = resolve(import.meta.dirname, "..");
  const resolvedManifestPath = resolveRepositoryFile(rootDir, manifestPath, "manifest");
  const manifest = decodeReleasePublicationManifest(
    JSON.parse(readFileSync(resolvedManifestPath, "utf8")),
  );
  validateReleasePublication(rootDir, manifest);
  assertOpenAgentsReleaseUrl(releaseUrl);
  if (!args.includes("--publish")) {
    process.stdout.write(
      `${JSON.stringify({ dryRun: true, issues: manifest.sourceIssues, testers: manifest.requestedTesters, forumTopicId }, null, 2)}\n`,
    );
    return;
  }
  const result = await ingestReleaseFeedback({
    manifest,
    releaseUrl,
    ...(forumTopicId === undefined ? {} : { forumTopicId }),
    port: createReleaseFeedbackPort(),
  });
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
};

if (import.meta.main) {
  main().catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
