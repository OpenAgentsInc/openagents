export type VerseSceneDiagnostic = Readonly<{
  at: string
  event: string
  detail: Record<string, unknown>
}>

const MAX_LOGS = 300

const logStore = (): VerseSceneDiagnostic[] => {
  const host = globalThis as typeof globalThis & {
    __OA_VERSE_SCENE_LOGS?: VerseSceneDiagnostic[]
    __OA_DUMP_VERSE_SCENE_LOGS?: () => readonly VerseSceneDiagnostic[]
  }
  if (!Array.isArray(host.__OA_VERSE_SCENE_LOGS)) {
    host.__OA_VERSE_SCENE_LOGS = []
  }
  host.__OA_DUMP_VERSE_SCENE_LOGS = () => [...host.__OA_VERSE_SCENE_LOGS!]
  return host.__OA_VERSE_SCENE_LOGS
}

export const recordVerseSceneDiagnostic = (
  event: string,
  detail: Record<string, unknown> = {},
): void => {
  const entry = { at: new Date().toISOString(), event, detail }
  const logs = logStore()
  logs.push(entry)
  if (logs.length > MAX_LOGS) {
    logs.splice(0, logs.length - MAX_LOGS)
  }
  console.info("[verse-scene]", event, detail)
}

export const verseSceneDiagnostics = (): readonly VerseSceneDiagnostic[] =>
  [...logStore()]

export const clearVerseSceneDiagnosticsForTest = (): void => {
  logStore().length = 0
}
