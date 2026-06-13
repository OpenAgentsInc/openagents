import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"

export type SessionRecord = {
  sessionRef: string
  state: "queued" | "running" | "completed" | "failed" | "cancelled"
  events: Array<{ phase: string; observedAt: string }>
  artifactRefs: string[]
  cleanupReceiptRef: string | null
}

function sessionRecordPath(dir: string, sessionRef: string): string {
  return join(dir, `${sessionRef}.json`)
}

function createSessionRecord(sessionRef: string): SessionRecord {
  return {
    sessionRef,
    state: "queued",
    events: [],
    artifactRefs: [],
    cleanupReceiptRef: null,
  }
}

export async function appendSessionEvent(
  dir: string,
  sessionRef: string,
  event: { phase: string; observedAt: string },
): Promise<void> {
  await mkdir(dir, { recursive: true })
  const record = (await loadSessionRecord(dir, sessionRef)) ?? createSessionRecord(sessionRef)
  record.events.push(event)
  await writeFile(sessionRecordPath(dir, sessionRef), `${JSON.stringify(record, null, 2)}\n`, "utf8")
}

export async function loadSessionRecord(dir: string, sessionRef: string): Promise<SessionRecord | null> {
  try {
    return JSON.parse(await readFile(sessionRecordPath(dir, sessionRef), "utf8")) as SessionRecord
  } catch {
    return null
  }
}
