import {
  decodeProductSpecEditProposalRequest,
  decodeProductSpecEvidenceRequest,
  decodeProductSpecPacketBlockRequest,
  decodeProductSpecPlanProposalRequest,
  decodeProductSpecRunGetRequest,
} from "./product-spec-workroom-contract.ts"
import type { ProductSpecWorkroom } from "./product-spec-workroom.ts"
import type { CodexAppServerRequest } from "./codex-app-server-client.ts"

type JsonSchema = Readonly<Record<string, unknown>>
type DynamicFunction = Readonly<{
  type: "function"
  name: string
  description: string
  inputSchema: JsonSchema
}>

const ref = { type: "string", pattern: "^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$" } as const
const identity = {
  type: "object",
  additionalProperties: false,
  required: ["specRef", "relativePath", "revision", "digest"],
  properties: {
    specRef: ref,
    relativePath: { type: "string" },
    revision: { type: "integer", minimum: 1 },
    digest: { type: "string", pattern: "^sha256:[a-f0-9]{64}$" },
  },
} as const

const fn = (name: string, description: string, required: string[], properties: JsonSchema): DynamicFunction => ({
  type: "function",
  name,
  description,
  inputSchema: { type: "object", additionalProperties: false, required, properties },
})

/** Proposal/report-only host tools. Admission, verification, and owner review are intentionally absent. */
export const ProductSpecDynamicTools = [{
  type: "namespace" as const,
  name: "product_spec",
  description: "Typed OpenAgents ProductSpec proposal and work-report operations",
  tools: [
    fn("get_run", "Read one exact host-owned ProductSpec run projection.", ["runRef"], { runRef: ref }),
    fn("propose_edit", "Propose Markdown for the next ProductSpec revision; the owner must confirm it.",
      ["workContextRef", "expectedCurrent", "proposedMarkdown"], {
        workContextRef: ref,
        expectedCurrent: identity,
        proposedMarkdown: { type: "string", minLength: 1, maxLength: 1_000_000 },
      }),
    fn("propose_plan", "Propose criterion-addressed work packets and allocation; the owner must accept the plan.",
      ["workContextRef", "spec", "packets", "deferredCriterionIds"], {
        workContextRef: ref,
        spec: identity,
        packets: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["packetRef", "title", "criterionIds", "dependencyRefs", "allocation"],
            properties: {
              packetRef: ref,
              title: { type: "string", minLength: 1, maxLength: 500 },
              criterionIds: { type: "array", items: { type: "string" } },
              dependencyRefs: { type: "array", items: ref },
              allocation: { type: "string", enum: ["root", "child"] },
            },
          },
        },
        deferredCriterionIds: { type: "array", items: { type: "string" } },
      }),
    fn("report_blocked", "Report a blocker against the exact active packet lease.",
      ["runRef", "packetRef", "leaseRef", "reason", "expectedSpec"], {
        runRef: ref, packetRef: ref, leaseRef: ref,
        reason: { type: "string", minLength: 1, maxLength: 2_000 },
        expectedSpec: identity,
      }),
    fn("record_evidence", "Record typed terminal evidence from the exact active packet lease; this does not verify it.",
      ["runRef", "packetRef", "leaseRef", "evidenceRef", "evidenceKind", "expectedSpec"], {
        runRef: ref, packetRef: ref, leaseRef: ref, evidenceRef: ref,
        evidenceKind: { type: "string", enum: ["test_run", "behavior_eval", "artifact", "diff_review", "receipt"] },
        expectedSpec: identity,
      }),
  ],
}] as const

export type ProductSpecToolAuthority = Readonly<{
  workContextRef: string
  service: ProductSpecWorkroom
}>

const response = (value: unknown, success: boolean) => ({
  contentItems: [{ type: "inputText", text: JSON.stringify(value) }],
  success,
})

export const handleProductSpecDynamicTool = (
  request: CodexAppServerRequest,
  authority: ProductSpecToolAuthority | null,
): unknown | null => {
  if (request.method !== "item/tool/call") return null
  const params = request.params !== null && typeof request.params === "object"
    ? request.params as Record<string, unknown>
    : null
  if (params?.namespace !== "product_spec") return null
  if (authority === null) return response({ ok: false, reason: "incompatible_workflow", message: "No admitted ProductSpec work context is selected." }, false)
  const args = params.arguments
  const invoke = <A>(decoded: A | null, work: (value: A) => unknown): unknown => {
    if (decoded === null) return response({ ok: false, reason: "invalid_request", message: "The ProductSpec tool arguments are invalid." }, false)
    const result = work(decoded)
    return response(result, typeof result === "object" && result !== null && "ok" in result && result.ok === true)
  }
  switch (params.tool) {
    case "get_run":
      return invoke(decodeProductSpecRunGetRequest(args), value => authority.service.run(value.runRef))
    case "propose_edit":
      return invoke(decodeProductSpecEditProposalRequest(args), value => value.workContextRef !== authority.workContextRef
        ? { ok: false, reason: "invalid_request", message: "The ProductSpec work context is not selected." }
        : authority.service.proposeEdit(value))
    case "propose_plan":
      return invoke(decodeProductSpecPlanProposalRequest(args), value => value.workContextRef !== authority.workContextRef
        ? { ok: false, reason: "invalid_request", message: "The ProductSpec work context is not selected." }
        : authority.service.proposePlan(value))
    case "report_blocked":
      return invoke(decodeProductSpecPacketBlockRequest(args), value => authority.service.blockPacket(value))
    case "record_evidence":
      return invoke(decodeProductSpecEvidenceRequest(args), value => authority.service.recordEvidence(value))
    default:
      return response({ ok: false, reason: "invalid_request", message: "Unknown ProductSpec tool." }, false)
  }
}
