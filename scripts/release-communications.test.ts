import { describe, expect, test } from "vite-plus/test";

import {
  RELEASE_AUTHORITY_PROFILE_ID,
  RELEASE_PUBLICATION_SCHEMA,
  type ReleasePublicationManifest,
} from "./github-release.js";
import {
  communicateRelease,
  releaseCommunicationMarker,
  renderReleaseCommunication,
  type ReleaseCommunicationPort,
} from "./release-communications.js";

const manifest: ReleasePublicationManifest = {
  schema: RELEASE_PUBLICATION_SCHEMA,
  publicationClass: "desktop_experimental_prerelease",
  version: "0.1.0-rc.99",
  channel: "rc",
  sourceRevision: "a".repeat(40),
  title: "OpenAgents Desktop 0.1.0-rc.99",
  notesPath: "notes.md",
  artifacts: [],
  limitations: ["Fixture"],
  trigger: { kind: "tester_feedback", actor: "@fixture-tester", ref: "issue:#8995" },
  authority: {
    profileId: RELEASE_AUTHORITY_PROFILE_ID,
    profileRevision: 2,
    programRef: "program.full_auto_release",
    grantRef: "grant.autonomous_rc_release_and_communication",
    actorRole: "release_operator",
  },
  sourceIssues: [8995, 8993],
  requestedTesters: ["@fixture-tester"],
  forumSlug: "release-candidates",
};

describe("release communications", () => {
  test("candidate copy asks the named tester for a machine-readable result", () => {
    const body = renderReleaseCommunication({
      manifest,
      phase: "candidate",
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      changelogUrl: "https://openagents.com/changelog",
    });

    expect(body).toContain("@fixture-tester");
    expect(body).toContain(`Candidate-Version: ${manifest.version}`);
    expect(body).toContain("Result: PASS | BLOCKED");
    expect(body).toContain("tester_feedback");
    expect(body).toContain("revision 2");
    expect(body).toContain(releaseCommunicationMarker(manifest.version, "candidate"));
  });

  test("posts once per source issue and creates one Forum topic", async () => {
    const calls: string[] = [];
    const seen = new Set<number>();
    const port: ReleaseCommunicationPort = {
      issueHasMarker: async (issue) => seen.has(issue),
      commentOnIssue: async (issue) => {
        calls.push(`issue:${issue}`);
        seen.add(issue);
      },
      createForumTopic: async (input) => {
        calls.push(`forum:${input.forum}`);
        return { topicId: "topic.fixture" };
      },
      replyToForumTopic: async () => {
        calls.push("reply");
      },
    };

    const first = await communicateRelease({
      manifest,
      phase: "candidate",
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      changelogUrl: "https://openagents.com/changelog",
      port,
    });
    expect(first).toEqual({
      phase: "candidate",
      githubCommentsCreated: 2,
      forumTopicId: "topic.fixture",
    });
    expect(calls).toEqual(["issue:8995", "issue:8993", "forum:release-candidates"]);
  });

  test("published updates reply to the existing Forum topic", async () => {
    const calls: string[] = [];
    const port: ReleaseCommunicationPort = {
      issueHasMarker: async () => true,
      commentOnIssue: async () => undefined,
      createForumTopic: async () => ({ topicId: "unexpected" }),
      replyToForumTopic: async (input) => {
        calls.push(`${input.topicId}:${input.idempotencyKey}`);
      },
    };

    const result = await communicateRelease({
      manifest,
      phase: "published",
      releaseUrl:
        "https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.99",
      changelogUrl: "https://openagents.com/changelog",
      forumTopicId: "topic.fixture",
      port,
    });

    expect(result.githubCommentsCreated).toBe(0);
    expect(calls).toEqual(["topic.fixture:release-0.1.0-rc.99-published"]);
  });
});
