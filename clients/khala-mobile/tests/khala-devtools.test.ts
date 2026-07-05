import { describe, expect, test } from "bun:test"

import {
  createKhalaMobileDevtools,
  khalaDevtoolCommandNames,
} from "../src/devtools/khala-devtools"

describe("Khala mobile devtools", () => {
  test("exposes no commands in production mode", async () => {
    const devtools = createKhalaMobileDevtools({ dev: false })

    expect(devtools.available).toBe(false)
    expect(devtools.commands).toEqual([])
    await expect(devtools.execute("resetNavigation")).resolves.toEqual({
      messageSafe: "Khala mobile devtools are unavailable in production builds.",
      ok: false,
    })
  })

  test("runs public-safe dev commands through injected adapters", async () => {
    const calls: Array<string> = []
    const devtools = createKhalaMobileDevtools({
      connectivity: () => ({ reachable: true, targetKind: "simulator_loopback" }),
      dev: true,
      fixtures: {
        resetFixtureState: () => {
          calls.push("reset-fixtures")
        },
        seedFixtureThreads: () => {
          calls.push("seed-fixtures")
        },
      },
      navigation: {
        jumpToThread: thread => calls.push(`jump:${thread.threadId}:${thread.title}`),
        resetToThreads: () => calls.push("reset-navigation"),
      },
    })

    expect(devtools.available).toBe(true)
    expect(devtools.commands).toEqual(khalaDevtoolCommandNames)
    await expect(devtools.execute("resetNavigation")).resolves.toEqual({
      messageSafe: "Reset navigation to Threads.",
      ok: true,
    })
    await expect(devtools.execute("jumpToFixtureThread")).resolves.toEqual({
      messageSafe: "Opened public fixture thread.",
      ok: true,
    })
    await expect(devtools.execute("seedFixtureThreads")).resolves.toEqual({
      messageSafe: "Seeded public fixture threads.",
      ok: true,
    })
    await expect(devtools.execute("resetFixtureState")).resolves.toEqual({
      messageSafe: "Reset public fixture state.",
      ok: true,
    })
    await expect(devtools.execute("inspectConnectivity")).resolves.toEqual({
      messageSafe: "Connectivity reachable via simulator_loopback.",
      ok: true,
    })
    expect(calls).toEqual([
      "reset-navigation",
      "jump:thread.fixture.public:Fixture thread",
      "seed-fixtures",
      "reset-fixtures",
    ])
  })

  test("command names and messages avoid private data surfaces", () => {
    const joined = khalaDevtoolCommandNames.join(" ")
    expect(joined).not.toMatch(/token|bearer|chatBody|messageBody|providerPayload|email/i)
  })
})
