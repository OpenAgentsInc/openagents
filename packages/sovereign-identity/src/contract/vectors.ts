/**
 * IDR-00 frozen public test vectors.
 *
 * SAFETY: every value here comes from ONE published, well-known BIP-39 TEST
 * mnemonic. No value comes from a real user secret. The mnemonic below is the
 * canonical BIP-39 test phrase. Its derived identifiers are public and safe to
 * commit. The test suite re-derives these values from the mnemonic in code, so
 * they are real and deterministic, not hand-typed guesses.
 */

/**
 * The canonical published BIP-39 TEST mnemonic. It is NOT a real secret. It
 * exists only to freeze deterministic public vectors and fixtures.
 */
export const PUBLIC_TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/** Base64 of the UTF-8 test mnemonic (for the Electron insecure-mode fixture). */
export const PUBLIC_TEST_MNEMONIC_BASE64 =
  "YWJhbmRvbiBhYmFuZG9uIGFiYW5kb24gYWJhbmRvbiBhYmFuZG9uIGFiYW5kb24gYWJhbmRvbiBhYmFuZG9uIGFiYW5kb24gYWJhbmRvbiBhYmFuZG9uIGFib3V0";

/** The public identity the frozen empty-passphrase profile produces. */
export interface FrozenPublicIdentityVector {
  readonly npub: string;
  readonly nostrPublicKeyHex: string;
  readonly sparkPublicKeyHex: string;
  readonly sparkBip32FingerprintHex: string;
}

/**
 * The frozen expected public identity for `PUBLIC_TEST_MNEMONIC` under the
 * canonical EMPTY BIP-39 passphrase. `contract.test.ts` re-derives and asserts
 * every field.
 */
export const PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE: FrozenPublicIdentityVector = {
  npub: "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7",
  nostrPublicKeyHex: "e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f",
  sparkPublicKeyHex: "03aaeb52dd7494c361049de67cc680e83ebcbbbdbeb13637d92cd845f70308af5e",
  sparkBip32FingerprintHex: "d986ed01",
};

/**
 * The public identity for the SAME mnemonic under a NON-EMPTY passphrase
 * (`"TREZOR"`). It MUST differ from the empty-passphrase vector. The divergence
 * test uses it to prove that a passphrase can never be added silently.
 */
export const PUBLIC_TEST_IDENTITY_NONEMPTY_PASSPHRASE: FrozenPublicIdentityVector = {
  npub: "npub17v46v50fwtwq5rdc6xqxjznk6w2pqrk2k6u4jjcf2r5h5ua7k7hq2q3erf",
  nostrPublicKeyHex: "f32ba651e972dc0a0db8d180690a76d394100ecab6b9594b0950e97a73beb7ae",
  sparkPublicKeyHex: "027440c6c46ec617a202f44bc886a249b10f98a8ff5d8a0aa56a350ab930a0ec79",
  sparkBip32FingerprintHex: "f3ea0aba",
};

/** The non-empty passphrase used only by the divergence vector. */
export const DIVERGENCE_TEST_PASSPHRASE = "TREZOR";
