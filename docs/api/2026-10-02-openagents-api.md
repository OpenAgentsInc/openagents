# The OpenAgents API: OpenAgents itself, behind an API

2026-10-02. Speculative design. Nothing in this document is a public API
yet. It says what "OpenAgents behind an API" could mean, what already exists
for it to sit on, which shape to choose, and a phased path. It was written
after [Episode 288](../transcripts/288.md) (open, permissionless protocols
instead of a store that asks permission and shares no revenue) and
[Episode 289](../transcripts/289.md):

> You go to OpenAgents.com, you load the OpenAgents mobile app, you use a
> partner app consuming the OpenAgents API, or their self-hosted version of
> this API, or a subset of it they care about. [...] one OpenAgents
> ecosystem that has all of the best plugins with reputation, identity, and
> payments all figured out. [...] No blockchain garbage, just data, signed
> JSON over WebSockets.

## Summary

- **One call is the product.** Send a message (with an optional thread,
  context, and attachments); OpenAgents answers. Its router decides whether
  that is a prepared answer, a knowledge answer, a command, a plugin, a Coder
  run, or a delegation, and says which as a typed event.
- **The core protocol already exists.** A [NIP-CJ](../../nips/openagents/NIP-CJ.md)
  conversation job to the chat worker *is* this call, signed JSON over
  WebSockets. The website's Ask box is already a small HTTP front on it.
- **Recommendation: one core protocol, thin facades.** The Nostr contracts
  (CJ, HOST, RUN, KB, EXT, X402) stay the canonical, self-hostable API. An
  HTTP + server-sent events front (`POST /v1/messages`), an OpenAI-compatible
  front, and an MCP server are translations of the same event stream. No
  front has its own router, policy, or ledger.
- **First slice:** `POST /v1/messages` with server-sent events on the hosted
  worker, keyed, answers and knowledge only (no computer), plus
  `/v1/chat/completions` with `model: "openagents"`, and a `usage` block on
  every answer.

## 1. What "OpenAgents behind an API" means

OpenAgents is a composable general agent (289). Behind an API means a caller
gets the whole agent, not a model: the knowledge base, the router, the
plugins, Coder, the eval system, the wallet, and the background processes,
through one entry point and a few resources.

