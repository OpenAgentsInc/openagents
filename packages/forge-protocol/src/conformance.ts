import { Schema as S } from "effect";

export const ForgeInteropProfile = S.Literals([
  "nip34-repository-30617",
  "nip34-state-30618",
  "nip34-patch-1617",
  "ngit-pointer-pr-1618",
  "ngit-pointer-update-1619",
  "buzz-sdk-pointer-pr-1618",
  "buzz-sdk-pointer-update-1619",
  "buzz-desktop-target-branch-1618",
  "nip22-comment-1111",
]);
export type ForgeInteropProfile = typeof ForgeInteropProfile.Type;

export const ForgeInteropEvent = S.Struct({
  id: S.String,
  pubkey: S.String,
  created_at: S.Number,
  kind: S.Number,
  tags: S.Array(S.Array(S.String)),
  content: S.String,
});
export type ForgeInteropEvent = typeof ForgeInteropEvent.Type;

const ForgeFixtureSource = S.Struct({
  peer: S.String,
  revision: S.String,
  sourcePaths: S.Array(S.String),
});

const ForgeObjectRef = S.Struct({
  ref: S.String,
  oid: S.String,
});

export const ForgeInteropFixture = S.Struct({
  fixtureVersion: S.Literal("openagents.forge.interop-fixture.v1"),
  id: S.String,
  profile: ForgeInteropProfile,
  source: ForgeFixtureSource,
  event: ForgeInteropEvent,
  objectRefs: S.Array(ForgeObjectRef),
});
export type ForgeInteropFixture = typeof ForgeInteropFixture.Type;

export const ForgeInteropFixtureBundle = S.Struct({
  format: S.Literal("openagents.forge.interop-fixture-bundle.v1"),
  fixtures: S.Array(ForgeInteropFixture),
});
export type ForgeInteropFixtureBundle = typeof ForgeInteropFixtureBundle.Type;

const profileKinds: Readonly<Record<ForgeInteropProfile, number>> = {
  "nip34-repository-30617": 30617,
  "nip34-state-30618": 30618,
  "nip34-patch-1617": 1617,
  "ngit-pointer-pr-1618": 1618,
  "ngit-pointer-update-1619": 1619,
  "buzz-sdk-pointer-pr-1618": 1618,
  "buzz-sdk-pointer-update-1619": 1619,
  "buzz-desktop-target-branch-1618": 1618,
  "nip22-comment-1111": 1111,
};

const knownTags: Readonly<Record<ForgeInteropProfile, ReadonlySet<string>>> = {
  "nip34-repository-30617": new Set([
    "d",
    "name",
    "description",
    "web",
    "clone",
    "relays",
    "r",
    "maintainers",
    "t",
  ]),
  "nip34-state-30618": new Set(["d", "HEAD"]),
  "nip34-patch-1617": new Set([
    "a",
    "p",
    "r",
    "t",
    "commit",
    "parent-commit",
    "commit-pgp-sig",
    "committer",
    "branch-name",
  ]),
  "ngit-pointer-pr-1618": new Set([
    "a",
    "p",
    "subject",
    "alt",
    "r",
    "c",
    "clone",
    "branch-name",
    "merge-base",
    "t",
    "e",
  ]),
  "ngit-pointer-update-1619": new Set(["a", "p", "E", "P", "r", "c", "clone", "merge-base"]),
  "buzz-sdk-pointer-pr-1618": new Set([
    "a",
    "p",
    "subject",
    "r",
    "c",
    "clone",
    "branch-name",
    "merge-base",
    "t",
    "e",
  ]),
  "buzz-sdk-pointer-update-1619": new Set(["a", "p", "E", "P", "r", "c", "clone", "merge-base"]),
  "buzz-desktop-target-branch-1618": new Set([
    "a",
    "p",
    "subject",
    "c",
    "clone",
    "branch-name",
    "target-branch",
    "merge-base",
  ]),
  "nip22-comment-1111": new Set(["E", "K", "P", "e", "k", "p", "a", "q"]),
};

const isHex = (value: string, length: number): boolean =>
  new RegExp(`^[0-9a-f]{${length}}$`).test(value);

const tagsNamed = (event: ForgeInteropEvent, name: string): ReadonlyArray<ReadonlyArray<string>> =>
  event.tags.filter((tag) => tag[0] === name);

const firstTagValue = (event: ForgeInteropEvent, name: string): string | undefined =>
  tagsNamed(event, name)[0]?.[1];

const allTagValues = (event: ForgeInteropEvent, name: string): ReadonlyArray<string> =>
  tagsNamed(event, name).flatMap((tag) => tag.slice(1));

