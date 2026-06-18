import { afterEach, describe, expect, spyOn, test } from "bun:test"
import { classifySparkBackupReceive, detectSparkBackupBalance } from "../src/wallet"
import { resolveSparkBackupHelper, sanitizeSparkDebug } from "../src/spark-backup-helper"

// #5194: instrumentation so ONE more deterministic-repro run reveals exactly
// where/why the Spark read bails. These tests pin:
//   (1) the gate dump fires (and is empty-stderr-diagnosable) when a read fails;
//   (2) the gate distinguishes an inert stub (no in-process SDK build attempted)
//       from a real SDK helper that ran and failed;
//   (3) the resolver wires the in-process helper on an explicit opt-in intent
//       even when PYLON_SPARK_BACKUP_ENABLED is not in the env (the disabled /
//       missing-env short-circuit fix);
//   (4) the debug lines stay path-sanitized and stderr-only (never a projection).

const TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

const DEBUG_ENV = "PYLON_SPARK_DEBUG"

function withSparkDebug<T>(fn: () => T): { result: T; lines: string[] } {
  const prior = process.env[DEBUG_ENV]
  process.env[DEBUG_ENV] = "1"
  const lines: string[] = []
  const spy = spyOn(console, "error").mockImplementation((...args: unknown[]) => {
    lines.push(args.map((a) => String(a)).join(" "))
  })
  try {
    const result = fn()
    return { result, lines }
  } finally {
    spy.mockRestore()
    if (prior === undefined) delete process.env[DEBUG_ENV]
    else process.env[DEBUG_ENV] = prior
  }
}

async function withSparkDebugAsync<T>(fn: () => Promise<T>): Promise<{ result: T; lines: string[] }> {
  const prior = process.env[DEBUG_ENV]
  process.env[DEBUG_ENV] = "1"
  const lines: string[] = []
  const spy = spyOn(console, "error").mockImplementation((...args: unknown[]) => {
    lines.push(args.map((a) => String(a)).join(" "))
  })
  try {
    const result = await fn()
    return { result, lines }
  } finally {
    spy.mockRestore()
    if (prior === undefined) delete process.env[DEBUG_ENV]
    else process.env[DEBUG_ENV] = prior
  }
}

afterEach(() => {
  // Defensive: never leak the debug flag across files.
  delete process.env[DEBUG_ENV]
})

describe("#5194 spark read failure visibility", () => {
  test("gate dump fires with helperWired=false when no in-process helper was injected (inert stub)", async () => {
    // No `helper` injected => classify falls back to the inert stub, which is
    // EXACTLY the silent dead-end on a host where the resolver returned null.
    const { result, lines } = await withSparkDebugAsync(() =>
      classifySparkBackupReceive({
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      }),
    )
    expect(result.state).toBe("helper-unavailable")
    const gate = lines.find((l) => l.includes("[spark-helper:gate]") && l.includes("classify"))
    expect(gate).toBeDefined()
    // The operator MUST be able to tell the stub case apart in one shot.
    expect(gate).toContain("helperWired=false")
    expect(gate).toContain("no in-process SDK build attempted")
    // Even an empty stderr is diagnosable: the line carries lengths.
    expect(gate).toMatch(/exitCode=\d+/)
    expect(gate).toMatch(/stderrLen=\d+/)
  })

  test("gate dump distinguishes a real helper that ran and failed (helperWired=true)", async () => {
    const failingHelper = async () => ({ exitCode: 1, stdout: "", stderr: "" })
    const { result, lines } = await withSparkDebugAsync(() =>
      classifySparkBackupReceive({
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: failingHelper,
      }),
    )
    expect(result.state).toBe("helper-unavailable")
    // An EMPTY stderr from a real helper is the worst case — but now it is
    // explicitly visible (helperWired=true, stderrLen=0) instead of silent.
    const gate = lines.find((l) => l.includes("[spark-helper:gate]") && l.includes("classify"))
    expect(gate).toBeDefined()
    expect(gate).toContain("helperWired=true")
    expect(gate).toContain("stderrLen=0")
    expect(gate).not.toContain("no in-process SDK build attempted")
  })

  test("detectSparkBackupBalance dumps the status gate when the read fails", async () => {
    const failingHelper = async () => ({ exitCode: 1, stdout: "", stderr: "" })
    const { result, lines } = await withSparkDebugAsync(() => detectSparkBackupBalance(failingHelper))
    expect(result.helperReady).toBe(false)
    const gate = lines.find((l) => l.includes("[spark-helper:gate]") && l.includes("detect:status"))
    expect(gate).toBeDefined()
    expect(gate).toMatch(/exitCode=1/)
    expect(gate).toMatch(/stderrLen=0/)
  })

  test("no gate dump on a successful read (success path is untouched)", async () => {
    const okHelper = async (command: string) =>
      command === "status"
        ? { exitCode: 0, stdout: JSON.stringify({ balance_sats: 0, unclaimed_deposit_count: 0 }), stderr: "" }
        : { exitCode: 0, stdout: JSON.stringify({ spark_address: "spark1qexample" }), stderr: "" }
    const { lines } = await withSparkDebugAsync(() =>
      classifySparkBackupReceive({
        enabled: true,
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        helper: okHelper as never,
      }),
    )
    expect(lines.some((l) => l.includes("[spark-helper:gate]"))).toBe(false)
  })
})

