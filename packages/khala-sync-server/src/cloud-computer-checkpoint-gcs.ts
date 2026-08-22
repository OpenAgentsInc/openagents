import {
  CloudComputerCheckpointError,
  type CloudComputerCheckpointGcs,
  type CloudComputerGcsDeletionVerification,
  type CloudComputerGcsObject,
  type CloudComputerGcsUploadState,
  type Sha256Digest,
} from "./cloud-computer-checkpoint.js";

export type CloudComputerGcsHttpRequest = Readonly<{
  method: "DELETE" | "GET" | "PATCH" | "POST" | "PUT";
  url: string;
  headers?: Readonly<Record<string, string>>;
  body?: Readonly<Record<string, unknown>> | Uint8Array;
}>;

export type CloudComputerGcsHttpResponse = Readonly<{
  status: number;
  headers: Readonly<Record<string, string | undefined>>;
  body: unknown;
}>;

/** A small HTTP seam that production can implement with Req and tests can fake. */
export type CloudComputerGcsHttpPort = Readonly<{
  request: (request: CloudComputerGcsHttpRequest) => Promise<CloudComputerGcsHttpResponse>;
}>;

export type CloudComputerGcsResumableUpload = Readonly<{
  objectRef: string;
  sessionRef: string;
  byteCount: number;
  ciphertextDigest: Sha256Digest;
}>;

export type CloudComputerGcsResumableUploadStore = Readonly<{
  load: (objectRef: string) => Promise<CloudComputerGcsResumableUpload | null>;
  save: (upload: CloudComputerGcsResumableUpload) => Promise<void>;
  remove: (objectRef: string, sessionRef: string) => Promise<void>;
}>;

export type GoogleCloudStorageCheckpointConfig = Readonly<{
  bucket: string;
  http: CloudComputerGcsHttpPort;
  authorizationHeaders: () => Promise<Readonly<Record<string, string>>>;
  resumableUploads: CloudComputerGcsResumableUploadStore;
  jsonApiBaseUrl?: string;
  uploadApiBaseUrl?: string;
}>;

type GcsObjectMetadata = Readonly<{
  name?: unknown;
  generation?: unknown;
  size?: unknown;
  metadata?: unknown;
}>;

type TrackedUpload = CloudComputerGcsResumableUpload &
  Readonly<{ finalizedObject?: CloudComputerGcsObject }>;

const verifiedDeletionResults = new WeakSet<object>();

/** Rejects deletion evidence that did not originate from this GCS adapter module. */
export function assertGoogleCloudStorageDeletionVerification(
  value: unknown,
): asserts value is CloudComputerGcsDeletionVerification {
  if (typeof value !== "object" || value === null || !verifiedDeletionResults.has(value)) {
    throw new CloudComputerCheckpointError("integrity_mismatch", "gcs.deletionVerification");
  }
}

const header = (headers: Readonly<Record<string, string | undefined>>, name: string) => {
  const lowerName = name.toLowerCase();
  const entry = Object.entries(headers).find(([key]) => key.toLowerCase() === lowerName);
  return entry?.[1];
};

const encodePath = (value: string): string => encodeURIComponent(value);

const integerString = (value: unknown): string | null =>
  typeof value === "string" && /^\d+$/u.test(value) ? value : null;

const parseCommittedBytes = (response: CloudComputerGcsHttpResponse, byteCount: number): number => {
  if (response.status === 200 || response.status === 201) return byteCount;
  if (response.status !== 308) {
    throw new CloudComputerCheckpointError("object_unavailable", "gcs.resumableUpload");
  }
  const range = header(response.headers, "range");
  if (range === undefined) return 0;
  const match = /^bytes=0-(\d+)$/u.exec(range);
  if (match === null) {
    throw new CloudComputerCheckpointError("object_conflict", "gcs.uploadRange");
  }
  const lastByte = Number(match[1]);
  if (!Number.isSafeInteger(lastByte) || lastByte + 1 > byteCount) {
    throw new CloudComputerCheckpointError("upload_regressed", "gcs.uploadRange");
  }
  return lastByte + 1;
};

