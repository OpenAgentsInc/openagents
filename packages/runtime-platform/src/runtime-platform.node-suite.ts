import assert from "node:assert/strict"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, test } from "vite-plus/test"
import WebSocket, { type RawData } from "ws"

import { Runtime } from "./index.ts"

describe("runtime-platform under stock Node", () => {
  test("file, write, JSON, bytes, and stream preserve content", async () => {
    const directory = await mkdtemp(join(tmpdir(), "oa-runtime-platform-"))
    try {
      const path = join(directory, "fixture.json")
      const content = JSON.stringify({ node: true })
      assert.equal(await Runtime.write(path, content), Buffer.byteLength(content))
      const fixture = Runtime.file(path)
      assert.equal(await fixture.exists(), true)
      assert.equal(await fixture.text(), content)
      assert.deepEqual(await fixture.json(), { node: true })
      assert.equal(new TextDecoder().decode(await fixture.bytes()), content)
      assert.equal(new TextDecoder().decode(await new Response(fixture.stream()).arrayBuffer()), content)
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  })

  test("spawn and spawnSync expose exit status and exact output", async () => {
    const child = Runtime.spawn([process.execPath, "-e", "process.stdout.write('node-child')"], { stdout: "pipe" })
    assert.equal(new TextDecoder().decode(await new Response(child.stdout).arrayBuffer()), "node-child")
    assert.equal(await child.exited, 0)

    const sync = Runtime.spawnSync({ cmd: [process.execPath, "-e", "process.stderr.write('node-sync');process.exit(7)"] })
    assert.equal(sync.exitCode, 7)
    assert.equal(new TextDecoder().decode(sync.stderr), "node-sync")
  })

  test("isMain resolves symlinked entry paths", async () => {
    const directory = await mkdtemp(join(tmpdir(), "oa-runtime-main-"))
    try {
      const entry = join(directory, "entry.mjs")
      const runtimeUrl = new URL("./index.ts", import.meta.url).href
      await writeFile(entry, `import { Runtime } from ${JSON.stringify(runtimeUrl)}; if (Runtime.isMain(import.meta.url)) process.stdout.write("main")\n`)
      const alias = entry.replace(/^\/private\/tmp\//, "/tmp/")
      const child = Runtime.spawn([process.execPath, alias], { stdout: "pipe", stderr: "pipe" })
      assert.equal(await new Response(child.stdout).text(), "main")
      assert.equal(await child.exited, 0)
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  })

  test("zstd round-trips exact bytes", () => {
    const source = new TextEncoder().encode("openagents-node-zstd\0".repeat(32))
    assert.deepEqual(Runtime.zstdDecompressSync(Runtime.zstdCompressSync(source)), source)
  })

  test("build emits named disk artifacts and in-memory browser outputs", async () => {
    const directory = await mkdtemp(join(tmpdir(), "oa-runtime-build-"))
    try {
      const entry = join(directory, "entry.ts")
      const outdir = join(directory, "dist")
      await writeFile(entry, "export const answer: number = 42\n")
      const disk = await Runtime.build({ entrypoints: [entry], outdir, naming: "qa.js", target: "node" })
      assert.equal(disk.success, true)
      assert.equal(await readFile(join(outdir, "qa.js"), "utf8").then((source) => source.includes("answer")), true)

      const memory = await Runtime.build({ entrypoints: [entry], target: "browser", format: "iife" })
      assert.equal(memory.success, true)
      assert.equal(memory.outputs.length, 1)
      assert.match(await memory.outputs[0].text(), /answer/)
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  })

  test("HTTP port zero becomes usable only after explicit readiness", async () => {
    const server = Runtime.serve({
      hostname: "127.0.0.1",
      port: 0,
      fetch: (request) => new Response(new URL(request.url).pathname),
    })
    await server.ready
    try {
      assert.notEqual(server.port, 0)
      assert.equal(await fetch(new URL("/ready", server.url)).then((response) => response.text()), "/ready")
    } finally {
      await server.stop(true)
    }
  })

  test("WebSocket upgrade carries typed data and echoes binary-safe messages", async () => {
    const server = Runtime.serve<{ client: string }>({
      hostname: "127.0.0.1",
      port: 0,
      fetch: (request, runtimeServer) => runtimeServer.upgrade(request, { data: { client: "node" } })
        ? undefined
        : new Response("upgrade required", { status: 426 }),
      websocket: {
        message: (socket, message) => {
          socket.send(`${socket.data.client}:${typeof message === "string" ? message : new TextDecoder().decode(message)}`)
        },
      },
    })
    await server.ready
    try {
      const response = await new Promise<string>((resolve, reject) => {
        const socket = new WebSocket(server.url.href.replace("http:", "ws:"))
        socket.once("open", () => socket.send("hello"))
        socket.once("message", (data: RawData) => { resolve(data.toString()); socket.close() })
        socket.once("error", reject)
      })
      assert.equal(response, "node:hello")
    } finally {
      await server.stop(true)
    }
  })
})
