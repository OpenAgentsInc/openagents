import { createHash, generateKeyPairSync } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";
import {
  RELEASE_SET_SCHEMA_ID,
  V1_MIGRATION_END,
  canonicalJson,
  canonicalizeReleaseSet,
  decodeReleaseSelection,
  decodeReleaseSet,
  finalizeReleaseSet,
  minimumOsByTarget,
  preferredFormatByTarget,
  releaseTargetKeys,
  requiredFormatsByTarget,
  selectReleaseArtifact,
  verifyReleaseSetArtifact,
  verifySignedReleaseSet,
  type ReleaseFormat,
  type ReleaseSet,
  type ReleaseTargetKey,
} from "../src/release-set-contract.ts";
import type { PinnedReleaseKey, UpdateManifest } from "../src/update-contract.ts";
import type { ReleaseSigningKey } from "../src/release-publish.ts";

// Ephemeral fixture keypair only. Production private material is never read.
const fixturePair = generateKeyPairSync("ed25519");
const fixturePrivate = fixturePair.privateKey.export({ format: "jwk" }) as { d?: string };
const fixturePublic = fixturePair.publicKey.export({ format: "jwk" }) as { x?: string };
const fixtureKey: ReleaseSigningKey = { d: fixturePrivate.d ?? "", kid: "fixture-release-set-v2" };
const fixturePin: PinnedReleaseKey = {
  alg: "ed25519",
  kid: fixtureKey.kid,
  x: fixturePublic.x ?? "",
};

const version = "2.4.0-rc.3";
const channel = "rc" as const;
const sourceRevision = "a".repeat(40);
const publishedAt = "2026-07-16T16:00:00Z";
const signingPolicyId = "desktop-release-policy-v2";
const bytesByIdentity = new Map<string, Uint8Array>();

const artifactName = (target: ReleaseTargetKey, format: ReleaseFormat): string => {
  const [platform, arch] = target.split("-");
  if (platform === "darwin") return `OpenAgents-${version}-${channel}-darwin-${arch}.${format}`;
  if (platform === "win32") return `OpenAgents-${version}-${channel}-win32-${arch}-setup.exe`;
  return `OpenAgents-${version}-${channel}-linux-${arch}.${format === "appimage" ? "AppImage" : format}`;
};

const targetRows = releaseTargetKeys.map((target) => ({
  target,
  minimumOs: minimumOsByTarget[target],
  preferredFormat: preferredFormatByTarget[target],
  artifacts: requiredFormatsByTarget[target].map((format) => {
    const name = artifactName(target, format);
    const objectIdentity = `desktop/${channel}/${version}/${target}/${name}`;
    const bytes = new TextEncoder().encode(`fixture artifact:${target}:${format}`);
    bytesByIdentity.set(objectIdentity, bytes);
    return {
      target,
      format,
      version,
      sourceRevision,
      name,
      url: `https://storage.googleapis.com/openagents-fixtures/${objectIdentity}`,
      objectIdentity,
      sha256: createHash("sha256").update(bytes).digest("hex"),
      byteLength: bytes.byteLength,
      componentLedgerSha256: createHash("sha256").update(`ledger:${target}`).digest("hex"),
      componentLedgerRef: `receipt/ledger/${target}`,
      buildReceiptRef: `receipt/build/${target}/${format}`,
      signingPolicyId,
    };
  }),
}));

const validReleaseSet: ReleaseSet = {
  schema: RELEASE_SET_SCHEMA_ID,
  schemaVersion: 2,
  app: "openagents-desktop",
  channel,
  version,
  sourceRevision,
  publishedAt,
  signingPolicy: { id: signingPolicyId, algorithm: "ed25519", keyId: fixtureKey.kid },
  releaseNotes: {
    summary: "Cross-platform fixture release.",
    human: { ref: "docs/changelog/human/fixture.md", sha256: "b".repeat(64) },
    agent: { ref: "docs/changelog/agent/fixture.md", sha256: "c".repeat(64) },
  },
  targets: targetRows,
};

// Mutation fixtures intentionally need writable projections of readonly schema
// values; each mutated value is fed back through the unknown decoder.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const clone = (value: unknown): any => structuredClone(value);

