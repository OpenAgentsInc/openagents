import { safeStorage } from "electron";
import { Context, Effect, Layer } from "effect";
import fs from "node:fs";
import path from "node:path";

export type DesktopSecureStorageErrorCode =
  | "storage_unavailable"
  | "storage_read_failed"
  | "storage_write_failed"
  | "storage_decode_failed";

export class DesktopSecureStorageError extends Error {
  readonly code: DesktopSecureStorageErrorCode;

  constructor(code: DesktopSecureStorageErrorCode, message: string) {
    super(message);
    this.name = "DesktopSecureStorageError";
    this.code = code;
  }
}

export type DesktopSecureStorageConfig = Readonly<{
  readonly userDataPath: string;
  readonly allowInsecureFallback: boolean;
}>;

export const defaultDesktopSecureStorageConfig = (input: {
  readonly userDataPath: string;
  readonly env: NodeJS.ProcessEnv;
}): DesktopSecureStorageConfig => ({
  userDataPath: input.userDataPath,
  allowInsecureFallback: input.env.OA_DESKTOP_ALLOW_INSECURE_SECRET_STORAGE === "1",
});

export class DesktopSecureStorageConfigService extends Context.Tag(
  "@openagents/desktop/DesktopSecureStorageConfigService",
)<DesktopSecureStorageConfigService, DesktopSecureStorageConfig>() {}

export const DesktopSecureStorageConfigLive = (config: DesktopSecureStorageConfig) =>
  Layer.succeed(DesktopSecureStorageConfigService, config);

type PersistedSecret = Readonly<{
  readonly encrypted: boolean;
  readonly valueB64: string;
}>;

type PersistedSecureStore = Readonly<{
  readonly version: 1;
  readonly entries: Record<string, PersistedSecret>;
}>;

const emptyStore = (): PersistedSecureStore => ({
  version: 1,
  entries: {},
});

const buildStorePath = (userDataPath: string): string =>
  path.join(userDataPath, "secure", "desktop-secure-storage.json");

const parseStore = (raw: string): PersistedSecureStore => {
  const parsed = JSON.parse(raw) as Partial<PersistedSecureStore>;
  if (!parsed || parsed.version !== 1 || typeof parsed.entries !== "object" || !parsed.entries) {
    throw new Error("invalid_secure_store_shape");
  }
  return parsed as PersistedSecureStore;
};

const loadStore = (storePath: string): PersistedSecureStore => {
  if (!fs.existsSync(storePath)) return emptyStore();
  return parseStore(fs.readFileSync(storePath, "utf8"));
};

const saveStore = (storePath: string, store: PersistedSecureStore): void => {
  fs.mkdirSync(path.dirname(storePath), { recursive: true });
  fs.writeFileSync(storePath, `${JSON.stringify(store, null, 2)}\n`, "utf8");
};

const encodeSecret = (
  value: string,
  encryptionAvailable: boolean,
  allowInsecureFallback: boolean,
): PersistedSecret => {
  if (encryptionAvailable) {
    return {
      encrypted: true,
      valueB64: safeStorage.encryptString(value).toString("base64"),
    };
  }

  if (!allowInsecureFallback) {
    throw new DesktopSecureStorageError(
      "storage_unavailable",
      "OS secure encryption backend is not available",
    );
  }

  return {
    encrypted: false,
    valueB64: Buffer.from(value, "utf8").toString("base64"),
  };
};

const decodeSecret = (
  stored: PersistedSecret,
  encryptionAvailable: boolean,
  allowInsecureFallback: boolean,
): string => {
  const encoded = Buffer.from(stored.valueB64, "base64");
  if (stored.encrypted) {
    if (!encryptionAvailable) {
      throw new DesktopSecureStorageError(
        "storage_unavailable",
        "OS secure encryption backend is not available for decrypt",
      );
    }
    return safeStorage.decryptString(encoded);
  }

  if (!allowInsecureFallback) {
    throw new DesktopSecureStorageError(
      "storage_unavailable",
      "Insecure secret store entry exists while fallback is disabled",
    );
  }

  return encoded.toString("utf8");
};

