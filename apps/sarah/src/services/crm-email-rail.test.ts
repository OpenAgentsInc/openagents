/**
 * CRM rail guard regression: rich internal enqueue must ignore an unchecked
 * forcedStatus (e.g. "sent") and persist pending_approval unless local
 * suppression authority marks the address suppressed. File-backed projection
 * only — no operator HTTP / network path.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { rm } from "node:fs/promises"
import { join } from "node:path"

import {
  enqueueSarahEmailDraft,
  listSarahEmailDrafts,
  suppressSarahEmail,
  type SarahEmailDraftRecord,
} from "./crm-email-rail.ts"

const QUEUE_FILE = `crm-email-rail-forced-status-queue-${process.pid}.json`
const SUPPRESSION_FILE =
  `crm-email-rail-forced-status-suppressions-${process.pid}.json`

const envSnapshot = {
  queue: process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH,
  suppression: process.env.SARAH_EMAIL_SUPPRESSION_LIST_PATH,
  bearer: process.env.SARAH_OPERATOR_BEARER,
  opToken: process.env.OPENAGENTS_OPERATOR_TOKEN,
}

function restoreEnv(key: string, value: string | undefined) {
  if (value === undefined) delete process.env[key]
  else process.env[key] = value
}

async function removeProjectionFiles() {
  await rm(join(process.cwd(), ".sarah", QUEUE_FILE), { force: true })
  await rm(join(process.cwd(), ".sarah", SUPPRESSION_FILE), { force: true })
}

function richInputWithForcedSent(toEmail: string) {
  // forcedStatus is not on the public rich enqueue type; the runtime still
  // receives it if a caller spreads an unchecked record.
  return {
    fromEmail: "sarah@openagents.com",
    toEmail,
    subject: "OpenAgents follow up",
    inboundText: "Please follow up with me.",
    proposedReply: "I'm Sarah, OpenAgents' AI sales employee. Happy to help.",
    prospectRef: `email:${toEmail}`,
    threadId: `email:${toEmail}:forced-status-guard`,
    messageId: null as string | null,
    continuationToken: `email:${toEmail}:forced-status-guard`,
    forcedStatus: "sent" as const,
  }
}

beforeEach(async () => {
  process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH = QUEUE_FILE
  process.env.SARAH_EMAIL_SUPPRESSION_LIST_PATH = SUPPRESSION_FILE
  delete process.env.SARAH_OPERATOR_BEARER
  delete process.env.OPENAGENTS_OPERATOR_TOKEN
  await removeProjectionFiles()
})

afterEach(async () => {
  await removeProjectionFiles()
  restoreEnv("SARAH_EMAIL_APPROVAL_QUEUE_PATH", envSnapshot.queue)
  restoreEnv("SARAH_EMAIL_SUPPRESSION_LIST_PATH", envSnapshot.suppression)
  restoreEnv("SARAH_OPERATOR_BEARER", envSnapshot.bearer)
  restoreEnv("OPENAGENTS_OPERATOR_TOKEN", envSnapshot.opToken)
})

describe("crm-email-rail forcedStatus guard", () => {
  test("unchecked rich forcedStatus sent persists pending_approval unless suppressed", async () => {
    const originalFetch = globalThis.fetch
    let fetchCalls = 0
    globalThis.fetch = (async () => {
      fetchCalls += 1
      throw new Error("crm-email-rail test must not call network")
    }) as unknown as typeof fetch

    try {
      const openEmail = "buyer-forced-status@example.com"
      const openDraft = (await enqueueSarahEmailDraft(
        richInputWithForcedSent(openEmail) as Parameters<
          typeof enqueueSarahEmailDraft
        >[0],
      )) as SarahEmailDraftRecord

      expect(openDraft.status).toBe("pending_approval")
      expect(openDraft.status).not.toBe("sent")
      expect(openDraft.sentAt).toBeNull()
      expect(openDraft.toEmail).toBe(openEmail)

      const listedOpen = (await listSarahEmailDrafts()).find(
        (d) => d.id === openDraft.id,
      )
      expect(listedOpen?.status).toBe("pending_approval")

      const suppressedEmail = "suppressed-forced-status@example.com"
      await suppressSarahEmail({
        email: suppressedEmail,
        reason: "unsubscribe",
        source: "crm-email-rail.forced-status.test",
      })

      const suppressedDraft = (await enqueueSarahEmailDraft(
        richInputWithForcedSent(suppressedEmail) as Parameters<
          typeof enqueueSarahEmailDraft
        >[0],
      )) as SarahEmailDraftRecord

      expect(suppressedDraft.status).toBe("suppressed")
      expect(suppressedDraft.status).not.toBe("sent")
      expect(suppressedDraft.sentAt).toBeNull()
      expect(suppressedDraft.toEmail).toBe(suppressedEmail)

      const listedSuppressed = (await listSarahEmailDrafts()).find(
        (d) => d.id === suppressedDraft.id,
      )
      expect(listedSuppressed?.status).toBe("suppressed")

      expect(fetchCalls).toBe(0)
    } finally {
      globalThis.fetch = originalFetch
    }
  })
})
