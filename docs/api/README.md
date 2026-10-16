# OpenAgents API

Design notes for putting OpenAgents itself, the open network of agents
behind one conversation, behind an API that partner apps, websites, other agents, and self-hosters
can call. Calling the API and organization administration describe live routes. A page marked proposed describes routes that are not mounted yet. The public face is
plain HTTP with an API key; Nostr stays inside our own front, as the CLI
hides it today; payment is x402.

## API address

The API is at **`https://api.openagents.com/v1`**. Every public route
answers there: models, responses, chat completions, decisions, agent
sign-up, account, keys, usage, workspaces, projects, files, chats, and the
rest. The two older addresses, `https://openagents.com/v1` and
`https://openagents.com/api/v1`, still answer the same routes the same way
but are deprecated: each answer carries `Deprecation: true` and
`Link: <https://api.openagents.com/v1/...>; rel="successor-version"` naming
the same path on the API's address. They are never redirected (a redirect
would drop a request body), and no end date is set yet.

Keys (`oak_`, `oa_sk_`, `sess_`) work the same on every address. Website
cookie sessions are for openagents.com only; the API's address ignores
cookies.

| Document | What it covers |
| --- | --- |
| [API design: public and internal (2026-10-09)](design.md) | The target shape of every endpoint we serve: principles (one base URL, OpenAPI 3.1, versioning, idempotency, cursor pagination, one error shape, auth and `oak_` scopes, tenancy, events, privacy classes), every route mapped by data domain to PUBLIC, FIRST-PARTY, or INTERNAL with its status, which service owns what, what is public at launch and never, the inconsistencies, and the migration plan. Start here. |
| [Calling the API (2026-10-12)](calling.md) | Authentication, the workspace header, response headers, list cursors, the live body ceiling, and which path to call. |
| [The OpenAgents API (design, 2026-10-02)](2026-10-02-openagents-api.md) | The owner's decisions; plain HTTP at `api.openagents.com` with curl examples for every main call; how our front turns each call into NIP traffic and where keys live; x402 Lightning payment with the 402, pay, retry flow; the endpoint-to-NIP table and the NIP gaps; where it runs; the phased path; and what is still open. |
| [A router for agentic execution (plan, 2026-10-02)](2026-10-02-agentic-execution-router.md) | Outcome-based routing, explicit execution and disclosure grants, durable task recovery, payment and settlement boundaries, evidence, and phased implementation gates. |
| [Managed Agents API (proposed, 2026-10-11)](managed-agents.md) | Saved agents, sessions, events, tools, multiagent threads, workflow runs, environments, vaults, memory, skills, files, and scheduled deployments. Same resource paths and JSON as the managed-agents surface, on this API's auth, errors, cursor, and sats. |
| [Managed agent events (proposed, 2026-10-11)](managed-agents-events.md) | Sending and reading session events, live previews, reconnecting, the budget pause, usage and cache counts, and outcome grading. |
| [Managed agent operations (proposed, 2026-10-11)](managed-agents-operations.md) | Workspace webhooks, cron fires and deployment failures, the JSON and skill files you keep, and how a session's spend relates to one `/v1/responses` call. |
| [Decisions (proposed, 2026-10-12)](decisions.md) | `POST /v1/decisions`: typed answers (predicate, choice, score) about text and images, in OpenAI's Decisions API shape so its SDKs work by base URL; how it maps onto the NIP-DEC decision `/v1/systemone` and the relay already serve; models (Clef on Pylons and our GPUs first), batching many questions, limits, errors, price in sats, privacy, and the build steps. |
| [Model calls (proposed, 2026-10-12)](model-calls.md) | Token counts, response batches, files and citations on a response, cache breakpoints, strict tools, the tool loop, reasoning replay, refusals, and context edits. Extends `/v1/responses`. |
| [Projects, threads, and files (proposed, 2026-10-12)](projects.md) | Your projects, the chats in them, and the files and notes you keep there: list, read, create, attach, and delete through the API, `openagents projects`, and Coder. Anthropic's content-API shapes for chats, files, and projects, for everyone and for their own data, on this API's keys, cursors, and errors. Also what exists today and how to set it up. |
| [Connections and Drive sources (2026-10-14)](connections.md) | Connecting Google Drive, browsing it, attaching Drive folders and files to a project, asking a project chat that answers from them, and the admin's access list, through the API and `openagents connections` / `openagents projects sources`. Live, for allowed accounts. |
| [Organization administration (2026-10-12)](organization.md) | Members, invitations, bearer keys, and the caps on those keys for an organization workspace. Customer-managed encryption keys are proposed. |

Related: the existing keyed HTTP [decision gateway](../decision-models/service/gateway.md)
(System One judgments, not the agent), the [chat worker](../deployment/chat-worker.md)
that serves OpenAgents chat today, and the [NIPs](../../nips/openagents/README.md).
