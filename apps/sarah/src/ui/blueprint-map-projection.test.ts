import { describe, expect, test } from "bun:test"

import {
  blueprintMapFactFromDelta,
  blueprintMapFactFromProfileFact,
  blueprintMapProjection,
} from "./blueprint-map-projection.ts"
import {
  CUSTOMER_BLUEPRINT_SCHEMA,
  type CustomerBlueprintDraft,
} from "../services/customer-blueprint.ts"

const draft = (
  overrides: Partial<CustomerBlueprintDraft> = {},
): CustomerBlueprintDraft => ({
  schema: CUSTOMER_BLUEPRINT_SCHEMA,
  prospectRef: "prospect-a",
  revision: 2,
  createdAt: "2026-07-09T16:00:00.000Z",
  business: {
    facts: [
      {
        fact: 'company: "Acme Retail"',
        sourceTurnId: "turn-company",
        at: "2026-07-09T16:00:00.000Z",
      },
      {
        fact: 'stack: "Shopify and email"',
        sourceTurnId: "turn-stack",
        at: "2026-07-09T16:01:00.000Z",
      },
    ],
  },
  contacts: { email: "buyer@example.com", contactId: "oa_user:buyer" },
  needs: [
    {
      need: 'need: "we need support automation"',
      sourceTurnId: "turn-need",
      at: "2026-07-09T16:02:00.000Z",
    },
  ],
  suggestedModules: [
    {
      ref: "module.customer_support_ai",
      name: "Customer support AI",
      kind: "ai_employee_module",
      availability: "operator_assisted",
      pricingStatus: "owner_pricing_required",
      matchedNeedTurnIds: ["turn-need"],
      matchBasis: "semantic",
    },
    {
      ref: "workspace_pack.ecommerce",
      name: "E-commerce workspace pack",
      kind: "workspace_pack",
      availability: "operator_assisted",
      pricingStatus: "owner_pricing_required",
      matchedNeedTurnIds: [],
      matchBasis: "candidate_default",
    },
  ],
  sources: {
    turnIds: ["turn-company", "turn-stack", "turn-need"],
    factCount: 3,
    provenance:
      "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
  },
  handoff: {
    pipeline: "operator_assisted_business_workspace",
    automatedProvisioning: false,
    convergesWith:
      "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
    note: "Draft only.",
  },
  ...overrides,
})

describe("blueprint map projection (BM-2 #8628)", () => {
  test("empty state renders the prospect, pending fact slots, and pending account", () => {
    const projection = blueprintMapProjection({
      draft: null,
      facts: [],
      contactEmail: null,
      accountLinked: false,
      live: false,
    })
    expect(projection.nodes.map((node) => node.id)).toContain("prospect")
    for (const slot of ["company", "role", "stack", "contact"]) {
      const node = projection.nodes.find((entry) => entry.id === `fact:${slot}`)
      expect(node?.status).toBe("pending")
    }
    expect(projection.nodes.find((node) => node.id === "account")?.status).toBe("pending")
  })

  test("draft facts, needs, account, and semantic module matches project to GraphFigure models", () => {
    const projection = blueprintMapProjection({
      draft: draft(),
      facts: [],
      contactEmail: null,
      accountLinked: true,
      live: true,
    })
    expect(projection.nodes.find((node) => node.id === "prospect")?.status).toBe("active")
    expect(projection.nodes.find((node) => node.id === "fact:company")?.label).toContain("Acme")
    expect(projection.nodes.find((node) => node.id === "need:turn-need")?.status).toBe("active")
    expect(projection.nodes.find((node) => node.id === "offering:module.customer_support_ai")?.status).toBe("success")
    expect(projection.nodes.find((node) => node.id === "account")?.status).toBe("success")
    expect(
      projection.edges.find(
        (edge) =>
          edge.from === "need:turn-need" &&
          edge.to === "offering:module.customer_support_ai",
      )?.status,
    ).toBe("success")
    expect(
      projection.edges.some((edge) => edge.to === "offering:workspace_pack.ecommerce"),
    ).toBe(false)
  })

  test("candidate-default offerings collapse honestly without lighting provenance edges", () => {
    const projection = blueprintMapProjection({
      draft: draft({
        suggestedModules: [
          {
            ref: "module.sales_employee_ai",
            name: "Sales employee AI",
            kind: "ai_employee_module",
            availability: "operator_assisted",
            pricingStatus: "owner_pricing_required",
            matchedNeedTurnIds: [],
            matchBasis: "candidate_default",
          },
        ],
      }),
      facts: [],
      contactEmail: null,
      accountLinked: false,
      live: false,
    })
    expect(projection.nodes.find((node) => node.id === "offering:candidates")?.status).toBe("idle")
    expect(
      projection.edges.some((edge) => edge.from.startsWith("need:") && edge.status === "success"),
    ).toBe(false)
  })

  test("fact parsers convert stored facts and BM-1 deltas into projection facts", () => {
    expect(
      blueprintMapFactFromProfileFact({
        fact: 'role: "Head of Support"',
        sourceTurnId: "turn-role",
      }),
    ).toEqual({
      label: "role",
      text: "Head of Support",
      sourceTurnId: "turn-role",
    })
    expect(
      blueprintMapFactFromDelta({
        kind: "fact_added",
        label: "need",
        text: "we need billing support",
        sourceTurnId: "turn-need-2",
      }),
    ).toEqual({
      label: "need",
      text: "we need billing support",
      sourceTurnId: "turn-need-2",
    })
  })
})
