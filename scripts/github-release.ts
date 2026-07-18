#!/usr/bin/env node
/**
 * Idempotent GitHub prerelease publisher.
 *
 * GitHub is a public mirror and tester-discovery surface. It is never the
 * OpenAgents update-selection authority: signed ReleaseSet/feed verification
 * remains independent of this adapter.
 */
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { basename, isAbsolute, resolve, sep } from "node:path";

import {
  RELEASE_VERSION_PATTERN,
  releaseTargetKeys,
  type ReleaseChannel,
  type ReleaseTargetKey,
} from "./release.js";

export const RELEASE_PUBLICATION_SCHEMA = "openagents.release_publication.v1" as const;
export const RELEASE_AUTHORITY_PROFILE_ID = "openagents.owner-delegated-autonomy" as const;

export type ReleaseTriggerKind =
  | "owner_direction"
  | "agent_change"
  | "tester_feedback"
  | "release_incident";

export type ReleasePublicationClass =
  | "desktop_signed_release_set"
  | "desktop_experimental_prerelease";

export type ReleasePublicationArtifact = Readonly<{
  path: string;
  name: string;
  sha256: string;
  byteLength: number;
  target: ReleaseTargetKey | "release-metadata";
  format: string;
  receiptRef: string;
}>;

export type ReleasePublicationManifest = Readonly<{
  schema: typeof RELEASE_PUBLICATION_SCHEMA;
  publicationClass: ReleasePublicationClass;
  version: string;
  channel: ReleaseChannel;
  sourceRevision: string;
  title: string;
  notesPath: string;
  artifacts: readonly ReleasePublicationArtifact[];
  limitations: readonly string[];
  trigger: Readonly<{
    kind: ReleaseTriggerKind;
    actor: string;
    ref: string;
  }>;
  authority: Readonly<{
    profileId: typeof RELEASE_AUTHORITY_PROFILE_ID;
    profileRevision: number;
    programRef: string;
    grantRef: string;
    actorRole: "release_operator";
  }>;
  sourceIssues: readonly number[];
  requestedTesters: readonly string[];
  forumSlug: "release-candidates";
}>;

export type ValidatedPublication = Readonly<{
  manifest: ReleasePublicationManifest;
  tag: string;
  notes: string;
  artifactPaths: readonly string[];
}>;

export type RemoteReleaseAsset = Readonly<{
  name: string;
  size: number;
  digest: string | null;
}>;

export type RemoteRelease = Readonly<{
  tagName: string;
  targetCommitish: string;
  isDraft: boolean;
  isPrerelease: boolean;
  url: string;
  assets: readonly RemoteReleaseAsset[];
}>;

export interface GitHubReleasePort {
  get(tag: string): Promise<RemoteRelease | null>;
  createDraft(
    input: Readonly<{
      tag: string;
      title: string;
      target: string;
      notes: string;
    }>,
  ): Promise<void>;
  upload(tag: string, artifactPaths: readonly string[]): Promise<void>;
  publishPrerelease(tag: string): Promise<void>;
}

export type GitHubPublicationResult = Readonly<{
  releaseUrl: string;
  tag: string;
  outcome: "published" | "already_published";
  artifactCount: number;
}>;

const SHA256 = /^[0-9a-f]{64}$/;
const SOURCE_REVISION = /^[0-9a-f]{40}$/;
const PUBLIC_REF = /^[A-Za-z0-9@#][A-Za-z0-9@#._:/ -]{0,239}$/;
const ARTIFACT_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,159}$/;

const requiredFormatsByTarget: Readonly<Record<ReleaseTargetKey, readonly string[]>> = {
  "darwin-arm64": ["dmg", "zip"],
  "darwin-x64": ["dmg", "zip"],
  "win32-x64": ["nsis"],
  "linux-arm64": ["appimage", "deb", "rpm"],
  "linux-x64": ["appimage", "deb", "rpm"],
};

