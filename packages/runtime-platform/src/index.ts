/**
 * Narrow Node platform seam for retained TypeScript runtimes during VP-2.
 *
 * Domain code imports this object instead of reaching into Bun globals. The
 * surface intentionally mirrors only the process/file/server primitives the
 * retained applications use. It is implemented entirely with stock Node 24
 * built-ins plus `ws`; no legacy runtime or source-TypeScript loader is needed in
 * a compiled artifact.
 */
import { spawn as nodeSpawn, spawnSync as nodeSpawnSync } from "node:child_process"
import { createHash } from "node:crypto"
import { createReadStream, existsSync, realpathSync } from "node:fs"
import { access, readFile, stat, writeFile } from "node:fs/promises"
import * as http from "node:http"
import { delimiter, join, resolve } from "node:path"
import { Readable, Writable } from "node:stream"
import { fileURLToPath } from "node:url"
import { zstdCompressSync, zstdDecompressSync } from "node:zlib"
import { WebSocketServer } from "ws"

type Stdio = "inherit" | "ignore" | "pipe"
type RuntimeEnv = Record<string, string | undefined>

const cleanEnv = (env: RuntimeEnv | undefined): NodeJS.ProcessEnv | undefined => {
  if (env === undefined) return undefined
  return Object.fromEntries(Object.entries(env).filter((entry): entry is [string, string] => entry[1] !== undefined))
}

export interface RuntimeSpawnOptions {
  readonly cwd?: string
  readonly env?: RuntimeEnv
  readonly stdin?: Stdio
  readonly stdout?: Stdio
  readonly stderr?: Stdio
  readonly detached?: boolean
}

export interface RuntimeSubprocess {
  readonly exited: Promise<number>
  readonly stdout: ReadableStream<Uint8Array>
  readonly stderr: ReadableStream<Uint8Array>
  readonly stdin: NodeJS.WritableStream
  readonly pid: number
  readonly exitCode: number | null
  readonly signalCode: NodeJS.Signals | null
  kill(signal?: NodeJS.Signals | number): void
  unref(): void
}

export const spawn = (
  command: ReadonlyArray<string>,
  options: RuntimeSpawnOptions = {},
): RuntimeSubprocess => {
  const [executable, ...args] = command
  if (executable === undefined) throw new TypeError("runtime spawn requires a command")
  const child = nodeSpawn(executable, args, {
    ...(options.cwd === undefined ? {} : { cwd: options.cwd }),
    ...(options.env === undefined ? {} : { env: cleanEnv(options.env) }),
    ...(options.detached === undefined ? {} : { detached: options.detached }),
    stdio: [options.stdin ?? "ignore", options.stdout ?? "ignore", options.stderr ?? "ignore"],
  })
  const exited = new Promise<number>((resolve) => {
    child.once("error", () => resolve(127))
    child.once("close", (code) => resolve(code ?? 0))
  })
  const emptyReadable = () => new ReadableStream<Uint8Array>({ start: (controller) => controller.close() })
  const emptyWritable = () => new Writable({ write: (_chunk, _encoding, callback) => callback() })
  return {
    exited,
    stdout: child.stdout === null ? emptyReadable() : Readable.toWeb(child.stdout) as unknown as ReadableStream<Uint8Array>,
    stderr: child.stderr === null ? emptyReadable() : Readable.toWeb(child.stderr) as unknown as ReadableStream<Uint8Array>,
    stdin: child.stdin ?? emptyWritable(),
    pid: child.pid ?? -1,
    get exitCode() { return child.exitCode },
    get signalCode() { return child.signalCode },
    kill: (signal) => { child.kill(signal) },
    unref: () => { child.unref() },
  }
}

type RuntimeSpawnSyncOptions = {
  readonly cwd?: string
  readonly env?: RuntimeEnv
  readonly stdin?: Stdio
  readonly stdout?: Stdio
  readonly stderr?: Stdio
}

