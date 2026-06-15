import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  ControlEndpointError,
  resolveControlEndpoint,
  runControlCommand,
} from "../src/node/control-cli"

// CL-5035: the CLI bridge resolves the loopback control endpoint (home +
// control-token) the same way the node does, then forwards typed commands.
const dirs: string[] = []
function tempHome(withToken?: string): string {
  const dir = mkdtempSync(join(tmpdir(), "pylon-cli-"))
  dirs.push(dir)
  if (withToken !== undefined) writeFileSync(join(dir, "control-token"), `${withToken}\n`)
  return dir
}

afterEach(() => {
  while (dirs.length > 0) {
    const dir = dirs.pop()!
    try {
      rmSync(dir, { recursive: true, force: true })
    } catch {
      // best-effort
    }
  }
})

describe("resolveControlEndpoint", () => {
  test("reads the control token from the resolved home", async () => {
    const home = tempHome("0123456789abcdef0123")
    const endpoint = await resolveControlEndpoint({ PYLON_HOME: home } as NodeJS.ProcessEnv)
    expect(endpoint.token).toBe("0123456789abcdef0123")
    expect(endpoint.tokenPath).toBe(join(home, "control-token"))
    expect(endpoint.baseUrl).toBe("http://127.0.0.1:4716")
  })

  test("prefers PYLON_CONTROL_TOKEN over the on-disk file", async () => {
    const home = tempHome("file-token-value-xxxx")
    const endpoint = await resolveControlEndpoint({
      PYLON_HOME: home,
      PYLON_CONTROL_TOKEN: "env-token-value-yyyy",
    } as NodeJS.ProcessEnv)
    expect(endpoint.token).toBe("env-token-value-yyyy")
  })

  test("honors PYLON_CONTROL_HOST + PYLON_CONTROL_PORT", async () => {
    const home = tempHome("0123456789abcdef0123")
    const endpoint = await resolveControlEndpoint({
      PYLON_HOME: home,
      PYLON_CONTROL_HOST: "127.0.0.1",
      PYLON_CONTROL_PORT: "4799",
    } as NodeJS.ProcessEnv)
    expect(endpoint.baseUrl).toBe("http://127.0.0.1:4799")
  })

  test("throws no_token when no token is available", async () => {
    const home = tempHome()
    await expect(resolveControlEndpoint({ PYLON_HOME: home } as NodeJS.ProcessEnv)).rejects.toMatchObject({
      code: "no_token",
    })
  })
})

describe("runControlCommand", () => {
  test("maps a refused connection to ControlEndpointError(no_node)", async () => {
    const home = tempHome("0123456789abcdef0123")
    // Point at a port nothing is listening on.
    let caught: unknown
    try {
      await runControlCommand({ type: "session.list" }, {
        PYLON_HOME: home,
        PYLON_CONTROL_PORT: "4798",
      } as NodeJS.ProcessEnv)
    } catch (error) {
      caught = error
    }
    expect(caught).toBeInstanceOf(ControlEndpointError)
    expect((caught as ControlEndpointError).code).toBe("no_node")
  })
})
