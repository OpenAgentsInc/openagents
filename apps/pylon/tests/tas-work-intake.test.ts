import { describe, expect, test } from "bun:test"

import { normalizeIntake, type RawWorkIntake } from "../src/tas/work-intake"

describe("tas unified work intake", () => {
  test("normalizes a GitHub issue into the unified shape", () => {
    const intake = normalizeIntake({
      source: "github",
      owner: "OpenAgentsInc",
      repo: "openagents",
      issueNumber: 4775,
      title: "Forum to coding request",
      body: "Bridge forum requests into coding work.",
      receivedAt: "2026-06-11T10:00:00.000Z",
    })

    expect(intake).toEqual({
      intakeId: expect.stringMatching(/^work_intake\.[a-f0-9]{24}$/),
      source: "github",
      title: "Forum to coding request",
      body: "Bridge forum requests into coding work.",
      originRef: "github.issue.OpenAgentsInc.openagents.4775",
      receivedAt: "2026-06-11T10:00:00.000Z",
    })
  })

  test("normalizes a Forum post and carries the origin ref", () => {
    const intake = normalizeIntake({
      source: "forum",
      forumRef: "forum.product_promises",
      postRef: "topic.42.post.7",
      title: "Request a bounded coding job",
      body: "Please turn this report into a work request.",
      receivedAt: "2026-06-11T11:00:00.000Z",
    })

    expect(intake).toEqual({
      intakeId: expect.stringMatching(/^work_intake\.[a-f0-9]{24}$/),
      source: "forum",
      title: "Request a bounded coding job",
      body: "Please turn this report into a work request.",
      originRef: "forum.product_promises.post.topic.42.post.7",
      receivedAt: "2026-06-11T11:00:00.000Z",
    })
  })

  test("normalizes a direct mobile intent and carries the client intent ref", () => {
    const intake = normalizeIntake({
      source: "intent",
      intentId: "intent.mobile.001",
      submittedByClientRef: "client.ios.testflight.17",
      title: "Ship the mobile approval flow",
      body: "Create the approval handoff for a pending agent action.",
      receivedAt: "2026-06-11T12:00:00.000Z",
    })

    expect(intake).toEqual({
      intakeId: expect.stringMatching(/^work_intake\.[a-f0-9]{24}$/),
      source: "intent",
      title: "Ship the mobile approval flow",
      body: "Create the approval handoff for a pending agent action.",
      originRef: "client.ios.testflight.17.intent.intent.mobile.001",
      receivedAt: "2026-06-11T12:00:00.000Z",
    })
  })

  test("builds deterministic intake ids from the supplied inputs", () => {
    const raw: RawWorkIntake = {
      source: "forum",
      forumRef: "forum.product_promises",
      postRef: "topic.42.post.7",
      title: "Request a bounded coding job",
      body: "Please turn this report into a work request.",
      receivedAt: "2026-06-11T11:00:00.000Z",
    }

    const first = normalizeIntake(raw)
    const second = normalizeIntake({ ...raw })
    const changed = normalizeIntake({
      ...raw,
      postRef: "topic.42.post.8",
    })

    expect(second.intakeId).toBe(first.intakeId)
    expect(changed.intakeId).not.toBe(first.intakeId)
  })
})
