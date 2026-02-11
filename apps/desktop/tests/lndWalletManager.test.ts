import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import {
  LndWalletManagerError,
  LndWalletManagerService,
  projectLndWalletSnapshotForRenderer,
} from "../src/main/lndWalletManager";
import { LndRuntimeManagerService } from "../src/main/lndRuntimeManager";
import { makeLndWalletHarness } from "./support/lndWalletHarness";

describe("lnd wallet manager", () => {
  it.effect("handles wallet lifecycle state transitions", () => {
    const harness = makeLndWalletHarness();

    return Effect.gen(function* () {
      const runtime = yield* LndRuntimeManagerService;
      const wallet = yield* LndWalletManagerService;

      yield* runtime.start();
      yield* wallet.bootstrap();

      const initial = yield* wallet.snapshot();
      expect(initial.walletState).toBe("uninitialized");

      yield* wallet.initializeWallet({
        passphrase: "correct horse battery staple",
      });
      const initialized = yield* wallet.snapshot();
      expect(initialized.walletState).toBe("locked");
      expect(initialized.passphraseStored).toBe(true);
      expect(initialized.recoveryState).toBe("seed_backup_pending");

      yield* wallet.unlockWallet();
      const unlocked = yield* wallet.snapshot();
      expect(unlocked.walletState).toBe("unlocked");

      yield* wallet.lockWallet();
      const relocked = yield* wallet.snapshot();
      expect(relocked.walletState).toBe("locked");

      yield* runtime.stop();
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });

  it.effect("does not project secrets across renderer-safe wallet snapshot", () => {
    const harness = makeLndWalletHarness();
    const passphrase = "super-secret-wallet-passphrase";

    return Effect.gen(function* () {
      const runtime = yield* LndRuntimeManagerService;
      const wallet = yield* LndWalletManagerService;

      yield* runtime.start();
      yield* wallet.bootstrap();
      yield* wallet.initializeWallet({ passphrase });
      yield* wallet.unlockWallet({ passphrase });

      const snapshot = yield* wallet.snapshot();
      const projected = projectLndWalletSnapshotForRenderer(snapshot);
      const serialized = JSON.stringify(projected);

      expect(serialized.includes(passphrase)).toBe(false);
      expect(projected.passphraseStored).toBe(true);
      expect(projected.walletState).toBe("unlocked");

      yield* runtime.stop();
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });

  it.effect("supports explicit recovery path with typed failures", () => {
    const harness = makeLndWalletHarness();

    return Effect.gen(function* () {
      const runtime = yield* LndRuntimeManagerService;
      const wallet = yield* LndWalletManagerService;

      yield* runtime.start();
      yield* wallet.bootstrap();

      yield* wallet.initializeWallet({
        passphrase: "initial wallet passphrase",
      });

      const preAck = yield* wallet.snapshot();
      expect(preAck.recoveryState).toBe("seed_backup_pending");

      yield* wallet.acknowledgeSeedBackup();
      const acknowledged = yield* wallet.snapshot();
      expect(acknowledged.seedBackupAcknowledged).toBe(true);
      expect(acknowledged.recoveryState).toBe("seed_backup_acknowledged");

      const restoreWithoutPrepare = yield* Effect.either(
        wallet.restoreWallet({
          passphrase: "restored passphrase",
          seedMnemonic: new Array(12).fill("seedword"),
        }),
      );
      expect(restoreWithoutPrepare._tag).toBe("Left");
      if (restoreWithoutPrepare._tag === "Left") {
        expect(restoreWithoutPrepare.left).toBeInstanceOf(LndWalletManagerError);
        expect((restoreWithoutPrepare.left as LndWalletManagerError).code).toBe("restore_not_prepared");
      }

      yield* wallet.prepareRestore();
      const restorePrepared = yield* wallet.snapshot();
      expect(restorePrepared.restorePrepared).toBe(true);
      expect(restorePrepared.recoveryState).toBe("restore_ready");

      const invalidSeed = yield* Effect.either(
        wallet.restoreWallet({
          passphrase: "restored passphrase",
          seedMnemonic: ["too", "short"],
        }),
      );
      expect(invalidSeed._tag).toBe("Left");
      if (invalidSeed._tag === "Left") {
        expect(invalidSeed.left).toBeInstanceOf(LndWalletManagerError);
        expect((invalidSeed.left as LndWalletManagerError).code).toBe("invalid_seed_phrase");
      }

      yield* wallet.restoreWallet({
        passphrase: "restored passphrase",
        seedMnemonic: new Array(12).fill("seedword"),
      });

      const restored = yield* wallet.snapshot();
      expect(restored.walletState).toBe("locked");
      expect(restored.recoveryState).toBe("restored");
      expect(restored.seedBackupAcknowledged).toBe(true);
      expect(restored.restorePrepared).toBe(false);

      yield* runtime.stop();
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });
});