describe("ReleaseSet v2 bounded schema and canonicalization", () => {
  test("accepts the exact complete matrix and produces insertion-order-independent canonical bytes", () => {
    expect(decodeReleaseSet(validReleaseSet)).toEqual({ ok: true, releaseSet: validReleaseSet });
    const reordered = {
      targets: validReleaseSet.targets,
      releaseNotes: validReleaseSet.releaseNotes,
      signingPolicy: validReleaseSet.signingPolicy,
      publishedAt: validReleaseSet.publishedAt,
      sourceRevision: validReleaseSet.sourceRevision,
      version: validReleaseSet.version,
      channel: validReleaseSet.channel,
      app: validReleaseSet.app,
      schemaVersion: validReleaseSet.schemaVersion,
      schema: validReleaseSet.schema,
    };
    expect(canonicalizeReleaseSet(reordered)).toEqual(canonicalizeReleaseSet(validReleaseSet));
  });

  test("rejects unknown fields and every incomplete, duplicate, or impossible target/format shape", () => {
    expect(decodeReleaseSet({ ...validReleaseSet, transportTrusted: true })).toMatchObject({
      ok: false,
      reason: "unknown_field",
    });

    const missingTarget = clone(validReleaseSet);
    missingTarget.targets.pop();
    expect(decodeReleaseSet(missingTarget)).toMatchObject({ ok: false });

    const duplicateTarget = clone(validReleaseSet);
    duplicateTarget.targets[5] = duplicateTarget.targets[4]!;
    expect(decodeReleaseSet(duplicateTarget)).toMatchObject({
      ok: false,
      detail: "target_set_incomplete_or_not_canonical",
    });

    const partialFormats = clone(validReleaseSet);
    partialFormats.targets[0]!.artifacts.pop();
    expect(decodeReleaseSet(partialFormats)).toMatchObject({
      ok: false,
      detail: "format_set_incomplete_or_not_canonical",
    });

    const impossible = clone(validReleaseSet);
    impossible.targets[0]!.artifacts[0]!.format = "rpm";
    expect(decodeReleaseSet(impossible)).toMatchObject({
      ok: false,
      detail: "format_set_incomplete_or_not_canonical",
    });

    const conflict = clone(validReleaseSet);
    conflict.targets[0]!.artifacts[0]!.target = "linux-x64";
    expect(decodeReleaseSet(conflict)).toMatchObject({
      ok: false,
      detail: "artifact_target_conflict",
    });

    const versionConflict = clone(validReleaseSet);
    versionConflict.targets[0].artifacts[0].version = "2.4.0-rc.2";
    expect(decodeReleaseSet(versionConflict)).toMatchObject({
      ok: false,
      detail: "artifact_version_conflict",
    });

    const sourceConflict = clone(validReleaseSet);
    sourceConflict.targets[0].artifacts[0].sourceRevision = "e".repeat(40);
    expect(decodeReleaseSet(sourceConflict)).toMatchObject({
      ok: false,
      detail: "artifact_source_conflict",
    });
  });

  test("rejects version/source conflicts, malformed URLs, identity drift, and bounded-value overflow", () => {
    expect(decodeReleaseSet({ ...validReleaseSet, channel: "stable" })).toMatchObject({
      ok: false,
      detail: "stable_channel_prerelease",
    });
    expect(decodeReleaseSet({ ...validReleaseSet, sourceRevision: "abc" })).toMatchObject({
      ok: false,
      reason: "schema_invalid",
    });

    const credentialed = clone(validReleaseSet);
    credentialed.targets[0]!.artifacts[0]!.url = "https://user:secret@storage.example/release.dmg";
    expect(decodeReleaseSet(credentialed)).toMatchObject({
      ok: false,
      detail: "artifact_url_invalid",
    });

    const query = clone(validReleaseSet);
    query.targets[0]!.artifacts[0]!.url += "?generation=mutable";
    expect(decodeReleaseSet(query)).toMatchObject({ ok: false, detail: "artifact_url_invalid" });

    const wrongName = clone(validReleaseSet);
    wrongName.targets[0]!.artifacts[0]!.name = "other.dmg";
    expect(decodeReleaseSet(wrongName)).toMatchObject({
      ok: false,
      detail: "artifact_identity_invalid",
    });

    const huge = clone(validReleaseSet);
    huge.releaseNotes.summary = "x".repeat(4_097);
    expect(decodeReleaseSet(huge)).toMatchObject({ ok: false, reason: "schema_invalid" });
  });
});