const requireTag = (event: ForgeInteropEvent, name: string, diagnostics: Array<string>): void => {
  if (firstTagValue(event, name) === undefined)
    diagnostics.push(`event is missing the ${name} tag`);
};

const pointerProfiles: ReadonlySet<ForgeInteropProfile> = new Set([
  "ngit-pointer-pr-1618",
  "ngit-pointer-update-1619",
  "buzz-sdk-pointer-pr-1618",
  "buzz-sdk-pointer-update-1619",
]);

export interface ForgeInteropProjection {
  readonly fixtureId: string;
  readonly profile: ForgeInteropProfile;
  readonly event: ForgeInteropEvent;
  readonly cloneUrls: ReadonlyArray<string>;
  readonly unknownTags: ReadonlyArray<ReadonlyArray<string>>;
  readonly requiredObjectIds: ReadonlyArray<string>;
  readonly objectRefs: ReadonlyArray<{
    readonly ref: string;
    readonly oid: string;
  }>;
}

export type ForgeInteropFixtureResult =
  | Readonly<{
      state: "FixtureValid";
      fixture: ForgeInteropFixture;
      projection: ForgeInteropProjection;
    }>
  | Readonly<{
      state: "FixtureInvalid";
      diagnostics: ReadonlyArray<string>;
    }>;

export const validateForgeInteropFixture = (value: unknown): ForgeInteropFixtureResult => {
  let fixture: ForgeInteropFixture;
  try {
    fixture = S.decodeUnknownSync(ForgeInteropFixture)(value, {
      onExcessProperty: "error",
    });
  } catch {
    return {
      state: "FixtureInvalid",
      diagnostics: ["fixture does not match the closed schema"],
    };
  }

  const diagnostics: Array<string> = [];
  if (!isHex(fixture.source.revision, 40))
    diagnostics.push("source revision must be a full 40-character lowercase SHA");
  if (!isHex(fixture.event.id, 64))
    diagnostics.push("event id must be 64 lowercase hexadecimal characters");
  if (!isHex(fixture.event.pubkey, 64))
    diagnostics.push("event pubkey must be 64 lowercase hexadecimal characters");
  if (fixture.event.kind !== profileKinds[fixture.profile])
    diagnostics.push("event kind does not match the declared profile");

  const event = fixture.event;
  switch (fixture.profile) {
    case "nip34-repository-30617":
      requireTag(event, "d", diagnostics);
      requireTag(event, "clone", diagnostics);
      requireTag(event, "maintainers", diagnostics);
      break;
    case "nip34-state-30618":
      requireTag(event, "d", diagnostics);
      requireTag(event, "HEAD", diagnostics);
      if (!event.tags.some((tag) => tag[0]?.startsWith("refs/")))
        diagnostics.push("repository state has no ref tag");
      break;
    case "nip34-patch-1617":
      requireTag(event, "a", diagnostics);
      if (!/^From [0-9a-f]{40} /m.test(event.content) || !event.content.includes("diff --git"))
        diagnostics.push("kind 1617 content is not a git format-patch mbox");
      break;
    case "ngit-pointer-pr-1618":
      for (const tag of ["a", "p", "subject", "alt", "c", "clone"])
        requireTag(event, tag, diagnostics);
      break;
    case "buzz-sdk-pointer-pr-1618":
      for (const tag of ["a", "p", "subject", "c", "clone"]) requireTag(event, tag, diagnostics);
      break;
    case "ngit-pointer-update-1619":
    case "buzz-sdk-pointer-update-1619":
      for (const tag of ["a", "p", "E", "P", "c", "clone"]) requireTag(event, tag, diagnostics);
      break;
    case "buzz-desktop-target-branch-1618":
      for (const tag of ["a", "p", "subject", "c", "clone", "target-branch"])
        requireTag(event, tag, diagnostics);
      break;
    case "nip22-comment-1111":
      for (const tag of ["E", "K", "P"]) requireTag(event, tag, diagnostics);
      if (firstTagValue(event, "e") !== undefined) {
        requireTag(event, "k", diagnostics);
        requireTag(event, "p", diagnostics);
      }
      break;
  }

  const commit = firstTagValue(event, "c");
  if (pointerProfiles.has(fixture.profile) && commit !== undefined) {
    const pointerRef = `refs/nostr/${event.id}`;
    if (
      !fixture.objectRefs.some(
        (objectRef) => objectRef.ref === pointerRef && objectRef.oid === commit,
      )
    )
      diagnostics.push("pointer ref does not resolve to the event commit");
  }

  const requiredObjectIds =
    fixture.profile === "nip34-repository-30617"
      ? tagsNamed(event, "r")
          .filter((tag) => tag[2] === "euc")
          .flatMap((tag) => tag.slice(1, 2))
      : fixture.profile === "nip34-state-30618"
        ? event.tags.filter((tag) => tag[0]?.startsWith("refs/")).flatMap((tag) => tag.slice(1))
        : pointerProfiles.has(fixture.profile) && commit !== undefined
          ? [commit]
          : [];

  for (const oid of requiredObjectIds) {
    if (!isHex(oid, 40)) diagnostics.push(`event names an invalid Git object id: ${oid}`);
  }
  for (const objectRef of fixture.objectRefs) {
    if (!isHex(objectRef.oid, 40))
      diagnostics.push(`object ref names an invalid Git object id: ${objectRef.oid}`);
  }
  if (diagnostics.length > 0) return { state: "FixtureInvalid", diagnostics };

  return {
    state: "FixtureValid",
    fixture,
    projection: {
      fixtureId: fixture.id,
      profile: fixture.profile,
      event: structuredClone(event),
      cloneUrls: allTagValues(event, "clone"),
      unknownTags: structuredClone(
        event.tags.filter((tag) => {
          const name = tag[0];
          return (
            name !== undefined &&
            !knownTags[fixture.profile].has(name) &&
            !(fixture.profile === "nip34-state-30618" && name.startsWith("refs/"))
          );
        }),
      ),
      requiredObjectIds,
      objectRefs: structuredClone(fixture.objectRefs),
    },
  };
};

