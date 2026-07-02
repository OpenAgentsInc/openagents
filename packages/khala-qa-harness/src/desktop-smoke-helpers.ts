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

export type KhalaQaRect = Readonly<{
  height: number
  width: number
  x: number
  y: number
}>

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
