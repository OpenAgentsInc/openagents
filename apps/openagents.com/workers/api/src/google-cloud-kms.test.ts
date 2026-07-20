import { describe, expect, test, vi } from "vitest";

import {
  GoogleCloudKmsError,
  makeGoogleCloudKmsDekClient,
  makeGoogleCloudWorkloadIdentityAccessTokenProvider,
} from "./google-cloud-kms";

const resource =
  "projects/openagentsgemini/locations/us-central1/keyRings/portable-checkpoints/cryptoKeys/checkpoint-deks";
const b64 = (bytes: Uint8Array): string => Buffer.from(bytes).toString("base64");

describe("Google Cloud KMS DEK client", () => {
  test("uses the metadata workload identity and exact allowlisted CryptoKey", async () => {
    const calls: Array<Request> = [];
    const fetchImpl: typeof fetch = vi.fn(async (input, init) => {
      const request = new Request(input, init);
      calls.push(request);
      if (request.url.includes("metadata.google.internal")) {
        return new Response(JSON.stringify({ access_token: "workload-token-123456", expires_in: 3600 }), {
          headers: { "metadata-flavor": "Google" },
        });
      }
      return new Response(JSON.stringify({ ciphertext: b64(new Uint8Array([9, 8, 7])) }));
    });
    const tokenProvider = makeGoogleCloudWorkloadIdentityAccessTokenProvider({ fetchImpl });
    const client = makeGoogleCloudKmsDekClient({ cryptoKeyResource: resource, tokenProvider, fetchImpl });
    const dek = new Uint8Array(32).fill(4);
    const aad = new TextEncoder().encode("exact-authority");
    expect(await client.wrapDek(dek, aad)).toEqual(new Uint8Array([9, 8, 7]));
    expect(calls[0]?.headers.get("metadata-flavor")).toBe("Google");
    expect(calls[1]?.url).toBe(`https://cloudkms.googleapis.com/v1/${resource}:encrypt`);
    expect(calls[1]?.headers.get("authorization")).toBe("Bearer workload-token-123456");
    expect(await calls[1]?.json()).toEqual({
      plaintext: b64(dek),
      additionalAuthenticatedData: b64(aad),
    });
  });

  test("unwraps only an exact 32-byte plaintext response", async () => {
    const fetchImpl: typeof fetch = vi.fn(async () =>
      new Response(JSON.stringify({ plaintext: b64(new Uint8Array(32).fill(6)) })),
    );
    const client = makeGoogleCloudKmsDekClient({
      cryptoKeyResource: resource,
      tokenProvider: async () => "workload-token-123456",
      fetchImpl,
    });
    expect(await client.unwrapDek(new Uint8Array([1, 2, 3]), new Uint8Array([4]))).toEqual(
      new Uint8Array(32).fill(6),
    );

    const invalid = makeGoogleCloudKmsDekClient({
      cryptoKeyResource: resource,
      tokenProvider: async () => "workload-token-123456",
      fetchImpl: async () =>
        new Response(JSON.stringify({ plaintext: b64(new Uint8Array(31).fill(6)) })),
    });
    await expect(invalid.unwrapDek(new Uint8Array([1]), new Uint8Array([2]))).rejects.toMatchObject({
      _tag: "GoogleCloudKmsError",
      code: "invalid_response",
    });
  });

  test("fails closed for invalid resources, byte sizes, identity, and KMS refusal", async () => {
    expect(() =>
      makeGoogleCloudKmsDekClient({
        cryptoKeyResource: "projects/other/cryptoKeys/unsafe",
        tokenProvider: async () => "token",
      }),
    ).toThrow(GoogleCloudKmsError);
    const client = makeGoogleCloudKmsDekClient({
      cryptoKeyResource: resource,
      tokenProvider: async () => {
        throw new Error("metadata absent");
      },
      fetchImpl: async () => new Response(null, { status: 503 }),
    });
    await expect(client.wrapDek(new Uint8Array(31), new Uint8Array([1]))).rejects.toMatchObject({
      code: "invalid_input",
    });
    await expect(client.wrapDek(new Uint8Array(32), new Uint8Array([1]))).rejects.toMatchObject({
      code: "identity_unavailable",
    });
  });
});
