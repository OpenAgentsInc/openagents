/**
 * IDR-06 — the ISOLATED secret-export custody module.
 *
 * This is the ONLY module in `sovereign-identity` that reaches the `nostr-effect`
 * key-export escape hatches (`exportPrivateKeyBytes` / `exportNsec`). It is
 * DELIBERATELY NOT re-exported from `machinery/index.ts` or the package root
 * `index.ts`, so a normal Pylon or Desktop caller cannot import it through the
 * package barrel. Only a custody or recovery composition root imports it by this
 * explicit deep path, under owner authority, to move the root into the platform
 * secret store (IDR-05) or to run an attended recovery.
 *
 * The static secret-export boundary test (`../secret-export-boundary.test.ts`)
 * proves that normal caller source (`packages/pylon-core`,
 * `apps/openagents-desktop`) never imports this module or references the
 * escape-hatch symbols.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect, Layer } from "effect";
import { IdentityKeys } from "nostr-effect/identity";
import { CustodyKeyExport, type CustodyKeyExportInterface, SignerError } from "./signer.ts";

/**
 * Build the custody key-export surface for one mnemonic. It exposes the two
 * secret-returning operations reserved for custody import and attended recovery.
 * The returned `Uint8Array` and `nsec` MUST be handled inside the smallest
 * possible scope and never logged, cached, or placed in a process-wide object.
 */
export const makeCustodyKeyExport = (mnemonic: string): CustodyKeyExportInterface => {
  const keys = IdentityKeys.fromOpenAgentsLegacyMnemonic(mnemonic);
  return {
    exportPrivateKeyBytes: () =>
      Effect.try({
        try: () => keys.exportPrivateKeyBytes(),
        catch: () => new SignerError({ reason: "unavailable" }),
      }),
    exportNsec: () =>
      Effect.try({
        try: () => keys.exportNsec(),
        catch: () => new SignerError({ reason: "unavailable" }),
      }),
  };
};

/**
 * The custody key-export layer for one mnemonic. A normal caller must never
 * provide or resolve this layer; only a custody/recovery root does, under owner
 * authority.
 */
export const custodyKeyExportLayer = (mnemonic: string): Layer.Layer<CustodyKeyExport> =>
  Layer.succeed(CustodyKeyExport, CustodyKeyExport.of(makeCustodyKeyExport(mnemonic)));
