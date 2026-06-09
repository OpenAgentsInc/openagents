declare const process: {
  argv: string[]
  env: Record<string, string | undefined>
}

const input = process.argv[2] ?? process.env.OPENAGENTS_NOSTR_RELAY_URL ?? "ws://127.0.0.1:8787"

const normalizeRelayUrl = (value: string): URL => {
  const withProtocol = /^[a-z]+:\/\//i.test(value) ? value : `wss://${value}`
  const url = new URL(withProtocol)
  if (url.protocol === "http:") url.protocol = "ws:"
  if (url.protocol === "https:") url.protocol = "wss:"
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error(`Expected ws/wss/http/https relay URL, got ${value}`)
  }
  return url
}

const httpUrlForRelay = (relayUrl: URL): URL => {
  const url = new URL(relayUrl.toString())
  url.protocol = relayUrl.protocol === "wss:" ? "https:" : "http:"
  return url
}

const waitForOpen = (socket: WebSocket) =>
  new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("WebSocket open timed out")), 10_000)
    socket.addEventListener("open", () => {
      clearTimeout(timeout)
      resolve()
    })
    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error("WebSocket failed before open"))
    })
  })

const waitForHandshake = (socket: WebSocket, subscriptionId: string) =>
  new Promise<unknown[]>((resolve, reject) => {
    const messages: unknown[] = []
    const timeout = setTimeout(() => {
      reject(new Error(`Timed out waiting for EOSE or NOTICE; saw ${JSON.stringify(messages)}`))
    }, 10_000)

    socket.addEventListener("message", (event) => {
      const parsed = JSON.parse(String(event.data)) as unknown
      messages.push(parsed)

      if (
        Array.isArray(parsed) &&
        ((parsed[0] === "EOSE" && parsed[1] === subscriptionId) || parsed[0] === "NOTICE")
      ) {
        clearTimeout(timeout)
        resolve(messages)
      }
    })

    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error("WebSocket error during handshake"))
    })
  })

const relayUrl = normalizeRelayUrl(input)
const httpUrl = httpUrlForRelay(relayUrl)

console.log(`NIP-11 GET ${httpUrl.toString()}`)
const infoResponse = await fetch(httpUrl, {
  headers: { accept: "application/nostr+json" },
})

if (!infoResponse.ok) {
  throw new Error(`NIP-11 request failed with ${infoResponse.status}: ${await infoResponse.text()}`)
}

const relayInfo = await infoResponse.json()
console.log(`Relay info: ${JSON.stringify(relayInfo)}`)

console.log(`WebSocket connect ${relayUrl.toString()}`)
const socket = new WebSocket(relayUrl)
await waitForOpen(socket)

const subscriptionId = `openagents-poc-${Date.now()}`
const request = ["REQ", subscriptionId, { kinds: [1], limit: 1 }]
socket.send(JSON.stringify(request))

const messages = await waitForHandshake(socket, subscriptionId)
socket.send(JSON.stringify(["CLOSE", subscriptionId]))
socket.close(1000, "smoke complete")

console.log(`Handshake messages: ${JSON.stringify(messages)}`)
console.log("Nostr relay smoke passed")
