import { describe, expect, test } from "bun:test"
import {
  composeFrlmRecursiveResponse,
  emitFrlmRlmStepTrace,
  planFrlmConductorExecution,
  type FrlmResponseCompositionSegment,
  FrlmConductor,
  scheduleFrlmConductor,
  type FrlmConductorSubQuery,
} from "../src/frlm-conductor-execution"
import { assertPublicProjectionSafe } from "../src/state"

const observedAt = "2026-06-28T16:40:00.000Z"

describe("FRLM Conductor execution planning", () => {
  test("keeps the recursive plan when all sub-queries completed", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: completedSubQueries(),
    })

    expect(projection.canExecute).toBe(true)
    expect(projection.executionMode).toBe("recursive_parallel")
    expect(projection.budgetPolicyRef).toBe("budget_policy.artanis.frlm_conductor.issue_6683.v1")
    expect(projection.tokenBudget).toBe(12_000)
    expect(projection.projectedTokenCount).toBe(4_800)
    expect(projection.depthLimit).toBe(2)
    expect(projection.projectedMaxDepth).toBe(1)
    expect(projection.blockerRefs).toEqual([])
    expect(projection.failedSubQueryRefs).toEqual([])
    expect(projection.linearFallbackStepRefs).toEqual([])
    expect(projection.executionPlanRef).toMatch(/^plan\.artanis\.frlm\.recursive_parallel\.[a-f0-9]{20}$/)
    assertPublicProjectionSafe(projection)
  })

  test("keeps planned and running sub-queries in the recursive fanout schedule", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.collect_context.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.collect_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "running",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.verify_blueprint_boundary.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "planned",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection.canExecute).toBe(true)
    expect(projection.executionMode).toBe("recursive_parallel")
    expect(projection.recursiveSubQueryRefs).toEqual([
      "subquery.artanis.frlm.collect_context.v1",
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    expect(projection.failedSubQueryRefs).toEqual([])
    expect(projection.linearFallbackStepRefs).toEqual([])
    expect(projection.evidenceRefs).toContain("result.artanis.frlm.collect_context.v1")
    assertPublicProjectionSafe(projection)
  })

  test("falls back to linear execution when any recursive sub-query fails", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        completedSubQueries()[0],
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "failed",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        completedSubQueries()[2],
      ],
    })

    expect(projection.canExecute).toBe(true)
    expect(projection.executionMode).toBe("fallback_linear")
    expect(projection.blockerRefs).toEqual([])
    expect(projection.failedSubQueryRefs).toEqual(["subquery.artanis.frlm.optimize_route.v1"])
    expect(projection.linearFallbackStepRefs).toHaveLength(3)
    expect(projection.linearFallbackStepRefs.every((ref) => ref.startsWith("step.artanis.frlm.linear."))).toBe(true)
    expect(projection.fallbackReasonRef).toMatch(/^reason\.artanis\.frlm\.sub_query_failure\.[a-f0-9]{20}$/)
    expect(projection.evidenceRefs).toContain("failure.artanis.frlm.optimize_route.timeout.v1")
    expect(projection.evidenceRefs).toContain("executor.artanis.frlm.linear_local.v1")
    assertPublicProjectionSafe(projection)
  })

  test("deduplicates recursive fanout refs and emits deterministic fallback steps", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "timed_out",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.collect_context.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.collect_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "failed",
          failureRef: "failure.artanis.frlm.optimize_route.retry_exhausted.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })
    const replayed = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.collect_context.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.collect_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "timed_out",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection.canExecute).toBe(true)
    expect(projection.executionMode).toBe("fallback_linear")
    expect(projection.recursiveSubQueryRefs).toEqual([
      "subquery.artanis.frlm.collect_context.v1",
      "subquery.artanis.frlm.optimize_route.v1",
    ])
    expect(projection.failedSubQueryRefs).toEqual(["subquery.artanis.frlm.optimize_route.v1"])
    expect(projection.linearFallbackStepRefs).toHaveLength(2)
    expect(projection.linearFallbackStepRefs).toEqual(replayed.linearFallbackStepRefs)
    expect(projection.evidenceRefs).toContain("failure.artanis.frlm.optimize_route.retry_exhausted.v1")
    expect(projection.evidenceRefs).toContain("failure.artanis.frlm.optimize_route.timeout.v1")
    assertPublicProjectionSafe(projection)
  })

  test("blocks a failed sub-query when linear fallback is not enabled", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      linearFallbackEnabled: false,
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.collect_blueprint_signatures.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "rejected",
          failureRef: "failure.artanis.frlm.collect_blueprint_signatures.policy.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.executionMode).toBe("blocked")
    expect(projection.blockerRefs).toContain(
      "blocker.artanis.frlm_conductor.sub_query_failure_without_linear_fallback",
    )
  })

  test("blocks execution when recursive sub-query tokens exceed the BudgetPolicy", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      budgetPolicy: {
        budgetPolicyRef: "budget_policy.artanis.frlm_conductor.tight_tokens.v1",
        maxTokens: 4_000,
        maxDepth: 3,
      },
      subQueries: completedSubQueries(),
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.executionMode).toBe("blocked")
    expect(projection.tokenBudget).toBe(4_000)
    expect(projection.projectedTokenCount).toBe(4_800)
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.token_budget_exceeded")
    expect(projection.executionPlanRef).toBeNull()
    assertPublicProjectionSafe(projection)
  })

  test("blocks execution when recursive sub-query depth exceeds the BudgetPolicy", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        completedSubQueries()[0],
        {
          subQueryRef: "subquery.artanis.frlm.inspect_nested_context.v1",
          parentRef: "subquery.artanis.frlm.collect_context.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.inspect_nested_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
          tokenCount: 1_200,
        },
        {
          subQueryRef: "subquery.artanis.frlm.inspect_leaf_context.v1",
          parentRef: "subquery.artanis.frlm.inspect_nested_context.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.inspect_leaf_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
          tokenCount: 900,
        },
      ],
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.executionMode).toBe("blocked")
    expect(projection.depthLimit).toBe(2)
    expect(projection.projectedMaxDepth).toBe(3)
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.depth_limit_exceeded")
    expect(projection.executionPlanRef).toBeNull()
    assertPublicProjectionSafe(projection)
  })

  test("blocks linear fallback when no local executor ref is present", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      linearExecutorRef: null,
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.collect_blueprint_signatures.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "failed",
          failureRef: "failure.artanis.frlm.collect_blueprint_signatures.policy.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.executionMode).toBe("blocked")
    expect(projection.executionPlanRef).toBeNull()
    expect(projection.linearExecutorRef).toBeNull()
    expect(projection.blockerRefs).toEqual([
      "blocker.artanis.frlm_conductor.linear_executor_ref_missing",
    ])
    expect(projection.fallbackReasonRef).toMatch(/^reason\.artanis\.frlm\.sub_query_failure\.[a-f0-9]{20}$/)
    assertPublicProjectionSafe(projection)
  })

  test("blocks execution when the BudgetPolicy is missing or invalid", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      budgetPolicy: null,
      subQueries: completedSubQueries(),
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.executionMode).toBe("blocked")
    expect(projection.budgetPolicyRef).toBeNull()
    expect(projection.tokenBudget).toBeNull()
    expect(projection.depthLimit).toBeNull()
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.budget_policy_ref_missing")
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.budget_policy_invalid")
    assertPublicProjectionSafe(projection)
  })

  test("blocks unsafe refs before they reach public projection fields", () => {
    const projection = planFrlmConductorExecution({
      ...validInput(),
      subQueries: [
        {
          subQueryRef: "/Users/operator/private/subquery.json",
          state: "completed",
          resultRef: "result.artanis.frlm.collect_context.v1",
        },
      ],
    })

    expect(projection.canExecute).toBe(false)
    expect(projection.recursiveSubQueryRefs).toEqual([])
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.sub_query_plan_missing")
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.unsafe_ref")
    assertPublicProjectionSafe(projection)
  })

  test("integrates FRLM conductor evidence into a public-safe scheduler projection", () => {
    const projection = planFrlmConductorExecution({
      observedAt,
      executionRef: "execution.artanis.frlm.issue_6688.scheduler.v1",
      rootTaskRef: "task.artanis.frlm.root.issue_6654.v1",
      blueprintSignatureRef: "signature.blueprint.frlm_conductor.v1",
      budgetPolicy: {
        budgetPolicyRef: "budget_policy.artanis.frlm_conductor.issue_6688.v1",
        maxTokens: 12_000,
        maxDepth: 2,
      },
      linearFallbackEnabled: true,
      linearExecutorRef: "executor.artanis.frlm.local_executor.v1",
      subQueries: [
        {
          subQueryRef: "subquery.artanis.frlm.nip90.collect_context.v1",
          parentRef: "task.artanis.frlm.root.issue_6654.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.nip90.collect_context.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.nip90.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.issue_6654.v1",
          state: "rejected",
          failureRef: "failure.artanis.frlm.nip90.policy_budget.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.nip90.verify_blueprint_boundary.v1",
          parentRef: "task.artanis.frlm.root.issue_6654.v1",
          state: "running",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection).toMatchObject({
      contentRedacted: true,
      canExecute: true,
      executionMode: "fallback_linear",
      linearExecutorRef: "executor.artanis.frlm.local_executor.v1",
      blockerRefs: [],
    })
    expect(projection.executionPlanRef).toMatch(/^plan\.artanis\.frlm\.fallback_linear\.[a-f0-9]{20}$/)
    expect(projection.recursiveSubQueryRefs).toEqual([
      "subquery.artanis.frlm.nip90.collect_context.v1",
      "subquery.artanis.frlm.nip90.optimize_route.v1",
      "subquery.artanis.frlm.nip90.verify_blueprint_boundary.v1",
    ])
    expect(projection.failedSubQueryRefs).toEqual([
      "subquery.artanis.frlm.nip90.optimize_route.v1",
    ])
    expect(projection.linearFallbackStepRefs).toHaveLength(3)
    expect(projection.evidenceRefs).toContain("signature.blueprint.frlm_conductor.v1")
    expect(projection.evidenceRefs).toContain("signature.blueprint.rlm_subquery.v1")
    expect(projection.evidenceRefs).toContain("failure.artanis.frlm.nip90.policy_budget.v1")
    expect(projection.authorityBoundary).toContain("does not dispatch workers")
    assertPublicProjectionSafe(projection)
  })
})

describe("FRLM recursive response composition", () => {
  test("composes completed recursive sub-query responses in deterministic order", () => {
    const executionPlan = planFrlmConductorExecution({
      ...validInput(),
      subQueries: completedSubQueries(),
    })
    const projection = composeFrlmRecursiveResponse({
      observedAt,
      compositionRef: "composition.artanis.frlm.issue_6682.response.v1",
      executionPlan,
      responseBlueprintSignatureRef: "signature.blueprint.frlm_response_composer.v1",
      segments: completedResponseSegments(),
    })

    expect(projection.canComposeResponse).toBe(true)
    expect(projection.blockerRefs).toEqual([])
    expect(projection.orderedSubQueryRefs).toEqual([
      "subquery.artanis.frlm.collect_context.v1",
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    expect(projection.composedResponseText).toBe([
      "[1] Collect public FRLM, RLM, and Blueprint signature context before planning.",
      "[2] Prefer recursive fanout while every sub-query has completed evidence.",
      "[3] Keep the response under the Blueprint evidence-only authority boundary.",
    ].join("\n\n"))
    expect(projection.composedResponseDigest).toMatch(/^[a-f0-9]{32}$/)
    expect(projection.composedResponseRef).toMatch(/^response\.artanis\.frlm\.composed\.[a-f0-9]{20}$/)
    expect(projection.evidenceRefs).toContain("signature.blueprint.frlm_response_composer.v1")
    expect(projection.evidenceRefs).toContain("response.artanis.frlm.collect_context.v1")
    assertPublicProjectionSafe(projection)
  })

  test("blocks response composition until every sub-query has result and response text", () => {
    const executionPlan = planFrlmConductorExecution({
      ...validInput(),
      subQueries: completedSubQueries(),
    })
    const projection = composeFrlmRecursiveResponse({
      observedAt,
      compositionRef: "composition.artanis.frlm.issue_6682.response.v1",
      executionPlan,
      responseBlueprintSignatureRef: "signature.blueprint.frlm_response_composer.v1",
      segments: [
        completedResponseSegments()[0],
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          state: "running",
          responseRef: "response.artanis.frlm.optimize_route.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        {
          subQueryRef: "subquery.artanis.frlm.verify_blueprint_boundary.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.verify_blueprint_boundary.v1",
          responseText: " ",
          responseRef: "response.artanis.frlm.verify_blueprint_boundary.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(projection.canComposeResponse).toBe(false)
    expect(projection.composedResponseText).toBeNull()
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_response_composition.sub_query_incomplete")
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_response_composition.sub_query_result_missing")
    expect(projection.blockerRefs).toContain(
      "blocker.artanis.frlm_response_composition.sub_query_response_text_missing",
    )
    expect(projection.incompleteSubQueryRefs).toEqual(["subquery.artanis.frlm.optimize_route.v1"])
    expect(projection.missingResultSubQueryRefs).toEqual(["subquery.artanis.frlm.optimize_route.v1"])
    expect(projection.missingResponseTextSubQueryRefs).toEqual([
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    assertPublicProjectionSafe(projection)
  })

  test("blocks unsafe response content before it reaches public projection fields", () => {
    const executionPlan = planFrlmConductorExecution({
      ...validInput(),
      subQueries: completedSubQueries(),
    })
    const projection = composeFrlmRecursiveResponse({
      observedAt,
      compositionRef: "composition.artanis.frlm.issue_6682.response.v1",
      executionPlan,
      responseBlueprintSignatureRef: "signature.blueprint.frlm_response_composer.v1",
      segments: [
        {
          ...completedResponseSegments()[0],
          responseText: "The sandbox printed bearer abcdef123456 and must be redacted.",
        },
      ],
    })

    expect(projection.canComposeResponse).toBe(false)
    expect(projection.composedResponseText).toBeNull()
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_response_composition.unsafe_content")
    assertPublicProjectionSafe(projection)
  })
})

describe("FRLM RLM structured step trace emission", () => {
  test("emits public-safe structured RLM steps for recursive execution", () => {
    const trace = emitFrlmRlmStepTrace({
      ...validInput(),
      subQueries: completedSubQueries(),
    })

    expect(trace.executionMode).toBe("recursive_parallel")
    expect(trace.blockerRefs).toEqual([])
    expect(trace.traceRef).toMatch(/^trace\.artanis\.frlm\.rlm\.[a-f0-9]{20}$/)
    expect(trace.traceDigestRef).toMatch(/^trace\.artanis\.frlm\.rlm\.digest\.[a-f0-9]{20}$/)
    expect(trace.steps.map((step) => step.kind)).toEqual([
      "blueprint_gate",
      "recursive_sub_query",
      "recursive_sub_query",
      "recursive_sub_query",
      "result_synthesis",
    ])
    expect(trace.steps.map((step) => step.stepIndex)).toEqual([0, 1, 2, 3, 4])
    expect(trace.steps[0]?.blueprintSignatureRef).toBe("signature.blueprint.frlm_conductor.v1")
    expect(trace.steps[1]?.blueprintSignatureRef).toBe("signature.blueprint.rlm_subquery.v1")
    expect(trace.steps[1]?.evidenceRefs).toContain("result.artanis.frlm.collect_context.v1")
    expect(trace.evidenceRefs).toContain(trace.steps[4]!.stepRef)
    assertPublicProjectionSafe(trace)
  })

  test("emits linear fallback steps for failed recursive sub-queries", () => {
    const trace = emitFrlmRlmStepTrace({
      ...validInput(),
      subQueries: [
        completedSubQueries()[0]!,
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "failed",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(trace.executionMode).toBe("fallback_linear")
    expect(trace.blockerRefs).toEqual([])
    expect(trace.steps.filter((step) => step.kind === "linear_fallback")).toHaveLength(2)
    expect(trace.steps.some((step) => step.evidenceRefs.includes("executor.artanis.frlm.linear_local.v1"))).toBe(true)
    expect(trace.steps.some((step) => step.evidenceRefs.includes("failure.artanis.frlm.optimize_route.timeout.v1"))).toBe(true)
    assertPublicProjectionSafe(trace)
  })

  test("redacts unsafe RLM refs from trace steps and blocks the projection", () => {
    const trace = emitFrlmRlmStepTrace({
      ...validInput(),
      subQueries: [
        {
          subQueryRef: "/Users/operator/private/rlm-step.json",
          parentRef: "task.artanis.frlm.root.v1",
          state: "completed",
          resultRef: "result.artanis.frlm.safe.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
    })

    expect(trace.executionMode).toBe("blocked")
    expect(trace.blockerRefs).toContain("blocker.artanis.frlm_conductor.unsafe_ref")
    expect(trace.steps).toHaveLength(1)
    expect(JSON.stringify(trace)).not.toContain("/Users/operator")
    assertPublicProjectionSafe(trace)
  })
})

describe("FRLM Conductor core scheduler", () => {
  test("batches planned recursive sub-queries through the configured submitter", () => {
    const conductor = new FrlmConductor({
      recursiveSubmitterRef: "submitter.artanis.frlm.nip90.v1",
      maxParallelSubQueries: 2,
      linearFallbackEnabled: true,
      linearExecutorRef: "executor.artanis.frlm.linear_local.v1",
    })
    const schedule = conductor.schedule({
      ...validInput(),
      subQueries: plannedSubQueries(),
    })

    expect(schedule.canSchedule).toBe(true)
    expect(schedule.state).toBe("recursive_fanout_ready")
    expect(schedule.recursiveBatches).toHaveLength(2)
    expect(schedule.recursiveBatches.map((batch) => batch.subQueryRefs)).toEqual([
      [
        "subquery.artanis.frlm.collect_context.v1",
        "subquery.artanis.frlm.optimize_route.v1",
      ],
      ["subquery.artanis.frlm.verify_blueprint_boundary.v1"],
    ])
    expect(schedule.recursiveBatches.every((batch) =>
      batch.submitterRef === "submitter.artanis.frlm.nip90.v1"
    )).toBe(true)
    expect(schedule.nextActionRef).toBe(schedule.recursiveBatches[0]?.batchRef)
    expect(schedule.blockerRefs).toEqual([])
    assertPublicProjectionSafe(schedule)
  })

  test("turns recursive failures into ordered local linear fallback steps", () => {
    const schedule = scheduleFrlmConductor({
      ...validInput(),
      recursiveSubmitterRef: "submitter.artanis.frlm.nip90.v1",
      subQueries: [
        completedSubQueries()[0],
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "timed_out",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
        completedSubQueries()[2],
      ],
    })

    expect(schedule.canSchedule).toBe(true)
    expect(schedule.state).toBe("linear_fallback_ready")
    expect(schedule.recursiveBatches).toEqual([])
    expect(schedule.linearFallbackSteps.map((step) => step.subQueryRef)).toEqual([
      "subquery.artanis.frlm.collect_context.v1",
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    expect(schedule.linearFallbackSteps.every((step) =>
      step.executorRef === "executor.artanis.frlm.linear_local.v1"
    )).toBe(true)
    expect(schedule.nextActionRef).toBe(schedule.linearFallbackSteps[0]?.stepRef)
    assertPublicProjectionSafe(schedule)
  })

  test("blocks planned fanout without a recursive submitter ref", () => {
    const schedule = scheduleFrlmConductor({
      ...validInput(),
      recursiveSubmitterRef: null,
      subQueries: plannedSubQueries(),
    })

    expect(schedule.canSchedule).toBe(false)
    expect(schedule.state).toBe("blocked")
    expect(schedule.blockerRefs).toContain(
      "blocker.artanis.frlm_conductor.recursive_submitter_ref_missing",
    )
    expect(schedule.recursiveBatches).toEqual([])
  })

  test("blocks plans that exceed the scheduler's public budget", () => {
    const schedule = scheduleFrlmConductor({
      ...validInput(),
      recursiveSubmitterRef: "submitter.artanis.frlm.nip90.v1",
      subQueries: plannedSubQueries(),
      budget: { maxSubQueries: 2 },
    })

    expect(schedule.canSchedule).toBe(false)
    expect(schedule.state).toBe("blocked")
    expect(schedule.blockerRefs).toContain(
      "blocker.artanis.frlm_conductor.sub_query_budget_exceeded",
    )
    assertPublicProjectionSafe(schedule)
  })
})

function validInput() {
  return {
    observedAt,
    executionRef: "execution.artanis.frlm.issue_6684.v1",
    rootTaskRef: "task.artanis.frlm.root.v1",
    blueprintSignatureRef: "signature.blueprint.frlm_conductor.v1",
    budgetPolicy: {
      budgetPolicyRef: "budget_policy.artanis.frlm_conductor.issue_6683.v1",
      maxTokens: 12_000,
      maxDepth: 2,
    },
    linearFallbackEnabled: true,
    linearExecutorRef: "executor.artanis.frlm.linear_local.v1",
  }
}

function completedSubQueries(): FrlmConductorSubQuery[] {
  return [
    {
      subQueryRef: "subquery.artanis.frlm.collect_context.v1",
      parentRef: "task.artanis.frlm.root.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.collect_context.v1",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      tokenCount: 1_500,
    },
    {
      subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
      parentRef: "task.artanis.frlm.root.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.optimize_route.v1",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      tokenCount: 1_800,
    },
    {
      subQueryRef: "subquery.artanis.frlm.verify_blueprint_boundary.v1",
      parentRef: "task.artanis.frlm.root.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.verify_blueprint_boundary.v1",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      tokenCount: 1_500,
    },
  ]
}

function completedResponseSegments(): FrlmResponseCompositionSegment[] {
  return [
    {
      subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.optimize_route.v1",
      responseRef: "response.artanis.frlm.optimize_route.v1",
      responseText: "Prefer recursive fanout while every sub-query has completed evidence.",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      order: 2,
    },
    {
      subQueryRef: "subquery.artanis.frlm.collect_context.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.collect_context.v1",
      responseRef: "response.artanis.frlm.collect_context.v1",
      responseText: "Collect public FRLM, RLM, and Blueprint signature context before planning.",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      order: 1,
    },
    {
      subQueryRef: "subquery.artanis.frlm.verify_blueprint_boundary.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.verify_blueprint_boundary.v1",
      responseRef: "response.artanis.frlm.verify_blueprint_boundary.v1",
      responseText: "Keep the response under the Blueprint evidence-only authority boundary.",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
      order: 3,
    },
  ]
}

function plannedSubQueries(): FrlmConductorSubQuery[] {
  return completedSubQueries().map((subQuery) => ({
    subQueryRef: subQuery.subQueryRef,
    parentRef: subQuery.parentRef,
    state: "planned",
    blueprintSignatureRef: subQuery.blueprintSignatureRef,
  }))
}