const expectedArtifactName = (
  version: string,
  channel: ReleaseChannel,
  target: ReleaseTargetKey,
  format: string,
): string => {
  const [platform, architecture] = target.split("-") as [
    "darwin" | "win32" | "linux",
    "arm64" | "x64",
  ];
  if (platform === "darwin")
    return `OpenAgents-${version}-${channel}-darwin-${architecture}.${format}`;
  if (platform === "win32")
    return `OpenAgents-${version}-${channel}-win32-${architecture}-setup.exe`;
  const extension = format === "appimage" ? "AppImage" : format;
  return `OpenAgents-${version}-${channel}-linux-${architecture}.${extension}`;
};

const assertBoundedText = (value: string, label: string, max = 240): void => {
  if (value.length === 0 || value.length > max || !PUBLIC_REF.test(value)) {
    throw new Error(`${label} is not a bounded public-safe reference`);
  }
};

export const resolveRepositoryFile = (rootDir: string, path: string, label: string): string => {
  if (isAbsolute(path) || path.includes("\0")) throw new Error(`${label} must be relative`);
  const absolute = resolve(rootDir, path);
  const rootPrefix = `${resolve(rootDir)}${sep}`;
  if (!absolute.startsWith(rootPrefix)) throw new Error(`${label} escapes the repository root`);
  const realRootPrefix = `${realpathSync(resolve(rootDir))}${sep}`;
  const real = realpathSync(absolute);
  if (!real.startsWith(realRootPrefix))
    throw new Error(`${label} resolves outside the repository root`);
  return real;
};

