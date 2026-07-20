/**
 * IDR-00 frozen secret-store identifiers.
 *
 * The canonical shared-root secret lives in ONE local platform secret-store
 * entry. These identifiers are stable source data. A secret-store adapter
 * (IDR-05) reads and writes exactly one entry per identity with these keys. The
 * frozen contract never puts the secret payload in Git, JSON config, SQLite,
 * logs, command arguments, or cloud storage.
 */

/** The stable secret-store service name for the shared root. */
export const SECRET_STORE_SERVICE = "com.openagents.identity.root.v1";

/** Build the stable secret-store account name for one identity. */
export function secretStoreAccount(identityRef: string): `identity:${string}` {
  return `identity:${identityRef}`;
}

/**
 * The admitted secret-store locator types. Version one is local only. No locator
 * type may resolve to cloud custody or a retired Cloudflare service.
 */
export const SECRET_STORE_LOCATOR_TYPES = [
  "macos_keychain",
  "windows_credential_manager",
  "linux_secret_service",
  "ios_keychain",
  "android_keystore",
  "in_memory_test",
] as const;

export type SecretStoreLocatorType = (typeof SECRET_STORE_LOCATOR_TYPES)[number];
