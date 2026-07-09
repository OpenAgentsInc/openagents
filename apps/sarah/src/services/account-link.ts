/**
 * KHS-7 (#8606): in-conversation account linking on /sarah.
 *
 * Sarah only LINKS an anonymous prospect_ref to a real openagents.com user —
 * the openagents.com API remains the identity/credit authority and there is
 * no new auth system here. /sarah is path-mounted on openagents.com, so the
 * OpenAuth session cookies (`oa_access`/`oa_refresh`, Path=/, host-only) are
 * first-party on every /sarah/api/* request. Verification forwards the
 * request's cookie header to the canonical `GET /api/auth/session` endpoint
 * instead of re-implementing token verification.
 *
 * PII law: the linked identity (user ref + email) lands ONLY in
 * `sarah_prospect_contacts` via the existing persistSarahProspectContact
 * upsert — no new PII store, nothing in collective/trace paths.
 */

import {
  persistSarahProspectContact,
  readSarahStore,
  sarahTurnStoreStatus,
} from "./turn-store.ts"
import { prospectRefAliases } from "./prospect-memory.ts"

export type OpenAgentsSessionUser = {
  userId: string
  email: string | null
  name: string | null
}

/** contact_id prefix marking an account link (vs a CRM contact id). */
export const SARAH_ACCOUNT_CONTACT_ID_PREFIX = "oa_user:"
/** mode value recorded on the contact row for account links. */
export const SARAH_ACCOUNT_LINK_MODE = "account_link"

function authBaseUrl(): string {
  return (
    process.env.SARAH_OPENAGENTS_AUTH_BASE_URL?.trim().replace(/\/+$/, "") ||
    process.env.SARAH_OPENAGENTS_BASE_URL?.trim().replace(/\/+$/, "") ||
    "https://openagents.com"
  )
}

export function sarahAccountLinkTestMode(): boolean {
  return process.env.SARAH_ACCOUNT_LINK_TEST_MODE === "1"
}

function parseTestSessionHeader(raw: string): OpenAgentsSessionUser | null {
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>
    if (typeof parsed.userId !== "string" || !parsed.userId.trim()) return null
    return {
      userId: parsed.userId,
      email:
        typeof parsed.email === "string" && parsed.email ? parsed.email : null,
      name: typeof parsed.name === "string" && parsed.name ? parsed.name : null,
    }
  } catch {
    return null
  }
}

/**
 * Resolve the authenticated openagents.com user for this request, or null.
 *
 * Fast path: without an `oa_access` cookie the request is anonymous — no
 * network call. Otherwise the cookie header is forwarded to the canonical
 * `GET /api/auth/session` who-am-I endpoint (never a locally re-implemented
 * verifier). Fail-soft: any transport/shape failure resolves to null (401 at
 * the route), never a thrown error into the conversation.
 */
export async function resolveOpenAgentsSession(
  request: Request,
): Promise<OpenAgentsSessionUser | null> {
  if (sarahAccountLinkTestMode()) {
    const raw = request.headers.get("x-sarah-test-oa-session")
    return raw ? parseTestSessionHeader(raw) : null
  }
  const cookie = request.headers.get("cookie")
  if (!cookie || !/(?:^|;\s*)oa_access=/.test(cookie)) return null
  try {
    const response = await fetch(`${authBaseUrl()}/api/auth/session`, {
      headers: { cookie, accept: "application/json" },
      signal: AbortSignal.timeout(5_000),
    })
    if (!response.ok) return null
    const data = (await response.json()) as {
      authenticated?: boolean
      bootstrap?: {
        session?: { userId?: unknown; email?: unknown; name?: unknown }
      }
    }
    if (data.authenticated !== true) return null
    const session = data.bootstrap?.session
    if (!session || typeof session.userId !== "string" || !session.userId) {
      return null
    }
    return {
      userId: session.userId,
      email:
        typeof session.email === "string" && session.email
          ? session.email
          : null,
      name:
        typeof session.name === "string" && session.name ? session.name : null,
    }
  } catch {
    return null
  }
}

/** Pure row shape for the linked upsert — unit-tested without a database. */
export function buildAccountLinkContactRow(
  prospectRef: string,
  user: OpenAgentsSessionUser,
): {
  prospectRef: string
  contactId: string
  contactEmail: string | null
  mode: string
} {
  return {
    prospectRef,
    contactId: `${SARAH_ACCOUNT_CONTACT_ID_PREFIX}${user.userId}`,
    contactEmail: user.email,
    mode: SARAH_ACCOUNT_LINK_MODE,
  }
}

export type SarahAccountLinkStatus = {
  linked: boolean
  email?: string
  storeConfigured: boolean
}

/**
 * Read link state for ONE prospect ref (aliases are the deterministic
 * re-encodings of the same identity — prospect-memory scoping law).
 */
export async function getSarahAccountLinkStatus(
  prospectRef: string,
): Promise<SarahAccountLinkStatus> {
  const storeConfigured = sarahTurnStoreStatus().configured
  const aliases = prospectRefAliases(prospectRef)
  if (!storeConfigured || aliases.length === 0) {
    return { linked: false, storeConfigured }
  }
  const rows = await readSarahStore(
    async (sql) =>
      (await sql`
        SELECT contact_id, contact_email
        FROM sarah_prospect_contacts
        WHERE prospect_ref IN ${sql(aliases)}
        LIMIT ${aliases.length}
      `) as Array<Record<string, unknown>>,
  )
  const linkedRow = rows?.find((row) =>
    String(row.contact_id ?? "").startsWith(SARAH_ACCOUNT_CONTACT_ID_PREFIX),
  )
  if (!linkedRow) return { linked: false, storeConfigured }
  const email = String(linkedRow.contact_email ?? "")
  return { linked: true, ...(email ? { email } : {}), storeConfigured }
}

/** Upsert the prospect→account link into sarah_prospect_contacts. */
export async function linkSarahProspectAccount(
  prospectRef: string,
  user: OpenAgentsSessionUser,
): Promise<{ contactId: string; email: string | null }> {
  const row = buildAccountLinkContactRow(prospectRef, user)
  await persistSarahProspectContact(row)
  return { contactId: row.contactId, email: row.contactEmail }
}

/**
 * One account-awareness line for the code-side system prompt assembly (the
 * owner-managed LiveAvatar base context is never edited here). Pure so KHS
 * tests can oracle the copy. Null when the store is not configured (linking
 * would not persist, so Sarah must not pitch it).
 */
export function accountPromptLine(
  status: SarahAccountLinkStatus | null,
): string | null {
  if (!status || !status.storeConfigured) return null
  if (status.linked) {
    return `[account] This prospect is signed in with a linked OpenAgents account${
      status.email ? ` (${status.email})` : ""
    }; never ask them to create an account.`
  }
  return "[account] This prospect has no OpenAgents account linked yet; if they are engaged and it fits naturally, you may mention once — one short sentence, never pushy — that they can create an account or sign in without leaving this chat via the Account button in this panel."
}

/** Prompt line for a maybe-absent prospect ref; null-safe and fail-soft. */
export async function getSarahAccountPromptLine(
  prospectRef: string | undefined,
): Promise<string | null> {
  if (!prospectRef) return null
  try {
    return accountPromptLine(await getSarahAccountLinkStatus(prospectRef))
  } catch {
    return null
  }
}