const exactKeys = (value: Record<string, unknown>, keys: readonly string[]): boolean => {
  const actual = Object.keys(value).toSorted();
  const expected = [...keys].toSorted();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

/** Strict, unknown-field-refusing decoder for the release authority boundary. */
export const decodeReleasePublicationManifest = (value: unknown): ReleasePublicationManifest => {
  const topKeys = [
    "artifacts",
    "authority",
    "channel",
    "forumSlug",
    "limitations",
    "notesPath",
    "publicationClass",
    "requestedTesters",
    "schema",
    "sourceIssues",
    "sourceRevision",
    "title",
    "trigger",
    "version",
  ] as const;
  if (!isRecord(value) || !exactKeys(value, topKeys))
    throw new Error("release publication manifest shape is invalid");
  if (
    value.schema !== RELEASE_PUBLICATION_SCHEMA ||
    (value.publicationClass !== "desktop_signed_release_set" &&
      value.publicationClass !== "desktop_experimental_prerelease") ||
    (value.channel !== "rc" && value.channel !== "stable") ||
    typeof value.version !== "string" ||
    typeof value.sourceRevision !== "string" ||
    typeof value.title !== "string" ||
    typeof value.notesPath !== "string" ||
    value.forumSlug !== "release-candidates" ||
    !Array.isArray(value.artifacts) ||
    !Array.isArray(value.limitations) ||
    !Array.isArray(value.sourceIssues) ||
    !Array.isArray(value.requestedTesters) ||
    !isRecord(value.trigger) ||
    !isRecord(value.authority)
  ) {
    throw new Error("release publication manifest field types are invalid");
  }
  if (!exactKeys(value.trigger, ["actor", "kind", "ref"]))
    throw new Error("release trigger shape is invalid");
  if (
    !exactKeys(value.authority, [
      "actorRole",
      "grantRef",
      "profileId",
      "profileRevision",
      "programRef",
    ])
  )
    throw new Error("release authority shape is invalid");
  const triggerKinds: readonly string[] = [
    "owner_direction",
    "agent_change",
    "tester_feedback",
    "release_incident",
  ];
  if (
    !triggerKinds.includes(String(value.trigger.kind)) ||
    typeof value.trigger.actor !== "string" ||
    typeof value.trigger.ref !== "string" ||
    value.authority.profileId !== RELEASE_AUTHORITY_PROFILE_ID ||
    !Number.isSafeInteger(value.authority.profileRevision) ||
    typeof value.authority.programRef !== "string" ||
    typeof value.authority.grantRef !== "string" ||
    value.authority.actorRole !== "release_operator"
  ) {
    throw new Error("release trigger or authority values are invalid");
  }
  for (const artifact of value.artifacts) {
    if (
      !isRecord(artifact) ||
      !exactKeys(artifact, [
        "byteLength",
        "format",
        "name",
        "path",
        "receiptRef",
        "sha256",
        "target",
      ]) ||
      typeof artifact.path !== "string" ||
      typeof artifact.name !== "string" ||
      typeof artifact.sha256 !== "string" ||
      !Number.isSafeInteger(artifact.byteLength) ||
      typeof artifact.target !== "string" ||
      typeof artifact.format !== "string" ||
      typeof artifact.receiptRef !== "string"
    ) {
      throw new Error("release artifact shape is invalid");
    }
  }
  return value as unknown as ReleasePublicationManifest;
};

export const renderReleaseProvenance = (manifest: ReleasePublicationManifest): string =>
  [
    "## Release provenance",
    "",
    `- Trigger: ${manifest.trigger.kind} — ${manifest.trigger.actor} (${manifest.trigger.ref})`,
    `- Published by: ${manifest.authority.actorRole}`,
    `- Authority: ${manifest.authority.profileId} revision ${manifest.authority.profileRevision}; ${manifest.authority.programRef}; ${manifest.authority.grantRef}`,
    `- Source: ${manifest.sourceRevision}`,
    manifest.publicationClass === "desktop_experimental_prerelease"
      ? "- Distribution: experimental GitHub prerelease only; this does not promote the signed Desktop update feed."
      : "- Distribution: non-authoritative GitHub mirror of the separately verified signed ReleaseSet.",
  ].join("\n");

export const validateReleasePublication = (
  rootDir: string,
  manifest: ReleasePublicationManifest,
): ValidatedPublication => {
  if (!RELEASE_VERSION_PATTERN.test(manifest.version))
    throw new Error("release version is invalid");
  if (!SOURCE_REVISION.test(manifest.sourceRevision)) throw new Error("source revision is invalid");
  if (manifest.channel === "rc" && !manifest.version.includes("-rc."))
    throw new Error("RC publication requires an RC version");
  if (manifest.channel === "stable" && manifest.version.includes("-rc."))
    throw new Error("stable publication rejects an RC version");
  if (manifest.publicationClass === "desktop_experimental_prerelease") {
    if (manifest.channel !== "rc") throw new Error("experimental publication is RC-only");
    if (manifest.limitations.length === 0)
      throw new Error("experimental publication requires honest limitations");
  }
  if (manifest.authority.profileRevision < 2)
    throw new Error(
      "autonomous release publication requires authority profile revision 2 or newer",
    );
  for (const [label, value] of [
    ["title", manifest.title],
    ["trigger actor", manifest.trigger.actor],
    ["trigger ref", manifest.trigger.ref],
    ["program ref", manifest.authority.programRef],
    ["grant ref", manifest.authority.grantRef],
  ] as const)
    assertBoundedText(value, label);
  if (manifest.sourceIssues.some((issue) => !Number.isSafeInteger(issue) || issue <= 0))
    throw new Error("source issue identifiers are invalid");
  if (manifest.sourceIssues.length > 20) throw new Error("source issue list is unbounded");
  if (new Set(manifest.sourceIssues).size !== manifest.sourceIssues.length)
    throw new Error("source issue identifiers must be unique");
  for (const limitation of manifest.limitations)
    assertBoundedText(limitation, "release limitation", 500);
  if (manifest.limitations.length > 20) throw new Error("release limitations are unbounded");
  if (manifest.requestedTesters.length > 20) throw new Error("requested tester list is unbounded");
  for (const tester of manifest.requestedTesters) {
    if (!/^@[A-Za-z0-9][A-Za-z0-9-]{0,38}$/.test(tester))
      throw new Error(`requested tester ${tester} is invalid`);
  }
  if (
    new Set(manifest.requestedTesters.map((tester) => tester.toLowerCase())).size !==
    manifest.requestedTesters.length
  )
    throw new Error("requested tester identifiers must be unique");
  const notesPath = resolveRepositoryFile(rootDir, manifest.notesPath, "notesPath");
  const notesSource = readFileSync(notesPath, "utf8").trim();
  if (notesSource.length === 0 || notesSource.length > 100_000)
    throw new Error("release notes are empty or unbounded");
  const seenNames = new Set<string>();
  const artifactPaths: string[] = [];
  if (manifest.artifacts.length > 20) throw new Error("release artifact list is unbounded");
  for (const artifact of manifest.artifacts) {
    if (!ARTIFACT_NAME.test(artifact.name) || basename(artifact.path) !== artifact.name)
      throw new Error(`artifact identity is invalid: ${artifact.name}`);
    if (!SHA256.test(artifact.sha256) || artifact.byteLength <= 0)
      throw new Error(`artifact digest or length is invalid: ${artifact.name}`);
    if (seenNames.has(artifact.name)) throw new Error(`duplicate artifact: ${artifact.name}`);
    seenNames.add(artifact.name);
    assertBoundedText(artifact.receiptRef, `receiptRef for ${artifact.name}`);
    const absolute = resolveRepositoryFile(rootDir, artifact.path, `artifact ${artifact.name}`);
    const bytes = readFileSync(absolute);
    const stat = statSync(absolute);
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (stat.size !== artifact.byteLength || digest !== artifact.sha256)
      throw new Error(`local artifact bytes do not match the manifest: ${artifact.name}`);
    artifactPaths.push(absolute);
  }
  if (manifest.artifacts.length === 0) throw new Error("release has no artifacts");

  if (manifest.publicationClass === "desktop_signed_release_set") {
    const expectedCount = Object.values(requiredFormatsByTarget).reduce(
      (count, formats) => count + formats.length,
      0,
    );
    if (manifest.artifacts.length !== expectedCount)
      throw new Error(`signed ReleaseSet requires exactly ${expectedCount} artifacts`);
    for (const target of releaseTargetKeys) {
      const observed = manifest.artifacts
        .filter((artifact) => artifact.target === target)
        .map((artifact) => artifact.format);
      const required = requiredFormatsByTarget[target];
      if (observed.join(",") !== required.join(","))
        throw new Error(`signed ReleaseSet artifact matrix is incomplete for ${target}`);
      for (const artifact of manifest.artifacts.filter((row) => row.target === target)) {
        if (
          artifact.name !==
          expectedArtifactName(manifest.version, manifest.channel, target, artifact.format)
        )
          throw new Error(`signed ReleaseSet artifact name is invalid: ${artifact.name}`);
      }
    }
  }

  return {
    manifest,
    tag: `openagents-desktop-v${manifest.version}`,
    notes: `${notesSource}\n\n${renderReleaseProvenance(manifest)}\n`,
    artifactPaths,
  };
};

const remoteArtifactsMatch = (
  remote: RemoteRelease,
  manifest: ReleasePublicationManifest,
): boolean => {
  if (remote.assets.length !== manifest.artifacts.length) return false;
  const byName = new Map(remote.assets.map((asset) => [asset.name, asset]));
  return manifest.artifacts.every((artifact) => {
    const remoteAsset = byName.get(artifact.name);
    return (
      remoteAsset !== undefined &&
      remoteAsset.size === artifact.byteLength &&
      remoteAsset.digest === `sha256:${artifact.sha256}`
    );
  });
};

export const publishGitHubRelease = async (
  publication: ValidatedPublication,
  port: GitHubReleasePort,
): Promise<GitHubPublicationResult> => {
  const { manifest, tag } = publication;
  let remote = await port.get(tag);
  if (remote !== null) {
    if (remote.targetCommitish !== manifest.sourceRevision)
      throw new Error(`existing release ${tag} targets a different source revision`);
    if (!remote.isPrerelease) throw new Error(`existing release ${tag} is not a prerelease`);
    if (remoteArtifactsMatch(remote, manifest) && !remote.isDraft) {
      return {
        releaseUrl: remote.url,
        tag,
        outcome: "already_published",
        artifactCount: manifest.artifacts.length,
      };
    }
    if (remote.assets.length > 0 && !remoteArtifactsMatch(remote, manifest))
      throw new Error(`existing release ${tag} has different assets; version reuse is forbidden`);
  } else {
    await port.createDraft({
      tag,
      title: manifest.title,
      target: manifest.sourceRevision,
      notes: publication.notes,
    });
  }
  remote = await port.get(tag);
  if (remote === null) throw new Error(`draft release ${tag} did not become observable`);
  if (remote.assets.length === 0) await port.upload(tag, publication.artifactPaths);
  remote = await port.get(tag);
  if (remote === null || !remoteArtifactsMatch(remote, manifest))
    throw new Error(`GitHub server digests do not match the local manifest for ${tag}`);
  if (remote.isDraft) await port.publishPrerelease(tag);
  remote = await port.get(tag);
  if (
    remote === null ||
    remote.isDraft ||
    !remote.isPrerelease ||
    !remoteArtifactsMatch(remote, manifest)
  )
    throw new Error(`published GitHub prerelease verification failed for ${tag}`);
  return {
    releaseUrl: remote.url,
    tag,
    outcome: "published",
    artifactCount: manifest.artifacts.length,
  };
};

const ghJson = (args: readonly string[]): unknown => {
  const output = execFileSync("gh", args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  return JSON.parse(output) as unknown;
};

export const createGhReleasePort = (repo = "OpenAgentsInc/openagents"): GitHubReleasePort => ({
  get: async (tag) => {
    try {
      const value = ghJson([
        "release",
        "view",
        tag,
        "--repo",
        repo,
        "--json",
        "tagName,targetCommitish,isDraft,isPrerelease,url,assets",
      ]);
      if (!isRecord(value) || !Array.isArray(value.assets))
        throw new Error("GitHub release response is malformed");
      return {
        tagName: String(value.tagName),
        targetCommitish: String(value.targetCommitish),
        isDraft: Boolean(value.isDraft),
        isPrerelease: Boolean(value.isPrerelease),
        url: String(value.url),
        assets: value.assets.map((asset) => {
          if (!isRecord(asset)) throw new Error("GitHub asset response is malformed");
          return {
            name: String(asset.name),
            size: Number(asset.size),
            digest: typeof asset.digest === "string" ? asset.digest : null,
          };
        }),
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message.includes("release not found") || message.includes("HTTP 404")) return null;
      throw error;
    }
  },
  createDraft: async (input) => {
    execFileSync(
      "gh",
      [
        "release",
        "create",
        input.tag,
        "--repo",
        repo,
        "--target",
        input.target,
        "--title",
        input.title,
        "--notes-file",
        "-",
        "--draft",
        "--prerelease",
      ],
      { input: input.notes, stdio: ["pipe", "pipe", "pipe"] },
    );
  },
  upload: async (tag, artifactPaths) => {
    execFileSync("gh", ["release", "upload", tag, "--repo", repo, ...artifactPaths], {
      stdio: "inherit",
    });
  },
  publishPrerelease: async (tag) => {
    execFileSync("gh", ["release", "edit", tag, "--repo", repo, "--draft=false", "--prerelease"], {
      stdio: "inherit",
    });
  },
});

const argValue = (args: readonly string[], flag: string): string | null => {
  const index = args.indexOf(flag);
  return index >= 0 ? (args[index + 1] ?? null) : null;
};

const main = async (): Promise<void> => {
  const args = process.argv.slice(2);
  const manifestArg = argValue(args, "--manifest");
  if (manifestArg === null)
    throw new Error("usage: pnpm release:github -- --manifest <path> [--publish]");
  const rootDir = resolve(import.meta.dirname, "..");
  const manifestPath = resolveRepositoryFile(rootDir, manifestArg, "manifest");
  const manifest = decodeReleasePublicationManifest(JSON.parse(readFileSync(manifestPath, "utf8")));
  const publication = validateReleasePublication(rootDir, manifest);
  if (!args.includes("--publish")) {
    process.stdout.write(
      `${JSON.stringify({ tag: publication.tag, artifactCount: publication.artifactPaths.length, dryRun: true }, null, 2)}\n`,
    );
    return;
  }
  const result = await publishGitHubRelease(publication, createGhReleasePort());
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
};

if (import.meta.main) {
  main().catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
