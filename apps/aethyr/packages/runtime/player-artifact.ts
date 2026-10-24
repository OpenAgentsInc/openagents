import {
  validateStreamPackage,
  streamPackageIdentity,
  type StreamPackageManifest,
} from "./cell-stream.ts";

/** Local static-player format, separate from the proposed signed release format. */
export const PLAYER_ARTIFACT_SCHEMA = "aethyrforge.player-artifact.v1" as const;
export const PLAYER_ARTIFACT_ABI = 1 as const;
export const PLAYER_FEATURES = [
  "render",
  "materials",
  "collision",
  "controller",
  "rules",
  "interaction",
  "character",
  "sky",
] as const;
export type PlayerFeature = (typeof PLAYER_FEATURES)[number];
export type PlayerProfile = "empty" | "room" | "character";

/** The controller includes the existing Rust fixed-step movement and physics. */
export const PLAYER_FEATURE_DEPENDENCIES: Readonly<
  Record<PlayerFeature, readonly PlayerFeature[]>
> = Object.freeze({
  render: Object.freeze([]),
  materials: Object.freeze(["render"] as const),
  collision: Object.freeze([]),
  controller: Object.freeze(["collision"] as const),
  rules: Object.freeze([]),
  interaction: Object.freeze(["rules", "collision"] as const),
  character: Object.freeze(["controller", "materials"] as const),
  sky: Object.freeze(["render"] as const),
});
export const PLAYER_PROFILE_FEATURES: Readonly<
  Record<PlayerProfile, readonly PlayerFeature[]>
> = Object.freeze({
  empty: Object.freeze(["render"] as const),
  room: Object.freeze(["interaction", "controller", "materials"] as const),
  character: Object.freeze([
    "interaction",
    "controller",
    "materials",
    "character",
    "sky",
  ] as const),
});
export interface PlayerArtifactPin {
  abi: typeof PLAYER_ARTIFACT_ABI;
  url: string;
  sha256: string;
  bytes: number;
}
export interface PlayerArtifact {
  schema: typeof PLAYER_ARTIFACT_SCHEMA;
  abi: typeof PLAYER_ARTIFACT_ABI;
  profile: PlayerProfile;
  worldId: string;
  sourceIdentity: string;
  runtime: PlayerArtifactPin;
  kernel?: PlayerArtifactPin;
  features: PlayerFeature[];
  content?: {
    package: StreamPackageManifest;
    entryCell: string;
    worldURL: string;
  };
  /** Each enabled feature declares its cooked data, including an empty code-only set. */
  resources: Partial<Record<PlayerFeature, string[]>>;
}
export interface PlayerArtifactCompatibility {
  profile?: PlayerProfile;
  worldId?: string;
  runtime?: Pick<PlayerArtifactPin, "abi" | "sha256">;
  kernel?: Pick<PlayerArtifactPin, "abi" | "sha256">;
}
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const integer = (
  value: unknown,
  max = Number.MAX_SAFE_INTEGER,
): value is number =>
  typeof value === "number" &&
  Number.isSafeInteger(value) &&
  value >= 0 &&
  value <= max;
const hash = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const profile = (value: unknown): value is PlayerProfile =>
  value === "empty" || value === "room" || value === "character";
const feature = (value: unknown): value is PlayerFeature =>
  typeof value === "string" &&
  (PLAYER_FEATURES as readonly string[]).includes(value);
function keys(
  value: unknown,
  allowed: readonly string[],
): asserts value is Record<string, unknown> {
  if (
    !record(value) ||
    Object.keys(value).some((key) => !allowed.includes(key))
  )
    throw new Error("Choose supported player artifact fields.");
}
function localURL(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 512 &&
    /^\/[a-zA-Z0-9_./-]+$/.test(value) &&
    value
      .slice(1)
      .split("/")
      .every((part) => part && part !== "." && part !== "..")
  );
}
function pin(value: unknown, extension: ".js" | ".wasm"): PlayerArtifactPin {
  keys(value, ["abi", "url", "sha256", "bytes"]);
  if (
    value.abi !== PLAYER_ARTIFACT_ABI ||
    !localURL(value.url) ||
    !value.url.endsWith(extension) ||
    !hash(value.sha256) ||
    !integer(value.bytes, 64 * 1024 * 1024) ||
    value.bytes === 0
  )
    throw new Error(
      "Choose a compatible player runtime with a local URL, hash, and size.",
    );
  return {
    abi: PLAYER_ARTIFACT_ABI,
    url: value.url,
    sha256: value.sha256,
    bytes: value.bytes,
  };
}
export function playerFeatureClosure(
  features: readonly PlayerFeature[],
): PlayerFeature[] {
  if (
    !Array.isArray(features) ||
    features.length > PLAYER_FEATURES.length ||
    features.some((value) => !feature(value))
  )
    throw new Error("Choose supported player features.");
  const selected = new Set<PlayerFeature>();
  const add = (value: PlayerFeature) => {
    if (selected.has(value)) return;
    for (const dependency of PLAYER_FEATURE_DEPENDENCIES[value])
      add(dependency);
    selected.add(value);
  };
  features.forEach(add);
  return PLAYER_FEATURES.filter((value) => selected.has(value));
}
export function playerProfileFeatures(value: PlayerProfile): PlayerFeature[] {
  if (!profile(value))
    throw new Error("Choose an empty, room, or character player profile.");
  return playerFeatureClosure(PLAYER_PROFILE_FEATURES[value]);
}

