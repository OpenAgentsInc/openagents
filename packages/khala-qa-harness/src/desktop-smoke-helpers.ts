import { createServer } from "node:net"

export type KhalaQaViteServer = Readonly<{
  kill: () => void
}>

export type KhalaQaStartViteServerOptions = Readonly<{
  cwd: string
  label: string
  port: number
}>

export type KhalaQaWaitForHttpOptions = Readonly<{
  fetch?: typeof fetch
  intervalMs?: number
  now?: () => number
  sleep?: (ms: number) => Promise<void>
  timeoutMs?: number
}>

export type KhalaQaConsoleDiagnosticKind = "console" | "pageerror"

export type KhalaQaConsoleDiagnostic = Readonly<{
  columnNumber?: number
  kind: KhalaQaConsoleDiagnosticKind
  lineNumber?: number
  message: string
  stack?: string
  type?: string
  url?: string
}>

export type KhalaQaConsoleAllowlistEntry =
  | string
  | RegExp
  | ((diagnostic: KhalaQaConsoleDiagnostic) => boolean)
  | Readonly<{
    kind?: KhalaQaConsoleDiagnosticKind
    pattern: string | RegExp
    reason: string
  }>

export type KhalaQaConsoleOracle = Readonly<{
  assertNoUnexpected: () => void
  diagnostics: readonly KhalaQaConsoleDiagnostic[]
  unexpectedDiagnostics: () => readonly KhalaQaConsoleDiagnostic[]
}>

export type KhalaQaConsoleOracleOptions = Readonly<{
  allowlist?: readonly KhalaQaConsoleAllowlistEntry[]
  consoleTypes?: readonly string[]
  label?: string
}>

type KhalaQaConsoleMessageLike = Readonly<{
  location: () => Readonly<{
    columnNumber?: number
    lineNumber?: number
    url?: string
  }>
  text: () => string
  type: () => string
}>

type KhalaQaConsolePageLike = Readonly<{
  on: {
    (event: "console", handler: (message: KhalaQaConsoleMessageLike) => void): unknown
    (event: "pageerror", handler: (error: Error) => void): unknown
  }
}>

export type KhalaQaRect = Readonly<{
  height: number
  width: number
  x: number
  y: number
}>

export const findKhalaQaAvailablePort = async (
  preferredPort: number,
  fallbackPorts: ReadonlyArray<number> = [],
): Promise<number> => {
  if (await canListenOnPort(preferredPort)) return preferredPort
  for (const fallbackPort of fallbackPorts) {
    if (await canListenOnPort(fallbackPort)) return fallbackPort
  }
  return await allocateEphemeralPort()
}

const canListenOnPort = (port: number): Promise<boolean> =>
  new Promise(resolvePort => {
    const server = createServer()
    server.once("error", () => resolvePort(false))
    server.listen(port, "127.0.0.1", () => {
      server.close(() => resolvePort(true))
    })
  })

const allocateEphemeralPort = (): Promise<number> =>
  new Promise((resolvePort, rejectPort) => {
    const server = createServer()
    server.once("error", rejectPort)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      server.close(() => {
        if (typeof address === "object" && address !== null) resolvePort(address.port)
        else rejectPort(new Error("failed to allocate visual smoke port"))
      })
    })
  })

export const startKhalaQaViteServer = (
  input: KhalaQaStartViteServerOptions,
): KhalaQaViteServer => {
  const proc = Bun.spawn(
    [
      "bunx",
      "vite",
      "--host",
      "127.0.0.1",
      "--port",
      String(input.port),
      "--strictPort",
    ],
    {
      cwd: input.cwd,
      stderr: "pipe",
      stdout: "pipe",
    },
  )
  void streamKhalaQaServerOutput(input.label, proc.stdout)
  void streamKhalaQaServerOutput(input.label, proc.stderr)
  return {
    kill: () => {
      proc.kill()
    },
  }
}

export const streamKhalaQaServerOutput = async (
  label: string,
  stream: ReadableStream<Uint8Array>,
): Promise<void> => {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  try {
    for (;;) {
      const chunk = await reader.read()
      if (chunk.done) return
      const text = decoder.decode(chunk.value, { stream: true })
      for (const line of text.split("\n")) {
        if (line.trim().length > 0) console.error(`[${label}] ${line}`)
      }
    }
  } catch {
    // Server output is diagnostic only.
  }
}

export const waitForKhalaQaHttp = async (
  url: string,
  options: KhalaQaWaitForHttpOptions = {},
): Promise<void> => {
  const fetchLike = options.fetch ?? fetch
  const intervalMs = options.intervalMs ?? 250
  const now = options.now ?? Date.now
  const sleep = options.sleep ?? Bun.sleep
  const deadline = now() + (options.timeoutMs ?? 30_000)
  let lastError: unknown = null
  while (now() < deadline) {
    try {
      const response = await fetchLike(url)
      if (response.ok) return
      lastError = new Error(`${url} returned ${response.status}`)
    } catch (error) {
      lastError = error
    }
    await sleep(intervalMs)
  }
  throw new Error(`Timed out waiting for ${url}: ${String(lastError)}`)
}

