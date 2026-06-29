import { describe, expect, test } from "bun:test"

import {
  buildCloudListRequest,
  buildDeployCloudRequest,
  parseCloudList,
} from "./cloud-client.js"

describe("cloud client request builders", () => {
  test("buildDeployCloudRequest carries objective and byo-key selection", () => {
    expect(buildDeployCloudRequest({
      objective: "ship the cloud coordinator",
      selection: "byo_key",
    })).toEqual({
      type: "cloud.deploy",
      objective: "ship the cloud coordinator",
      selection: "byo_key",
    })
  })

  test("buildDeployCloudRequest carries credits selection", () => {
    expect(buildDeployCloudRequest({
      objective: "run a paid cloud session",
      selection: "credits",
    }).selection).toBe("credits")
  })

  test("buildCloudListRequest builds the cloud list command", () => {
    expect(buildCloudListRequest()).toEqual({ type: "cloud.list" })
  })
})

describe("cloud client response parsers", () => {
  test("parseCloudList decodes cloud sessions", () => {
    const raw = [
      {
        cloudSessionRef: "cloud.session.fixture.0001",
        origin: "cloud",
        state: "running",
        region: "iad",
        costRef: "cost.fixture.0001",
      },
      {
        cloudSessionRef: "cloud.session.fixture.0002",
        origin: "cloud",
        state: "completed",
      },
    ]

    expect(parseCloudList(raw)).toEqual(raw as ReturnType<typeof parseCloudList>)
  })

  test("parseCloudList rejects non-array responses", () => {
    expect(() => parseCloudList({ cloudSessionRef: "cloud.session.fixture.0001" })).toThrow(
      "Expected cloud session list response to be an array",
    )
  })
})
