#!/usr/bin/env bun

import { spawn } from "node:child_process"

import { parseOvernightArgs, runOvernight, usage, validateEnv, type RunCommand } from "./dse-overnight-lib"

const runCommand: RunCommand = async ({ cwd, command, args, env, timeoutMs }) => {
  const startedAt = Date.now()

  return await new Promise((resolve) => {
    const child = spawn(command, [...args], {
      cwd,
      env: { ...process.env, ...(env ?? {}) },
      stdio: ["ignore", "pipe", "pipe"],
    })

    let stdout = ""
    let stderr = ""

    child.stdout?.on("data", (buf) => {
      stdout += String(buf)
    })
    child.stderr?.on("data", (buf) => {
      stderr += String(buf)
    })

    let timedOut = false
    const t = setTimeout(() => {
      timedOut = true
      try {
        child.kill("SIGKILL")
      } catch {
        // ignore
      }
    }, Math.max(1, timeoutMs))

    child.on("close", (code) => {
      clearTimeout(t)
      resolve({
        ok: code === 0 && !timedOut,
        code: typeof code === "number" ? code : null,
        stdout,
        stderr,
        durationMs: Date.now() - startedAt,
        timedOut,
      })
    })
  })
}

const main = async (): Promise<number> => {
  const parsed = parseOvernightArgs(process.argv.slice(2))
  if (!parsed.ok) {
    console.error(parsed.error)
    console.error(parsed.usage)
    return 2
  }

  const env = validateEnv(process.env as Record<string, string | undefined>)
  if (!env) {
    console.error("missing OA_DSE_ADMIN_SECRET")
    console.error(usage())
    return 2
  }

  const summary = await runOvernight({
    options: parsed.options,
    env,
    fetchFn: fetch,
    runCommand,
  })

  // Keep stdout machine-readable for agents.
  try {
    console.log(JSON.stringify(summary, null, 2))
  } catch {
    console.log(String(summary.ok))
  }

  return summary.ok ? 0 : 1
}

main()
  .then((code) => {
    process.exitCode = code
  })
  .catch((err) => {
    console.error(String(err))
    process.exitCode = 1
  })

