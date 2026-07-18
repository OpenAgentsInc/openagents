import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vite-plus/test";

import {
  RELEASE_AUTHORITY_PROFILE_ID,
  RELEASE_PUBLICATION_SCHEMA,
  decodeReleasePublicationManifest,
  publishGitHubRelease,
  renderReleaseProvenance,
  validateReleasePublication,
  type GitHubReleasePort,
  type ReleasePublicationManifest,
  type RemoteRelease,
} from "./github-release.js";

const fixture = (): Readonly<{
  root: string;
  manifest: ReleasePublicationManifest;
  digest: string;
}> => {
  const root = mkdtempSync(join(tmpdir(), "github-release-"));
  mkdirSync(join(root, "release"));
  const bytes = Buffer.from("signed candidate bytes");
  const digest = createHash("sha256").update(bytes).digest("hex");
  writeFileSync(join(root, "release", "candidate.zip"), bytes);
  writeFileSync(join(root, "release", "notes.md"), "# Candidate\n\nA bounded RC.");
  return {
    root,
    digest,
    manifest: {
      schema: RELEASE_PUBLICATION_SCHEMA,
      publicationClass: "desktop_experimental_prerelease",
      version: "0.1.0-rc.99",
      channel: "rc",
      sourceRevision: "a".repeat(40),
      title: "OpenAgents Desktop 0.1.0-rc.99",
      notesPath: "release/notes.md",
      artifacts: [
        {
          path: "release/candidate.zip",
          name: "candidate.zip",
          sha256: digest,
          byteLength: bytes.length,
          target: "darwin-arm64",
          format: "experimental_zip",
          receiptRef: "receipt.fixture.candidate",
        },
      ],
      limitations: ["Fixture candidate only."],
      trigger: { kind: "tester_feedback", actor: "@fixture-tester", ref: "issue:#8995" },
      authority: {
        profileId: RELEASE_AUTHORITY_PROFILE_ID,
        profileRevision: 2,
        programRef: "program.full_auto_release",
        grantRef: "grant.autonomous_rc_release_and_communication",
        actorRole: "release_operator",
      },
      sourceIssues: [8995],
      requestedTesters: ["@fixture-tester"],
      forumSlug: "release-candidates",
    },
  };
};

const fakePort = (fixtureValue: ReturnType<typeof fixture>) => {
  let remote: RemoteRelease | null = null;
  const calls: string[] = [];
  const port: GitHubReleasePort = {
    get: async () => remote,
    createDraft: async (input) => {
      calls.push("create");
      remote = {
        tagName: input.tag,
        targetCommitish: input.target,
        isDraft: true,
        isPrerelease: true,
        url: `https://github.example/${input.tag}`,
        assets: [],
      };
    },
    upload: async () => {
      calls.push("upload");
      remote = {
        ...remote!,
        assets: [
          {
            name: "candidate.zip",
            size: Buffer.byteLength("signed candidate bytes"),
            digest: `sha256:${fixtureValue.digest}`,
          },
        ],
      };
    },
    publishPrerelease: async () => {
      calls.push("publish");
      remote = { ...remote!, isDraft: false };
    },
  };
  return { calls, port, getRemote: () => remote };
};

describe("GitHub release publisher", () => {
  test("validates local bytes and appends explicit trigger and authority provenance", () => {
    const value = fixture();
    const publication = validateReleasePublication(value.root, value.manifest);

    expect(publication.tag).toBe("openagents-desktop-v0.1.0-rc.99");
    expect(publication.notes).toContain("tester_feedback");
    expect(publication.notes).toContain("revision 2");
    expect(renderReleaseProvenance(value.manifest)).toContain(
      "experimental GitHub prerelease only",
    );
  });

  test("creates a draft, verifies server digests, then publishes", async () => {
    const value = fixture();
    const fake = fakePort(value);
    const result = await publishGitHubRelease(
      validateReleasePublication(value.root, value.manifest),
      fake.port,
    );

    expect(fake.calls).toEqual(["create", "upload", "publish"]);
    expect(result.outcome).toBe("published");
    expect(fake.getRemote()?.isDraft).toBe(false);
  });

  test("is idempotent when the same published bytes already exist", async () => {
    const value = fixture();
    const fake = fakePort(value);
    const publication = validateReleasePublication(value.root, value.manifest);
    await publishGitHubRelease(publication, fake.port);
    fake.calls.length = 0;

    const result = await publishGitHubRelease(publication, fake.port);

    expect(result.outcome).toBe("already_published");
    expect(fake.calls).toEqual([]);
  });

  test("refuses version reuse when remote bytes differ", async () => {
    const value = fixture();
    const fake = fakePort(value);
    const publication = validateReleasePublication(value.root, value.manifest);
    await publishGitHubRelease(publication, fake.port);
    const remote = fake.getRemote()!;
    const badPort: GitHubReleasePort = {
      ...fake.port,
      get: async () => ({
        ...remote,
        assets: [{ ...remote.assets[0]!, digest: `sha256:${"b".repeat(64)}` }],
      }),
    };

    await expect(publishGitHubRelease(publication, badPort)).rejects.toThrow(
      /version reuse is forbidden/,
    );
  });

  test("refuses a manifest that lies about local bytes", () => {
    const value = fixture();
    const broken: ReleasePublicationManifest = {
      ...value.manifest,
      artifacts: [{ ...value.manifest.artifacts[0]!, sha256: "b".repeat(64) }],
    };

    expect(() => validateReleasePublication(value.root, broken)).toThrow(
      /local artifact bytes do not match/,
    );
  });

  test("requires profile revision 2 for autonomous publication", () => {
    const value = fixture();
    const old: ReleasePublicationManifest = {
      ...value.manifest,
      authority: { ...value.manifest.authority, profileRevision: 1 },
    };

    expect(() => validateReleasePublication(value.root, old)).toThrow(/revision 2 or newer/);
  });

  test("refuses a partial matrix presented as a signed ReleaseSet", () => {
    const value = fixture();
    const partial: ReleasePublicationManifest = {
      ...value.manifest,
      publicationClass: "desktop_signed_release_set",
      limitations: [],
    };

    expect(() => validateReleasePublication(value.root, partial)).toThrow(/exactly 11 artifacts/);
  });

  test("strict decoder refuses unknown manifest fields", () => {
    const value = fixture();
    expect(() =>
      decodeReleasePublicationManifest({ ...value.manifest, publishAnyway: true }),
    ).toThrow(/shape is invalid/);
  });
});
