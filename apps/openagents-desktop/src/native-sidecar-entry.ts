import {
  DesktopNativeSidecarFrameLimit,
  decodeDesktopNativeSidecarBootstrapRequest,
  executeDesktopNativeSidecarBootstrap,
} from "./native-sidecar-contract.ts"

const readBoundedStdin = async (): Promise<Buffer> => {
  const chunks: Buffer[] = []
  let length = 0
  for await (const chunk of process.stdin) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
    length += bytes.length
    if (length > DesktopNativeSidecarFrameLimit) {
      throw new Error("Native sidecar request exceeded the 64 KiB frame limit.")
    }
    chunks.push(bytes)
  }
  return Buffer.concat(chunks, length)
}

const main = async (): Promise<void> => {
  const bytes = await readBoundedStdin()
  let value: unknown
  try {
    value = JSON.parse(bytes.toString("utf8"))
  } catch {
    throw new Error("Native sidecar request was not valid JSON.")
  }
  const request = decodeDesktopNativeSidecarBootstrapRequest(value)
  if (request === null) throw new Error("Native sidecar request failed its closed schema.")
  const receipt = await executeDesktopNativeSidecarBootstrap(request)
  process.stdout.write(`${JSON.stringify(receipt)}\n`)
}

void main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : "Native sidecar failed."
  process.stderr.write(`[openagents-native-sidecar] ${message}\n`)
  process.exitCode = 1
})
