import { describe, expect, test } from "vite-plus/test";

import {
  assertVerifiedCloudComputerCommandArtifact,
  CloudComputerCommandArtifactError,
  downloadCloudComputerCommandArtifact,
  persistCloudComputerCommandOutput,
  type CloudComputerCommandArtifactObject,
  type CloudComputerCommandArtifactStorage,
} from "./cloud-computer-command-artifact.js";

class MemoryStorage implements CloudComputerCommandArtifactStorage {
  readonly objects = new Map<
    string,
    { object: CloudComputerCommandArtifactObject; bytes: Uint8Array }
  >();
  creates = 0;

  async inspect(objectRef: string) {
    return this.objects.get(objectRef)?.object ?? null;
  }

  async createOnly(input: {
    objectRef: string;
    bytes: Uint8Array;
    contentDigest: `sha256:${string}`;
  }) {
    this.creates += 1;
    const object = {
      objectRef: input.objectRef,
      generation: "17",
      contentDigest: input.contentDigest,
      byteCount: input.bytes.byteLength,
    };
    this.objects.set(input.objectRef, { object, bytes: Uint8Array.from(input.bytes) });
    return object;
  }

  async download(object: CloudComputerCommandArtifactObject) {
    const stored = this.objects.get(object.objectRef);
    if (stored === undefined) throw new Error("missing object");
    return Uint8Array.from(stored.bytes);
  }
}

const fixture = (storage: CloudComputerCommandArtifactStorage, bytes: Uint8Array) => ({
  storage,
  ownerRef: "owner.command.artifact",
  tenantRef: "tenant.command.artifact",
  commandRef: "command.artifact.one",
  runtimeGeneration: 4,
  kind: "stdout" as const,
  bytes,
  inlineByteLimit: 4,
  commandByteLimit: 64,
  priorCommandByteCount: 0,
  retainUntil: "2026-08-23T12:00:00.000Z",
});

describe("cloud computer command artifacts", () => {
  test("keeps small binary-safe output inline", async () => {
    const storage = new MemoryStorage();
    await expect(
      persistCloudComputerCommandOutput(fixture(storage, Uint8Array.from([0, 1, 2, 3]))),
    ).resolves.toMatchObject({
      storage: "inline",
      encoding: "base64",
      bytesBase64: "AAECAw==",
      byteCount: 4,
    });
    expect(storage.creates).toBe(0);
  });

  test("creates and safely reuses a content-addressed large artifact", async () => {
    const storage = new MemoryStorage();
    const bytes = new TextEncoder().encode("large-output");
    const first = await persistCloudComputerCommandOutput(fixture(storage, bytes));
    const replay = await persistCloudComputerCommandOutput(fixture(storage, bytes));
    expect(first).toMatchObject({
      storage: "artifact",
      reused: false,
      byteCount: bytes.byteLength,
    });
    expect(replay).toMatchObject({ storage: "artifact", reused: true });
    expect(storage.creates).toBe(1);
    if (first.storage !== "artifact") throw new Error("expected artifact");
    expect(() => assertVerifiedCloudComputerCommandArtifact(first)).not.toThrow();
    await expect(
      downloadCloudComputerCommandArtifact({ storage, object: first.object }),
    ).resolves.toEqual(bytes);
  });

  test("uses command-scoped opaque refs for identical bytes", async () => {
    const storage = new MemoryStorage();
    const bytes = new TextEncoder().encode("shared-output");
    const first = await persistCloudComputerCommandOutput(fixture(storage, bytes));
    const second = await persistCloudComputerCommandOutput({
      ...fixture(storage, bytes),
      commandRef: "command.artifact.two",
    });
    if (first.storage !== "artifact" || second.storage !== "artifact") {
      throw new Error("expected artifact output");
    }
    expect(second.contentDigest).toBe(first.contentDigest);
    expect(second.artifactRef).not.toBe(first.artifactRef);
    expect(second.object.objectRef).not.toBe(first.object.objectRef);
  });

  test("rejects a caller-asserted artifact binding", () => {
    expect(() =>
      assertVerifiedCloudComputerCommandArtifact({
        storage: "artifact",
        artifactRef: `artifact.${"0".repeat(64)}`,
        object: {
          objectRef: `command-artifacts/${"0".repeat(64)}/${"0".repeat(64)}`,
          generation: "1",
          contentDigest: `sha256:${"0".repeat(64)}`,
          byteCount: 1,
        },
        contentDigest: `sha256:${"0".repeat(64)}`,
        byteCount: 1,
        retainUntil: "2026-08-23T12:00:00.000Z",
        reused: false,
      }),
    ).toThrowError(CloudComputerCommandArtifactError);
  });

  test("rejects output beyond the durable command budget", async () => {
    const storage = new MemoryStorage();
    await expect(
      persistCloudComputerCommandOutput({
        ...fixture(storage, new TextEncoder().encode("too-large")),
        priorCommandByteCount: 60,
      }),
    ).rejects.toBeInstanceOf(CloudComputerCommandArtifactError);
    expect(storage.creates).toBe(0);
  });

  test("detects corrupt artifact downloads", async () => {
    const storage = new MemoryStorage();
    const output = await persistCloudComputerCommandOutput(
      fixture(storage, new TextEncoder().encode("artifact-content")),
    );
    if (output.storage !== "artifact") throw new Error("expected artifact");
    storage.objects.set(output.object.objectRef, {
      object: output.object,
      bytes: new TextEncoder().encode("corrupt-content!"),
    });
    await expect(
      downloadCloudComputerCommandArtifact({ storage, object: output.object }),
    ).rejects.toMatchObject({ code: "corrupt" });
  });
});
