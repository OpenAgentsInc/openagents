export const KHALA_MOBILE_PREFERENCES_DB_NAME = "khala-mobile-preferences.db"
export const KHALA_MOBILE_PREFERENCES_KIND = "expo-sqlite-nonsecret-preferences"

export type ExpoPreferenceSqliteDatabase = Readonly<{
  execAsync: (statement: string) => Promise<void>
  getFirstAsync: <T>(statement: string, ...params: ReadonlyArray<unknown>) => Promise<T | null>
  runAsync: (statement: string, ...params: ReadonlyArray<unknown>) => Promise<unknown>
  closeAsync?: () => Promise<void>
}>

export type ExpoPreferenceSqliteModule = Readonly<{
  openDatabaseAsync: (name: string) => Promise<ExpoPreferenceSqliteDatabase>
}>

export type KhalaNonsecretPreferences = Readonly<{
  hasSeenTailnetPairingHint: boolean
  threadListDisplayMode: "comfortable" | "compact"
}>

export type KhalaNonsecretPreferenceKey = keyof KhalaNonsecretPreferences

export class KhalaNonsecretPreferenceError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "KhalaNonsecretPreferenceError"
  }
}

export type KhalaNonsecretPreferenceStore = Readonly<{
  kind: typeof KHALA_MOBILE_PREFERENCES_KIND
  databaseName: string
  get: <K extends KhalaNonsecretPreferenceKey>(key: K) => Promise<KhalaNonsecretPreferences[K]>
  reset: (key: KhalaNonsecretPreferenceKey) => Promise<void>
  set: <K extends KhalaNonsecretPreferenceKey>(
    key: K,
    value: KhalaNonsecretPreferences[K],
  ) => Promise<void>
}>

const loadExpoSqlite = async (): Promise<ExpoPreferenceSqliteModule> =>
  (await import("expo-sqlite")) as ExpoPreferenceSqliteModule

const KHALA_MOBILE_PREFERENCES_SCHEMA = `
CREATE TABLE IF NOT EXISTS khala_nonsecret_preferences (
  key TEXT PRIMARY KEY NOT NULL,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const preferenceDefinitions = {
  hasSeenTailnetPairingHint: {
    defaultValue: false,
    is: (value: unknown): value is boolean => typeof value === "boolean",
  },
  threadListDisplayMode: {
    defaultValue: "comfortable",
    is: (value: unknown): value is "comfortable" | "compact" =>
      value === "comfortable" || value === "compact",
  },
} satisfies {
  [K in KhalaNonsecretPreferenceKey]: {
    defaultValue: KhalaNonsecretPreferences[K]
    is: (value: unknown) => value is KhalaNonsecretPreferences[K]
  }
}

const SECRET_KEY_PATTERN =
  /(api[_-]?key|access[_-]?token|auth[_-]?token|bearer|chat[_-]?body|credential|message[_-]?body|password|private|prompt|secret|sync[_-]?projection|token)/i

export const khalaNonsecretPreferenceKeys = Object.keys(
  preferenceDefinitions,
) as ReadonlyArray<KhalaNonsecretPreferenceKey>

export const isKhalaNonsecretPreferenceKey = (
  key: string,
): key is KhalaNonsecretPreferenceKey =>
  Object.hasOwn(preferenceDefinitions, key)

const assertAllowedKey = (key: string): KhalaNonsecretPreferenceKey => {
  if (SECRET_KEY_PATTERN.test(key)) {
    throw new KhalaNonsecretPreferenceError(
      `nonsecret preferences cannot store secret/private key "${key}"`,
    )
  }
  if (!isKhalaNonsecretPreferenceKey(key)) {
    throw new KhalaNonsecretPreferenceError(
      `unknown nonsecret preference key "${key}"`,
    )
  }
  return key
}

const decodePreferenceValue = <K extends KhalaNonsecretPreferenceKey>(
  key: K,
  valueJson: string | null,
): KhalaNonsecretPreferences[K] => {
  const definition = preferenceDefinitions[key] as {
    defaultValue: KhalaNonsecretPreferences[K]
    is: (value: unknown) => value is KhalaNonsecretPreferences[K]
  }
  if (valueJson === null) return definition.defaultValue

  try {
    const parsed: unknown = JSON.parse(valueJson)
    if (definition.is(parsed)) return parsed
  } catch {
    // Fall through to the default below.
  }

  return definition.defaultValue
}

export const openKhalaNonsecretPreferences = async (
  input: {
    readonly databaseName?: string
    readonly sqliteLoader?: () => Promise<ExpoPreferenceSqliteModule>
  } = {},
): Promise<KhalaNonsecretPreferenceStore> => {
  const databaseName = input.databaseName ?? KHALA_MOBILE_PREFERENCES_DB_NAME
  const sqlite = await (input.sqliteLoader ?? loadExpoSqlite)()
  const db = await sqlite.openDatabaseAsync(databaseName)
  await db.execAsync(KHALA_MOBILE_PREFERENCES_SCHEMA)

  return {
    databaseName,
    kind: KHALA_MOBILE_PREFERENCES_KIND,
    get: async key => {
      assertAllowedKey(key)
      const row = await db.getFirstAsync<{ readonly value_json: string }>(
        "SELECT value_json FROM khala_nonsecret_preferences WHERE key = ?",
        key,
      )
      return decodePreferenceValue(key, row?.value_json ?? null)
    },
    reset: async key => {
      const allowedKey = assertAllowedKey(key)
      await db.runAsync(
        "DELETE FROM khala_nonsecret_preferences WHERE key = ?",
        allowedKey,
      )
    },
    set: async (key, value) => {
      const allowedKey = assertAllowedKey(key)
      const definition = preferenceDefinitions[allowedKey] as {
        is: (candidate: unknown) => candidate is KhalaNonsecretPreferences[typeof allowedKey]
      }
      if (!definition.is(value)) {
        throw new KhalaNonsecretPreferenceError(
          `invalid value for nonsecret preference "${allowedKey}"`,
        )
      }
      await db.runAsync(
        `INSERT INTO khala_nonsecret_preferences (key, value_json, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT (key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at`,
        allowedKey,
        JSON.stringify(value),
        new Date().toISOString(),
      )
    },
  }
}
