# Apps, routes and the connection protocol

## Execution location

An **environment** is the T3 server installation that owns projects, threads,
the filesystem, credentials and execution. A client can connect to several
environments. A project/thread belongs to its environment; grouping repositories
across machines does not move execution between them.

| Surface | What runs there | How it reaches coding agents |
| --- | --- | --- |
| Mobile | Expo/React Native UI, platform credential/cache storage, network and lifecycle integration | Shared client runtime connects to an environment; agent execution remains there |
| Web | React UI and browser-local connection catalog/cache | Direct HTTP/WebSocket connection to each selected environment |
| Desktop renderer | Web client in Electron with desktop bridge capabilities | Same environment RPC boundary as web/mobile |
| Desktop main | Native integrations and backend/SSH lifecycle | Normally launches the primary environment server; can also manage remote routes |
| Headless/remote server | Node T3 server, persistent state, agents and tools | Launches or contacts provider runtimes on that environment |

The mobile connection setup actually imports `Connection`, snapshot loaders and
thread history controllers from `@t3tools/client-runtime`; it does not contain a
second agent control loop. Its platform layer supplies the device-specific
effects. Desktop's backend manager spawns a server and delivers bootstrap data
through an extra descriptor, or stdin where WSL drops descriptors. The local
authorization helper exchanges that bootstrap for an environment bearer session.
These are distinct from renderer IPC and from provider stdio. Sources:
[mobile runtime][mobile], [desktop backend][desktop], [desktop auth][desktop-auth],
[remote architecture][remote].

Desktop can disable its local environment and operate only saved remote ones.
Its bundled renderer uses the `t3code://` scheme (Vite in development), so the
renderer need not be served by its local backend. This is an architectural
demonstration that the desktop UI and the execution host are separable, not an
assumption that every desktop thread executes locally. [Remote architecture][remote]
and [desktop settings][desktop-settings].

## What is on the app-to-server wire

The application protocol is typed **Effect RPC with JSON serialization over
WebSocket**, plus HTTP operations such as authorization, uploads and snapshots.
`packages/contracts/src/rpc.ts` and `orchestrationV2.ts` define the application
methods and shapes. This is not Codex JSON-RPC passed straight through to the
phone, nor the Claude SDK exposed remotely. [Contracts][contracts],
[V2 contracts][v2], [RPC session][session].

The shared session builds `RpcClient.makeProtocolSocket`, uses
`RpcSerialization.layerJson`, and disables its internal transport retries. The
connection supervisor owns retries instead. It also requests socket-frame
splitting support and rejoins parts at the socket boundary. On the server,
`RpcServer.makeProtocolWithHttpEffectWebsocket` and `RpcServer.make` use matching
JSON serialization and per-method authorization middleware.
[Client transport][session], [server boundary][ws].

Conceptually, the client does the following:

1. Resolve a saved route for the desired environment.
2. Prepare authentication for that route and construct its RPC socket URL.
3. Open the socket and wait for the initial server configuration.
4. Synchronize the shell/thread read models through snapshots and resumable
   subscriptions.
5. Send application commands using the live session and render committed
   projections/events.

Socket-open is not ready: configuration is awaited and its environment identity
is checked. A healthy socket can coexist with a failed data subscription. That
distinction is carried into the client's state rather than making every failure
look like a transport reconnect. [Session factory][session],
[connection design][connection].

## Routes do not change execution semantics

| Route | What it supplies | What still belongs to T3 |
| --- | --- | --- |
| Loopback/LAN/HTTPS endpoint | An HTTP/WebSocket address | Pairing/session authentication, scopes, projects and execution |
| Tailscale | Reachable tailnet/Serve address | Environment login and RPC authorization |
| SSH | Port forwarding and, when requested, remote server startup | Same application protocol through the forward |
| T3 Connect | Cloud-account discovery/bootstrap plus managed tunnel hostname | The environment issues the application session and executes work |

The route driver tries an ordered list. It checks a direct address's public
descriptor before sending a saved credential, avoiding accidental disclosure to
a different environment now using the address. An environment ID is a routing
check, not cryptographic proof by itself. Earlier preferred routes can be
preflighted while a later one is working. Learned LAN/tailnet addresses reuse
the appropriate credentials subject to transport restrictions; a bearer token
paired over HTTPS must not silently migrate to cleartext LAN HTTP.
[Route driver][driver], [remote design][remote].

SSH ownership is explicit. Desktop main can launch SSH and handle its prompts;
the renderer only sees the forwarded environment. A launcher stops a remote
server only when it owns that process. Merely finding an existing server does
not authorize killing it on disconnect. [SSH tunnel][ssh].

