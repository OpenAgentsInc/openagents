// A throwaway in-memory relay for dry runs of the publisher:
//   bun local-relay.ts [PORT]   then   bun publish.ts publish --relay ws://127.0.0.1:PORT
import { startRelay } from "nostr-effect/relay"
const port = Number(process.argv[2] ?? 7447)
const relay = await startRelay({ port })
console.log(`local relay on ws://127.0.0.1:${(relay as { port?: number }).port ?? port}`)