const objectFromMetadata = (
  body: unknown,
  expected: Readonly<{
    objectRef: string;
    byteCount?: number;
    ciphertextDigest?: Sha256Digest;
    generation?: string;
    state?: "committed" | "tombstoned";
  }>,
): CloudComputerGcsObject => {
  if (body === null || typeof body !== "object" || Array.isArray(body)) {
    throw new CloudComputerCheckpointError("object_conflict", expected.objectRef);
  }
  const metadata = body as GcsObjectMetadata;
  const generation = integerString(metadata.generation);
  const sizeText = integerString(metadata.size);
  const custom =
    metadata.metadata !== null && typeof metadata.metadata === "object"
      ? (metadata.metadata as Readonly<Record<string, unknown>>)
      : {};
  const digest = custom.ciphertextDigest;
  const state = custom.checkpointState;
  if (
    metadata.name !== expected.objectRef ||
    generation === null ||
    sizeText === null ||
    typeof digest !== "string" ||
    !/^sha256:[0-9a-f]{64}$/u.test(digest) ||
    (expected.generation !== undefined && generation !== expected.generation) ||
    (expected.byteCount !== undefined && Number(sizeText) !== expected.byteCount) ||
    (expected.ciphertextDigest !== undefined && digest !== expected.ciphertextDigest)
  ) {
    throw new CloudComputerCheckpointError("object_conflict", expected.objectRef);
  }
  return {
    objectRef: expected.objectRef,
    generation,
    byteCount: Number(sizeText),
    ciphertextDigest: digest as Sha256Digest,
    state: expected.state ?? (state === "tombstoned" ? "tombstoned" : "committed"),
  };
};

/**
 * GCS JSON API adapter with generation-pinned reads and deletes.
 *
 * The injected resumable-upload store must be durable. GCS does not expose an
 * unfinished resumable session by object name after a process restart.
 */
export class GoogleCloudStorageCheckpoint implements CloudComputerCheckpointGcs {
  readonly #jsonApiBaseUrl: string;
  readonly #uploadApiBaseUrl: string;
  readonly #uploads = new Map<string, TrackedUpload>();