The hosted website is also just a client. It keeps its connection catalog in
the browser and talks to the environment, rather than having the hosted UI
origin hold all pairing sessions or proxy every application request. A hosted
pairing link puts its secret in the fragment, exchanges it with the environment,
then strips it from browser history. HTTPS hosting cannot by itself remove a
browser's restrictions on reaching a cleartext LAN backend.
[Hosted pairing][pairing], [remote design][remote].

## T3 Connect has two authentication boundaries

Cloud identity uses Clerk. A cloud login identifies the user to the relay;
it is not an environment session. The linked environment accepts signed,
replay-guarded requests from the relay to mint a one-time bootstrap bound to the
client's DPoP key. The client exchanges that credential with the environment.
The relay does not receive the resulting environment session token. Signed
responses bind the bootstrap result to the request and proof key.
[Connect design][connect], [environment link service][cloud-link].

After bootstrap, application traffic uses the managed tunnel hostname. The
relay Worker is a broker, not the ordinary HTTP/WebSocket application proxy;
Cloudflare tunnel infrastructure remains in the traffic path. This is not
evidence of end-to-end encryption against the tunnel operator. The relay also
has a separate automation-webhook forwarding/optional holding path, which must
not be confused with live chat transport. [Connect design][connect].

The broker is trusted: its signing authority can request mint operations.
DPoP prevents reuse of an honestly issued bootstrap by someone without the
private proof key; it does not remove the authority of a compromised broker.
For headless CLI cloud login, T3 uses the device authorization grant; an
interactive local CLI can use a loopback flow. Neither is the provider's
Claude/ChatGPT login. [Connect design][connect].

## Retry, foregrounding and cached state

The supervisor is the single transport retry owner. Its jittered exponential
backoff caps at five minutes. Authentication-blocked and offline conditions
wait for a relevant wakeup. Foregrounding, network changes and explicit retry
probe an existing socket before replacing it. The mobile suspension path also
handles sockets the OS killed without a close event, using route-sensitive
deadlines and, where applicable, a fresh public-descriptor request as evidence.
These details are a substantial part of mobile reliability.
[Supervisor][supervisor], [connection design][connection].

The registry scopes connections per environment. Involuntary disconnection
retains cached data and registration; explicit removal closes it and clears
credentials/projections/platform-owned state. A cloud account change affects
relay registrations without deleting directly paired environments.
[Registry][registry].

Shell state and thread detail have separate synchronization. HTTP can fetch a
shell snapshot before the RPC config is ready, but a late response cannot
overwrite newer socket state. Thread consumers share one stream; when the last
consumer unmounts it closes, while a registry-local idle cache retains state and
cursor for five minutes. Desktop additionally keeps active threads subscribed;
web/mobile do not use that desktop keep-alive. Large histories use bounded
snapshots, pages and on-demand thread families; absence from the shell list does
not prove deletion. [Connection design][connection], [thread state][threads].

HTTP credential renewal does not require replacing a healthy socket. Connect
tokens last an hour; the driver attempts renewal during the last minute and
the server can rebind the live socket to a compatible newer session. Revocation
or expiry closes it when renewal cannot maintain authorization.
[Authorization design][connection], [session store][sessions].

Finally, reconnect does **not** replay arbitrary mutations. Subscriptions follow
replacement sessions; commands need their own idempotency/receipt semantics.
That responsibility lives in the orchestration layer described next.
[RPC client][client].

[mobile]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/connection/runtime.ts
[desktop]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/desktop/src/backend/DesktopBackendManager.ts#L416
[desktop-auth]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/desktop/src/backend/DesktopLocalEnvironmentAuth.ts
[desktop-settings]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/desktop/src/settings/DesktopAppSettings.ts
[remote]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/remote.md
[contracts]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/contracts/src/rpc.ts
[v2]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/contracts/src/orchestrationV2.ts
[session]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/rpc/session.ts#L229
[ws]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/ws.ts#L3540
[connection]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/connection-runtime.md
[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/connection/driver.ts
[ssh]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/ssh/src/tunnel.ts
[pairing]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/web/src/hostedPairing.ts
[connect]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/t3-connect.md
[cloud-link]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/cloud/CloudLink.ts
[supervisor]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/connection/supervisor.ts
[registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/connection/registry.ts
[threads]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/state/threads.ts
[sessions]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/auth/SessionStore.ts
[client]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/client-runtime/src/rpc/client.ts