describe("#5194 resolver opt-in short-circuit fix", () => {
  test("explicit enabled:true wires the in-process helper even with PYLON_SPARK_BACKUP_ENABLED unset", () => {
    // The historical bug: the resolver consulted ONLY env.PYLON_SPARK_BACKUP_ENABLED,
    // so an operator who never exported it got a null helper => the gate ran the
    // inert stub and NEVER attempted the in-process SDK build.
    const helper = resolveSparkBackupHelper({
      env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv, // note: NO PYLON_SPARK_BACKUP_ENABLED
      mnemonic: TEST_MNEMONIC,
      enabled: true,
    })
    expect(helper).not.toBeNull()
    expect(typeof helper).toBe("function")
  })

  test("null only when the OFF override is set (#5304 default-ON)", () => {
    // #5304: the backup is ON by default. A node with a seed + credential and NO
    // flag now resolves a helper; the resolver is null ONLY when the operator
    // sets an explicit OFF override.
    expect(
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        mnemonic: TEST_MNEMONIC,
      }),
    ).not.toBeNull()
    const { result, lines } = withSparkDebug(() =>
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k", PYLON_SPARK_BACKUP_DISABLED: "1" } as NodeJS.ProcessEnv,
        mnemonic: TEST_MNEMONIC,
      }),
    )
    expect(result).toBeNull()
    expect(lines.some((l) => l.includes("[spark-helper:resolve]") && l.includes("opt-out override"))).toBe(true)
  })

  test("env flag alone still wires the helper (back-compat)", () => {
    const helper = resolveSparkBackupHelper({
      env: { PYLON_SPARK_BACKUP_ENABLED: "1", OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
      mnemonic: TEST_MNEMONIC,
      enabled: false,
    })
    // enabled:false does not override a true env flag — the env flag still counts.
    expect(helper).not.toBeNull()
  })

  test("explicit enabled:true still null without a seed (seed gate preserved)", () => {
    const { result, lines } = withSparkDebug(() =>
      resolveSparkBackupHelper({
        env: { OPENAGENTS_SPARK_API_KEY: "k" } as NodeJS.ProcessEnv,
        mnemonic: null,
        enabled: true,
      }),
    )
    expect(result).toBeNull()
    expect(lines.some((l) => l.includes("[spark-helper:resolve]") && l.includes("no wallet seed"))).toBe(true)
  })
})

describe("#5194 debug sanitizer", () => {
  test("strips absolute home/temp paths but keeps URLs, hosts, and error types", () => {
    const raw =
      "StorageError: failed to open /Users/orrery/.pylon/wallet/spark-backup/sdk/storage.sql via https://api.breez.example/sync"
    const out = sanitizeSparkDebug(raw)
    expect(out).not.toContain("/Users/orrery")
    expect(out).toContain("<path>")
    expect(out).toContain("StorageError")
    expect(out).toContain("https://api.breez.example/sync")
  })
})