/** Validate a complete candidate without changing active player state or fetching files. */
export function validatePlayerArtifact(
  value: unknown,
  compatibility: PlayerArtifactCompatibility = {},
): PlayerArtifact {
  keys(value, [
    "schema",
    "abi",
    "profile",
    "worldId",
    "sourceIdentity",
    "runtime",
    "kernel",
    "features",
    "content",
    "resources",
  ]);
  if (
    value.schema !== PLAYER_ARTIFACT_SCHEMA ||
    value.abi !== PLAYER_ARTIFACT_ABI ||
    !profile(value.profile) ||
    typeof value.worldId !== "string" ||
    !/^[a-zA-Z0-9][a-zA-Z0-9_.:-]{0,95}$/.test(value.worldId) ||
    !hash(value.sourceIdentity) ||
    !Array.isArray(value.features) ||
    value.features.length > PLAYER_FEATURES.length ||
    value.features.some((item) => !feature(item)) ||
    new Set(value.features).size !== value.features.length
  )
    throw new Error("Open a complete compatible player artifact.");
  const selected = playerProfileFeatures(value.profile);
  const features = value.features;
  if (
    selected.length !== features.length ||
    selected.some((item) => !features.includes(item))
  )
    throw new Error(
      "The player features do not match their profile and dependencies.",
    );
  const runtime = pin(value.runtime, ".js");
  const kernel =
    value.kernel === undefined ? undefined : pin(value.kernel, ".wasm");
  let content: PlayerArtifact["content"];
  if (value.content !== undefined) {
    keys(value.content, ["package", "entryCell", "worldURL"]);
    const cooked = validateStreamPackage(value.content.package);
    if (
      typeof value.content.entryCell !== "string" ||
      value.content.entryCell !== cooked.entryCell
    )
      throw new Error("Open the cooked package at its declared entry cell.");
    const startup = cooked.cells.find((cell) => cell.id === cooked.entryCell)!;
    const worldURL = value.content.worldURL;
    if (
      !localURL(worldURL) ||
      !startup.resources.some((file) => file.url === worldURL)
    )
      throw new Error(
        "Include the authored world settings in the first playable cell.",
      );
    content = {
      package: cooked,
      entryCell: cooked.entryCell,
      worldURL,
    };
  }
  if (
    value.profile === "empty"
      ? kernel !== undefined || content !== undefined
      : !kernel || !content
  )
    throw new Error(
      "The player profile needs its matching cooked content and kernel.",
    );
  if (
    !record(value.resources) ||
    Object.keys(value.resources).some(
      (key) => !selected.includes(key as PlayerFeature),
    )
  )
    throw new Error("Declare resources only for the enabled player features.");
  const resources: PlayerArtifact["resources"] = {};
  const inventory = new Set(
    content?.package.files.map((file) => file.url) ?? [],
  );
  for (const name of selected) {
    const urls = value.resources[name];
    if (value.profile === "empty" && urls === undefined) continue;
    if (
      !Array.isArray(urls) ||
      urls.length > 16_384 ||
      urls.some((url) => typeof url !== "string" || !inventory.has(url)) ||
      new Set(urls).size !== urls.length
    )
      throw new Error(
        "Declare complete cooked resource lists for every player feature.",
      );
    resources[name] = [...urls] as string[];
  }
  if (content) {
    const claimed = new Set(Object.values(resources).flat());
    if ([...inventory].some((url) => !claimed.has(url)))
      throw new Error(
        "Assign every cooked resource to an enabled player feature.",
      );
    const startup = new Set(
      content.package.cells
        .find((cell) => cell.id === content!.entryCell)!
        .resources.map((file) => file.url),
    );
    for (const name of ["collision", "rules"] as const)
      if (
        !resources[name]?.length ||
        resources[name]!.some((url) => !startup.has(url))
      )
        throw new Error(
          "Include collision and rule data in the first playable cell.",
        );
  }
  if (
    (compatibility.profile !== undefined &&
      compatibility.profile !== value.profile) ||
    (compatibility.worldId !== undefined &&
      compatibility.worldId !== value.worldId) ||
    (compatibility.runtime &&
      (compatibility.runtime.abi !== runtime.abi ||
        compatibility.runtime.sha256 !== runtime.sha256)) ||
    (compatibility.kernel &&
      (!kernel ||
        compatibility.kernel.abi !== kernel.abi ||
        compatibility.kernel.sha256 !== kernel.sha256))
  )
    throw new Error(
      "This artifact needs its pinned player runtime and kernel.",
    );
  return {
    schema: PLAYER_ARTIFACT_SCHEMA,
    abi: PLAYER_ARTIFACT_ABI,
    profile: value.profile,
    worldId: value.worldId,
    sourceIdentity: value.sourceIdentity,
    runtime,
    ...(kernel ? { kernel } : {}),
    features: selected,
    ...(content ? { content } : {}),
    resources,
  };
}
/** Verify the existing cooked descriptor identity before any player activation. */
export async function verifyPlayerArtifact(
  value: unknown,
  compatibility: PlayerArtifactCompatibility = {},
): Promise<PlayerArtifact> {
  const artifact = validatePlayerArtifact(value, compatibility);
  if (artifact.content) {
    if (
      (await streamPackageIdentity(artifact.content.package)) !==
      artifact.content.package.sha256
    )
      throw new Error("The cooked player descriptor does not match its hash.");
  }
  return artifact;
}