**The entry point.** A message goes in. The router, a Jev (System One)
judgment over a reviewed question set
([chat router design](https://github.com/OpenAgentsInc/openagents/blob/main/docs/coder/design/2026-09-28-chat-router.md),
`chat-router-v4` in
[`crates/coder/src/router/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/coder/src/router)),
chooses what happens, and code acts on the choice:

- a prepared answer from the reviewed bank,
- an answer grounded in the knowledge base (product, codebase, NIP-KB),
- a proposed `openagents` command from the command tree
  ([`crates/coder/src/cli_route/tree.json`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/coder/src/cli_route/tree.json)),
- a plugin, or an offer to build a missing one (`capability.missing`),
- a Coder run on a computer, or a delegation to a coding agent,
- a wallet or account action, always as an offer the caller confirms,
- the model's own answer.

The answer streams as events, and the routing decision is one of them.

**The resources.** Under the entry point: threads, Coder runs, plugins and
their evals, knowledge, the wallet, background rules, and usage records.

**Who calls it.**

| Caller | What it wants |
| --- | --- |
| Partner apps | OpenAgents inside their product: the whole agent, or the subset they care about (knowledge only, plugins only, Coder on the user's own computer). |
| Websites | An Ask box. Ours is the first: [`crates/openagents-web/src/ask.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-web/src/ask.rs) ([#10106](https://github.com/OpenAgentsInc/openagents/issues/10106)). |
| Other agents | OpenAgents as one capability among their own: over MCP, over Nostr, or over an agent-to-agent card. |
| Self-hosters | Their own copy of the API, or a subset, with their own keys and model doors, speaking the same protocol so their users stay in the one ecosystem. |
| Our own apps | Already callers: phone, desktop, terminal, and web all speak NIP-CJ to the same worker. The API is the same door, opened to others. |

**What it is not.** It is not a model API with OpenAgents' name on it, and
it is not the existing decision gateway
([`crates/gateway`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/gateway)),
which answers typed Jev questions (`POST /v1/systemone`). The decision
gateway is a lower layer the agent uses; the OpenAgents API is the agent.

## 2. What exists today

| Piece | Where | What it gives the API |
| --- | --- | --- |
| Conversation jobs | [NIP-CJ](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-CJ.md) kinds `25900` / `26900` / `27000` | The core call: a signed, NIP-44 encrypted request; partial feedback; a result carrying the router's judgment, offers, cards, and follow-ups. Bounded `context` (surface, computer, engines, project, `coder_run`). Ephemeral on the relay. |
| The chat worker | `coder-worker` chat on oa-coder-worker-1, [deployment doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/deployment/chat-worker.md) | The hosted OpenAgents: router, bank, knowledge, model doors with failover (OpenRouter primary, Vercel AI Gateway fallback), Jev doors in order. No usage limit ([#10120](https://github.com/OpenAgentsInc/openagents/issues/10120)); every job logged. |
| Surfaces | `Surface::{Phone, Desktop, Terminal, Web}` in [`crates/openagents-chat/src/router.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-chat/src/router.rs); `coder::router::policy::for_web` | A per-surface policy already narrows what the router may offer. `Web` answers and uses knowledge only, never Coder, a computer, or an offer. An API surface is one more policy. |
| The Ask box | `POST /ask` in `crates/openagents-web/src/ask.rs` | The first HTTP front: per-visitor signing keys derived server-side from a cookie and a secret, a NIP-CJ job through `relay.openagents.com`, newline-delimited JSON back. |
| The chat client | `Command` in [`service.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-chat/src/service.rs), `Event` in [`client.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-chat/src/client.rs) | The typed operations (send, read, list, retry, stop, run Coder, bind Coder) and the typed event stream (`Accepted`, `Partial`, `Reply`, `Starting`, `Coder`, `Line`, `Stop`, `Offline`...). `openagents chat --json` prints them as NDJSON ([`docs/cli/chat.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/cli/chat.md)). This is most of the API's event schema already. |
| The host | [`crates/coder-host`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/coder-host), [NIP-HOST](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-HOST.md) | A computer's resident agent: the control socket (same-user Unix socket), NIP-HOST grants with scoped rights, and the thread operations `thread.list`, `thread.read`, `thread.send`, `thread.stop`, `thread.run`, over the relay, iroh, a WebSocket listener, or the tailnet. |
| Coder tasks | [`crates/coder/src/task/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/coder/src/task) | Start, steer (`steer.rs`), follow, stop, review, publish; run records carry engine and Jev cost in micro-dollars with a priced/partial/unknown status ([#10161](https://github.com/OpenAgentsInc/openagents/issues/10161), [7437fe5ef1](https://github.com/OpenAgentsInc/openagents/commit/7437fe5ef1)), kept in the records and not shown on the result card ([f9a8cce433](https://github.com/OpenAgentsInc/openagents/commit/f9a8cce433)). |
| Plugins and evals | [`docs/plugins/`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plugins/README.md), [NIP-EXT](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EXT.md), [NIP-EVAL](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-EVAL.md), [hosted eval runner](https://github.com/OpenAgentsInc/openagents/blob/main/docs/deployment/eval-runner.md) (`crates/eval-runner`, NIP-CJ execution jobs) | Plugins made in conversation, test sets run on our computers, and publication to the registry. |
| Knowledge | [`crates/knowledge`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/knowledge), [NIP-KB](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-KB.md) | Cited search over product, codebase, and published entries. |
| The command as MCP | `openagents mcp serve` ([`crates/openagents-cli/src/mcp.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/openagents-cli/src/mcp.rs)) | One MCP tool per command group, each result the `--json` document. `openagents x402 mcp-serve` puts a Lightning toll on every call. |
| Payments | [`crates/x402`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/x402) (x402 v2 Lightning over `http:1` and `mcp:1`), [`crates/wallet`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/wallet), [NIP-X402](https://github.com/OpenAgentsInc/openagents/blob/main/nips/openagents/NIP-X402.md) | Pay-per-call with a challenge before any work, a replay store, buyer ceilings, and the wallet. |
| Keys, accounts, receipts | The decision gateway: [`docs/agents/api-catalog.json`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/agents/api-catalog.json), [gateway doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/decision-models/service/gateway.md), discovery in [`crates/gateway/src/discovery.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/gateway/src/discovery.rs) | Already built: `oak_` keys with scopes, pause, rotate, and revoke; workspaces; sealed execution receipts; usage reads and export; an OpenAPI 3.1 fragment, `llms.txt`, an MCP tool snapshot, and an agent skill at well-known paths. No usage limits since [#10121](https://github.com/OpenAgentsInc/openagents/issues/10121). |
| Background processes | [`docs/background/`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/background/README.md) | Durable host rules: trigger, condition, typed actions. |
| Cost evidence | [System One cost efficiency audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/cost/2026-10-02-system-one-cost-efficiency-audit.md) | Why an OpenAgents call can be cheaper than raw frontier delegation (section 7). |

The main finding: the protocol and most of the plumbing exist. The API is a
naming of resources, an HTTP front, an auth mapping, and one more surface
policy, not a new system.

## 3. Shape options

| Option | For | Against |
| --- | --- | --- |
| **(a) HTTP + server-sent events, JSON**, shaped like OpenAI's Responses and Assistants, plus an OpenAI-compatible `/v1/chat/completions` with OpenAgents as the "model" | Every developer and SDK already speaks it; drop-in for existing apps; easy to put behind a key and a price. | Bearer keys, not signatures; a hosted endpoint is a central point; the completions shape cannot carry offers, cards, Coder runs, or the routing decision except as extensions. |
| **(b) Signed JSON over WebSockets** on the existing Nostr contracts (CJ, HOST, RUN, KB, EXT, X402) | Open and permissionless (288); self-hostable by anyone with a relay and a key; identity is the signature; the relay keeps nothing; already how every OpenAgents app talks. Exactly 289's "signed JSON over WebSockets". | Unfamiliar to most developers; needs a Nostr library and key handling; ephemeral kinds put recovery on the worker. |
| **(c) MCP server** exposing OpenAgents as tools to other agents | Other agents (Claude, Codex, Cursor) can use OpenAgents today; `openagents mcp serve` already exists; x402 tolls work over `mcp:1`. | A tool call is request/response; streaming, offers, and confirmations fit poorly. The calling agent's model chooses by name, so OpenAgents' own router sits behind one tool. |
| **(d) Agent-to-agent (A2A)** cards and tasks | Matches "OpenAgents as a peer agent"; long tasks with status fit Coder runs. | Young and moving; overlaps what NIP-CJ execution and NIP-RUN already do; adds a second identity story. |

**Recommendation: layered.**

1. **Core: the Nostr contracts.** NIP-CJ conversation and execution jobs,
   NIP-HOST for a computer, NIP-RUN for recoverable runs, NIP-KB, NIP-EXT and
   NIP-EVAL for plugins, NIP-X402 for payment. This is the API's definition:
   a self-hoster implements this and is in the ecosystem.
2. **One typed event schema** (section 4), defined once in
   `openagents-chat`, that every front renders. The chat client's `Event`
   already is most of it.
3. **Thin fronts, translation only:**
   - `POST /v1/messages` with server-sent events: the OpenAgents API most
     developers use. Full event stream, offers, confirmations.
   - `POST /v1/chat/completions` and `POST /v1/responses` with
     `model: "openagents"`: the reply text, the routing decision in an
     `openagents` extension field, and nothing that needs a confirmation.
   - MCP: `openagents mcp serve` grows an `ask` tool that sends one message
     and returns the reply and its route, beside the command groups.
   - A2A: an agent card listing the same capabilities, later.

A front authenticates, maps the caller to a key, sends the core request, and
renders the core events. It never reads the message to decide anything: the
router decides (no keyword or string intent routing anywhere in a front).

## 4. Resources and events

### The message call

```http
POST /v1/messages
Authorization: Bearer oak_<id>.<secret>
Accept: text/event-stream

{
  "thread": "th_…",            // optional; omitted starts a thread
  "message": "Turn my meeting notes into Linear tickets",
  "attachments": [ … ],        // optional, bounded: files, images, links
  "context": {                 // optional, the NIP-CJ context, bounded
    "computer": "host_<pubkey>", // a computer this key holds a HOST grant on
    "project": "openagents"
  },
  "allow": ["answer", "knowledge", "plugin", "coder", "wallet"], // optional narrowing
  "instructions": "…"          // caller guidance; cannot override policy
}
```

`allow` narrows the routes the surface policy permits, the way `for_web`
does; it never widens them. A route outside it comes back as a refusal or a
plain answer, chosen by the router's own fallback, not by the front.

### Events

| Event | From today's code | Meaning |
| --- | --- | --- |
| `accepted` | `Event::Accepted` | The message is in the thread; ids for thread and request. |
| `route` | the judgment in the `27000` feedback / `reply.meta` | The routing decision as a first-class typed event: route id, tier, probabilities, risk, lane, question set and bank by digest (`chat-router-v4@…`, `chat-answers-v1@…`), judge door and model, judge time. A caller can log it, show it, or check it. |
| `delta` | `Event::Partial` | Reply text as it grows. |
| `command` | the CLI route's proposal | A proposed `openagents` command and its arguments, read-only until confirmed. |
| `plugin` | plugin offers, eval cards | A plugin used, or an offer to build one, with its draft test set. |
| `offer` | Run Coder, Connect a computer, wallet, screen offers | Something that changes state or spends. Carries a `confirm` id; nothing happens until the caller confirms (section 6). |
| `coder.starting`, `coder.event`, `coder.result` | `Event::Starting`, `Event::Coder`, `Event::Line`, `coder_run` | A Coder run: engine, model, its event lines, and the ending (`finished`, `failed`, `stopped`) with summary, files, commands, and diff. |
| `reply` | `Event::Reply` | The finished reply with follow-ups and cards. |
| `usage` | usage log line, run record | Cost and time for this call (section 7). |
| `error` | `ReplyFailed`, `Failure`, `Offline` | Typed failure: refused, stopped, unreachable (with retry), invalid. |

Vocabulary: plugins are not "tools" ([plugins doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plugins/README.md));
the API says `command` for a proposed CLI command and `plugin` for a plugin.
Only the OpenAI and MCP fronts say "tool", because those wires do.

### Resources

| Resource | Operations | Today's seam |
| --- | --- | --- |
| Threads | list, read (paged), create, rename, pin, archive, restore, retry, stop; export as an ATIF trajectory | `Command` in `service.rs`; `openagents_chat::thread` renders ATIF |
| Runs (Coder) | start (from an offer or directly with a prompt, project, engine), read, follow events, steer, stop, result (summary, diff, checks, cost) | `Command::RunCoder`, `crates/coder/src/task/{steer,view,review}.rs`, NIP-HOST `thread.run`, NIP-RUN. Needs a computer. |
| Plugins | list and search the registry, read one (parts, test results, author, reputation), use one in a thread, create through conversation, run its evals on the hosted eval runner, publish | NIP-EXT, NIP-EVAL, `crates/eval-runner`, the `capability.missing` route |
| Knowledge | query with citations; read an entry; publish an entry (scoped) | `crates/knowledge`, NIP-KB |
| Wallet | read balance and history (read scope); pay an invoice or send sats only through an offer and a confirmation (spend scope, with a ceiling) | `crates/wallet`, `crates/x402::policy` (ceilings, allowlist, ledger) |
| Background rules | list, read, draft from conversation, enable, disable, delete, read their run history. Host only. | `docs/background/` |
| Usage | per-call records and per-key totals, for API keys only | the chat worker's usage log, Coder run records, gateway receipts and `/v1/workspaces/{w}/usage` |
| Judgments | pass-through to the decision gateway for callers that want System One directly | `POST /v1/systemone` |

## 5. Where it runs

| Place | What it serves | How the API reaches it |
| --- | --- | --- |
| **Hosted** (chat worker, eval runner, openagents.com) | Answers, knowledge, plugin registry reads, plugin evals on our computers, delegation that needs no user computer, judgments | The HTTP fronts in `openagents-web` (beside `/ask`) or the gateway, sending NIP-CJ to the worker through `relay.openagents.com`; or NIP-CJ directly. |
| **A user's own computer** (the host) | Everything above, plus Coder runs, local `openagents` commands, the wallet on that device, background rules, local files and projects | Locally: the control socket, and an optional `127.0.0.1` HTTP front the host serves with the same events. Remotely: NIP-HOST grants over the relay, iroh, the WebSocket listener, or the tailnet. A partner app that wants Coder asks the user to grant its key on their host, the way a phone pairs. |
| **Self-hosted subsets** | Whatever the operator chooses: knowledge only, a plugin registry mirror, the chat worker with their own model doors and keys | The same `coder-worker` and fronts with their own relay and keys; their users' keys and plugins still work across instances because everything is signed. |

Execution boundary: a Coder run, a local command, and a background rule need
a computer someone owns and has granted. The hosted side never runs a user's
code on its own machines except plugin test sets in the eval runner's
sandbox. A hosted API key with no computer gets answers, knowledge, plugins,
and offers, never a run. This is the `Web` surface's rule today.

## 6. Identity, auth, and safety

**Principal is a key.** Every call resolves to a signing key, because the
usage log, NIP-HOST grants, and payments all key off the verified signer.

- **Nostr keys** (native): the caller signs NIP-CJ requests itself. HTTP
  callers can do the same with NIP-98 HTTP auth.
- **API keys** (`oak_<id>.<secret>`, from the gateway's tenancy): the front
  maps each key to a server-held derived signing key, as the Ask box derives
  one per visitor. Scopes, pause, rotate, and revoke already exist.
- **Agent tokens** (`oa_agent_…`): an agent's registered identity, mapped the
  same way.

**Scopes.** `read`, `chat`, `knowledge`, `plugins.use`, `plugins.publish`,
`run` (Coder on a granted computer), `host.*` (the NIP-HOST rights,
unchanged), `background`, `spend` (with a per-key ceiling). Default for a new
key: `read`, `chat`, `knowledge`, `plugins.use`.

**Approvals.** Anything that changes state outside the thread or spends
money arrives as an `offer` with a `confirm` id and runs only after
`POST /v1/confirmations/{id}` from the same principal. Payment always needs
a confirmation, even with the `spend` scope; the scope only allows asking.
NIP-POL decisions stay POL decisions: no HOST right or API scope approves a
POL action. A key may hold a standing approval for a narrow, bounded action
(say, start Coder runs in one project) that the owner set in their app, never
one granted by the key itself.

**Audit.** Each call leaves a usage line (ids and fixed words, no message
text), a sealed receipt for paid calls, a run record for Coder, and the
thread's ATIF trajectory. A caller can read its own; the owner reads all.

**Abuse protection without usage limits.** The owner forbids usage limits in
the OpenAgents apps, and none are shown or enforced there
([#10120](https://github.com/OpenAgentsInc/openagents/issues/10120),
[#10121](https://github.com/OpenAgentsInc/openagents/issues/10121)). For
third-party keys, protection comes from: the engineering bounds NIP-CJ
already requires of every worker (input and output bytes, active jobs, time),
per-key concurrency, pausing or revoking a key that misbehaves, and payment
as the throttle for anonymous and partner traffic. None of it is a quota
shown to a person in our apps.

**Pricing for partners (speculative).** Per 289, Lightning micropayments:

- **Per call, x402.** A partner call without a prepaid balance gets an x402
  challenge quoting the price before anything runs (`crates/x402` already
  does challenge, settle, then execute over HTTP and MCP). The price can
  follow the route: a bank answer costs almost nothing, a Coder run costs its
  engine plus a margin.
- **Plugin authors get paid** from the same flow: a paid call that used a
  plugin splits a share to its author's Lightning address.
- **Prepaid balance** through the gateway's workspace billing for partners
  that prefer an invoice.
- Our own apps and the owner's keys pay nothing and see no price.

## 7. The cost-efficiency selling point

An API consumer calling OpenAgents gets System One for free: the router
spends a Jev judgment (about 200 ms and a fraction of a cent) to send each
message to the cheapest thing that answers it well, instead of sending
everything to a frontier model. A prepared answer from the bank costs no
model call. A knowledge answer costs one retrieval and a fast model. A Coder
run uses the four-part recipe in the
[cost audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/cost/2026-10-02-system-one-cost-efficiency-audit.md)
(Jev scouts and briefs, six tools and a trimmed prompt, the five-minute
cache, lower effort): 61% cheaper and 37% faster than the same model run raw
across eight development tasks, and 45% cheaper with more passes on 26
Terminal-Bench 4.0 tasks. A partner gets that without building it.

Every API answer ends with a `usage` event, so the claim is checkable per
call:

```json
{"type": "usage", "route": "knowledge.product", "tier": "model",
 "model": "stealth/space-bunny-alpha", "door": "openrouter.ai",
 "judge": {"door": "ai-gateway.vercel.sh", "ms": 240, "micro_usd": 300},
 "model_cost": {"micro_usd": 1100, "status": "priced"},
 "run_cost": null, "first_token_ms": 1050, "total_ms": 3900}
```

Cost and time are shown to API callers. In the owner's own apps they stay
recorded and unshown, as the Coder result card does since
[f9a8cce433](https://github.com/OpenAgentsInc/openagents/commit/f9a8cce433).

## 8. A phased path

**Phase 0: nearly there (exists).** The Ask box (`POST /ask`, NDJSON),
`openagents chat --json`, NIP-CJ conversation jobs, `openagents mcp serve`
with an optional x402 toll, NIP-HOST thread operations, gateway keys and
receipts, Coder run cost records.

**Phase 1: the first slice, hosted, no computer.**

- `POST /v1/messages` with server-sent events, carrying `accepted`, `route`,
  `delta`, `command`, `plugin`, `reply`, `usage`, `error`. Answers,
  knowledge, plugin reads, and command proposals only.
- `POST /v1/chat/completions` (and `/v1/responses`) with
  `model: "openagents"`, streaming and not, the route in an `openagents`
  extension field.
- Auth by gateway `oak_` keys mapped to derived signing keys; the usage log
  records the key id.
- A new `Surface::Api` with its own router policy, starting as `for_web`
  plus `allow` narrowing.
- An OpenAPI fragment, `llms.txt` entry, and agent skill, published the way
  the gateway's discovery documents are.
- Crates: `openagents-web` (the fronts, beside `ask.rs`), `openagents-chat`
  (`Surface::Api`, the shared event schema made serializable),
  `coder` (`router::policy::for_api`, `relay::usage` key id), `gateway`
  (key lookup only). Size: small to medium; the Ask box is most of it.
- Open: run the fronts in `openagents-web` or in the gateway? Server-kept
  threads (a thread id the caller can resume) in this phase, or stateless
  with the caller sending the transcript as NIP-CJ does?

**Phase 2: threads, plugins, knowledge, MCP.**

- Server-kept threads and their reads; ATIF export.
- Plugin registry reads, plugin creation through conversation, evals on the
  hosted eval runner, publishing under `plugins.publish`.
- Knowledge query and entry reads.
- MCP `ask` tool beside the command groups; an A2A agent card.
- Crates: `openagents-chat`, `openagents-web`, `eval-runner`, `knowledge`,
  `openagents-cli` (`mcp.rs`). Size: medium.
- Open: who may publish a plugin through the API, and how reputation shows.

**Phase 3: runs on a computer, wallet, payment.**

- Partner keys as NIP-HOST devices: the user grants the partner's key on
  their host from the app; `/v1/runs` goes to the host as `thread.run`, with
  follow, steer, stop, and result with diff.
- Wallet reads; payments through offers and confirmations.
- x402 per-call pricing for partner keys; plugin author payouts.
- Crates: `coder-host` (grant UI hooks, `serve/threads.rs`), `coder` task,
  `wallet`, `x402`, `openagents-web`. Size: large.
- Open: price table per route; payout share; what a standing approval may
  cover.

**Phase 4: self-hosted.**

- The host serves the same HTTP front on `127.0.0.1` over its control
  socket (`openagents serve api`, a name to decide), so local apps and
  scripts get OpenAgents with no hosted call.
- A packaged worker for self-hosters with their own relay, model doors, and
  keys; a conformance suite over the event schema.
- The event schema written up as a NIP (or a NIP-CJ amendment) so another
  implementation can be checked against it.
- Crates: `coder-host`, `coder-worker` packaging, `nips/openagents`. Size:
  medium.

## 9. Open questions for the owner

1. Is the HTTP front "the OpenAgents API", with Nostr as the open
   definition underneath, or should the docs lead with Nostr and present
   HTTP as the convenience?
2. Server-kept threads in the first slice, or stateless calls first?
3. Should partner calls ever reach a Coder run on our computers, or only on
   computers the user owns and grants?
4. Pricing: free hosted answers and knowledge for everyone (as the Ask box is
   today), with payment only for runs and paid plugins? Or x402 on every
   partner call?
5. Plugin author payouts: a share of the call price, a fixed per-use fee the
   author sets, or both?
6. Should API callers see the full `route` event (probabilities, digests), or
   only the route id?
7. Which name and host: `api.openagents.com`, or `/v1` on `openagents.com`?
8. Does the OpenAI-compatible front belong in the first slice, or after
   `/v1/messages` has users?
