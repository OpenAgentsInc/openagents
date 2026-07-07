import { describe, expect, test } from "bun:test"

import {
  KhalaMobileStoreSubmissionSchemaId,
  khalaMobileStoreSubmissionOwnerActions,
  khalaMobileStoreSubmissionReceipt,
  khalaMobileStoreSubmissionsAreP0Complete,
} from "../src/qa/store-submissions"

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

// Guard for khala_mobile.qa.store_submission_receipts.v1.
describe("Khala Mobile P0.9 store submission receipt", () => {
  test("store_submission_receipts_are_not_faked.unit — P0 exit stays false without both submission IDs", () => {
    expect(khalaMobileStoreSubmissionReceipt.schema).toBe(KhalaMobileStoreSubmissionSchemaId)
    expect(khalaMobileStoreSubmissionReceipt.issue).toBe(8544)
    expect(khalaMobileStoreSubmissionReceipt.p0ExitSatisfied).toBe(false)
    expect(khalaMobileStoreSubmissionsAreP0Complete(khalaMobileStoreSubmissionReceipt)).toBe(false)

    expect(khalaMobileStoreSubmissionReceipt.submissions.map(record => record.platform)).toEqual([
      "ios_app_store_connect",
      "android_play_console",
    ])

    for (const submission of khalaMobileStoreSubmissionReceipt.submissions) {
      expect(submission.submissionId).toBeNull()
      expect(submission.reviewState).toBe("not_submitted")
      expect(submission.blockerRefs.length).toBeGreaterThan(0)
      expect(submission.evidenceRefs).toContain("NEEDS_OWNER.md#khala-mobile-p09-store-submissions")
    }
  })

  test("store_submission_owner_gate_documented.source — owner console actions and URLs are explicit", async () => {
    const ownerDoc = await Bun.file(repoPath("NEEDS_OWNER.md")).text()
    expect(ownerDoc).toContain("Khala Mobile P0.9 Store Submissions")
    expect(ownerDoc).toContain("Source issue: OpenAgentsInc/openagents#8544")
    expect(ownerDoc).toContain("https://appstoreconnect.apple.com/apps")
    expect(ownerDoc).toContain("https://play.google.com/console")

    for (const action of khalaMobileStoreSubmissionOwnerActions) {
      expect(ownerDoc).toContain(action)
    }
  })
})
