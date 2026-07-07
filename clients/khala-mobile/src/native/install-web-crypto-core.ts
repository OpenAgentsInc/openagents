/**
 * Pure Web Crypto shim installer, split out from the `expo-crypto`-importing
 * side-effect module so it can be unit-tested without the native module.
 *
 * React Native's Hermes engine ships no `globalThis.crypto`; dependencies that
 * reach for Web Crypto (e.g. `@tanstack/db`'s optimistic-transaction
 * `safeRandomUUID`) then throw. This fills in only the missing methods on the
 * target's `crypto` object, creating that object if absent, and never
 * overwrites an already-present real implementation.
 */
export type WebCryptoImpl = Readonly<{
  getRandomValues: <T>(typedArray: T) => T
  randomUUID: () => string
}>

type CryptoBag = Record<string, unknown>
type CryptoHost = { crypto?: CryptoBag }

export const applyWebCryptoShim = (
  target: CryptoHost,
  impl: WebCryptoImpl,
): void => {
  let cryptoObject = target.crypto
  if (cryptoObject === undefined || cryptoObject === null) {
    cryptoObject = {}
    try {
      target.crypto = cryptoObject
    } catch {
      try {
        Object.defineProperty(target, "crypto", {
          configurable: true,
          value: cryptoObject,
          writable: true,
        })
      } catch {
        return
      }
    }
  }

  if (typeof cryptoObject.getRandomValues !== "function") {
    try {
      cryptoObject.getRandomValues = impl.getRandomValues as unknown as () => unknown
    } catch {
      // read-only property on an existing native crypto; leave intact.
    }
  }

  if (typeof cryptoObject.randomUUID !== "function") {
    try {
      cryptoObject.randomUUID = impl.randomUUID as unknown as () => string
    } catch {
      // read-only property on an existing native crypto; leave intact.
    }
  }
}