export function spawnSync(input: { readonly cmd: ReadonlyArray<string> } & RuntimeSpawnSyncOptions): { readonly exitCode: number; readonly stdout: Buffer; readonly stderr: Buffer }
export function spawnSync(command: ReadonlyArray<string>, options?: RuntimeSpawnSyncOptions): { readonly exitCode: number; readonly stdout: Buffer; readonly stderr: Buffer }
export function spawnSync(input: ({
  readonly cmd: ReadonlyArray<string>
} & RuntimeSpawnSyncOptions) | ReadonlyArray<string>, separateOptions: RuntimeSpawnSyncOptions = {}): { readonly exitCode: number; readonly stdout: Buffer; readonly stderr: Buffer } {
  const hasCommandProperty = "cmd" in input
  const options: RuntimeSpawnSyncOptions = hasCommandProperty ? input : separateOptions
  const command: ReadonlyArray<string> = hasCommandProperty ? input.cmd : input
  const [executable, ...args] = command
  if (executable === undefined) throw new TypeError("runtime spawnSync requires a command")
  const result = nodeSpawnSync(executable, args, {
    ...(options.cwd === undefined ? {} : { cwd: options.cwd }),
    ...(options.env === undefined ? {} : { env: cleanEnv(options.env) }),
    stdio: [options.stdin ?? "ignore", options.stdout ?? "pipe", options.stderr ?? "pipe"],
    encoding: "buffer",
  })
  return {
    exitCode: result.status ?? (result.error === undefined ? 0 : 127),
    stdout: result.stdout instanceof Uint8Array ? Buffer.from(result.stdout) : Buffer.alloc(0),
    stderr: result.stderr instanceof Uint8Array ? Buffer.from(result.stderr) : Buffer.alloc(0),
  }
}

export interface RuntimeBuildOptions {
  readonly entrypoints: ReadonlyArray<string>
  readonly outdir?: string
  readonly naming?: string
  readonly target?: "node" | "browser"
  readonly format?: "esm" | "cjs" | "iife"
  readonly minify?: boolean
  readonly external?: ReadonlyArray<string>
  readonly define?: Readonly<Record<string, string>>
  readonly sourcemap?: boolean | "inline" | "external"
  readonly plugins?: ReadonlyArray<{
    readonly name: string
    setup(build: unknown): void | Promise<void>
  }>
}

export interface RuntimeBuildArtifact {
  readonly path: string
  bytes(): Promise<Uint8Array>
  arrayBuffer(): Promise<ArrayBuffer>
  text(): Promise<string>
}

export interface RuntimeBuildResult {
  readonly success: boolean
  readonly logs: ReadonlyArray<string>
  readonly outputs: ReadonlyArray<RuntimeBuildArtifact>
}

export const build = async (options: RuntimeBuildOptions): Promise<RuntimeBuildResult> => {
  const { build: esbuild } = await import("esbuild")
  try {
    const result = await esbuild({
      entryPoints: [...options.entrypoints],
      ...(options.outdir === undefined ? { write: false } : { outdir: options.outdir }),
      ...(options.naming === undefined ? {} : { entryNames: options.naming.replace(/\.js$/, "") }),
      bundle: true,
      platform: options.target === "browser" ? "browser" : "node",
      target: options.target === "browser" ? "es2022" : "node24",
      format: options.format ?? "esm",
      ...(
        options.target !== "browser" && (options.format ?? "esm") === "esm"
          ? { banner: { js: 'import { createRequire as __openagentsCreateRequire } from "node:module";const require=__openagentsCreateRequire(import.meta.url);' } }
          : {}
      ),
      minify: options.minify ?? false,
      external: options.external === undefined ? [] : [...options.external],
      ...(options.define === undefined ? {} : { define: { ...options.define } }),
      ...(options.sourcemap === undefined ? {} : { sourcemap: options.sourcemap }),
      ...(options.plugins === undefined ? {} : { plugins: [...options.plugins] as never }),
      logLevel: "silent",
    })
    const outputs = (result.outputFiles ?? []).map((output): RuntimeBuildArtifact => ({
      path: output.path,
      bytes: async () => new Uint8Array(output.contents),
      arrayBuffer: async () => output.contents.buffer.slice(
        output.contents.byteOffset,
        output.contents.byteOffset + output.contents.byteLength,
      ) as ArrayBuffer,
      text: async () => output.text,
    }))
    return { success: true, logs: [], outputs }
  } catch (error) {
    const logs = typeof error === "object" && error !== null && "errors" in error && Array.isArray(error.errors)
      ? error.errors.map((entry) => JSON.stringify(entry))
      : [error instanceof Error ? error.message : String(error)]
    return { success: false, logs, outputs: [] }
  }
}

export interface RuntimeFile {
  exists(): Promise<boolean>
  text(): Promise<string>
  json(): Promise<unknown>
  bytes(): Promise<Uint8Array>
  arrayBuffer(): Promise<ArrayBuffer>
  stat(): ReturnType<typeof stat>
  stream(): ReadableStream<Uint8Array>
}

