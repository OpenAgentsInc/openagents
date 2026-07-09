/**
 * Durable Postgres store for Sarah conversation turns (#8598, owner directive
 * 2026-07-09: every turn saved with the associated user when there is one).
 *
 * The local `.sarah/session-index.json` projection is ephemeral on Cloud Run;
 * this store is the durable record. Sarah owns ONLY `sarah_*` tables — the
 * openagents.com API remains the authority for CRM, credits, and checkout;
 * these tables link outward via prospect_ref / contact_email / contact_id.
 *
 * Fail-soft: without a configured database (local dev, tests) every call is a
 * recorded no-op — the conversation never breaks because persistence is down —
 * but ops surfaces the store state so a silently-degraded prod is visible.
 */

import { SQL } from "bun"

type TurnRow = {
  prospectRef: string
  sessionId: string
  threadId: string
  modality: string
  role: string
  sourceEvent: string
  text: string
}

type ProspectContactRow = {
  prospectRef: string
  contactId: string | null
  contactEmail: string | null
  mode: string | null
}

type AvatarSessionRow = {
  event: string
  sessionId: string
  conversationRef: string
  sandbox?: boolean
  minutes?: number
}

let sqlClient: SQL | null | undefined
let schemaReady: Promise<boolean> | null = null
let lastError: string | null = null

function databaseUrl(): string | null {
  return (
    process.env.SARAH_DATABASE_URL?.trim() ||
    process.env.KHALA_SYNC_DATABASE_URL?.trim() ||
    null
  )
}

function client(): SQL | null {
  if (sqlClient !== undefined) return sqlClient
  const url = databaseUrl()
  if (!url) {
    sqlClient = null
    return null
  }
  try {
    sqlClient = new SQL(url)
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
    sqlClient = null
  }
  return sqlClient
}

async function ensureSchema(sql: SQL): Promise<boolean> {
  schemaReady ??= (async () => {
    try {
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_transcript_turns (
          id BIGSERIAL PRIMARY KEY,
          prospect_ref TEXT NOT NULL,
          session_id TEXT NOT NULL,
          thread_id TEXT NOT NULL,
          modality TEXT NOT NULL,
          role TEXT NOT NULL,
          source_event TEXT NOT NULL,
          text TEXT NOT NULL,
          recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      await sql`
        CREATE INDEX IF NOT EXISTS sarah_transcript_turns_prospect_idx
          ON sarah_transcript_turns (prospect_ref, recorded_at)`
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_prospect_contacts (
          prospect_ref TEXT PRIMARY KEY,
          contact_id TEXT,
          contact_email TEXT,
          mode TEXT,
          updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      await sql`
        CREATE TABLE IF NOT EXISTS sarah_avatar_sessions (
          id BIGSERIAL PRIMARY KEY,
          event TEXT NOT NULL,
          session_id TEXT NOT NULL,
          conversation_ref TEXT NOT NULL,
          sandbox BOOLEAN,
          minutes INTEGER,
          recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )`
      return true
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error)
      return false
    }
  })()
  return schemaReady
}

async function withStore(run: (sql: SQL) => Promise<void>): Promise<void> {
  const sql = client()
  if (!sql) return
  if (!(await ensureSchema(sql))) return
  try {
    await run(sql)
    lastError = null
  } catch (error) {
    lastError = error instanceof Error ? error.message : String(error)
  }
}

export async function persistSarahTurn(row: TurnRow): Promise<void> {
  await withStore(
    (sql) => sql`
      INSERT INTO sarah_transcript_turns
        (prospect_ref, session_id, thread_id, modality, role, source_event, text)
      VALUES
        (${row.prospectRef}, ${row.sessionId}, ${row.threadId}, ${row.modality},
         ${row.role}, ${row.sourceEvent}, ${row.text})
    `.then(() => {}),
  )
}

export async function persistSarahProspectContact(
  row: ProspectContactRow,
): Promise<void> {
  await withStore(
    (sql) => sql`
      INSERT INTO sarah_prospect_contacts (prospect_ref, contact_id, contact_email, mode, updated_at)
      VALUES (${row.prospectRef}, ${row.contactId}, ${row.contactEmail}, ${row.mode}, now())
      ON CONFLICT (prospect_ref) DO UPDATE SET
        contact_id = COALESCE(EXCLUDED.contact_id, sarah_prospect_contacts.contact_id),
        contact_email = COALESCE(EXCLUDED.contact_email, sarah_prospect_contacts.contact_email),
        mode = COALESCE(EXCLUDED.mode, sarah_prospect_contacts.mode),
        updated_at = now()
    `.then(() => {}),
  )
}

export async function persistSarahAvatarSession(
  row: AvatarSessionRow,
): Promise<void> {
  await withStore(
    (sql) => sql`
      INSERT INTO sarah_avatar_sessions (event, session_id, conversation_ref, sandbox, minutes)
      VALUES (${row.event}, ${row.sessionId}, ${row.conversationRef},
              ${row.sandbox ?? null}, ${row.minutes ?? null})
    `.then(() => {}),
  )
}

export function sarahTurnStoreStatus() {
  return {
    configured: Boolean(databaseUrl()),
    lastError,
  }
}