export const encodeForgeInteropProjection = (
  projection: ForgeInteropProjection,
): ForgeInteropEvent => structuredClone(projection.event);

export type ForgeProjectionGate =
  | Readonly<{
      state: "ProjectionReady";
      projection: ForgeInteropProjection;
    }>
  | Readonly<{
      state: "ProjectionBlocked";
      reason: "object_unavailable";
      missingObjectIds: ReadonlyArray<string>;
    }>;

export const evaluateForgeProjectionGate = (
  projection: ForgeInteropProjection,
  availableObjectIds: ReadonlySet<string>,
): ForgeProjectionGate => {
  const missingObjectIds = projection.requiredObjectIds.filter(
    (oid) => !availableObjectIds.has(oid),
  );
  return missingObjectIds.length === 0
    ? { state: "ProjectionReady", projection }
    : {
        state: "ProjectionBlocked",
        reason: "object_unavailable",
        missingObjectIds,
      };
};

export const ForgeConformanceProof = S.Literals(["fixture", "live-peer", "owned-service"]);
export type ForgeConformanceProof = typeof ForgeConformanceProof.Type;

export const ForgeConformanceResult = S.Literals([
  "fixture-pass",
  "live-pass",
  "blocked",
  "failed",
]);
export type ForgeConformanceResult = typeof ForgeConformanceResult.Type;

export const ForgeConformanceClaim = S.Literals(["none", "fixture-reader", "works-with-peer"]);
export type ForgeConformanceClaim = typeof ForgeConformanceClaim.Type;

const MatrixPeer = S.Struct({
  id: S.String,
  revision: S.String,
});
const MatrixEvidence = S.Struct({
  path: S.String,
  sha256: S.String,
});
const MatrixRow = S.Struct({
  id: S.String,
  profile: S.String,
  peer: S.String,
  peerRevision: S.String,
  proof: ForgeConformanceProof,
  result: ForgeConformanceResult,
  claim: ForgeConformanceClaim,
  evidence: S.Array(MatrixEvidence),
  blockerRefs: S.Array(S.String),
});
export const ForgeConformanceMatrix = S.Struct({
  format: S.Literal("openagents.forge.conformance-matrix.v1"),
  openAgentsRevision: S.String,
  nostrEffectRevision: S.String,
  peers: S.Array(MatrixPeer),
  receipts: S.Array(MatrixEvidence),
  rows: S.Array(MatrixRow),
});
export type ForgeConformanceMatrix = typeof ForgeConformanceMatrix.Type;

export type ForgeConformanceCompilation =
  | Readonly<{
      state: "ConformanceReady";
      fixtureClaims: ReadonlyArray<string>;
      liveClaims: ReadonlyArray<string>;
      blockedRows: ReadonlyArray<string>;
    }>
  | Readonly<{
      state: "ConformanceUnavailable";
      diagnostics: ReadonlyArray<string>;
    }>;

