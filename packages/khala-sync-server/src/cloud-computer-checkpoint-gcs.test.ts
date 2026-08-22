import { describe, expect, test } from "vite-plus/test";

import type { CloudComputerGcsObject } from "./cloud-computer-checkpoint.js";
import {
  GoogleCloudStorageCheckpoint,
  assertGoogleCloudStorageDeletionVerification,
  type CloudComputerGcsHttpRequest,
  type CloudComputerGcsHttpResponse,
  type CloudComputerGcsResumableUpload,
} from "./cloud-computer-checkpoint-gcs.js";

const digest = `sha256:${"a".repeat(64)}` as const;
const objectRef = `checkpoints/${"b".repeat(64)}/${"a".repeat(64)}`;
const sessionRef = "https://upload.example.test/session/opaque-secret";

const metadata = (state: "committed" | "tombstoned" = "committed") => ({
  name: objectRef,
  generation: "1700000000000001",
  size: "4",
  metadata: { checkpointState: state, ciphertextDigest: digest },
});

const response = (
  status: number,
  body: unknown = null,
  headers: Readonly<Record<string, string>> = {},
): CloudComputerGcsHttpResponse => ({ status, body, headers });

const harness = (
  responses: CloudComputerGcsHttpResponse[],
  initialUpload: CloudComputerGcsResumableUpload | null = null,
) => {
  const requests: CloudComputerGcsHttpRequest[] = [];
  let durableUpload = initialUpload;
  const storage = new GoogleCloudStorageCheckpoint({
    bucket: "private-checkpoints",
    authorizationHeaders: async () => ({ authorization: "Bearer test-token" }),
    http: {
      request: async (request) => {
        requests.push(request);
        const next = responses.shift();
        if (next === undefined)
          throw new Error(`unexpected request: ${request.method} ${request.url}`);
        return next;
      },
    },
    resumableUploads: {
      load: async (ref) => (durableUpload?.objectRef === ref ? durableUpload : null),
      save: async (upload) => {
        durableUpload = upload;
      },
      remove: async (ref, session) => {
        if (durableUpload?.objectRef === ref && durableUpload.sessionRef === session) {
          durableUpload = null;
        }
      },
    },
    jsonApiBaseUrl: "https://storage.example.test",
    uploadApiBaseUrl: "https://upload.example.test",
  });
  return { storage, requests, durableUpload: () => durableUpload };
};

const committedObject = (): CloudComputerGcsObject => ({
  objectRef,
  generation: "1700000000000001",
  byteCount: 4,
  ciphertextDigest: digest,
  state: "committed",
});

describe("GoogleCloudStorageCheckpoint", () => {
  test("starts a create-only resumable upload and finalizes the returned generation", async () => {
    const { storage, requests, durableUpload } = harness([
      response(201, null, { Location: sessionRef }),
      response(200, metadata()),
    ]);

    const started = await storage.startUpload({
      objectRef,
      byteCount: 4,
      ciphertextDigest: digest,
      metadata: { manifestDigest: `sha256:${"c".repeat(64)}` },
    });
    expect(started).toEqual({ sessionRef, committedBytes: 0 });
    expect(durableUpload()).toMatchObject({ objectRef, sessionRef, byteCount: 4 });

    const progress = await storage.appendUpload({
      sessionRef,
      offset: 0,
      bytes: Uint8Array.from([1, 2, 3, 4]),
    });
    expect(progress).toEqual({ committedBytes: 4 });
    await expect(
      storage.finalizeUpload({ sessionRef, byteCount: 4, ciphertextDigest: digest }),
    ).resolves.toEqual(committedObject());
    expect(durableUpload()).toBeNull();

    expect(requests[0]?.url).toContain("uploadType=resumable");
    expect(requests[0]?.url).toContain("ifGenerationMatch=0");
    expect(requests[0]?.headers?.authorization).toBe("Bearer test-token");
    expect(requests[1]?.headers?.["content-range"]).toBe("bytes 0-3/4");
  });

  test("restores a durable session and queries its committed offset", async () => {
    const upload = { objectRef, sessionRef, byteCount: 4, ciphertextDigest: digest };
    const { storage, requests } = harness(
      [response(404), response(308, null, { Range: "bytes=0-1" })],
      upload,
    );

    await expect(storage.inspectUpload(objectRef)).resolves.toEqual({
      state: "partial",
      sessionRef,
      committedBytes: 2,
    });
    expect(requests[1]).toMatchObject({
      method: "PUT",
      url: sessionRef,
      headers: expect.objectContaining({ "content-range": "bytes */4" }),
    });
  });

  test("pins downloads to the committed object generation", async () => {
    const bytes = Uint8Array.from([1, 2, 3, 4]);
    const { storage, requests } = harness([response(200, bytes)]);

    await expect(storage.download(committedObject())).resolves.toEqual(bytes);
    expect(requests[0]?.url).toContain("alt=media&generation=1700000000000001");
  });

  test("tombstones with a generation precondition", async () => {
    const { storage, requests } = harness([response(200, metadata("tombstoned"))]);

    await expect(
      storage.tombstone(committedObject(), "deletion.evidence.1"),
    ).resolves.toMatchObject({
      state: "tombstoned",
      generation: "1700000000000001",
    });
    expect(requests[0]?.url).toContain("ifGenerationMatch=1700000000000001");
    expect(requests[0]?.body).toMatchObject({
      metadata: { checkpointState: "tombstoned", deletionEvidenceRef: "deletion.evidence.1" },
    });
  });

  test("deletes one generation only after independent exact, head, and version checks", async () => {
    const { storage, requests } = harness([
      response(204),
      response(404),
      response(404),
      response(200, {
        items: [{ name: "checkpoints/unrelated", generation: "7" }],
        nextPageToken: "next",
      }),
      response(200, {}),
    ]);

    const verification = await storage.deleteGeneration(committedObject());
    expect(verification).toEqual({
      schema: "openagents.cloud_computer_gcs_deletion_verification.v1",
      objectRef,
      objectGeneration: "1700000000000001",
      generationPreconditionMet: true,
      allVersionsAbsent: true,
    });
    expect(() => assertGoogleCloudStorageDeletionVerification(verification)).not.toThrow();
    expect(requests[0]?.url).toContain("generation=1700000000000001");
    expect(requests[0]?.url).toContain("ifGenerationMatch=1700000000000001");
    expect(requests[1]?.url).toContain("?generation=1700000000000001");
    expect(requests[2]?.url.endsWith(encodeURIComponent(objectRef))).toBe(true);
    expect(requests[3]?.url).toContain("versions=true");
    expect(requests[4]?.url).toContain("pageToken=next");
  });

  test("fails deletion verification when any live version remains", async () => {
    const { storage } = harness([
      response(204),
      response(404),
      response(404),
      response(200, { items: [{ name: objectRef, generation: "1700000000000002" }] }),
    ]);

    await expect(storage.deleteGeneration(committedObject())).rejects.toMatchObject({
      code: "object_conflict",
      field: objectRef,
    });
  });

  test("rejects structurally valid deletion verification forged by a caller", () => {
    expect(() =>
      assertGoogleCloudStorageDeletionVerification({
        schema: "openagents.cloud_computer_gcs_deletion_verification.v1",
        objectRef,
        objectGeneration: "1700000000000001",
        generationPreconditionMet: true,
        allVersionsAbsent: true,
      }),
    ).toThrowError(/integrity_mismatch/u);
  });
});
