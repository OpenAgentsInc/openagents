import { describe, expect, test } from "bun:test"
import {
  planFrlmConductorExecution,
  planFrlmConductorPylonDispatch,
  type FrlmConductorPylonSlot,
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
    expect(projection.blockerRefs).toEqual([])
    expect(projection.failedSubQueryRefs).toEqual([])
    expect(projection.linearFallbackStepRefs).toEqual([])
    expect(projection.executionPlanRef).toMatch(/^plan\.artanis\.frlm\.recursive_parallel\.[a-f0-9]{20}$/)
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

  test("dispatches planned recursive sub-queries across available Pylon slots", () => {
    const projection = planFrlmConductorPylonDispatch({
      ...validInput(),
      subQueries: plannedSubQueries(),
      pylonSlots: pylonSlots(),
    })

    expect(projection.canDispatch).toBe(true)
    expect(projection.blockerRefs).toEqual([])
    expect(projection.dispatchWidth).toBe(3)
    expect(projection.dispatchAssignments.map((assignment) => assignment.subQueryRef)).toEqual([
      "subquery.artanis.frlm.collect_context.v1",
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    expect(projection.dispatchAssignments.map((assignment) => assignment.slotRef)).toEqual([
      "slot.pylon.codex.primary.0",
      "slot.pylon.codex.primary.0",
      "slot.pylon.codex.secondary.0",
    ])
    expect(projection.dispatchAssignments.map((assignment) => assignment.laneIndex)).toEqual([0, 1, 0])
    expect(projection.dispatchAssignments.every((assignment) =>
      assignment.dispatchRef.startsWith("dispatch.artanis.frlm.pylon."))).toBe(true)
    expect(projection.queuedSubQueryRefs).toEqual([])
    assertPublicProjectionSafe(projection)
  })

  test("queues planned sub-queries beyond advertised Pylon capacity", () => {
    const projection = planFrlmConductorPylonDispatch({
      ...validInput(),
      subQueries: plannedSubQueries(),
      pylonSlots: [
        {
          slotRef: "slot.pylon.codex.primary.0",
          pylonRef: "pylon.operator.primary",
          accountRef: "codex.primary",
          capacity: 1,
          busy: 0,
          ready: true,
        },
      ],
    })

    expect(projection.canDispatch).toBe(true)
    expect(projection.dispatchWidth).toBe(1)
    expect(projection.dispatchAssignments).toHaveLength(1)
    expect(projection.queuedSubQueryRefs).toEqual([
      "subquery.artanis.frlm.optimize_route.v1",
      "subquery.artanis.frlm.verify_blueprint_boundary.v1",
    ])
    assertPublicProjectionSafe(projection)
  })

  test("blocks dispatch when the execution plan fell back to linear mode", () => {
    const projection = planFrlmConductorPylonDispatch({
      ...validInput(),
      subQueries: [
        plannedSubQueries()[0],
        {
          subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
          parentRef: "task.artanis.frlm.root.v1",
          state: "failed",
          failureRef: "failure.artanis.frlm.optimize_route.timeout.v1",
          blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
        },
      ],
      pylonSlots: pylonSlots(),
    })

    expect(projection.canDispatch).toBe(false)
    expect(projection.dispatchAssignments).toEqual([])
    expect(projection.blockerRefs).toContain(
      "blocker.artanis.frlm_conductor.dispatch.execution_not_recursive_parallel",
    )
    assertPublicProjectionSafe(projection)
  })

  test("blocks unsafe Pylon slot refs before dispatch projection", () => {
    const projection = planFrlmConductorPylonDispatch({
      ...validInput(),
      subQueries: plannedSubQueries(),
      pylonSlots: [
        {
          slotRef: "/Users/operator/private/slot.json",
          pylonRef: "pylon.operator.primary",
          capacity: 1,
          ready: true,
        },
      ],
    })

    expect(projection.canDispatch).toBe(false)
    expect(projection.dispatchAssignments).toEqual([])
    expect(projection.availableSlotRefs).toEqual([])
    expect(projection.blockerRefs).toContain(
      "blocker.artanis.frlm_conductor.dispatch.no_available_pylon_slots",
    )
    expect(projection.blockerRefs).toContain("blocker.artanis.frlm_conductor.dispatch.unsafe_ref")
    assertPublicProjectionSafe(projection)
  })
})

function validInput() {
  return {
    observedAt,
    executionRef: "execution.artanis.frlm.issue_6684.v1",
    rootTaskRef: "task.artanis.frlm.root.v1",
    blueprintSignatureRef: "signature.blueprint.frlm_conductor.v1",
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
    },
    {
      subQueryRef: "subquery.artanis.frlm.optimize_route.v1",
      parentRef: "task.artanis.frlm.root.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.optimize_route.v1",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
    },
    {
      subQueryRef: "subquery.artanis.frlm.verify_blueprint_boundary.v1",
      parentRef: "task.artanis.frlm.root.v1",
      state: "completed",
      resultRef: "result.artanis.frlm.verify_blueprint_boundary.v1",
      blueprintSignatureRef: "signature.blueprint.rlm_subquery.v1",
    },
  ]
}

function plannedSubQueries(): FrlmConductorSubQuery[] {
  return completedSubQueries().map((subQuery) => ({
    ...subQuery,
    resultRef: null,
    state: "planned",
  }))
}

function pylonSlots(): FrlmConductorPylonSlot[] {
  return [
    {
      slotRef: "slot.pylon.codex.primary.0",
      pylonRef: "pylon.operator.primary",
      accountRef: "codex.primary",
      capacity: 2,
      busy: 0,
      ready: true,
      capabilityRefs: ["capability.pylon.local_codex"],
    },
    {
      slotRef: "slot.pylon.codex.secondary.0",
      pylonRef: "pylon.operator.secondary",
      accountRef: "codex.secondary",
      capacity: 1,
      busy: 0,
      ready: true,
      capabilityRefs: ["capability.pylon.local_codex"],
    },
  ]
}