export type DesktopSecureStorageApi = Readonly<{
  readonly setSecret: (key: string, value: string) => Effect.Effect<void, DesktopSecureStorageError>;
  readonly getSecret: (key: string) => Effect.Effect<string | null, DesktopSecureStorageError>;
  readonly deleteSecret: (key: string) => Effect.Effect<void, DesktopSecureStorageError>;
}>;

export class DesktopSecureStorageService extends Context.Tag(
  "@openagents/desktop/DesktopSecureStorageService",
)<DesktopSecureStorageService, DesktopSecureStorageApi>() {}

export const DesktopSecureStorageLive = Layer.effect(
  DesktopSecureStorageService,
  Effect.gen(function* () {
    const config = yield* DesktopSecureStorageConfigService;
    const storePath = buildStorePath(config.userDataPath);

    const withStore = <A>(
      f: (store: PersistedSecureStore, encryptionAvailable: boolean) => A,
    ): Effect.Effect<A, DesktopSecureStorageError> =>
      Effect.try({
        try: () => {
          const store = loadStore(storePath);
          return f(store, safeStorage.isEncryptionAvailable());
        },
        catch: (error) => {
          if (error instanceof DesktopSecureStorageError) return error;
          return new DesktopSecureStorageError(
            "storage_read_failed",
            `Failed to access secure storage: ${String(error)}`,
          );
        },
      });

    const persistStore = (
      nextEntries: Record<string, PersistedSecret>,
    ): Effect.Effect<void, DesktopSecureStorageError> =>
      Effect.try({
        try: () => {
          saveStore(storePath, {
            version: 1,
            entries: nextEntries,
          });
        },
        catch: (error) =>
          new DesktopSecureStorageError(
            "storage_write_failed",
            `Failed to persist secure storage: ${String(error)}`,
          ),
      });

    return DesktopSecureStorageService.of({
      setSecret: (key, value) =>
        withStore((store, encryptionAvailable) => ({ store, encryptionAvailable })).pipe(
          Effect.flatMap(({ store, encryptionAvailable }) =>
            Effect.try({
              try: () => {
                const encoded = encodeSecret(value, encryptionAvailable, config.allowInsecureFallback);
                return {
                  ...store.entries,
                  [key]: encoded,
                };
              },
              catch: (error) => {
                if (error instanceof DesktopSecureStorageError) return error;
                return new DesktopSecureStorageError(
                  "storage_write_failed",
                  `Failed to encode secure secret: ${String(error)}`,
                );
              },
            }),
          ),
          Effect.flatMap((nextEntries) => persistStore(nextEntries)),
        ),

      getSecret: (key) =>
        withStore((store, encryptionAvailable) => ({ store, encryptionAvailable })).pipe(
          Effect.flatMap(({ store, encryptionAvailable }) => {
            const current = store.entries[key];
            if (!current) return Effect.succeed<string | null>(null);
            return Effect.try({
              try: () => decodeSecret(current, encryptionAvailable, config.allowInsecureFallback),
              catch: (error) => {
                if (error instanceof DesktopSecureStorageError) return error;
                return new DesktopSecureStorageError(
                  "storage_decode_failed",
                  `Failed to decode secure secret: ${String(error)}`,
                );
              },
            });
          }),
        ),

      deleteSecret: (key) =>
        withStore((store) => store).pipe(
          Effect.map((store) => {
            if (!(key in store.entries)) return store.entries;
            const nextEntries = { ...store.entries };
            delete nextEntries[key];
            return nextEntries;
          }),
          Effect.flatMap((nextEntries) => persistStore(nextEntries)),
        ),
    });
  }),
);

export const DesktopSecureStorageInMemoryTestLayer = Layer.sync(DesktopSecureStorageService, () => {
  const map = new Map<string, string>();
  return DesktopSecureStorageService.of({
    setSecret: (key, value) => Effect.sync(() => void map.set(key, value)),
    getSecret: (key) => Effect.sync(() => map.get(key) ?? null),
    deleteSecret: (key) => Effect.sync(() => void map.delete(key)),
  });
});