describe("ReleaseSet signing, finalization, and mutation resistance", () => {
  test("finalizes only the complete byte-verified matrix and self-verifies through the pinned seam", () => {
    const finalized = finalizeReleaseSet({
      candidate: validReleaseSet,
      artifactBytesByObjectIdentity: bytesByIdentity,
      key: fixtureKey,
    });
    expect(finalized.pin).toEqual(fixturePin);
    expect(
      verifySignedReleaseSet(finalized.payloadBytes, finalized.envelope, fixturePin, channel),
    ).toEqual({ ok: true, releaseSet: validReleaseSet });
    for (const row of validReleaseSet.targets) {
      for (const artifact of row.artifacts) {
        expect(
          verifyReleaseSetArtifact(artifact, bytesByIdentity.get(artifact.objectIdentity)!),
        ).toBe(true);
      }
    }
  });

  test("refuses missing, extra, hash-mismatched, and length-mismatched artifact bytes before signing", () => {
    const missing = new Map(bytesByIdentity);
    missing.delete(validReleaseSet.targets[0]!.artifacts[0]!.objectIdentity);
    expect(() =>
      finalizeReleaseSet({
        candidate: validReleaseSet,
        artifactBytesByObjectIdentity: missing,
        key: fixtureKey,
      }),
    ).toThrow(/artifact_byte_set_incomplete/);

    const extra = new Map(bytesByIdentity).set("unledgered", new Uint8Array([1]));
    expect(() =>
      finalizeReleaseSet({
        candidate: validReleaseSet,
        artifactBytesByObjectIdentity: extra,
        key: fixtureKey,
      }),
    ).toThrow(/artifact_byte_set_incomplete/);

    const bad = new Map(bytesByIdentity);
    bad.set(validReleaseSet.targets[0]!.artifacts[0]!.objectIdentity, new Uint8Array([1, 2, 3]));
    expect(() =>
      finalizeReleaseSet({
        candidate: validReleaseSet,
        artifactBytesByObjectIdentity: bad,
        key: fixtureKey,
      }),
    ).toThrow(/artifact_verification_failed/);

    const wrongLength = clone(validReleaseSet);
    wrongLength.targets[0]!.artifacts[0]!.byteLength += 1;
    expect(() =>
      finalizeReleaseSet({
        candidate: wrongLength,
        artifactBytesByObjectIdentity: bytesByIdentity,
        key: fixtureKey,
      }),
    ).toThrow(/artifact_verification_failed/);
  });

  test("changing any signed selection class invalidates the signature", () => {
    const finalized = finalizeReleaseSet({
      candidate: validReleaseSet,
      artifactBytesByObjectIdentity: bytesByIdentity,
      key: fixtureKey,
    });
    const mutations: ReleaseSet[] = [];
    for (const field of ["url", "sha256", "byteLength", "format"] as const) {
      const value = clone(validReleaseSet);
      const artifact = value.targets[0]!.artifacts[0]!;
      if (field === "url") artifact.url = artifact.url.replace(".dmg", "-mutated.dmg");
      if (field === "sha256") artifact.sha256 = "0".repeat(64);
      if (field === "byteLength") artifact.byteLength += 1;
      if (field === "format") artifact.format = "zip";
      mutations.push(value);
    }
    mutations.push({ ...validReleaseSet, sourceRevision: "d".repeat(40) });

    for (const mutation of mutations) {
      const bytes = new TextEncoder().encode(canonicalJson(mutation));
      const envelope = {
        ...finalized.envelope,
        sha256: createHash("sha256").update(bytes).digest("hex"),
      };
      expect(verifySignedReleaseSet(bytes, envelope, fixturePin, channel)).toEqual({
        ok: false,
        reason: "signature_invalid",
      });
    }
  });

  test("rejects pin/channel/policy mismatch and non-monotonic finalization", () => {
    const finalized = finalizeReleaseSet({
      candidate: validReleaseSet,
      artifactBytesByObjectIdentity: bytesByIdentity,
      key: fixtureKey,
    });
    expect(
      verifySignedReleaseSet(
        finalized.payloadBytes,
        finalized.envelope,
        { ...fixturePin, kid: "other" },
        channel,
      ),
    ).toEqual({ ok: false, reason: "kid_not_pinned" });
    expect(
      verifySignedReleaseSet(finalized.payloadBytes, finalized.envelope, fixturePin, "stable"),
    ).toEqual({ ok: false, reason: "channel_mismatch" });

    const wrongPolicy = clone(validReleaseSet);
    wrongPolicy.signingPolicy.keyId = "other-release-key";
    expect(() =>
      finalizeReleaseSet({
        candidate: wrongPolicy,
        artifactBytesByObjectIdentity: bytesByIdentity,
        key: fixtureKey,
      }),
    ).toThrow(/signing_key_policy_mismatch/);

    expect(() =>
      finalizeReleaseSet({
        candidate: validReleaseSet,
        currentReleaseSet: validReleaseSet,
        artifactBytesByObjectIdentity: bytesByIdentity,
        key: fixtureKey,
      }),
    ).toThrow(/not_strictly_newer/);
  });
});

