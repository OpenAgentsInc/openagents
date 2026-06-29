declare const process: {
  argv: Array<string>
  env: Record<string, string | undefined>
}

type RelayMessage = ReadonlyArray<unknown>

const smokeEvent = {
  id: "d19397889f7c0b1556b7d385dc59ea6d131e4cb7fe16d8b175983853ba06111a",
  pubkey: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  created_at: 1781050000,
  kind: 5050,
  tags: [
    ["i", "OpenAgents market relay smoke request", "text"],
    ["output", "text/plain"],
    ["param", "model", "openagents-smoke"],
    ["t", "openagents-market-relay-smoke-fixed-v1"],
  ],
  content: "",
  sig: "c58b73e034bf927973fd60386173a1d598a186192a199d0d145ceb9ec84f7cfae5ad7fff552b7b383a0868a5a533c1a6d6fbe28e61809705ff1798d8c741a485",
} as const

const input =
  process.argv[2] ?? process.env.OPENAGENTS_NOSTR_RELAY_URL ?? "ws://127.0.0.1:8787"

const normalizeRelayUrl = (value: string): URL => {
  const withProtocol = /^[a-z]+:\/\//i.test(value) ? value : `wss://${value}`
  const url = new URL(withProtocol)

  if (url.protocol === "http:") {
    url.protocol = "ws:"
  }

  if (url.protocol === "https:") {
    url.protocol = "wss:"
  }

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
    const timeout = setTimeout(
      () => reject(new Error("WebSocket open timed out")),
      10_000,
    )

    socket.addEventListener("open", () => {
      clearTimeout(timeout)
      resolve()
    })
    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error("WebSocket failed before open"))
    })
  })

const waitForMessage = (
  socket: WebSocket,
  label: string,
  predicate: (message: RelayMessage) => boolean,
) =>
  new Promise<RelayMessage>((resolve, reject) => {
    const seen: Array<RelayMessage> = []
    const timeout = setTimeout(() => {
      reject(
        new Error(
          `Timed out waiting for ${label}; saw ${JSON.stringify(seen)}`,
        ),
      )
    }, 15_000)

    socket.addEventListener("message", (event) => {
      const parsed = JSON.parse(String(event.data)) as RelayMessage
      seen.push(parsed)

      if (predicate(parsed)) {
        clearTimeout(timeout)
        resolve(parsed)
      }
    })

    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error(`WebSocket error while waiting for ${label}`))
    })
  })

const relayUrl = normalizeRelayUrl(input)
const httpUrl = httpUrlForRelay(relayUrl)
const smokeTag = "openagents-market-relay-smoke-fixed-v1"

console.log(`NIP-11 GET ${httpUrl.toString()}`)
const infoResponse = await fetch(httpUrl, {
  headers: { accept: "application/nostr+json" },
})

if (!infoResponse.ok) {
  throw new Error(
    `NIP-11 request failed with ${infoResponse.status}: ${await infoResponse.text()}`,
  )
}

const relayInfo = await infoResponse.json()
console.log(`Relay info: ${JSON.stringify(relayInfo)}`)

console.log(`WebSocket subscribe ${relayUrl.toString()}`)
const reader = new WebSocket(relayUrl)
await waitForOpen(reader)

const subscriptionId = `openagents-market-${Date.now()}`
reader.send(
  JSON.stringify([
    "REQ",
    subscriptionId,
    {
      "#t": [smokeTag],
      authors: [smokeEvent.pubkey],
      kinds: [smokeEvent.kind],
      limit: 5,
    },
  ]),
)

console.log(`WebSocket publish ${relayUrl.toString()}`)
const publisher = new WebSocket(relayUrl)
await waitForOpen(publisher)
publisher.send(JSON.stringify(["EVENT", smokeEvent]))

const okMessage = await waitForMessage(
  publisher,
  "OK accepted response",
  message =>
    message[0] === "OK" &&
    message[1] === smokeEvent.id &&
    message[2] === true,
)
const eventMessage = await waitForMessage(
  reader,
  "subscribed EVENT response",
  message =>
    message[0] === "EVENT" &&
    message[1] === subscriptionId &&
    typeof message[2] === "object" &&
    message[2] !== null &&
    (message[2] as { id?: unknown }).id === smokeEvent.id,
)

reader.send(JSON.stringify(["CLOSE", subscriptionId]))
reader.close(1000, "smoke complete")
publisher.close(1000, "smoke complete")

console.log(`Published event: ${smokeEvent.id}`)
console.log(`OK message: ${JSON.stringify(okMessage)}`)
console.log(`EVENT message: ${JSON.stringify(eventMessage)}`)
console.log("Nostr market relay smoke passed")
