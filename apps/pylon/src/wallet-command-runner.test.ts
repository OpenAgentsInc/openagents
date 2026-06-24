import { describe, expect, test } from "bun:test"
import { agentWalletArgs, agentWalletCommandTimeoutMs } from "./wallet.js"

describe("agentWalletArgs", () => {
  test("passes MDK_WALLET_PORT through to the agent-wallet CLI", () => {
    expect(agentWalletArgs(["receive", "1"], { MDK_WALLET_PORT: "3457" } as NodeJS.ProcessEnv)).toEqual([
      "receive",
      "1",
      "--port",
      "3457",
    ])
  })

  test("does not duplicate an explicit port argument", () => {
    expect(
      agentWalletArgs(["send", "lnbc...", "--port", "3458"], {
        MDK_WALLET_PORT: "3457",
      } as NodeJS.ProcessEnv),
    ).toEqual(["send", "lnbc...", "--port", "3458"])
  })

  test("uses a live-friendly wallet command timeout with env override", () => {
    expect(agentWalletCommandTimeoutMs({} as NodeJS.ProcessEnv)).toBe(30_000)
    expect(
      agentWalletCommandTimeoutMs({
        MDK_WALLET_COMMAND_TIMEOUT_MS: "45000",
      } as NodeJS.ProcessEnv),
    ).toBe(45_000)
    expect(
      agentWalletCommandTimeoutMs({
        MDK_WALLET_COMMAND_TIMEOUT_MS: "nope",
      } as NodeJS.ProcessEnv),
    ).toBe(30_000)
  })
})