export const forgeConformanceRequiredRows: ReadonlyArray<string> = [
  "ngit-repository-announcement-30617",
  "ngit-repository-state-30618",
  "ngit-standard-patch-1617",
  "ngit-pointer-pr-1618",
  "ngit-pointer-update-1619",
  "ngit-unknown-tag-round-trip",
  "gitworkshop-nip22-comment-1111",
  "buzz-sdk-pointer-pr-1618",
  "buzz-sdk-pointer-update-1619",
  "buzz-desktop-target-branch-read-only",
  "fixture-object-projection-race",
  "ngit-live-clone-fetch",
  "gitworkshop-live-discovery-read",
  "owned-service-object-projection-race",
];

export const compileForgeConformanceMatrix = (value: unknown): ForgeConformanceCompilation => {
  let matrix: ForgeConformanceMatrix;
  try {
    matrix = S.decodeUnknownSync(ForgeConformanceMatrix)(value, {
      onExcessProperty: "error",
    });
  } catch {
    return {
      state: "ConformanceUnavailable",
      diagnostics: ["matrix does not match the closed schema"],
    };
  }

  const diagnostics: Array<string> = [];
  if (matrix.receipts.length === 0) diagnostics.push("matrix has no suite receipt");
  for (const receipt of matrix.receipts) {
    if (!isHex(receipt.sha256, 64)) diagnostics.push("matrix has an invalid suite receipt digest");
  }
  const peers = new Map(matrix.peers.map((peer) => [peer.id, peer.revision]));
  const rowIds = new Set<string>();
  for (const peer of matrix.peers) {
    if (!isHex(peer.revision, 40))
      diagnostics.push(`peer ${peer.id} does not have a full revision`);
  }
  for (const row of matrix.rows) {
    if (rowIds.has(row.id)) diagnostics.push(`duplicate row ${row.id}`);
    rowIds.add(row.id);
    if (peers.get(row.peer) !== row.peerRevision)
      diagnostics.push(`row ${row.id} does not match its peer revision`);
    if (row.evidence.length === 0 && row.result.endsWith("pass"))
      diagnostics.push(`row ${row.id} has a pass without evidence`);
    // A fixture proves only that our reader accepts pinned bytes. A live result
    // needs a separate, durable receipt. This prevents a future matrix edit
    // from converting a fixture path into a peer compatibility claim.
    if (
      row.result === "live-pass" &&
      !row.evidence.some((evidence) =>
        evidence.path.startsWith("packages/forge-protocol/conformance/receipts/"),
      )
    )
      diagnostics.push(`row ${row.id} has a live pass without a live receipt`);
    if (row.result === "blocked" && row.blockerRefs.length === 0)
      diagnostics.push(`row ${row.id} is blocked without a blocker`);
    if (
      row.claim === "fixture-reader" &&
      !(row.proof === "fixture" && row.result === "fixture-pass")
    )
      diagnostics.push(`row ${row.id} has an invalid fixture claim`);
    if (
      row.claim === "works-with-peer" &&
      !(row.proof === "live-peer" && row.result === "live-pass")
    )
      diagnostics.push(`row ${row.id} has an invalid peer compatibility claim`);
    if (row.claim !== "none" && row.result === "blocked")
      diagnostics.push(`row ${row.id} claims blocked behavior`);
    for (const evidence of row.evidence) {
      if (!isHex(evidence.sha256, 64))
        diagnostics.push(`row ${row.id} has an invalid evidence digest`);
    }
  }
  const requiredRows = new Set(forgeConformanceRequiredRows);
  for (const id of forgeConformanceRequiredRows) {
    if (!rowIds.has(id)) diagnostics.push(`matrix is missing required row ${id}`);
  }
  for (const id of rowIds) {
    if (!requiredRows.has(id)) diagnostics.push(`matrix has unknown row ${id}`);
  }
  if (!isHex(matrix.openAgentsRevision, 40))
    diagnostics.push("OpenAgents revision is not a full SHA");
  if (!isHex(matrix.nostrEffectRevision, 40))
    diagnostics.push("nostr-effect revision is not a full SHA");
  if (diagnostics.length > 0) return { state: "ConformanceUnavailable", diagnostics };

  return {
    state: "ConformanceReady",
    fixtureClaims: matrix.rows.filter((row) => row.claim === "fixture-reader").map((row) => row.id),
    liveClaims: matrix.rows.filter((row) => row.claim === "works-with-peer").map((row) => row.id),
    blockedRows: matrix.rows.filter((row) => row.result === "blocked").map((row) => row.id),
  };
};
