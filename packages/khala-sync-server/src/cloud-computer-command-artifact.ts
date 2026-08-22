import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";

export type CloudComputerCommandOutputDigest = `sha256:${string}`;

const digest = (bytes: Uint8Array): CloudComputerCommandOutputDigest =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;

export class CloudComputerCommandArtifactError extends Error {
  constructor(
    readonly code: "conflict" | "corrupt" | "invalid" | "limit_exceeded" | "unavailable",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerCommandArtifactError";
  }
}

export type CloudComputerCommandArtifactObject = Readonly<{
  objectRef: string;
  generation: string;
  contentDigest: CloudComputerCommandOutputDigest;
  byteCount: number;
}>;

export interface CloudComputerCommandArtifactStorage {
  inspect(objectRef: string): Promise<CloudComputerCommandArtifactObject | null>;
  createOnly(input: {
    objectRef: string;
    bytes: Uint8Array;
    contentDigest: CloudComputerCommandOutputDigest;
    metadata: Readonly<Record<string, string>>;
  }): Promise<CloudComputerCommandArtifactObject>;
  download(object: CloudComputerCommandArtifactObject): Promise<Uint8Array>;
}

export type CloudComputerCommandOutput =
  | Readonly<{
      storage: "inline";
      encoding: "base64";
      bytesBase64: string;
      contentDigest: CloudComputerCommandOutputDigest;
      byteCount: number;
    }>
  | Readonly<{
      storage: "artifact";
      artifactRef: string;
      object: CloudComputerCommandArtifactObject;
      contentDigest: CloudComputerCommandOutputDigest;
      byteCount: number;
      retainUntil: string;
      reused: boolean;
    }>;

export type VerifiedCloudComputerCommandArtifact = Extract<
  CloudComputerCommandOutput,
  Readonly<{ storage: "artifact" }>
>;

const verifiedOutputs = new WeakSet<object>();

export const assertVerifiedCloudComputerCommandArtifact = (
  value: VerifiedCloudComputerCommandArtifact,
): void => {
  if (!verifiedOutputs.has(value))
    throw new CloudComputerCommandArtifactError(
      "invalid",
      "artifact was not issued by the output service",
    );
};

const assertRef = (value: string, field: string): void => {
  if (!REF.test(value))
    throw new CloudComputerCommandArtifactError("invalid", `${field} is invalid`);
};

const assertObject = (
  object: CloudComputerCommandArtifactObject,
  expected: Readonly<{
    objectRef: string;
    contentDigest: CloudComputerCommandOutputDigest;
    byteCount: number;
  }>,
): void => {
  if (
    object.objectRef !== expected.objectRef ||
    object.contentDigest !== expected.contentDigest ||
    object.byteCount !== expected.byteCount ||
    !/^\d+$/u.test(object.generation)
  )
    throw new CloudComputerCommandArtifactError("conflict", "artifact object binding differs");
};

export const cloudComputerCommandArtifactRef = (input: {
  ownerRef: string;
  tenantRef: string;
  commandRef: string;
  runtimeGeneration: number;
  kind: "stdout" | "stderr" | "result" | "diagnostic";
  contentDigest: CloudComputerCommandOutputDigest;
}): string => {
  const scopeDigest = createHash("sha256")
    .update(
      canonicalJson({
        ownerRef: input.ownerRef,
        tenantRef: input.tenantRef,
        commandRef: input.commandRef,
        runtimeGeneration: input.runtimeGeneration,
        kind: input.kind,
      }),
    )
    .digest("hex");
  return `command-artifacts/${scopeDigest}/${input.contentDigest.slice("sha256:".length)}`;
};

const opaqueArtifactRef = (objectRef: string): string =>
  `artifact.${createHash("sha256").update(objectRef).digest("hex")}`;

/** Keeps bounded output inline and uploads larger output through create-only storage. */
export const persistCloudComputerCommandOutput = async (input: {
  storage: CloudComputerCommandArtifactStorage;
  ownerRef: string;
  tenantRef: string;
  commandRef: string;
  runtimeGeneration: number;
  kind: "stdout" | "stderr" | "result" | "diagnostic";
  bytes: Uint8Array;
  inlineByteLimit: number;
  commandByteLimit: number;
  priorCommandByteCount: number;
  retainUntil: string;
}): Promise<CloudComputerCommandOutput> => {
  for (const [field, value] of Object.entries({
    ownerRef: input.ownerRef,
    tenantRef: input.tenantRef,
    commandRef: input.commandRef,
  }))
    assertRef(value, field);
  for (const [field, value] of Object.entries({
    runtimeGeneration: input.runtimeGeneration,
    inlineByteLimit: input.inlineByteLimit,
    commandByteLimit: input.commandByteLimit,
    priorCommandByteCount: input.priorCommandByteCount,
  })) {
    if (!Number.isSafeInteger(value) || value < (field === "runtimeGeneration" ? 1 : 0))
      throw new CloudComputerCommandArtifactError("invalid", `${field} is invalid`);
  }
  if (
    input.inlineByteLimit > input.commandByteLimit ||
    !Number.isFinite(Date.parse(input.retainUntil))
  )
    throw new CloudComputerCommandArtifactError("invalid", "output policy is invalid");
  if (input.priorCommandByteCount + input.bytes.byteLength > input.commandByteLimit)
    throw new CloudComputerCommandArtifactError("limit_exceeded", "command output limit exceeded");

  const contentDigest = digest(input.bytes);
  if (input.bytes.byteLength <= input.inlineByteLimit) {
    return {
      storage: "inline",
      encoding: "base64",
      bytesBase64: Buffer.from(input.bytes).toString("base64"),
      contentDigest,
      byteCount: input.bytes.byteLength,
    };
  }

  const objectRef = cloudComputerCommandArtifactRef({ ...input, contentDigest });
  const existing = await input.storage.inspect(objectRef);
  if (existing !== null) {
    assertObject(existing, { objectRef, contentDigest, byteCount: input.bytes.byteLength });
    const output: VerifiedCloudComputerCommandArtifact = Object.freeze({
      storage: "artifact",
      artifactRef: opaqueArtifactRef(objectRef),
      object: existing,
      contentDigest,
      byteCount: input.bytes.byteLength,
      retainUntil: input.retainUntil,
      reused: true,
    });
    verifiedOutputs.add(output);
    return output;
  }
  const object = await input.storage.createOnly({
    objectRef,
    bytes: input.bytes,
    contentDigest,
    metadata: {
      commandRef: input.commandRef,
      runtimeGeneration: String(input.runtimeGeneration),
      kind: input.kind,
    },
  });
  assertObject(object, { objectRef, contentDigest, byteCount: input.bytes.byteLength });
  const output: VerifiedCloudComputerCommandArtifact = Object.freeze({
    storage: "artifact",
    artifactRef: opaqueArtifactRef(objectRef),
    object,
    contentDigest,
    byteCount: input.bytes.byteLength,
    retainUntil: input.retainUntil,
    reused: false,
  });
  verifiedOutputs.add(output);
  return output;
};

export const downloadCloudComputerCommandArtifact = async (input: {
  storage: CloudComputerCommandArtifactStorage;
  object: CloudComputerCommandArtifactObject;
}): Promise<Uint8Array> => {
  const bytes = await input.storage.download(input.object);
  if (bytes.byteLength !== input.object.byteCount || digest(bytes) !== input.object.contentDigest) {
    bytes.fill(0);
    throw new CloudComputerCommandArtifactError("corrupt", "artifact content differs");
  }
  return bytes;
};