export const installKhalaQaConsoleErrorOracle = (
  page: KhalaQaConsolePageLike,
  options: KhalaQaConsoleOracleOptions = {},
): KhalaQaConsoleOracle => {
  const diagnostics: KhalaQaConsoleDiagnostic[] = []
  const failingConsoleTypes = new Set(options.consoleTypes ?? ["error"])

  page.on("console", message => {
    const type = message.type()
    if (!failingConsoleTypes.has(type)) return
    const location = message.location()
    diagnostics.push({
      ...(location.columnNumber === undefined ? {} : { columnNumber: location.columnNumber }),
      kind: "console",
      ...(location.lineNumber === undefined ? {} : { lineNumber: location.lineNumber }),
      message: message.text(),
      type,
      ...(location.url === undefined ? {} : { url: location.url }),
    })
  })

  page.on("pageerror", error => {
    diagnostics.push({
      kind: "pageerror",
      message: error.message,
      ...(error.stack === undefined ? {} : { stack: error.stack }),
    })
  })

  const unexpectedDiagnostics = (): readonly KhalaQaConsoleDiagnostic[] =>
    diagnostics.filter(diagnostic =>
      !(options.allowlist ?? []).some(entry => khalaQaConsoleDiagnosticAllowed(diagnostic, entry)),
    )

  return {
    assertNoUnexpected: () => {
      const unexpected = unexpectedDiagnostics()
      if (unexpected.length === 0) return
      throw new Error(formatKhalaQaConsoleDiagnostics(options.label, unexpected))
    },
    diagnostics,
    unexpectedDiagnostics,
  }
}

const khalaQaConsoleDiagnosticAllowed = (
  diagnostic: KhalaQaConsoleDiagnostic,
  entry: KhalaQaConsoleAllowlistEntry,
): boolean => {
  if (typeof entry === "function") return entry(diagnostic)
  if (typeof entry === "string" || entry instanceof RegExp) {
    return khalaQaConsolePatternMatches(diagnostic, entry)
  }
  if (entry.kind !== undefined && entry.kind !== diagnostic.kind) return false
  return khalaQaConsolePatternMatches(diagnostic, entry.pattern)
}

const khalaQaConsolePatternMatches = (
  diagnostic: KhalaQaConsoleDiagnostic,
  pattern: string | RegExp,
): boolean => {
  const haystack = [
    diagnostic.kind,
    diagnostic.type,
    diagnostic.message,
    diagnostic.stack,
    diagnostic.url,
  ].filter((part): part is string => typeof part === "string").join("\n")
  if (typeof pattern === "string") return haystack.includes(pattern)
  pattern.lastIndex = 0
  return pattern.test(haystack)
}

const formatKhalaQaConsoleDiagnostics = (
  label: string | undefined,
  diagnostics: readonly KhalaQaConsoleDiagnostic[],
): string => {
  const heading = label ?? "Khala QA visual smoke"
  const body = diagnostics
    .map((diagnostic, index) => {
      const location =
        diagnostic.url === undefined
          ? ""
          : ` (${diagnostic.url}:${diagnostic.lineNumber ?? 0}:${diagnostic.columnNumber ?? 0})`
      const type = diagnostic.type === undefined ? "" : `/${diagnostic.type}`
      return `${index + 1}. ${diagnostic.kind}${type}: ${diagnostic.message}${location}`
    })
    .join("\n")
  return `${heading} observed ${diagnostics.length} unexpected console/pageerror diagnostic(s):\n${body}`
}

export const khalaQaRectsOverlap = (left: KhalaQaRect, right: KhalaQaRect): boolean =>
  left.x < right.x + right.width - 1 &&
  right.x < left.x + left.width - 1 &&
  left.y < right.y + right.height - 1 &&
  right.y < left.y + left.height - 1

export const assertKhalaQaVisibleRect = (
  label: string,
  rect: KhalaQaRect,
  viewport: KhalaQaRect,
): void => {
  if (rect.width < 1 || rect.height < 1) {
    throw new Error(`${label} is not visible`)
  }
  if (rect.x < -1 || rect.y < -1) {
    throw new Error(`${label} is clipped outside the viewport`)
  }
  if (rect.x + rect.width > viewport.width + 1) {
    throw new Error(`${label} overflows the viewport width: ${JSON.stringify({ rect, viewport })}`)
  }
  if (rect.y + rect.height > viewport.height + 1) {
    throw new Error(`${label} overflows the viewport height: ${JSON.stringify({ rect, viewport })}`)
  }
}
