/**
 * Install a Web Crypto fallback onto a scope that is missing one.
 *
 * Hermes ships no Web Crypto. Nothing in the app noticed until an issue #31
 * device tried to answer a relay's NIP-42 challenge on an iOS Simulator, and
 * two separate failures fell out of the same gap:
 *
 * - Schnorr signing needs auxiliary randomness, `@noble/curves` asks for
 *   `crypto.getRandomValues`, and the throw was swallowed by the issue #31
 *   client's `auth_failure` path. The socket closed with code 1002 and
 *   reconnected forever, so the Workroom sat on "No unexpired announcement from
 *   an out-of-band admitted Omega host has arrived yet." — a message about
 *   discovery, for a failure that had nothing to do with discovery.
 * - The owner-private send path builds its idempotency ref with
 *   `crypto.randomUUID()` on the line *before* its `try`, so the throw escaped
 *   the handler's own error branch: the composer cleared, no command intent
 *   reached the relay, and the room reported nothing at all.
 *
 * Both were invisible off-device. Node has Web Crypto, and the in-process test
 * relay never issues a challenge, so every existing suite stayed green while
 * the entire owner-private room was unreachable on a phone.
 *
 * The randomness source is a parameter so this stays testable without pulling a
 * native module into a Node test. The one caller passes `expo-crypto`, which is
 * backed by the platform CSPRNG (`SecRandomCopyBytes` / `SecureRandom`). Do not
 * pass a `Math.random` shim: that would be worse than the original crash,
 * because it produces signatures that verify on randomness an attacker can
 * predict.
 *
 * Installation is additive. A real implementation is never displaced, so this
 * is inert on any platform that already has Web Crypto.
 */

export type GetRandomValues = <T extends ArrayBufferView>(array: T) => T;
export type RandomUuid = () => `${string}-${string}-${string}-${string}-${string}`;

export interface WebCryptoScope {
  crypto?: {
    getRandomValues?: GetRandomValues;
    randomUUID?: RandomUuid;
  };
}

/** Format 16 random bytes as an RFC 4122 version 4, variant 1 UUID. */
export const uuidV4FromBytes = (bytes: Uint8Array): ReturnType<RandomUuid> => {
  if (bytes.length !== 16) throw new Error("A version 4 UUID needs exactly 16 bytes.");
  const tagged = Uint8Array.from(bytes);
  tagged[6] = ((tagged[6] as number) & 0x0f) | 0x40;
  tagged[8] = ((tagged[8] as number) & 0x3f) | 0x80;
  const hex = [...tagged].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
};

/**
 * Add `getRandomValues` and `randomUUID` to `scope.crypto`, creating the
 * namespace when the platform has none. Returns what was installed, so a caller
 * or test can tell an install from a no-op.
 */
export const installWebCryptoFallback = (
  scope: WebCryptoScope,
  getRandomValues: GetRandomValues,
): ReadonlyArray<"getRandomValues" | "randomUUID"> => {
  if (scope.crypto === undefined) scope.crypto = {};
  const target = scope.crypto;
  const installed: Array<"getRandomValues" | "randomUUID"> = [];
  if (typeof target.getRandomValues !== "function") {
    target.getRandomValues = getRandomValues;
    installed.push("getRandomValues");
  }
  if (typeof target.randomUUID !== "function") {
    target.randomUUID = () => uuidV4FromBytes(getRandomValues(new Uint8Array(16)));
    installed.push("randomUUID");
  }
  return installed;
};