  constructor(private readonly config: GoogleCloudStorageCheckpointConfig) {
    if (config.bucket.length === 0) {
      throw new CloudComputerCheckpointError("invalid_manifest", "gcs.bucket");
    }
    this.#jsonApiBaseUrl = (config.jsonApiBaseUrl ?? "https://storage.googleapis.com").replace(
      /\/$/u,
      "",
    );
    this.#uploadApiBaseUrl = (config.uploadApiBaseUrl ?? "https://storage.googleapis.com").replace(
      /\/$/u,
      "",
    );
  }

  async #request(
    request: Omit<CloudComputerGcsHttpRequest, "headers"> & {
      headers?: Readonly<Record<string, string>>;
    },
  ): Promise<CloudComputerGcsHttpResponse> {
    return this.config.http.request({
      ...request,
      headers: { ...(await this.config.authorizationHeaders()), ...request.headers },
    });
  }

  #metadataUrl(objectRef: string, query = ""): string {
    return `${this.#jsonApiBaseUrl}/storage/v1/b/${encodePath(this.config.bucket)}/o/${encodePath(objectRef)}${query}`;
  }

  async #inspectObject(objectRef: string): Promise<CloudComputerGcsObject | null> {
    const response = await this.#request({ method: "GET", url: this.#metadataUrl(objectRef) });
    if (response.status === 404) return null;
    if (response.status !== 200) {
      throw new CloudComputerCheckpointError("object_unavailable", objectRef);
    }
    return objectFromMetadata(response.body, { objectRef });
  }

  async inspectUpload(objectRef: string): Promise<CloudComputerGcsUploadState> {
    const object = await this.#inspectObject(objectRef);
    if (object !== null) return { state: "committed", object };

    const upload =
      this.#uploads.get(objectRef) ?? (await this.config.resumableUploads.load(objectRef));
    if (upload === null || upload === undefined) return { state: "missing" };
    this.#uploads.set(objectRef, upload);
    const response = await this.#request({
      method: "PUT",
      url: upload.sessionRef,
      headers: {
        "content-length": "0",
        "content-range": `bytes */${upload.byteCount}`,
      },
      body: new Uint8Array(),
    });
    const committedBytes = parseCommittedBytes(response, upload.byteCount);
    if (response.status === 200 || response.status === 201) {
      const finalizedObject = objectFromMetadata(response.body, upload);
      this.#uploads.set(objectRef, { ...upload, finalizedObject });
      await this.config.resumableUploads.remove(objectRef, upload.sessionRef);
      return { state: "committed", object: finalizedObject };
    }
    return { state: "partial", sessionRef: upload.sessionRef, committedBytes };
  }

  async startUpload(input: {
    objectRef: string;
    byteCount: number;
    ciphertextDigest: Sha256Digest;
    metadata: Readonly<Record<string, string>>;
  }): Promise<Readonly<{ sessionRef: string; committedBytes: number }>> {
    const response = await this.#request({
      method: "POST",
      url: `${this.#uploadApiBaseUrl}/upload/storage/v1/b/${encodePath(this.config.bucket)}/o?uploadType=resumable&name=${encodePath(input.objectRef)}&ifGenerationMatch=0`,
      headers: {
        "content-type": "application/json; charset=utf-8",
        "x-upload-content-length": String(input.byteCount),
        "x-upload-content-type": "application/octet-stream",
      },
      body: {
        name: input.objectRef,
        metadata: {
          ...input.metadata,
          checkpointState: "committed",
          ciphertextDigest: input.ciphertextDigest,
        },
      },
    });
    const sessionRef = header(response.headers, "location");
    if ((response.status !== 200 && response.status !== 201) || sessionRef === undefined) {
      throw new CloudComputerCheckpointError("object_unavailable", input.objectRef);
    }
    const upload = { ...input, sessionRef };
    await this.config.resumableUploads.save(upload);
    this.#uploads.set(input.objectRef, upload);
    return { sessionRef, committedBytes: 0 };
  }

  async appendUpload(input: {
    sessionRef: string;
    offset: number;
    bytes: Uint8Array;
  }): Promise<Readonly<{ committedBytes: number }>> {
    const upload = [...this.#uploads.values()].find(
      (candidate) => candidate.sessionRef === input.sessionRef,
    );
    if (
      upload === undefined ||
      input.offset < 0 ||
      input.offset + input.bytes.byteLength > upload.byteCount
    ) {
      throw new CloudComputerCheckpointError("object_conflict", "gcs.resumableUpload");
    }
    if (input.bytes.byteLength === 0) return { committedBytes: input.offset };
    const end = input.offset + input.bytes.byteLength - 1;
    const response = await this.#request({
      method: "PUT",
      url: input.sessionRef,
      headers: {
        "content-length": String(input.bytes.byteLength),
        "content-range": `bytes ${input.offset}-${end}/${upload.byteCount}`,
        "content-type": "application/octet-stream",
      },
      body: input.bytes,
    });
    const committedBytes = parseCommittedBytes(response, upload.byteCount);
    if (committedBytes < input.offset) {
      throw new CloudComputerCheckpointError("upload_regressed", upload.objectRef);
    }
    if (response.status === 200 || response.status === 201) {
      const finalizedObject = objectFromMetadata(response.body, upload);
      this.#uploads.set(upload.objectRef, { ...upload, finalizedObject });
      await this.config.resumableUploads.remove(upload.objectRef, upload.sessionRef);
    }
    return { committedBytes };
  }

  async finalizeUpload(input: {
    sessionRef: string;
    byteCount: number;
    ciphertextDigest: Sha256Digest;
  }): Promise<CloudComputerGcsObject> {
    const upload = [...this.#uploads.values()].find(
      (candidate) => candidate.sessionRef === input.sessionRef,
    );
    if (
      upload === undefined ||
      upload.byteCount !== input.byteCount ||
      upload.ciphertextDigest !== input.ciphertextDigest
    ) {
      throw new CloudComputerCheckpointError("object_conflict", "gcs.resumableUpload");
    }
    if (upload.finalizedObject !== undefined) return upload.finalizedObject;
    const state = await this.inspectUpload(upload.objectRef);
    if (state.state !== "committed") {
      throw new CloudComputerCheckpointError("upload_incomplete", upload.objectRef);
    }
    return state.object;
  }

  async download(object: CloudComputerGcsObject): Promise<Uint8Array> {
    const response = await this.#request({
      method: "GET",
      url: `${this.#jsonApiBaseUrl}/download/storage/v1/b/${encodePath(this.config.bucket)}/o/${encodePath(object.objectRef)}?alt=media&generation=${encodePath(object.generation)}`,
    });
    if (response.status !== 200 || !(response.body instanceof Uint8Array)) {
      throw new CloudComputerCheckpointError("object_unavailable", object.objectRef);
    }
    return response.body;
  }

  async tombstone(
    object: CloudComputerGcsObject,
    evidenceRef: string,
  ): Promise<CloudComputerGcsObject> {
    const response = await this.#request({
      method: "PATCH",
      url: this.#metadataUrl(
        object.objectRef,
        `?generation=${encodePath(object.generation)}&ifGenerationMatch=${encodePath(object.generation)}`,
      ),
      headers: { "content-type": "application/json; charset=utf-8" },
      body: {
        metadata: {
          checkpointState: "tombstoned",
          ciphertextDigest: object.ciphertextDigest,
          deletionEvidenceRef: evidenceRef,
        },
      },
    });
    if (response.status !== 200) {
      throw new CloudComputerCheckpointError("object_unavailable", object.objectRef);
    }
    return objectFromMetadata(response.body, { ...object, state: "tombstoned" });
  }

  async deleteGeneration(
    object: CloudComputerGcsObject,
  ): Promise<CloudComputerGcsDeletionVerification> {
    const deleteResponse = await this.#request({
      method: "DELETE",
      url: this.#metadataUrl(
        object.objectRef,
        `?generation=${encodePath(object.generation)}&ifGenerationMatch=${encodePath(object.generation)}`,
      ),
    });
    if (deleteResponse.status !== 204 && deleteResponse.status !== 404) {
      throw new CloudComputerCheckpointError("object_unavailable", object.objectRef);
    }
    await this.#assertGenerationAbsent(object);
    const verification: CloudComputerGcsDeletionVerification = Object.freeze({
      schema: "openagents.cloud_computer_gcs_deletion_verification.v1",
      objectRef: object.objectRef,
      objectGeneration: object.generation,
      generationPreconditionMet: true,
      allVersionsAbsent: true,
    });
    verifiedDeletionResults.add(verification);
    return verification;
  }

  async #assertGenerationAbsent(object: CloudComputerGcsObject): Promise<void> {
    const exact = await this.#request({
      method: "GET",
      url: this.#metadataUrl(object.objectRef, `?generation=${encodePath(object.generation)}`),
    });
    if (exact.status !== 404) {
      throw new CloudComputerCheckpointError("object_conflict", object.objectRef);
    }
    const head = await this.#request({ method: "GET", url: this.#metadataUrl(object.objectRef) });
    if (head.status !== 404) {
      throw new CloudComputerCheckpointError("object_conflict", object.objectRef);
    }

    await this.#assertNoVersionPage(object.objectRef, null);
  }

  async #assertNoVersionPage(objectRef: string, pageToken: string | null): Promise<void> {
    const query = new URLSearchParams({ prefix: objectRef, versions: "true" });
    if (pageToken !== null) query.set("pageToken", pageToken);
    const response = await this.#request({
      method: "GET",
      url: `${this.#jsonApiBaseUrl}/storage/v1/b/${encodePath(this.config.bucket)}/o?${query.toString()}`,
    });
    if (response.status !== 200 || response.body === null || typeof response.body !== "object") {
      throw new CloudComputerCheckpointError("object_unavailable", objectRef);
    }
    const listing = response.body as {
      items?: ReadonlyArray<{ name?: unknown }>;
      nextPageToken?: unknown;
    };
    if ((listing.items ?? []).some((item) => item.name === objectRef)) {
      throw new CloudComputerCheckpointError("object_conflict", objectRef);
    }
    if (typeof listing.nextPageToken === "string") {
      await this.#assertNoVersionPage(objectRef, listing.nextPageToken);
    }
  }
}