export const file = (path: string | URL): RuntimeFile => ({
  exists: async () => access(path).then(() => true, () => false),
  text: () => readFile(path, "utf8"),
  json: async () => JSON.parse(await readFile(path, "utf8")) as unknown,
  bytes: async () => new Uint8Array(await readFile(path)),
  arrayBuffer: async () => {
    const bytes = await readFile(path)
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
  },
  stat: () => stat(path),
  stream: () => Readable.toWeb(createReadStream(path)) as unknown as ReadableStream<Uint8Array>,
})

export const write = async (path: string | URL, data: string | Uint8Array | ArrayBuffer): Promise<number> => {
  const bytes = typeof data === "string" ? Buffer.from(data) : data instanceof ArrayBuffer ? new Uint8Array(data) : data
  await writeFile(path, bytes)
  return bytes.byteLength
}

export const sleep = (milliseconds: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, milliseconds))

export const which = (name: string): string | null => {
  if (name.includes("/")) return existsSync(name) ? name : null
  for (const directory of (process.env.PATH ?? "").split(delimiter)) {
    if (directory.length === 0) continue
    const candidate = join(directory, name)
    if (existsSync(candidate)) return candidate
  }
  return null
}

export const hash = (value: string | Uint8Array): bigint =>
  BigInt(`0x${createHash("sha256").update(value).digest("hex").slice(0, 16)}`)

export const isMain = (url: string): boolean => {
  const entry = process.argv[1]
  if (entry === undefined) return false
  const modulePath = fileURLToPath(url)
  try {
    return realpathSync(modulePath) === realpathSync(entry)
  } catch {
    return resolve(modulePath) === resolve(entry)
  }
}

const readStdinBytes = async (): Promise<Uint8Array> => {
  const chunks: Array<Buffer> = []
  for await (const chunk of process.stdin) chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))
  return new Uint8Array(Buffer.concat(chunks))
}

type RuntimeWebSocketHandler<Data> = {
  readonly open?: (socket: RuntimeServerWebSocket<Data>) => void | Promise<void>
  readonly message?: (socket: RuntimeServerWebSocket<Data>, message: string | Uint8Array) => void | Promise<void>
  readonly close?: (socket: RuntimeServerWebSocket<Data>, code: number, reason: string) => void | Promise<void>
}

export interface RuntimeServerWebSocket<Data> {
  data: Data
  readonly readyState: number
  send(message: string | Uint8Array): number
  close(code?: number, reason?: string): void
}

export interface RuntimeServer<Data = unknown> {
  readonly hostname: string
  readonly port: number
  readonly url: URL
  /** Resolves only after Node has bound the socket and `port` is final. */
  readonly ready: Promise<void>
  upgrade(request: Request, options: { readonly data: Data }): boolean
  stop(closeActiveConnections?: boolean): Promise<void>
}

type RuntimeServeOptions<Data> = {
  readonly [key: string]: unknown
  readonly hostname?: string
  readonly port?: number
  readonly fetch: (request: Request, server: RuntimeServer<Data>) => Response | undefined | Promise<Response | undefined>
  readonly websocket?: RuntimeWebSocketHandler<Data> & Record<string, unknown>
}

type UpgradeContext = {
  readonly request: import("node:http").IncomingMessage
  readonly socket: import("node:stream").Duplex
  readonly head: Buffer
  upgraded: boolean
}

