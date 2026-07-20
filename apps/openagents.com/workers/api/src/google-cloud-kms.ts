import { Schema } from "effect";

import { parseJsonRecord } from "./json-boundary";

const CRYPTO_KEY_RESOURCE =
  /^projects\/[a-z][a-z0-9-]{4,62}\/locations\/[a-z0-9-]+\/keyRings\/[A-Za-z0-9_-]{1,63}\/cryptoKeys\/[A-Za-z0-9_-]{1,63}$/u;
const MAX_WRAPPED_DEK_BYTES = 128 * 1024;
const MAX_AAD_BYTES = 16 * 1024;
const METADATA_TOKEN_URL =
  "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

export class GoogleCloudKmsError extends Schema.TaggedErrorClass<GoogleCloudKmsError>()(
  "GoogleCloudKmsError",
  {
    code: Schema.Literals([
      "invalid_configuration",
      "invalid_input",
      "identity_unavailable",
      "kms_refused",
      "invalid_response",
    ]),
  },
) {}

export type GoogleCloudAccessTokenProvider = () => Promise<string>;

const encodeBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
};

const decodeBase64 = (value: unknown, maximumBytes: number): Uint8Array => {
  if (
    typeof value !== "string" ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)
  ) {
    throw new GoogleCloudKmsError({ code: "invalid_response" });
  }
  let binary: string;
  try {
    binary = atob(value);
  } catch {
    throw new GoogleCloudKmsError({ code: "invalid_response" });
  }
  if (binary.length === 0 || binary.length > maximumBytes) {
    throw new GoogleCloudKmsError({ code: "invalid_response" });
  }
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  if (encodeBase64(bytes) !== value) {
    bytes.fill(0);
    throw new GoogleCloudKmsError({ code: "invalid_response" });
  }
  return bytes;
};

export const makeGoogleCloudWorkloadIdentityAccessTokenProvider = (
  options: Readonly<{
    fetchImpl?: typeof fetch | undefined;
    now?: (() => Date) | undefined;
  }> = {},
): GoogleCloudAccessTokenProvider => {
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const now = options.now ?? (() => new Date());
  let cached: Readonly<{ token: string; expiresAt: number }> | undefined;
  return async () => {
    if (cached !== undefined && cached.expiresAt - 60_000 > now().getTime()) {
      return cached.token;
    }
    let response: Response;
    try {
      response = await fetchImpl(METADATA_TOKEN_URL, {
        headers: { "metadata-flavor": "Google" },
        signal: AbortSignal.timeout(5_000),
      });
    } catch {
      throw new GoogleCloudKmsError({ code: "identity_unavailable" });
    }
    if (!response.ok || response.headers.get("metadata-flavor") !== "Google") {
      throw new GoogleCloudKmsError({ code: "identity_unavailable" });
    }
    const body = parseJsonRecord(await response.text());
    const token = body?.["access_token"];
    const expiresIn = body?.["expires_in"];
    if (
      typeof token !== "string" ||
      token.length < 16 ||
      token.length > 8192 ||
      typeof expiresIn !== "number" ||
      !Number.isSafeInteger(expiresIn) ||
      expiresIn < 60 ||
      expiresIn > 86_400
    ) {
      throw new GoogleCloudKmsError({ code: "identity_unavailable" });
    }
    cached = { token, expiresAt: now().getTime() + expiresIn * 1_000 };
    return token;
  };
};

export type GoogleCloudKmsDekClient = Readonly<{
  wrapDek: (dek: Uint8Array, aad: Uint8Array) => Promise<Uint8Array>;
  unwrapDek: (wrappedDek: Uint8Array, aad: Uint8Array) => Promise<Uint8Array>;
}>;

export const makeGoogleCloudKmsDekClient = (config: Readonly<{
  cryptoKeyResource: string;
  tokenProvider: GoogleCloudAccessTokenProvider;
  fetchImpl?: typeof fetch | undefined;
}>): GoogleCloudKmsDekClient => {
  if (
    !CRYPTO_KEY_RESOURCE.test(config.cryptoKeyResource) ||
    typeof config.tokenProvider !== "function"
  ) {
    throw new GoogleCloudKmsError({ code: "invalid_configuration" });
  }
  const fetchImpl = config.fetchImpl ?? globalThis.fetch;
  const call = async (
    operation: "encrypt" | "decrypt",
    bytes: Uint8Array,
    aad: Uint8Array,
  ): Promise<Uint8Array> => {
    if (
      aad.byteLength === 0 ||
      aad.byteLength > MAX_AAD_BYTES ||
      (operation === "encrypt" && bytes.byteLength !== 32) ||
      (operation === "decrypt" &&
        (bytes.byteLength === 0 || bytes.byteLength > MAX_WRAPPED_DEK_BYTES))
    ) {
      throw new GoogleCloudKmsError({ code: "invalid_input" });
    }
    let token: string;
    try {
      token = await config.tokenProvider();
    } catch {
      throw new GoogleCloudKmsError({ code: "identity_unavailable" });
    }
    const requestBody =
      operation === "encrypt"
        ? { plaintext: encodeBase64(bytes), additionalAuthenticatedData: encodeBase64(aad) }
        : { ciphertext: encodeBase64(bytes), additionalAuthenticatedData: encodeBase64(aad) };
    let response: Response;
    try {
      response = await fetchImpl(
        `https://cloudkms.googleapis.com/v1/${config.cryptoKeyResource}:${operation}`,
        {
          method: "POST",
          headers: {
            authorization: `Bearer ${token}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(requestBody),
          signal: AbortSignal.timeout(10_000),
        },
      );
    } catch {
      throw new GoogleCloudKmsError({ code: "kms_refused" });
    }
    if (!response.ok) throw new GoogleCloudKmsError({ code: "kms_refused" });
    const body = parseJsonRecord(await response.text());
    const output = decodeBase64(
      operation === "encrypt" ? body?.["ciphertext"] : body?.["plaintext"],
      operation === "encrypt" ? MAX_WRAPPED_DEK_BYTES : 32,
    );
    if (operation === "decrypt" && output.byteLength !== 32) {
      output.fill(0);
      throw new GoogleCloudKmsError({ code: "invalid_response" });
    }
    return output;
  };
  return {
    wrapDek: (dek, aad) => call("encrypt", dek, aad),
    unwrapDek: (wrappedDek, aad) => call("decrypt", wrappedDek, aad),
  };
};
