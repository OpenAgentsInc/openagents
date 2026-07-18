import { describe, expect, test } from "vite-plus/test";

import {
  RELEASE_AUTHORITY_PROFILE_ID,
  RELEASE_PUBLICATION_SCHEMA,
  type ReleasePublicationManifest,
} from "./github-release.js";
import {
  ingestReleaseFeedback,
  parseTesterFeedback,
  releaseFeedbackMarker,
  type ReleaseFeedbackPort,
  type ReleaseIssueComment,
} from "./release-feedback.js";
import { releaseCommunicationMarker } from "./release-communications.js";

const manifest: ReleasePublicationManifest = {
  schema: RELEASE_PUBLICATION_SCHEMA,
  publicationClass: "desktop_experimental_prerelease",
  version: "0.1.0-rc.99",
  channel: "rc",
  sourceRevision: "a".repeat(40),
  title: "Fixture",
  notesPath: "notes.md",
  artifacts: [],
  limitations: ["Fixture"],
  trigger: { kind: "tester_feedback", actor: "@lathe-agent-oa", ref: "issue:#8995" },
  authority: {
    profileId: RELEASE_AUTHORITY_PROFILE_ID,
    profileRevision: 2,
    programRef: "program.full_auto_release",
    grantRef: "grant.autonomous_rc_release_and_communication",
    actorRole: "release_operator",
  },
  sourceIssues: [8995],
  requestedTesters: ["@lathe-agent-oa"],
  forumSlug: "release-candidates",
};

const candidateComment: ReleaseIssueComment = {
  id: "candidate",
  author: "AtlantisPleb",
  body: releaseCommunicationMarker(manifest.version, "candidate"),
  url: "https://github.example/candidate",
  createdAt: "2026-07-18T00:00:00Z",
};

describe("release feedback intake", () => {
  test("parses only the bounded fields after the release-feedback route is selected", () => {
    expect(
      parseTesterFeedback("Result: BLOCKED\nSeverity: P0\nObserved: Codex chat does not start."),
    ).toEqual({ result: "blocked", severity: "P0", observed: "Codex chat does not start." });
    expect(parseTesterFeedback("It still asks for Keychain.").result).toBe("unstructured");
    expect(parseTesterFeedback("Result: PASS\nObserved: Works.").result).toBe("pass");
  });

  test("turns blocked tester feedback into a linked P0 issue and source acknowledgement", async () => {
    const comments: ReleaseIssueComment[] = [
      candidateComment,
      {
        id: "feedback-1",
        author: "lathe-agent-oa",
        body: "Result: BLOCKED\nSeverity: P0\nObserved: Signed app cannot start Codex.",
        url: "https://github.example/feedback-1",
        createdAt: "2026-07-18T00:10:00Z",
      },
    ];
    const calls: string[] = [];
    const port: ReleaseFeedbackPort = {
      comments: async () => comments,
      findIssueByMarker: async () => null,
      createIssue: async (input) => {
        calls.push(
          `create:${input.labels.join(",")}:${input.body.includes(releaseFeedbackMarker("feedback-1"))}`,
        );
        return { number: 9001, url: "https://github.example/issues/9001" };
      },
      commentOnIssue: async (_issue, body) => {
        calls.push(`comment:${body.includes("#9001")}`);
      },
    };

    const result = await ingestReleaseFeedback({
      manifest,
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      port,
    });

    expect(result).toEqual({
      inspected: 1,
      passesAcknowledged: 0,
      followupIssuesCreated: 1,
      alreadyIngested: 0,
    });
    expect(calls).toEqual(["create:area:release,area:desktop,priority:P0:true", "comment:true"]);
  });

  test("records PASS without manufacturing a bug issue", async () => {
    const comments: ReleaseIssueComment[] = [
      candidateComment,
      {
        id: "feedback-pass",
        author: "lathe-agent-oa",
        body: "Result: PASS\nSeverity: P2\nObserved: Signed candidate works.",
        url: "https://github.example/feedback-pass",
        createdAt: "2026-07-18T00:10:00Z",
      },
    ];
    let created = false;
    let acknowledgement = "";
    const port: ReleaseFeedbackPort = {
      comments: async () => comments,
      findIssueByMarker: async () => null,
      createIssue: async () => {
        created = true;
        return { number: 1, url: "unused" };
      },
      commentOnIssue: async (_issue, body) => {
        acknowledgement = body;
      },
    };

    const result = await ingestReleaseFeedback({
      manifest,
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      port,
    });

    expect(created).toBe(false);
    expect(result.passesAcknowledged).toBe(1);
    expect(acknowledgement).toContain(releaseFeedbackMarker("feedback-pass"));
  });

  test("does not duplicate an already ingested feedback comment", async () => {
    const feedback: ReleaseIssueComment = {
      id: "feedback-existing",
      author: "lathe-agent-oa",
      body: "Result: BLOCKED\nObserved: Existing.",
      url: "https://github.example/existing",
      createdAt: "2026-07-18T00:10:00Z",
    };
    const port: ReleaseFeedbackPort = {
      comments: async () => [candidateComment, feedback],
      findIssueByMarker: async () => ({ number: 9000, url: "https://github.example/9000" }),
      createIssue: async () => {
        throw new Error("must not create");
      },
      commentOnIssue: async () => {
        throw new Error("must not comment");
      },
    };

    const result = await ingestReleaseFeedback({
      manifest,
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      port,
    });

    expect(result.alreadyIngested).toBe(1);
  });

  test("ingests a blocked reply from the release-candidates Forum topic", async () => {
    const forumCandidate: ReleaseIssueComment = {
      ...candidateComment,
      id: "forum-candidate",
      url: "https://openagents.com/forum/t/fixture#candidate",
    };
    const forumFeedback: ReleaseIssueComment = {
      id: "forum-feedback",
      author: "lathe-agent-oa",
      body: "Result: BLOCKED\nSeverity: P1\nObserved: Claude handoff stalled.",
      url: "https://openagents.com/forum/t/fixture#feedback",
      createdAt: "2026-07-18T00:15:00Z",
    };
    const calls: string[] = [];
    const port: ReleaseFeedbackPort = {
      comments: async () => [],
      forumComments: async () => [forumCandidate, forumFeedback],
      findIssueByMarker: async () => null,
      createIssue: async (input) => {
        calls.push(`create:${input.labels.join(",")}:${input.body.includes(forumFeedback.url)}`);
        return { number: 9002, url: "https://github.example/issues/9002" };
      },
      commentOnIssue: async () => {
        throw new Error("Forum feedback must be acknowledged on Forum");
      },
      replyToForumTopic: async (_topic, body, idempotencyKey) => {
        calls.push(`reply:${body.includes("#9002")}:${idempotencyKey}`);
      },
    };

    const result = await ingestReleaseFeedback({
      manifest,
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      forumTopicId: "fixture",
      port,
    });

    expect(result).toEqual({
      inspected: 1,
      passesAcknowledged: 0,
      followupIssuesCreated: 1,
      alreadyIngested: 0,
    });
    expect(calls).toEqual([
      "create:area:release,area:desktop,priority:P1-parallel:true",
      "reply:true:release-feedback-forum-feedback",
    ]);
  });
});