export const serve = <Data = unknown>(options: RuntimeServeOptions<Data>): RuntimeServer<Data> => {
  const WebSocketServerForRuntime = WebSocketServer as unknown as new (options: { noServer: boolean }) => {
      handleUpgrade(request: unknown, socket: unknown, head: Buffer, callback: (socket: RuntimeServerWebSocket<Data> & { on(event: string, listener: (...args: Array<any>) => void): void }) => void): void
      close(callback?: () => void): void
      clients: Set<{ terminate(): void }>
  }
  const hostname = options.hostname ?? "0.0.0.0"
  const requestedPort = options.port ?? 3000
  const idleTimeoutSeconds = typeof options.idleTimeout === "number"
    ? options.idleTimeout
    : typeof options.websocket?.idleTimeout === "number"
      ? options.websocket.idleTimeout
      : undefined
  const upgrades = new WeakMap<Request, UpgradeContext>()
  const webSocketServer = options.websocket === undefined ? undefined : new WebSocketServerForRuntime({ noServer: true })
  let actualPort = requestedPort
  let resolveReady!: () => void
  let rejectReady!: (error: Error) => void
  const ready = new Promise<void>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })

  const runtimeServer: RuntimeServer<Data> = {
    hostname,
    get port() { return actualPort },
    get url() { return new URL(`http://${hostname}:${actualPort}`) },
    ready,
    upgrade(request, upgradeOptions) {
      const context = upgrades.get(request)
      if (context === undefined || webSocketServer === undefined || options.websocket === undefined) return false
      context.upgraded = true
      webSocketServer.handleUpgrade(context.request, context.socket, context.head, (socket) => {
        socket.data = upgradeOptions.data
        socket.send = ((original) => (message: string | Uint8Array) => {
          if (socket.readyState !== 1) return -1
          original.call(socket, message)
          return message instanceof Uint8Array ? message.byteLength : Buffer.byteLength(message)
        })(socket.send)
        socket.on("message", (message: Buffer, isBinary: boolean) => {
          void options.websocket?.message?.(socket, isBinary ? new Uint8Array(message) : message.toString("utf8"))
        })
        socket.on("close", (code: number, reason: Buffer) => {
          void options.websocket?.close?.(socket, code, reason.toString("utf8"))
        })
        void options.websocket?.open?.(socket)
      })
      return true
    },
    stop(closeActiveConnections = false) {
      if (closeActiveConnections) {
        webSocketServer?.clients.forEach((client) => client.terminate())
        httpServer.closeAllConnections?.()
      }
      return new Promise((resolve, reject) => {
        webSocketServer?.close()
        httpServer.close((error) => error === undefined ? resolve() : reject(error))
      })
    },
  }

  const toRequest = (incoming: import("node:http").IncomingMessage): Request => {
    const origin = `http://${incoming.headers.host ?? `${hostname}:${actualPort}`}`
    const method = incoming.method ?? "GET"
    return new Request(new URL(incoming.url ?? "/", origin), {
      method,
      headers: incoming.headers as HeadersInit,
      ...(method === "GET" || method === "HEAD" ? {} : { body: Readable.toWeb(incoming) as unknown as ReadableStream<Uint8Array>, duplex: "half" as never }),
    })
  }

  const writeResponse = async (response: Response | undefined, outgoing: import("node:http").ServerResponse): Promise<void> => {
    if (response === undefined) return
    outgoing.writeHead(response.status, Object.fromEntries(response.headers.entries()))
    if (response.body === null) return void outgoing.end()
    Readable.fromWeb(response.body as never).pipe(outgoing)
  }

  const httpServer = http.createServer((incoming, outgoing) => {
    const request = toRequest(incoming)
    void Promise.resolve(options.fetch(request, runtimeServer)).then(
      (response) => writeResponse(response, outgoing),
      () => writeResponse(Response.json({ error: "internal_server_error" }, { status: 500 }), outgoing),
    )
  })
  httpServer.on("upgrade", (incoming, socket, head) => {
    if (idleTimeoutSeconds !== undefined) {
      const timedSocket = socket as typeof socket & {
        setTimeout(milliseconds: number, callback: () => void): unknown
      }
      timedSocket.setTimeout(idleTimeoutSeconds * 1_000, () => socket.destroy())
    }
    const request = toRequest(incoming)
    const context: UpgradeContext = { request: incoming, socket, head, upgraded: false }
    upgrades.set(request, context)
    void Promise.resolve(options.fetch(request, runtimeServer)).then((response) => {
      if (context.upgraded) return
      const status = response?.status ?? 426
      socket.end(`HTTP/1.1 ${status} Upgrade Required\r\nConnection: close\r\n\r\n`)
    }, () => socket.destroy())
  })
  if (idleTimeoutSeconds !== undefined) httpServer.setTimeout(idleTimeoutSeconds * 1_000)
  httpServer.listen(requestedPort, hostname)
  httpServer.on("listening", () => {
    const bound = httpServer.address()
    if (typeof bound === "object" && bound !== null) actualPort = bound.port
    resolveReady()
  })
  httpServer.on("error", rejectReady)
  return runtimeServer
}

export const Runtime = {
  env: process.env as RuntimeEnv,
  argv: process.argv,
  build,
  file,
  hash,
  isMain,
  serve,
  sleep,
  spawn,
  spawnSync,
  stdin: {
    bytes: readStdinBytes,
    stream: () => Readable.toWeb(process.stdin) as unknown as ReadableStream<Uint8Array>,
  },
  which,
  write,
  zstdCompressSync: (bytes: Uint8Array): Uint8Array => new Uint8Array(zstdCompressSync(bytes)),
  zstdDecompressSync: (bytes: Uint8Array): Uint8Array => new Uint8Array(zstdDecompressSync(bytes)),
} as const

export type RuntimeSpawn = typeof spawn
