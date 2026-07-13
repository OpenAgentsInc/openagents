import { describe, expect, test } from "bun:test"

import { makePylonRemoteManagedCloudFleetRunCapacity } from "./fleet-run-managed-cloud-runner.js"

describe("remote managed FleetRun capacity", () => {
  test("admits only exact public Codex account hashes", async () => {
    const requested: string[] = []
    const capacity = makePylonRemoteManagedCloudFleetRunCapacity({
      agentToken: "oa_agent_test",
      baseUrl: "https://openagents.example",
      pylonRef: "pylon.test",
      fetchImpl: (async (input) => {
        requested.push(String(input))
        return Response.json({
          schema: "openagents.pylon.managed_cloud_fleet_capacity.v1",
          accountRefHashes: [
            "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa",
            "account.pylon.claude_agent.bbbbbbbbbbbbbbbbbbbbbbbb",
            "account.pylon.managed_cloud.broker",
          ],
        })
      }) as typeof fetch,
    })

    const accounts = await capacity.accounts({} as never)

    expect(requested).toEqual([
      "https://openagents.example/api/pylons/pylon.test/fleet-runs/managed-capacity",
    ])
    expect(accounts).toEqual([
      expect.objectContaining({
        accountRef: "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa",
        accountRefHash: "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa",
        executionTarget: "managed_cloud",
      }),
    ])
  })
})
