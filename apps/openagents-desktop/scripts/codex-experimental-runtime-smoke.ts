import { spawn } from "node:child_process"
import { mkdtempSync, rmSync } from "node:fs"
import { createServer } from "node:net"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"
import { makeCodexExperimentalRuntime } from "../src/codex-experimental-runtime.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact installed Codex executable")
const root = mkdtempSync(join(tmpdir(), "oa-codex-experimental-"))
const port = await new Promise<number>((resolve, reject) => { const server = createServer(); server.once("error", reject); server.listen(0, "127.0.0.1", () => { const address = server.address(); if (typeof address !== "object" || address === null) return reject(new Error("ephemeral port missing")); server.close(error => error === undefined ? resolve(address.port) : reject(error)) }) })
const execServer = spawn(binary, ["exec-server", "--listen", `ws://127.0.0.1:${port}`], { cwd: root, env: process.env, stdio: ["ignore", "pipe", "pipe"] })
let execServerDiagnostic = ""; let execServerExit: number | null = null
execServer.stdout.on("data", value => { execServerDiagnostic = `${execServerDiagnostic}${String(value)}`.slice(-4_096) }); execServer.stderr.on("data", value => { execServerDiagnostic = `${execServerDiagnostic}${String(value)}`.slice(-4_096) }); execServer.on("exit", code => { execServerExit = code ?? -1 })
const waitForPort = async () => { for (let attempt = 0; attempt < 400; attempt += 1) { if (execServerExit !== null) throw new Error(`exec-server exited ${execServerExit}: ${execServerDiagnostic}`); if (execServerDiagnostic.includes(`ws://127.0.0.1:${port}`)) return; await new Promise(resolve => setTimeout(resolve, 50)) }; throw new Error(`exec-server did not listen: ${execServerDiagnostic}`) }
const supervisor = createCodexAppServerSupervisor({ strictGeneratedDecoding: true })
try {
  await waitForPort()
  const lease = await supervisor.acquire({ binary, env: process.env, cwd: root, accountRef: "codex-current", hostTarget: "cap10-smoke", experimentalApi: true })
  const runtime = makeCodexExperimentalRuntime({ lease, spoolRoot: join(root, ".spool"), receiptPath: join(root, ".receipts.json") })
  const environment = { environmentId: "cap10-local-exec-server", execServerUrl: `ws://127.0.0.1:${port}`, connectTimeoutMs: 10_000 }
  await runtime.addEnvironment(environment, runtime.authorize("environment_add", environment, runtime.snapshot().revision))
  const environmentRef = runtime.snapshot().environments[0]?.environmentRef
  if (environmentRef === undefined) throw new Error("remote environment omitted public identity")
  const threadResponse = await lease.request("thread/start", { cwd: root, environments: [runtime.turnEnvironment(environmentRef, root)], ephemeral: true, approvalPolicy: "never", sandbox: "read-only" })
  if (typeof (threadResponse as { thread?: { id?: unknown } }).thread?.id !== "string") throw new Error("remote-targeted thread omitted identity")

  const processInput = { command: [process.execPath, "-e", "process.stdout.write('cap10-process-ok')"], cwd: root, timeoutMs: 30_000 }
  const processRef = await runtime.spawnProcess(processInput, runtime.authorize("process_spawn", processInput, runtime.snapshot().revision))
  const immediate = runtime.snapshot().processes.find(value => value.processRef === processRef)
  const settled = immediate !== undefined && immediate.state !== "running" ? immediate : await new Promise<ReturnType<typeof runtime.snapshot>["processes"][number]>((resolve, reject) => { const timeout = setTimeout(() => { remove(); reject(new Error("experimental process did not exit")) }, 30_000); const remove = runtime.subscribe(snapshot => { const process = snapshot.processes.find(value => value.processRef === processRef); if (process !== undefined && process.state !== "running") { clearTimeout(timeout); remove(); resolve(process) } }) })
  if (settled.state !== "exited" || settled.exitCode !== 0 || settled.stdoutBytes === 0) throw new Error(`experimental process failed: ${JSON.stringify(settled)}`)
  runtime.close()
  console.log("Verified remote environment targeting and owned unsandboxed process execution through the exact packaged experimental app-server.")
} finally {
  supervisor.close()
  execServer.kill("SIGTERM")
  rmSync(root, { recursive: true, force: true })
}
