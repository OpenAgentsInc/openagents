/**
 * KHS-7 (#8606): unit oracles for in-conversation account linking — the pure
 * contact-row shape, the single-line prompt copy, and session resolution in
 * test mode plus the no-cookie anonymous fast path.
 *
 * Oracle for contract sarah.in_chat_account_linking.v1 (registered in
 * src/contracts/isolation-contracts.ts; human doc docs/sarah/SARAH_CONTRACTS.md).
 */

import { afterEach, describe, expect, test } from "bun:test"

import {
  accountPromptLine,
  buildAccountLinkContactRow,
  resolveOpenAgentsSession,
  SARAH_ACCOUNT_CONTACT_ID_PREFIX,
  SARAH_ACCOUNT_LINK_MODE,
} from "./account-link.ts"

const requestWith = (headers: Record<string, string>) =>
  new Request("http://localhost/sarah/api/account/link", {
    method: "POST",
    headers,
  })

describe("KHS-7 account linking (#8606)", () => {
  afterEach(() => {
    delete process.env.SARAH_ACCOUNT_LINK_TEST_MODE
  })

  test("linked contact row upserts the user ref + email onto the prospect", () => {
    const row = buildAccountLinkContactRow("prospect-abc", {
      userId: "user_123",
      email: "buyer@example.com",
      name: "Buyer",
    })
    expect(row).toEqual({
      prospectRef: "prospect-abc",
      contactId: `${SARAH_ACCOUNT_CONTACT_ID_PREFIX}user_123`,
      contactEmail: "buyer@example.com",
      mode: SARAH_ACCOUNT_LINK_MODE,
    })
  })

  test("contact row tolerates a session without an email", () => {
    const row = buildAccountLinkContactRow("prospect-abc", {
      userId: "user_456",
      email: null,
      name: null,
    })
    expect(row.contactId).toBe("oa_user:user_456")
    expect(row.contactEmail).toBeNull()
  })

  test("prompt line is null when the store is not configured", () => {
    expect(
      accountPromptLine({ linked: false, storeConfigured: false }),
    ).toBeNull()
    expect(accountPromptLine(null)).toBeNull()
  })

  test("anonymous prompt line is one gentle in-chat suggestion", () => {
    const line = accountPromptLine({ linked: false, storeConfigured: true })
    expect(line).toStartWith("[account]")
    expect(line).toContain("without leaving this chat")
    expect(line).toContain("never pushy")
    expect(line).toContain("once")
  })

  test("linked prompt line carries the email and forbids re-asking", () => {
    const line = accountPromptLine({
      linked: true,
      email: "buyer@example.com",
      storeConfigured: true,
    })
    expect(line).toContain("buyer@example.com")
    expect(line).toContain("never ask them to create an account")
  })

  test("test-mode session resolution accepts only a valid userId payload", async () => {
    process.env.SARAH_ACCOUNT_LINK_TEST_MODE = "1"
    expect(await resolveOpenAgentsSession(requestWith({}))).toBeNull()
    expect(
      await resolveOpenAgentsSession(
        requestWith({ "x-sarah-test-oa-session": "not json" }),
      ),
    ).toBeNull()
    expect(
      await resolveOpenAgentsSession(
        requestWith({
          "x-sarah-test-oa-session": JSON.stringify({ email: "x@y.z" }),
        }),
      ),
    ).toBeNull()
    expect(
      await resolveOpenAgentsSession(
        requestWith({
          "x-sarah-test-oa-session": JSON.stringify({
            userId: "u1",
            email: "x@y.z",
          }),
        }),
      ),
    ).toEqual({ userId: "u1", email: "x@y.z", name: null })
  })

  test("without an oa_access cookie the request resolves anonymous (no network)", async () => {
    const request = requestWith({ cookie: "sarah_prospect_ref=abc" })
    expect(await resolveOpenAgentsSession(request)).toBeNull()
  })
})
