import { describe, expect, test } from "bun:test"

import {
  makeSarahFleetBrowserCoordinator,
  type SarahFleetBrowserConfig,
  type SarahFleetBrowserRuntime,
} from "./fleet-browser-host.ts"
import {
  canonicalSarahFleetRunUrl,
  makeSarahFleetStartResultHandler,
  selectSarahFleetStartConfig,
} from "./fleet-start-result.ts"

const result = (digest = "a".repeat(20)) => ({
  toolCallId: `tool.fleet.${digest}`,
  toolName: "coding_fleet_start",
  ok: true,
  output: {
    ok: true,
    duplicate: false,
    policy: {
      source: "openagents_server_policy",
      relationshipMode: "operator",
      codingFleetStartAllowed: true,
      fleetObservationAllowed: true,
      retrievalScope: "owner_fleet_runs",
      responsePosture: "state_oriented",
      uiDensity: "dense",
      administratorToolsAllowed: false,
    },
    routeRef: "route.sarah.fleet_runs.authority.v1",
    run: {
      runRef: `fleet_run.sarah.${digest}`,
      scope: `scope.fleet_run.fleet_run.sarah.${digest}`,
      status: "pending_executor",
      objective: "Run the bounded issue list through the owner fleet.",
      repository: {
        owner: "OpenAgentsInc",
        name: "openagents",
        branch: "main",
        commit: "9".repeat(40),
      },
      verifier: { kind: "command", command: "bun test" },
      workSource: { kind: "issue_list", issueRefs: ["#8637"] },
      workerPolicy: { workerKind: "auto", targetPreference: "owner_local" },
      targetConcurrency: 3,
      createdAt: "2026-07-09T12:00:00.000Z",
      updatedAt: "2026-07-09T12:00:00.000Z",
      privateMaterialExcluded: true,
    },
  },
})

describe("successful Sarah FleetRun tool-result boundary", () => {
  test("selects only the exact successful result and derives its scope", () => {
    const selected = selectSarahFleetStartConfig([result()])
    expect(selected?.runRef).toBe(`fleet_run.sarah.${"a".repeat(20)}`)
    expect(String(selected?.scope)).toBe(
      `scope.fleet_run.fleet_run.sarah.${"a".repeat(20)}`,
    )
    expect(
      canonicalSarahFleetRunUrl(
        "https://openagents.com/sarah?panel=blueprint&runRef=hostile",
        selectSarahFleetStartConfig([result()])!,
      ),
    ).toBe(
      `https://openagents.com/sarah?panel=blueprint&fleet_run=fleet_run.sarah.${"a".repeat(20)}`,
    )
  })

  test("failures, malformed/private output, conflicting runs, and prose select nothing", () => {
    expect(
      selectSarahFleetStartConfig([
        { ...result(), ok: false, output: { error: "store_unavailable" } },
      ]),
    ).toBeNull()
    expect(
      selectSarahFleetStartConfig([
        {
          ...result(),
          output: {
            ...result().output,
            privatePrompt: "PRIVATE model prose",
          },
        },
      ]),
    ).toBeNull()
    expect(
      selectSarahFleetStartConfig([
        {
          ...result(),
          output: {
            ...result().output,
            run: { ...result().output.run, scope: "scope.fleet_run.foreign" },
          },
        },
      ]),
    ).toBeNull()
    expect(selectSarahFleetStartConfig([result(), result("b".repeat(20))])).toBeNull()
    // The decoder consumes toolResults only; model prose that resembles a
    // success envelope is never a selection source.
    expect(
      selectSarahFleetStartConfig(
        `Sarah says ${JSON.stringify(result())}`,
      ),
    ).toBeNull()
  })

  test("coalesces duplicate results and disposes before a new exact run", () => {
    const events: string[] = []
    const makeRuntime = (config: SarahFleetBrowserConfig): SarahFleetBrowserRuntime => ({
      config,
      start: async () => {
        events.push(`start:${config.runRef}`)
      },
      snapshot: () => ({
        config,
        connection: { phase: "idle" },
        projection: null,
      }),
      subscribe: (listener) => {
        listener({
          config,
          connection: { phase: "idle" },
          projection: null,
        })
        return () => events.push(`unsubscribe:${config.runRef}`)
      },
      commands: {} as SarahFleetBrowserRuntime["commands"],
      dispose: () => events.push(`dispose:${config.runRef}`),
    })
    const coordinator = makeSarahFleetBrowserCoordinator({
      makeRuntime,
      onState: () => {},
    })
    const navigations: string[] = []
    const handle = makeSarahFleetStartResultHandler({
      coordinator,
      currentUrl: () => "https://openagents.com/sarah",
      navigate: (url) => navigations.push(url),
    })

    expect(handle([result()])?.runRef).toEndWith("a".repeat(20))
    expect(handle([result(), result()])?.runRef).toEndWith("a".repeat(20))
    expect(handle([result("b".repeat(20))])?.runRef).toEndWith("b".repeat(20))
    expect(navigations).toHaveLength(2)
    expect(events).toEqual([
      `start:fleet_run.sarah.${"a".repeat(20)}`,
      `unsubscribe:fleet_run.sarah.${"a".repeat(20)}`,
      `dispose:fleet_run.sarah.${"a".repeat(20)}`,
      `start:fleet_run.sarah.${"b".repeat(20)}`,
    ])
  })
})