describe("ReleaseSet deterministic selection and bounded v1 migration", () => {
  test("selects exactly the preferred artifact for every native OS/architecture key", () => {
    for (const target of releaseTargetKeys) {
      const [platform, architecture] = target.split("-") as [
        "darwin" | "win32" | "linux",
        "arm64" | "x64",
      ];
      const selected = selectReleaseArtifact({
        releaseSet: validReleaseSet,
        installedChannel: channel,
        installedVersion: "2.4.0-rc.2",
        platform,
        architecture,
        hostVersion: target.startsWith("darwin")
          ? "14.0"
          : target.startsWith("win32")
            ? "11.0.26100"
            : "glibc 2.39",
      });
      expect(selected).toMatchObject({ ok: true, target });
      if (selected.ok) expect(selected.artifact.format).toBe(preferredFormatByTarget[target]);
    }
    expect(
      selectReleaseArtifact({
        releaseSet: validReleaseSet,
        installedChannel: channel,
        platform: "darwin" as const,
        architecture: "arm64" as const,
        hostVersion: "14.0",
        installedVersion: version,
      }),
    ).toEqual({ ok: false, reason: "not_monotonic" });

    expect(
      selectReleaseArtifact({
        releaseSet: validReleaseSet,
        installedChannel: channel,
        installedVersion: "2.4.0-rc.2",
        platform: "darwin",
        architecture: "arm64",
        hostVersion: "12.6",
      }),
    ).toEqual({ ok: false, reason: "minimum_os_not_met" });
  });

  test("v1 remains readable only as typed macOS arm64 compatibility input through the promised interval", () => {
    const v1 = JSON.parse(
      readFileSync(path.join(import.meta.dirname, "fixtures/release-set-v1.json"), "utf8"),
    ) as UpdateManifest;
    expect(decodeReleaseSelection(v1, V1_MIGRATION_END)).toEqual({
      kind: "v1-darwin-arm64",
      manifest: v1,
    });
    expect(decodeReleaseSelection(v1, "2026-10-15T00:00:00Z")).toBeNull();
    expect(
      decodeReleaseSelection(
        { ...v1, artifactName: "OpenAgents-1.2.3-x64.dmg" },
        "2026-07-16T00:00:00Z",
      ),
    ).toBeNull();
    expect(decodeReleaseSelection(validReleaseSet, "2099-01-01T00:00:00Z")).toEqual({
      kind: "v2",
      releaseSet: validReleaseSet,
    });
  });

  test("golden v2 canonical digest detects schema/canonicalization drift", () => {
    const golden = JSON.parse(
      readFileSync(path.join(import.meta.dirname, "fixtures/release-set-v2.json"), "utf8"),
    ) as unknown;
    expect(decodeReleaseSet(golden)).toMatchObject({ ok: true });
    expect(createHash("sha256").update(canonicalizeReleaseSet(golden)).digest("hex")).toBe(
      "cca8a70d58187743b626ef3413b8f5ba166bc53a1b321997969a4dfc17eb1727",
    );
  });
});
