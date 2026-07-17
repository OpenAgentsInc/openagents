import { describe, expect, test } from "vite-plus/test"

import {
  defaultMobileAccessibilityProfile,
  type KhalaInteraction,
} from "../src/screens/khala-core"
import { renderMobileInteractionCard } from "../src/screens/mobile-interaction-card"

const baseInteraction = {
  interactionRef: "interaction.mobile.card",
  turnRef: "turn.mobile.card",
  status: "pending",
  title: "Runtime request",
  prompt: "Review this request before continuing.",
  questions: [],
} as const

const render = (
  interaction: KhalaInteraction,
  selections: Readonly<Record<string, ReadonlyArray<string>>> = {},
  submitting = false,
): string => JSON.stringify(renderMobileInteractionCard(
  "entry.mobile.card",
  interaction,
  { selections, submitting, actionsAvailable: true },
  defaultMobileAccessibilityProfile,
))

describe("T3M-A3 mobile runtime interaction cards", () => {
  test("renders a request-scoped approval with explicit allow-once and deny actions", () => {
    const content = render({
      ...baseInteraction,
      kind: "tool_approval",
      title: "Run shell command?",
      prompt: "pnpm test",
    })

    expect(content).toContain("Approval needed")
    expect(content).toContain("Allow applies to this requested operation once")
    expect(content).toContain('"label":"Allow once"')
    expect(content).toContain('"label":"Deny"')
    expect(content).toContain('"outcome":"approve"')
    expect(content).toContain('"outcome":"deny"')
    expect(content).toContain('"minHeight":44')
    expect(content).not.toContain("Allow session")
    expect(content).not.toContain("always allow")
  })

  test("renders grouped single and multi-select questions and validates every answer", () => {
    const interaction: KhalaInteraction = {
      ...baseInteraction,
      kind: "provider_question",
      title: "Choose verification",
      prompt: "Answer both questions.",
      questions: [
        {
          questionRef: "question.tests",
          displayText: "Which tests?",
          multiSelect: true,
          options: [
            { optionRef: "option.unit", label: "Unit", description: "Fast focused suite" },
            { optionRef: "option.e2e", label: "End to end" },
          ],
        },
        {
          questionRef: "question.target",
          displayText: "Which target?",
          multiSelect: false,
          options: [{ optionRef: "option.mobile", label: "Mobile" }],
        },
      ],
    }

    const incomplete = render(interaction, { "question.tests": ["option.unit"] })
    expect(incomplete).toContain("Question 1 of 2 · Select one or more")
    expect(incomplete).toContain("Question 2 of 2 · Select one")
    expect(incomplete).toContain("Fast focused suite")
    expect(incomplete).toContain('"label":"Unit","variant":"secondary","size":"sm","pill":true,"selected":true')
    expect(incomplete).toContain('"label":"Submit answers","variant":"primary","loading":false,"block":true,"disabled":true')
    expect(incomplete).toContain("Answer every question before submitting")

    const complete = render(interaction, {
      "question.tests": ["option.unit", "option.e2e"],
      "question.target": ["option.mobile"],
    })
    expect(complete).toContain('"label":"Submit answers","variant":"primary","loading":false,"block":true,"disabled":false')

    const submitting = render(interaction, {
      "question.tests": ["option.unit"],
      "question.target": ["option.mobile"],
    }, true)
    expect(submitting).toContain("Submitting answers…")
    expect(submitting).toContain('"loading":true,"block":true,"disabled":true')
  })

  test("renders plan Markdown, native copy, and the three exact review outcomes", () => {
    const content = render({
      ...baseInteraction,
      kind: "plan_review",
      title: "Review implementation plan",
      prompt: "## Plan\n\n1. Inspect `Home`\n2. Run tests",
    })

    expect(content).toContain('"kind":"heading","level":2')
    expect(content).toContain('"kind":"code","text":"Home"')
    expect(content).toContain("Copy plan")
    expect(content).toContain('"label":"Accept plan"')
    expect(content).toContain('"label":"Request changes"')
    expect(content).toContain('"label":"Replan"')
    expect(content).toContain('"outcome":"accept"')
    expect(content).toContain('"outcome":"request_changes"')
    expect(content).toContain('"outcome":"replan"')
  })

  test.each([
    ["resolved", "Decision confirmed", "Your decision was confirmed by the runtime."],
    ["expired", "Expired", "This request expired"],
    ["revoked", "Access revoked", "Authority for this request was revoked"],
  ] as const)("renders %s as terminal read-only state", (status, label, summary) => {
    const content = render({
      ...baseInteraction,
      kind: "tool_approval",
      status,
      decisionRef: status === "resolved" ? "decision.mobile.card" : undefined,
    })

    expect(content).toContain(label)
    expect(content).toContain(summary)
    expect(content).not.toContain('"label":"Allow once"')
    expect(content).not.toContain('"label":"Deny"')
    expect(content).not.toContain("RuntimeInteractionDecisionSubmitted")
  })
})
