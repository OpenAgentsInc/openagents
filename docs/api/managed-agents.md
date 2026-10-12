# Managed Agents API

2026-10-11. Proposed. Nothing on this page is a live route.

This is the public API for a saved agent that runs in a session: a model, a system prompt, tools, skills, and MCP servers, on an environment you choose, with events you can read, steer, and stop. A coordinator can delegate inside that one session. Vaults hold credentials, memory stores keep notes across sessions, and a deployment runs the agent on a schedule.

It is a PUBLIC surface on `https://api.openagents.com/v1` (alias `https://openagents.com/api/v1`). It follows [the API design](design.md) and [the owner's decisions](2026-10-02-openagents-api.md): one error object, cursor lists, `oak_` keys, sats, and no house rate limit on your own keys. Headers, the workspace, list cursors, and body size that every call shares are in [Calling the API](calling.md). The JSON bodies, event names, and paths below are the contract. A client library may name the calls `agents.create` or `sessions.events.send`; the wire format is HTTP.

How you steer a session, including outcomes, previews, and the budget pause, is in [Managed agent events](managed-agents-events.md). Webhooks, cron fires, the files you keep, and spend are in [Managed agent operations](managed-agents-operations.md). A single model call with no session is [`POST /v1/responses`](../inference/gateway.md). Counts, batches, files, citations, cache breakpoints, the tool loop, and reasoning replay on that call are in [Model calls](model-calls.md).

The agent loop stays on OpenAgents. On a `cloud` environment, OpenAgents also runs the container, on a cloud computer admitted for the workspace and billed to its balance. On a `self_hosted` environment, bash and file tools run on a computer the workspace granted to the key ([decision D4](2026-10-02-openagents-api.md)). OpenAgents never dials into that computer. `web_search` and `web_fetch` run on OpenAgents in both cases.

## Conventions

| Topic | Rule |
| --- | --- |
| Auth | `Authorization: Bearer oak_<id>.<secret>`. The key's workspace owns every row. A session that belongs to several workspaces sends `X-Workspace-Id`. Another workspace's id is `404`. |
| Scopes | `agents` creates and runs agents, sessions, environments, deployments, files, skills, memory, and webhook endpoints. `agents:read` reads them. `vaults` creates credentials and passes `vault_ids`. `vaults:read` reads vault and credential metadata. Secrets are write-only. Scopes only narrow the key. |
| Environment key | A `self_hosted` environment has an `envk_` key, returned once. The worker sends it as `Authorization: Bearer envk_...`. It can claim and finish work for that environment and nothing else. |
| Idempotency | `Idempotency-Key` on every `POST` that creates or spends. The same key and body within 24 hours returns the first response. The same key and a different body is `409` `idempotency_conflict`. |
| Lists | `?limit=` (default 20, maximum 100) and `?after=<cursor>`. The body is `{"data": [...], "next": "<cursor>" or null}`. An expired cursor is `410` `cursor_expired`. Where a list accepts `order` (`asc` or `desc`), the cursor binds that order; reusing it with a different `order` is `400`. |
| Errors | `{"error": {"type", "code", "message", "param", "request_id"}}`. Statuses are in [Errors](#errors). Every response carries `x-request-id` and `openagents-version`. |
| Streaming | `GET .../events/stream` and `GET .../threads/{thread_id}/stream` are server-sent events. Send `Accept: text/event-stream`. |
| Money | Sats, as integers named `amount_sats`. Model calls inside a session draw the workspace balance at the served model's list price. A failed call is not charged. Every model response names the route, the model, the cost in sats, and the time. |
| Versioning | `/v1` only. Responses carry `openagents-version`. The toolset id `agent_toolset_20260401` and the multiagent id `multiagent_20261001` are resource versions: a change to the tools or the multiagent block ships as a new id. |
| Times | RFC 3339. |
| Metadata | At most 16 pairs. Keys up to 64 characters. Values up to 512 characters. |

OpenAgents does not impose a request-per-minute cap on your keys ([decision D16](2026-10-02-openagents-api.md)). A cap you set on the key returns `429` `rate_limited` with `Retry-After`.

### Shared paths

Four paths already name something else. This API keeps the paths and distinguishes the objects.

| Path | Already | This API |
| --- | --- | --- |
| `POST /v1/sessions` | Exchanges an `oak_` key for an account session (`sess_`). | A body with `agent` and `environment_id` creates an agent session (`sesn_`). A body without `agent` stays the account-session exchange. `GET /v1/sessions` lists agent sessions only. |
| `/v1/environments` | The website's repository-environment setup (`message`, `retry`, `save`), on the website host, not on `api.openagents.com`. | On `api.openagents.com`, an execution environment: `config.type` is `cloud` or `self_hosted`. The website routes are not mounted here. |
| `/v1/files` | Planned chat attachments. | One workspace file object. Chat attachments, session mounts, and outcome rubrics use it. |
| `/v1/skills` | The workspace skill directory, first-party today. | The public shape of that directory: a versioned `SKILL.md` an agent loads. A plugin stays `/v1/plugins`. A skill is the instructions inside a plugin, not a second product name for the plugin. |

### Identifiers

| Object | Prefix |
| --- | --- |
| Agent | `agent_` |
| Environment | `env_` |
| Environment key | `envk_` |
| Agent session | `sesn_` |
| Event | `sevt_` |
| Vault | `vlt_` |
| Deployment | `depl_` |
| Deployment run | `drun_` |
| Memory store | `memstore_` |
| Memory | `mem_` |
| Memory version | `memver_` |
| File | `file_` |
| Skill | `skill_` |
| Workflow run | `wrun_` |
| Work item | `work_` |
| Webhook endpoint | `whk_` |
| Webhook event | `whe_` |
| Outcome | `outc_` |

Account sessions stay `sess_`. Thread, resource, and credential ids are opaque. Webhook secrets are opaque and have no prefix.

### Models

`model` on an agent is a string id from `GET /v1/models`, or an object:

```json
{ "id": "openagents", "speed": "standard", "effort": "high", "inference_geo": "us" }
```

| Field | Values | Effect |
| --- | --- | --- |
| `id` | `openagents`, `openagents/auto`, `openagents/fast`, `openagents/chat`, `openagents/code`, `openagents/reason`, `openagents/long`, `openagents/classify`, or an id from `GET /v1/models?include=underlying` when the key allows it | Which model serves the agent. |
| `speed` | `standard` (default), `fast` | `fast` is accepted on `openagents` and `openagents/auto` and serves `openagents/fast`. Any other id returns `400` `model_speed_unsupported`. |
| `effort` | `low`, `medium`, `high`, `xhigh`, `max`, or `{"type": "<level>"}` | On `openagents` and `openagents/auto`, `low` and `medium` stay on `openagents/chat` (or `openagents/fast` when `speed` is `fast`). `high`, `xhigh`, and `max` ask `openagents/reason`, or `openagents/code` when the agent enables `bash`, `write`, or `edit`. A fixed id stays that id. The level is stored and returned. |
| `inference_geo` | `us`, `global` | Pins the region when the route has one. A roster's pins must all match or all be unset, on save and on a session `model` override. A mismatch is `400` `inference_geo_mismatch`. |

A string is `standard` speed and that model's default effort.

## Endpoints

There is no inline agent on session create. Create the agent first, then the session.

### Agents

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/agents` | List agents. |
| `POST` | `/v1/agents` | Create an agent. |
| `GET` | `/v1/agents/{agent_id}` | Read an agent. |
| `POST` | `/v1/agents/{agent_id}` | Update an agent. `version` is optional. Send it (1 or greater) for optimistic concurrency; a mismatch is `409` `version_conflict`. Omit it for last write wins. |
| `POST` | `/v1/agents/{agent_id}/archive` | Archive. The agent becomes read-only. Existing sessions continue. New sessions cannot reference it. There is no unarchive. |
| `GET` | `/v1/agents/{agent_id}/versions` | List versions. |

Agents have no delete.

```json
{
  "name": "Research lead",
  "description": "Plans research and checks the reports.",
  "model": "openagents",
  "system": "When a request splits into independent questions, delegate each one, then check and combine the reports.",
  "tools": [
    {
      "type": "agent_toolset_20260401",
      "default_config": { "enabled": true, "permission_policy": { "type": "auto" } },
      "configs": [
        { "name": "web_fetch", "enabled": false },
        { "name": "web_search", "enabled": false }
      ]
    }
  ],
  "skills": [{ "type": "custom", "skill_id": "skill_abc123", "version": "latest" }],
  "mcp_servers": [{ "type": "url", "name": "github", "url": "https://mcp.example.com/github" }],
  "multiagent": {
    "type": "coordinator",
    "agents": ["agent_abc123", { "type": "self" }]
  },
  "metadata": { "team": "research" }
}
```

| Field | Required | Limit |
| --- | --- | --- |
| `name` | Yes | 1–256 characters. Unique inside a roster. Do not name an agent `self` or `openagents.advisor`. |
| `model` | Yes | String or object, above. |
| `description` | No | Up to 2,048 characters. The coordinator reads this when it chooses whom to spawn. |
| `system` | No | Up to 100,000 characters. |
| `tools` | No | Up to 128 entries. |
| `skills` | No | Up to 20. |
| `mcp_servers` | No | Up to 20, unique `name`. No credential on this object. |
| `multiagent` | No | See [Multiagent](#multiagent). |
| `metadata` | No | The metadata limits above. |

### Sessions

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/sessions` | List agent sessions. |
| `POST` | `/v1/sessions` | Create a session. |
| `GET` | `/v1/sessions/{session_id}` | Read a session, including `status` and `usage`. |
| `POST` | `/v1/sessions/{session_id}` | Update `title`, `metadata`, `agent.tools`, `agent.mcp_servers`, or `budget`. Tool and MCP edits require `status: idle` and apply to this session only. |
| `DELETE` | `/v1/sessions/{session_id}` | Delete a session. |
| `POST` | `/v1/sessions/{session_id}/archive` | Archive a session. |

`agent` on create is one of:

| Form | Meaning |
| --- | --- |
| `"agent_abc123"` | Latest version at create time. |
| `{ "type": "agent", "id", "version" }` | That version. |
| `{ "type": "agent_with_overrides", "id", "version?", "model?", "system?", "tools?", "mcp_servers?", "skills?" }` | Those fields override the stored agent for this session only. |

An `effort` inside a `model` override applies. The agent's own effort is not carried over. A `model` override without `effort` runs at that model's default. An `inference_geo` on the override replaces the agent's pin; omitting it clears the pin for this session. Overrides apply to the coordinator and its `self` copies. Roster agents referenced by id keep the configuration they were saved with.

```json
{
  "agent": "agent_abc123",
  "environment_id": "env_abc123",
  "title": "Review auth",
  "vault_ids": ["vlt_abc123"],
  "resources": [],
  "initial_events": [
    {
      "type": "user.message",
      "content": [{ "type": "text", "text": "Review the auth module." }]
    }
  ],
  "budget": { "type": "limit", "max_list_cost": { "amount_sats": 25000 } },
  "metadata": {}
}
```

`initial_events` accepts at most 50 events, and only `user.message` and `user.define_outcome`. A deployment's `initial_events` also accepts `system.message`. Validation is all or nothing. More than 100 file-sourced document blocks across the list is `400`. A body over 32 MB is `413`. With no `initial_events`, create registers the session as `idle` and does not start the container. The container starts when the session first needs it. A non-empty `initial_events` creates the session already `running`.

`budget` is optional and create-only on a session. `amount_sats` is a hard cap on the session's list-priced spend, shared by every thread. You can raise or lower it later. The new value must be greater than the sats already consumed. You can remove it with `null`. Removal is one-way: you cannot add a budget after create. A deployment's budget is copied onto each fired session and, unlike a session's, can be added later and added again after you clear it.

`vault_ids` is create-only. An update that sets it is `400`.

`usage.list_cost.amount_sats` on the session is the figure to trust. Per-thread figures do not sum to it: on a cloud environment the session figure also includes container time, and each figure is rounded on its own. `usage` also names the route, the model, and the time for the calls so far. Token fields, cache counts, and what list cost includes are in [events](managed-agents-events.md#usage).

Session `status` is `running`, `idle`, `rescheduling`, or `terminated`. If any thread is `running`, the session is `running`. `GET` returns `outcome_evaluations` when the session has any.

`stop_reason` is on `session.status_idle` and `session.thread_status_idle`, not on the session object. Its `type` is one of:

| `stop_reason.type` | Meaning |
| --- | --- |
| `end_turn` | The agent finished its turn. |
| `requires_action` | A tool is waiting for you. `event_ids` names the `sevt_` events. |
| `budget_reached` | The session budget is spent. Threads pause on their own as the cap is hit. One thread can pause while another finishes the request already in flight. `requires_action` outranks the cap at session level. |
| `retries_exhausted` | The agent stopped after repeated errors. |

An empty workspace balance does not use a new stop reason. The next model call emits `session.error` with `error.type` `insufficient_balance`, and the session goes `idle`. Top up the balance, then send a `user.message`.

### Events

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/sessions/{session_id}/events` | List events. Optional `types[]` filter. |
| `POST` | `/v1/sessions/{session_id}/events` | Send events. |
| `GET` | `/v1/sessions/{session_id}/events/stream` | Stream events. `event_deltas[]=agent.message` and `event_deltas[]=agent.thinking` opt in to `event_start` and `event_delta` previews. |

```json
{
  "events": [
    {
      "type": "user.message",
      "content": [{ "type": "text", "text": "Hello" }]
    }
  ]
}
```

Events you send:

| Type | Role |
| --- | --- |
| `user.message` | A turn from you. On the primary thread, this text counts as your intent when a tool policy is `auto`. |
| `system.message` | Context appended for this turn and later ones. Accepted on every model id this API serves. Checked against the agent's primary model. |
| `user.define_outcome` | A goal and a rubric. See [Outcomes](#outcomes). |
| `user.tool_confirmation` | `tool_use_id` is the `sevt_` id of the `agent.tool_use` or `agent.mcp_tool_use` event (not a tool-call id). `result` is `allow` or `deny`. Optional `deny_message` is the tool result the agent sees. Several confirmations can go in one request. A confirmation for an event whose `evaluated_permission` is not `ask` is `400`, including a call the server denied under `auto`. |
| `user.custom_tool_result` | Your application's answer to `agent.custom_tool_use`. `custom_tool_use_id` is that event's `sevt_` id. `content` is an array of content blocks. `is_error` is a boolean. |
| `user.interrupt` | With no `session_thread_id`, interrupts every non-archived thread, including the primary. With `session_thread_id`, interrupts that thread. |
| `user.tool_result` | A settle event, processed on receipt. Permission asks use `user.tool_confirmation`. Custom tools use `user.custom_tool_result`. |

`system.message` content is 1 to 1,000 text blocks. `openagents/classify` rejects it with `400`. While `requires_action`, it is accepted only as a later event in the same request as a settle event. Sending rules, the idle gate, live-preview deltas, and the budget pause order are in [events](managed-agents-events.md).

Events the session emits include `agent.message`, `agent.thinking` (a progress signal, with no reasoning text), `agent.tool_use`, `agent.mcp_tool_use`, `agent.tool_result`, `agent.custom_tool_use`, `agent.thread_context_compacted`, `session.status_idle`, `session.status_running`, `session.status_terminated`, `session.usage`, `session.error`, `span.model_request_start`, `span.model_request_end`, the outcome spans, and the thread and workflow events in [Multiagent](#multiagent). Each stored event carries `id` (`sevt_`) and `processed_at`. Tolerate an event `type` you do not recognize.

A tool result larger than 100,000 characters is written to a file in the container. The agent receives a short preview and the path, and can `read` the rest. This covers agent tools and MCP tools.

### Threads

Per-agent event streams inside one session. The session stream is the primary thread.

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/sessions/{session_id}/threads` | List threads, including the primary. |
| `GET` | `/v1/sessions/{session_id}/threads/{thread_id}` | Read one thread. |
| `POST` | `/v1/sessions/{session_id}/threads/{thread_id}/archive` | Archive a thread. |
| `GET` | `/v1/sessions/{session_id}/threads/{thread_id}/events` | List that thread's events. |
| `GET` | `/v1/sessions/{session_id}/threads/{thread_id}/stream` | Stream that thread. Same `event_deltas[]` as the session stream. |

A thread has `id`, `status` (`running`, `idle`, `rescheduling`, `terminated`), `agent` (a snapshot), `parent_thread_id` (`null` on the primary), `workflow_run_id` (`null` except on a workflow run's threads), `archived_at`, and optional `stats` and `usage`. At most 25 child threads per session, idle ones included, until you archive them. Advisor threads and a workflow run's threads do not count toward 25.

Previews are thread-scoped. A child thread's `event_delta` events arrive only on that thread's stream. A reply to the coordinator rides `agent.thread_message_sent` and is not previewed. Break a per-thread read on `session.thread_status_idle`, and read its `stop_reason` the same way you read the session's.

### Resources

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/sessions/{session_id}/resources` | List resources on the session. |
| `POST` | `/v1/sessions/{session_id}/resources` | Attach a `file` or `github_repository`. The method is add, not create. |
| `GET` | `/v1/sessions/{session_id}/resources/{resource_id}` | Read one resource. |
| `POST` | `/v1/sessions/{session_id}/resources/{resource_id}` | Update a resource. You can rotate a repository's `authorization_token` here. |
| `DELETE` | `/v1/sessions/{session_id}/resources/{resource_id}` | Detach a resource. |

`memory_store` resources attach only in the session create body. A `self_hosted` environment accepts only `memory_store`. `file` and `github_repository` there are `400`.

Resources are resolved at create. A bad `file_id` or an unreachable repository fails the create, not a later turn. At most 999 file resources per session. More than one GitHub repository is allowed. At most 8 memory stores.

**File.** Upload with the Files API, then attach `{ "type": "file", "file_id", "mount_path"? }`. The session stores a copy, and `resources[].file_id` is that copy's id, not the upload's. You can delete the upload. The copy is removed with the session. `mount_path` is absolute and rooted under the uploads directory: `/data.csv` lands at `/mnt/session/uploads/data.csv`. Without it, the file lands at `/mnt/session/uploads/<file_id>`. Mounts are read-only. The agent writes new files elsewhere. The working directory is `/workspace`.

**GitHub repository.**

| Field | Required | Notes |
| --- | --- | --- |
| `type` | Yes | `github_repository`. |
| `url` | Yes | Repository URL. |
| `authorization_token` | Yes | A GitHub token with repository access. Never returned. `Contents: Read` clones. `Contents: Read and write` pushes. |
| `mount_path` | No | Defaults to `/workspace/<repo-name>`. |
| `checkout` | No | `{ "type": "branch", "name" }` or `{ "type": "commit", "sha" }`. Default is the repository's default branch. |

The token never enters the container. `git pull`, `git push`, and GitHub REST calls for that repository go through an OpenAgents proxy that adds the token after the request leaves the container. Cloud only. The repository is mounted for the life of the session. To mount a different repository, create a session.

A mounted repository's root `.agents/skills/<skill-name>/` directory is scanned once, at session start, from the checked-out revision. Each skill's name, description, and path are shown to the agent. A bare `.agents/skills/SKILL.md`, a deeper nest, a `skills/` directory outside `.agents`, and a `.agents/skills` inside a package subdirectory are not scanned. Commits pushed later are not picked up. A repository attached to a session that is already running is not scanned. If a repository skill shares a name with an attached skill, both are available, each with its own path. Treat repository skills as part of the trust boundary: anyone who can commit to that repository can change what the agent reads, and tools such as `bash` can act on it.

Opening a pull request needs a GitHub MCP server and a vault credential. The repository resource gives the filesystem and git, not the pull-request API.

**Memory store.** See [Memory stores](#memory-stores). Create-time only:

```json
{
  "type": "memory_store",
  "memory_store_id": "memstore_abc123",
  "access": "read_write",
  "instructions": "Check this before starting."
}
```

`access` is `read_write` (default) or `read_only`. `instructions` is at most 4,096 characters, added to the store's name and description.

### Environments

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/environments` | Create an environment. |
| `GET` | `/v1/environments` | List environments. |
| `GET` | `/v1/environments/{environment_id}` | Read an environment. |
| `POST` | `/v1/environments/{environment_id}` | Update an environment. Changes apply to new containers. Running sessions keep the config they started with. |
| `DELETE` | `/v1/environments/{environment_id}` | Delete. Returns `204`. |
| `POST` | `/v1/environments/{environment_id}/archive` | Archive. Read-only. Existing sessions continue. New sessions cannot reference it. No unarchive. |
| `GET` | `/v1/environments/{environment_id}/work` | Self-hosted: claim the next work item. Environment key. |
| `GET` | `/v1/environments/{environment_id}/work/stats` | Self-hosted queue stats. Workspace `oak_` key. |
| `POST` | `/v1/environments/{environment_id}/work/{work_id}/stop` | Self-hosted: stop a claimed work item. Workspace `oak_` key. |

Names are unique per workspace. A duplicate is `409` `name_taken`.

```json
{
  "name": "dev",
  "config": {
    "type": "cloud",
    "networking": {
      "type": "limited",
      "allowed_hosts": ["api.example.com"],
      "allow_package_managers": true,
      "allow_mcp_servers": true
    },
    "packages": {}
  }
}
```

`networking.type` is `unrestricted` or `limited`. On `limited`, egress is deny-by-default.

| Field | Default | Effect |
| --- | --- | --- |
| `allowed_hosts` | none | Hosts the container may reach. Also binds `web_search` and `web_fetch`: fetch of any other host is an error result, and search omits those hosts. An empty list means those tools return nothing. A host you add here is open to the container as well. |
| `allow_package_managers` | `false` | PyPI, npm, and the other registries. Required when `packages` is set. Listing a registry in `allowed_hosts` is not enough. |
| `allow_mcp_servers` | `false` | The agent's MCP server hosts, without listing each one. If this is false and the host is not in `allowed_hosts`, session create fails with `400` and names the hosts. |

`unrestricted` does not apply `allowed_hosts` to the web tools. `packages` names the system packages to install in a cloud container.

A `self_hosted` config is exactly `{ "type": "self_hosted" }`. `networking` and `packages` do not apply. You control egress on that computer. Create returns `environment_key` (`envk_`) once. Send `rotate_environment_key: true` on update to revoke it and receive a new key once.

The worker is outbound only. It long-polls `GET /v1/environments/{environment_id}/work` with the environment key. Query `block_ms` (1–999, or omit for a non-blocking read) and `reclaim_older_than_ms` (reclaim a lease from a worker that died). A claim is:

```json
{
  "id": "work_abc123",
  "environment_id": "env_abc123",
  "session_id": "sesn_abc123",
  "secret": "work_secret_once"
}
```

Pass `secret` only into the container for that session. The worker uses it to sync memory stores and to post that session's tool traffic. Do not log it. On a cloud environment OpenAgents runs this loop. On a granted computer, your worker does: bash, `read`, `write`, `edit`, `glob`, and `grep` run there; `web_search` and `web_fetch` still run on OpenAgents.

Stats returns `{ "type": "work_queue_stats", "depth", "pending", "oldest_queued_at", "workers_polling" }`.

A worker that fails a claim because a memory store did not mount leaves the session `idle` with `requires_action` and no `session.error`. Fix the computer, then send `user.interrupt`. The work is queued again.

The computer must provide `/bin/bash` at that path and a writable `/mnt/memory` (Linux or macOS). Skills attached to the agent are written to `{workdir}/skills/<name>/` before tools run, and those files are executable.

| Concern | `cloud` | `self_hosted` |
| --- | --- | --- |
| Container | OpenAgents, on a workspace cloud computer | The granted computer |
| `file`, `github_repository` | Mounted by OpenAgents | Rejected |
| `memory_store` | Live mount at `/mnt/memory/<store-name>/` | The worker downloads the store to the same path and syncs it |
| `environment_variable` vault credentials | Substituted at egress | Not supported. Use an MCP credential or a custom tool |
| Web tools | OpenAgents | OpenAgents |

### Deployments

A deployment (`depl_`) runs an agent on a cron schedule. Each fire creates a session.

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/deployments` | Create a deployment. |
| `POST` | `/v1/deployments/{deployment_id}` | Update it. |
| `POST` | `/v1/deployments/{deployment_id}/pause` | Stop scheduled fires. Reversible. Manual runs still work. |
| `POST` | `/v1/deployments/{deployment_id}/unpause` | Resume at the next occurrence. Missed fires are not made up. |
| `POST` | `/v1/deployments/{deployment_id}/archive` | Terminal. The schedule stops. The deployment is immutable. |
| `POST` | `/v1/deployments/{deployment_id}/run` | Run now. The run's `trigger_context.type` is `manual`. Works while paused. |

There is no list or get. Keep the id from create.

```json
{
  "name": "Weekly compliance scan",
  "agent": "agent_abc123",
  "environment_id": "env_abc123",
  "initial_events": [
    {
      "type": "user.message",
      "content": [{ "type": "text", "text": "Run the weekly compliance scan." }]
    }
  ],
  "schedule": {
    "type": "cron",
    "expression": "0 20 * * 5",
    "timezone": "America/New_York"
  },
  "budget": { "type": "limit", "max_list_cost": { "amount_sats": 25000 } }
}
```

`agent` accepts the same forms as session create. `resources`, `vault_ids`, and `budget` match session create and are copied onto each fired session. `initial_events` must include at least one `user.message`, `user.define_outcome`, or `system.message`. The response includes `status`, `paused_reason`, and `schedule.upcoming_runs_at` (the exact next times). Execution can lag by up to 15 percent of the interval, at least 5 seconds and at most 9 minutes. A workspace holds at most 1,000 deployments. Cron, daylight-saving skips and double fires, budget changes, and which failures pause the deployment are in [operations](managed-agents-operations.md#schedules). A fire that cannot create a session still writes a deployment run, except when the agent is archived: that archives the deployment and writes no run.

### Deployment runs

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/deployment_runs?deployment_id=` | List runs. `has_error=true` keeps failures. |
| `GET` | `/v1/deployment_runs/{deployment_run_id}` | Read one run (`drun_`). |

A run carries either `session_id` or `error.type`: `environment_archived`, `agent_archived`, `vault_not_found`, `computer_not_granted`, `session_rate_limited`, `insufficient_balance`, `service_unavailable`. `session_rate_limited` means a cap you set on the key. That fire is not retried. The next occurrence tries again. Webhook registration, the signature, and the `data.type` list are in [operations](managed-agents-operations.md#webhooks). A scheduled run emits `deployment_run.started`, `deployment_run.succeeded`, or `deployment_run.failed`. A manual run emits none of those. `data.id` is the `drun_` id.

### Vaults

A vault stores credentials OpenAgents applies for you. This `vlt_` object is the session credential store. A person's sealed files are a different vault, specified in [NIP-VAULT](../../nips/openagents/NIP-VAULT.md) and opened with `openagents vault`. Attach the session vault with `vault_ids` on session create. Vaults are workspace-scoped. A member with access can create them and read metadata. Secret fields are write-only and are never in a response.

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/vaults` | Create a vault. |
| `GET` | `/v1/vaults` | List vaults. |
| `GET` | `/v1/vaults/{vault_id}` | Read a vault. |
| `POST` | `/v1/vaults/{vault_id}` | Update a vault. |
| `DELETE` | `/v1/vaults/{vault_id}` | Delete a vault. |
| `POST` | `/v1/vaults/{vault_id}/archive` | Archive a vault. |

### Credentials

At most 20 credentials in a vault. `mcp_server_url` and `secret_name` are unique among active credentials. A duplicate is `409`. Those keys are immutable. To change `mcp_server_url`, `secret_name`, `token_endpoint`, or `client_id`, archive the credential and create another. Archive purges the secret and frees the key. Credentials are not checked at write time. A bad credential surfaces as `session.error` during the session and does not stop the session. An invalid MCP credential still lets the session create; auth is retried on the next `idle` to `running` transition.

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/vaults/{vault_id}/credentials` | Create a credential. |
| `GET` | `/v1/vaults/{vault_id}/credentials` | List credentials. |
| `GET` | `/v1/vaults/{vault_id}/credentials/{credential_id}` | Read metadata. |
| `POST` | `/v1/vaults/{vault_id}/credentials/{credential_id}` | Update a credential. |
| `DELETE` | `/v1/vaults/{vault_id}/credentials/{credential_id}` | Delete a credential. |
| `POST` | `/v1/vaults/{vault_id}/credentials/{credential_id}/archive` | Archive a credential. |
| `POST` | `/v1/vaults/{vault_id}/credentials/{credential_id}/mcp_oauth_validate` | Validate an MCP OAuth credential. |

**MCP credentials** (`mcp_oauth`, `static_bearer`) are keyed by `mcp_server_url`. The match is normalized: scheme and host are lowercased, and a default port and a trailing slash are stripped. A different path, subdomain, or non-default port does not match. If nothing matches, the connection is unauthenticated. `mcp_oauth` refresh uses the OAuth 2.0 `refresh_token` grant against `refresh.token_endpoint`.

```json
{
  "display_name": "Notion (workspace-foo)",
  "auth": {
    "type": "mcp_oauth",
    "mcp_server_url": "https://mcp.example.com/mcp",
    "access_token": "<access token>",
    "expires_at": "2026-04-02T14:00:00Z",
    "refresh": {
      "refresh_token": "<refresh token>",
      "client_id": "<client id>",
      "token_endpoint": "https://example.com/oauth/token",
      "token_endpoint_auth": { "type": "none" }
    }
  }
}
```

`token_endpoint_auth.type` is `none`, `client_secret_basic` (`client_secret`), or `client_secret_post` (`client_secret`). Omit `refresh` to keep a token that cannot be refreshed. Hosted MCP servers want the OAuth token for that server, not the product's REST API key. `static_bearer` is `{ "type": "static_bearer", "mcp_server_url", "token" }`.

**Environment variables** are keyed by `secret_name`. The container sees an opaque placeholder. The real value is substituted at egress, on requests to `networking.allowed_hosts` only, and only in the header and body. A secret in a URL path is never substituted. Cloud only.

```json
{
  "display_name": "Twilio",
  "auth": {
    "type": "environment_variable",
    "secret_name": "TWILIO_API_KEY",
    "secret_value": "sk-your-secret-here",
    "networking": { "type": "limited", "allowed_hosts": ["api.twilio.com", "*.twilio.com"] },
    "injection_location": { "header": true, "body": false }
  }
}
```

`networking` is `limited` with `allowed_hosts`, or `unrestricted`. `allowed_hosts` chooses which requests receive the secret. The environment's own networking still has to allow the host. `injection_location` chooses header, body, or both. On create, omit it to enable both; if you send the object, a field you omit is `false`. On update, fields merge one at a time. At least one location stays enabled. `null` is `400`. A placeholder in a disabled location is sent as the placeholder string. The response always returns both booleans. A client that checks the secret's shape before any network call sees the placeholder and can fail before a request exists.

Vault credentials never enter the container. MCP calls and git operations for an attached repository are proxied, and the credential is added after the request leaves the container. Do not put a key in `system` or in a `user.message`. If a self-hosted computer must call a service whose secret you hold, register a custom tool: your program runs the call and returns `user.custom_tool_result`. The container never sees the secret.

You can update `display_name`, the secret fields (`token`, `access_token`, `refresh_token`, `client_secret`, `secret_value`), and `injection_location`.

### Memory stores

A memory store is a workspace collection of text documents that outlives a session. It is not the knowledge base: the knowledge base is cited reference, and a store is notes for later sessions of this agent. Bytes live in the workspace bucket. Every row has `workspace_id`.

Do not put credentials in a store. A value written once is replayed into every later session that mounts it. Use a vault. If a secret is already there, delete the memory and redact the versions.

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/memory_stores` | Create a store (`name`, `description`, `metadata`). `description` is shown to the agent. |
| `GET` | `/v1/memory_stores` | List stores. Filters: `include_archived`, `created_at_gte`, `created_at_lte`. |
| `GET` | `/v1/memory_stores/{memory_store_id}` | Read a store. |
| `POST` | `/v1/memory_stores/{memory_store_id}` | Update a store. |
| `DELETE` | `/v1/memory_stores/{memory_store_id}` | Delete a store. |
| `POST` | `/v1/memory_stores/{memory_store_id}/archive` | Archive. Read-only. Existing sessions continue. New sessions cannot reference it. No unarchive. |

On a cloud session each attached store is mounted at `/mnt/memory/<store-name>/`. The agent uses `bash`, `read`, `write`, `edit`, `glob`, and `grep`. There is no memory tool. `read_only` is enforced by the filesystem. A short note (name, path, instructions, access) is added to the system prompt. Writes under the mount are stored and produce memory versions.

On a self-hosted session the worker copies the store to the same path and syncs on an interval (default 15 seconds, minimum 5). Other sessions see writes after a sync. Conflicts keep the store's copy. `read_only` is enforced by `write` and `edit` and by the upload path; `bash` can change the local file, and the next sync overwrites it. A claim with no `secret` for a session that has stores fails the work item.

### Memories

One text document, at most 100 KB. Prefer many small files.

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/memory_stores/{memory_store_id}/memories` | List. Returns a memory or a `memory_prefix` (`path` only). Filters: `path_prefix` (a trailing slash matches children, not a longer name), `depth`, `view` (`basic` or `full`). `basic` omits `content`. |
| `POST` | `/v1/memory_stores/{memory_store_id}/memories` | Create at `path`. If the path is taken, `409` `memory_path_conflict` and `conflicting_memory_id`. |
| `GET` | `/v1/memory_stores/{memory_store_id}/memories/{memory_id}` | Read one. Default `view` is `full`. |
| `PATCH` | `/v1/memory_stores/{memory_store_id}/memories/{memory_id}` | Change `content`, `path`, or both. Optional `precondition`: `{ "type": "content_sha256", "content_sha256" }`. A mismatch is `409` `memory_precondition_failed`. |
| `DELETE` | `/v1/memory_stores/{memory_store_id}/memories/{memory_id}` | Delete. Optional `expected_content_sha256`. |

Only `PATCH` takes `precondition`.

### Memory versions

An immutable snapshot per change (`memver_`). `operation` is `created`, `modified`, or `deleted`.

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/v1/memory_stores/{memory_store_id}/memory_versions` | Newest first. Filters: `memory_id`, `operation`, `session_id`, `api_key_id`, `created_at_gte`, `created_at_lte`. |
| `GET` | `/v1/memory_stores/{memory_store_id}/memory_versions/{version_id}` | The list fields plus `content`. |
| `POST` | `/v1/memory_stores/{memory_store_id}/memory_versions/{version_id}/redact` | Clear `content`, `content_sha256`, `content_size_bytes`, and `path`. Actor and timestamps stay. |

Versions have no delete and no archive. Each version records `created_by`, an actor whose `type` is `session_actor`, `api_actor`, or `user_actor`. After a redact it also has `redacted_at` and `redacted_by`.

### Files

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/files` | Upload. `multipart/form-data` with a `file` field. |
| `GET` | `/v1/files` | List. `scope_id=<session_id>` returns files the agent wrote under `/mnt/session/outputs/` for that session. Pass the `sesn_` id from create. |
| `GET` | `/v1/files/{file_id}` | Metadata (`filename`, `size_bytes`, `created_at`). |
| `GET` | `/v1/files/{file_id}/content` | Bytes. |
| `DELETE` | `/v1/files/{file_id}` | Delete. |

Outputs can lag one to three seconds after `session.status_idle`. The agent needs `write` or `bash` to create them. Files have no archive. When the organization has attached a customer-managed key, these bytes are wrapped with that key. See [Customer-managed keys](organization.md#customer-managed-keys).

### Skills

A skill is a `SKILL.md` plus the files it ships. The agent sees the name and description and reads the skill when the task matches. Attach skills on the agent, or let a mounted repository's `.agents/skills/` supply them (cloud only). At most 20 attached skills.

| `type` | `skill_id` | `version` |
| --- | --- | --- |
| `openagents` | An id OpenAgents publishes. List them with `GET /v1/skills?owner=openagents`. | `latest` (default) or a version number. |
| `custom` | A `skill_` id from this API. | `latest` (default) or a version number. |

| Method | Path | Description |
| --- | --- | --- |
| `POST` | `/v1/skills` | Create a skill. |
| `GET` | `/v1/skills` | List skills. |
| `GET` | `/v1/skills/{skill_id}` | Read a skill. |
| `DELETE` | `/v1/skills/{skill_id}` | Delete a skill. |
| `POST` | `/v1/skills/{skill_id}/versions` | Create a version. |
| `GET` | `/v1/skills/{skill_id}/versions` | List versions. |
| `GET` | `/v1/skills/{skill_id}/versions/{version}` | Read a version. |
| `DELETE` | `/v1/skills/{skill_id}/versions/{version}` | Delete a version. |

Skills have no archive. This is the workspace skill directory (`tenancy::skills`) on the public API. Publishing a payable package remains `/v1/plugins`.

## Tools

| Kind | Who runs it |
| --- | --- |
| Agent toolset `agent_toolset_20260401` | OpenAgents, in the session container, for `cloud`. Your worker, for `self_hosted`, supplies bash and the file tools. `web_search` and `web_fetch` always run on OpenAgents. |
| MCP toolset `mcp_toolset` | OpenAgents calls the MCP server and injects the vault credential. |
| Custom tool `custom` | Your application. The session emits `agent.custom_tool_use`, goes `idle`, and waits for `user.custom_tool_result`. |

A bare `{ "type": "agent_toolset_20260401" }` enables all eight tools. Start with the web tools off, and set `permission_policy` to `auto`. Turn a web tool on only when the job needs it.

| Tool | What it does |
| --- | --- |
| `bash` | Run a shell command. |
| `read` | Read a file, including text, images, PDFs, and notebooks. |
| `write` | Write a file. |
| `edit` | Replace a string in a file. |
| `glob` | Match paths. |
| `grep` | Search file text with a regular expression. |
| `web_fetch` | Fetch a URL. |
| `web_search` | Search the web. |

`default_config` applies to every tool in the toolset: `enabled` and `permission_policy`. `configs` overrides by `name`. `type` is optional on input (the server fills it from `name`) and always present on output. To enable a few tools, set `default_config.enabled` to `false` and list those names with `enabled: true`.

The toolset id is the version. A new tool set ships as a new id.

### Permission policies

Policies apply to the agent toolset and to MCP tools. They do not apply to custom tools.

| Policy | Behavior |
| --- | --- |
| `always_allow` | The tool runs. Default for the agent toolset. |
| `always_ask` | The session goes `idle` with `stop_reason.type` `requires_action` until you send `user.tool_confirmation`. Default for an MCP toolset. |
| `auto` | OpenAgents evaluates the call (tool, input, and session content so far) and runs it, denies it, or pauses for you. Neither toolset defaults to `auto`. Set it on the agent toolset. Leave an MCP toolset on `always_ask` unless you want that server callable with no review. |

Under `auto`, one call has one outcome:

| Outcome | What happens |
| --- | --- |
| Runs | The call is treated as safe and runs. Your client does not see it first. |
| Denied | The tool does not run. The agent receives an error result, `Permission to use {tool_name} has been denied.`, with `is_error: true`. The session keeps running. You cannot override the denial. |
| Pauses | No determination. The session pauses as it does for `always_ask`. |

`auto` is not a checkpoint. A call judged safe runs before a person sees it, and the effect can be permanent. If a person must review a tool, set `always_ask` on that tool.

Session content is material to assess, not instructions to follow. Text in a primary-thread `user.message`, including end-user text you relay there, counts as your intent and can allow a call that would otherwise be denied. Some calls are still denied. The same words in a tool result, a fetched page, an MCP response, or a message between threads do not count as your intent. If you relay untrusted text in `user.message`, put `always_ask` on the tools you would not let that sender run.

Every `agent.tool_use` and `agent.mcp_tool_use` carries `evaluated_permission` (`allow`, `ask`, or `deny`) and usually an `evaluation`:

| `evaluation` | Top-level permission | Meaning |
| --- | --- | --- |
| `{ "type": "always_allow" }` | `allow` | The policy is `always_allow`. The call ran. |
| `{ "type": "always_ask" }` | `ask` | Waiting for you. |
| `{ "type": "auto", "evaluated_permission": { "type": "allow" } }` | `allow` | Judged safe. It ran. |
| `{ "type": "auto", "evaluated_permission": { "type": "ask", "reason_code": "indeterminate" } }` | `ask` | No determination. Waiting for you. |
| `{ "type": "auto", "evaluated_permission": { "type": "deny", "reason_code": "high_risk" } }` | `deny` | Denied. |

`reason_code` is for your client and for audit records. `evaluation` is absent when the tool is not enabled (`evaluated_permission` is `deny`) and on events recorded before the field existed (read `allow` as `always_allow` and `ask` as `always_ask`). Tolerate an unknown `evaluation.type` or `reason_code`. `agent.custom_tool_use` has neither field.

In a multiagent session, only a primary-thread `user.message` can sway `auto` toward allow. A denial under `auto` is not copied to the primary stream. The event and the error result stay on that thread, and the thread keeps running. A call that pauses, and every custom tool call, is copied to the primary stream with `session_thread_id`. Reply with `tool_use_id` or `custom_tool_use_id` only. Do not send `session_thread_id` back.

### Web search and web fetch

Set `allowed_domains` or `blocked_domains` on that tool's `configs` entry, never both. Each tool has its own list. With `limited` networking, `allowed_hosts` also applies, as [Environments](#environments) says.

| Setting | Tools | Meaning |
| --- | --- | --- |
| `allowed_domains` | both | The only hosts. Mutually exclusive with `blocked_domains` on the same entry. |
| `blocked_domains` | both | Hosts the tool cannot reach. |
| `max_content_tokens` | `web_fetch` | Cap on fetched text entering context. Binary content such as PDFs is not capped. |
| `user_location` | `web_search` | `{ "type": "approximate", "city"?, "region"?, "country"?, "timezone"? }` with at least one optional field. `country` is an uppercase ISO 3166-1 alpha-2 code. `timezone` is an IANA name. |

A `web_fetch` outside its list returns an error result (`is_error: true`, content names `url_not_allowed`). `web_search` omits results outside its list.

Domain rules, or the request is `400` `invalid_request` and the message names the list and the zero-based index:

- 1–64 domains, each 1–255 characters. An empty list is rejected. Omit the field, or send `null`, for no list. No duplicates.
- A hostname only: `example.com`, not a scheme, a port, or `*.example.com`. Case-insensitive. One trailing slash is ignored.
- A domain covers itself and its subdomains. `www` is a subdomain. List the bare domain to cover both.
- Rejected: IP addresses, bare suffixes (`com`, `co.uk`), single-label names, `localhost`, and names ending in `.localhost`, `.local`, `.internal`, `.localdomain`, or `.invalid`. Non-ASCII must be Punycode (`xn--`).
- `web_fetch` domains have no path. `web_search` domains may have a path (`example.com/blog`) with no space, `?`, `#`, or `$ , | ^ !`. Prefer a hostname.
- A domain the fetcher will not access, an unsupported `user_location.country`, or a bad `timezone` is also `400`.

The session checks the config again when the tool is first used. If a setting that was accepted is no longer valid, it emits `session.error` and goes `idle` without retrying. Update the session tools while idle, update the agent so new sessions get the fix, then send a `user.message`.

In a multiagent session every list on the path to a thread applies at once: the roster agent's lists, the lists of every agent that called it, and the coordinator's current lists. Allow-lists intersect. Block-lists union. A roster agent can narrow and cannot widen. Disjoint allow-lists leave the tool present and fail every call with `url_not_allowed`. Keep each roster allow-list inside the coordinator's. `max_content_tokens` and `user_location` do not combine: the thread's own value, then the caller's, then the coordinator's. A `self` entry follows the coordinator. The outcome grader does not have the web tools. Updating an idle session's tools changes the coordinator's lists for every thread from its next turn. A roster agent's own lists stay as they were at session create.

### Custom tools

```json
{
  "type": "custom",
  "name": "get_weather",
  "description": "Fetch the current weather for a city.",
  "input_schema": {
    "type": "object",
    "properties": {
      "city": { "type": "string", "description": "City name" }
    },
    "required": ["city"]
  }
}
```

The name cannot start with `mcp__` or `oa__`, and cannot match a built-in tool name (`400`). `oa__` is reserved for OpenAgents. An agent that already stores an `oa__` tool accepts only an update whose `tools` omit it. A new session that would run with such a tool is `400`.

### MCP servers

Declare servers on the agent, without credentials. Put the credential in a vault, matched by URL, and pass `vault_ids` on the session.

| Field | Required | Meaning |
| --- | --- | --- |
| `type` | Yes | `url`. |
| `name` | Yes | Unique. Referenced by `mcp_toolset.mcp_server_name`. |
| `url` | Yes | Streamable HTTP endpoint. |

```json
{
  "mcp_servers": [{ "type": "url", "name": "linear", "url": "https://mcp.example.com/mcp" }],
  "tools": [{ "type": "mcp_toolset", "mcp_server_name": "linear" }]
}
```

An `mcp_toolset` can set `default_config.enabled` to `false` and list `configs` of `{ "name", "enabled", "permission_policy" }`. `name` is the tool name the server reports. No `type` field and no web settings.

While the session is `idle`, `POST /v1/sessions/{session_id}` can replace `agent.tools` and `agent.mcp_servers` for this session. The stored agent does not change.

## Outcomes

```json
{
  "type": "user.define_outcome",
  "description": "Build a discounted-cash-flow workbook for Costco.",
  "rubric": { "type": "text", "content": "The workbook has a sources sheet and a summary that ties out." },
  "max_iterations": 5
}
```

`rubric` is required: `{ "type": "text", "content" }` or `{ "type": "file", "file_id" }`. `max_iterations` defaults to 3 and is at most 20. The event is echoed with `outcome_id` (`outc_`) and `processed_at`. The grader does not have `web_search` or `web_fetch`. With an outcome defined, no evaluation starts while a workflow run is `running`. The agent's turn on the run's ending notice can start one. Grader events, results, and when to send the next outcome are in [events](managed-agents-events.md#outcomes).

## Multiagent

A coordinator delegates to other agents inside one session. They share the container and the filesystem. Each runs in its own thread: its own history, model, system prompt, tools, MCP servers, and skills. Threads stay. The coordinator can send a follow-up to a thread it already opened.

Use this when the work splits into independent pieces, or when one piece would fill the context with reading. Each piece gets a fresh context. Threads run in parallel. Only the report comes back. You still create one session and read one stream. The coordinator is given `list_agents` and `send_to_agent`. You do not write a second orchestrator.

Do not staff a roster to re-answer one question (a checker, a voter, a second model pass). Give the agent the documents and tools, tell it in `system` to check its own work, and raise `effort`. A check in your own code that calls no model is fine. An advisor the agent consults mid-turn is a separate choice. If you add a checker because you were asked for one, it spends tokens on every answer.

A small one-step task is a poor fit: every delegation costs a round trip and a re-brief. Subagents do not see the coordinator's conversation. Each task has to carry the paths, the constraints, and the report format. Spawning returns immediately. The report arrives on a later coordinator turn.

**Step 1.** The smallest roster is the agent itself.

```json
{
  "name": "Research assistant",
  "model": "openagents",
  "system": "When a request splits into independent sub-questions, delegate each to a copy of yourself, one task per copy, then check and combine the reports.",
  "tools": [
    {
      "type": "agent_toolset_20260401",
      "default_config": { "permission_policy": { "type": "auto" } },
      "configs": [
        { "name": "web_fetch", "enabled": true },
        { "name": "web_search", "enabled": true }
      ]
    }
  ],
  "multiagent": { "type": "coordinator", "agents": [{ "type": "self" }] }
}
```

**Step 2.** Put reading on a cheaper model. Create an agent on `openagents/fast` with a narrow `system` and only the tools it needs, and list its id next to `self`. A roster entry is a reference. The worker runs on its own model, prompt, and tools, and its tokens are priced at that model. The coordinator spends its tokens on planning, checking, and the final answer.

**Step 3.** Add specialists the same way: each with its own model, a narrow prompt, and the tools that job needs. One rostered agent can be spawned many times. Threads share files, not conversation. Put input and output paths in every task.

`multiagent` is a field on the agent, not a `tools` entry, and not a field on session create.

| Roster entry | Shape |
| --- | --- |
| String | `"agent_abc123"`. Latest version. |
| Agent | `{ "type": "agent", "id", "version"? }`. Omit `version` to pin the latest at save time. |
| Self | `{ "type": "self" }`. At most one. Copies cannot delegate further. |
| Advisor | `{ "type": "advisor", "model" }`. At most one. See [Advisor](#advisor). |

One to 20 entries. One level only: an agent that itself has `multiagent` set (either type) fails the create or update. Names are unique. The `self` entry is listed under the coordinator's own name. Write `name` and `description` for the coordinator to read.

Say in the coordinator's `system` what to hand off, to whom, how many at once, what to keep, and what is too small to delegate.

Web domain lists layer as [Web search and web fetch](#web-search-and-web-fetch) says.

### Dynamic workflows

When the job is too big to hand out one task at a time (more pieces than 25 child threads), set:

```json
{ "type": "multiagent_20261001", "workflows": { "type": "enabled" } }
```

The agent writes a plan. OpenAgents runs it in the background as a workflow run. The agent can keep working or end its turn. A run can use agents you list and agents the plan defines. It does not use the roster.

On this type, `workflows` and `subagents` default to enabled, and `advisor` defaults to disabled. Write the switch you mean. `workflows.predefined_agents` and `subagents.predefined_agents` take up to 20 entries in the roster forms except advisor. `inline_agents` defaults to `{ "type": "enabled" }`. Set it to `{ "type": "disabled" }` to stop the plan, or the coordinator, from defining agents; the list then needs at least one agent, or the request is `400`. An advisor on this type is the `advisor` member, `{ "type": "enabled", "model" }`, never a roster entry.

For a roster and no workflows, use `coordinator`, or set `workflows` to `{ "type": "disabled" }`. `null` restores the default, and the default for `workflows` is enabled. Say in `system` when a run should start, and name the small tasks the agent should do itself. Every agent in a run spends tokens, so set a session budget. You cannot add a budget after the session exists.

A listed agent runs with its own configuration. An agent the plan defines gets its system prompt from the plan, runs on the session agent's model, and receives the session agent's tools, MCP servers, and skills. Those tools keep their permission policies.

An update that sends `multiagent_20261001` to an agent that already has it merges level by level. Every object needs its `type`. A key you omit keeps its stored value. `null` resets that key and everything inside it to the default. A `predefined_agents` list replaces the stored list. A member whose `type` differs from the stored one is replaced whole. An update that omits `multiagent` leaves it. Sent to an agent that has `coordinator`, the update replaces the whole block: send the roster as `subagents.predefined_agents`.

Existing sessions copy the setting at create. Later agent edits do not change them. An agent with `multiagent` set cannot be rostered or listed in another agent's `workflows.predefined_agents`. A custom tool named `oa__...` must be removed in the update that turns workflows on. A new session whose agent, roster, or `workflows.predefined_agents` still has such a tool is `400`.

Runs pause when the session hits its budget, and resume when you raise or remove the budget.

You follow a run by its events on the primary stream and in the session event list. You do not see the plan, and these events have no webhook.

| Event | What it carries |
| --- | --- |
| `workflow_run.created` | `workflow_run_id` (`wrun_`), `name`, `description` (`null` if the plan gave none), `phases` (each `id`, `name`, `description`). A status event follows. |
| `workflow_run.status_running` | The run is executing. Also sent on each resume after a budget pause. |
| `workflow_run.status_idle` | The run is paused (the state name is `idle`). The event does not say why. |
| `workflow_run.status_ended` | `result.type` is `completed`, `stopped`, or `error`. Always the last status event. |
| `workflow_run.error` | `workflow_run_id` (`null` if no run was created) and `error`. A start refused because 10 runs are already open (the default) has `error.type` `max_workflow_runs_error` and a null id. A run that ends in error sends this event, then `status_ended`. |
| `workflow_run.phase_started` | `workflow_run_phase_id`. |
| `workflow_run.phase_ended` | `workflow_run_phase_id` and `phase_started_id`. |

`result.type` `completed` means the plan finished, not that the work passed. `stopped` means the agent stopped the run or the session was archived. `error` carries `result.error.type` and `message`. Known error types: `timeout_error` (the run's lifetime elapsed), `program_error` (the plan failed, or it let one thread's failure end the run), `thread_limit_error`, `unknown_error`. Tolerate an unknown `result.type` and an unknown `result.error.type`. A dependency failure (model, MCP, credentials) is `session.error` on that thread and does not by itself end the run. If the plan lets that failure end the run, the result is `program_error`.

Match a phase's end to its start by `phase_started_id`, not by position. Every phase that starts also ends, and a phase still open when the run ends gets `phase_ended` before `status_ended`. Tell phases apart by `id`. Names are not unique. After `status_ended`, the agent takes a turn on an ending notice that is not itself an event. Do not wait for an `agent.thread_message_received` from the run as the sign that the notice arrived.

A run's threads are ordinary `session_thread` rows with that `workflow_run_id`. You can list and stream them. You cannot send input to one, interrupt it by id, or archive it while the run is open (`400` `workflow_run_open`). Ask the agent to stop the run. The server archives each thread no later than the end of the run. A call still waiting on you when the thread is archived is void: a later tool result can be `400`. A late tool confirmation returns `200` and does not mean the tool ran. An inline agent's thread has `agent.type` `inline` and the plan's `name`, `description`, and `system`, and no `id`, `version`, or `multiagent`. No event posts the thread's result to the plan. `session.thread_status_terminated` on the primary stream says the thread is done. Lifecycle events are copied to the primary thread. Tell them from the primary's own events by `session_thread_id`.

There is no list-runs endpoint. After a reconnect, list events with `types[]` set to `workflow_run.created` and `workflow_run.status_ended`.

While a run is `running`, the session stays `running`. The idle you can see then is `requires_action`, when no thread is working and one is waiting on you. A paused run does not keep the session `running`. The work is done at the first `session.status_idle` with `stop_reason.type` `end_turn` and no run open, after the agent has taken its turn on each ending notice. `budget_reached`, `retries_exhausted`, and an idle caused by your interrupt do not mean the notice was read. A `user.interrupt` with no thread id, or naming the primary, stops the agent's turn and does not end a run. Naming a run's thread is `400`. To stop the runs, send a `user.message` asking the agent to stop them (`result.type` `stopped`). To continue, send a `user.message` asking it to continue. If the session is `idle` with `requires_action`, answer the waiting calls before any `user.message` (`400` otherwise).

While a run is open, archive, delete, and `sessions.update` of `agent` can be `400` `workflow_run_open`, or can be accepted and end the run. Do not send them while you still want the run's work. An accepted archive ends open runs with `result.type` `stopped`, and the end events are on the event list only. An accepted delete emits no `workflow_run` end event.

Tool calls from a run's threads appear on the primary stream as they happen. The session can stay `running` with no `requires_action` idle. Act on the tool-use event itself.

Limits, server-set: 64 threads working at once in one run (not a promise); 1,000 threads over a run's life (`thread_limit_error` past the server's limit; the count can still pass 1,000); a default lifetime of 24 hours, which a pause might not freeze (`timeout_error` past it; only the agent can set a shorter lifetime); 10 runs open at once per session, idle runs included. Run and phase names are cut to 64 characters and descriptions to 256.

### Thread events

On the stream of the thread the event belongs to:

| Event | Meaning |
| --- | --- |
| `session.thread_created` | `session_thread_id`, `agent_name`, `workflow_run_id` (`null` except for a run's thread). |
| `session.thread_status_running` | The thread started work. |
| `session.thread_status_idle` | Waiting, or paused at the shared budget (`stop_reason` `budget_reached`). Same `stop_reason` shape as the session. |
| `session.thread_status_rescheduled` | Retrying after a retryable error. |
| `session.thread_status_terminated` | Archived, an unrecoverable error, or an advisor consultation that ended. |
| `agent.thread_message_sent` | This thread sent `content` to `to_session_thread_id` (`to_agent_name`). |
| `agent.thread_message_received` | `content` arrived from `from_session_thread_id` (`from_agent_name`). |

Direction is relative to the thread whose stream you are reading. The same task is `agent.thread_message_sent` on the primary stream and `agent.thread_message_received` on the child's stream.

### Advisor

`{ "type": "advisor", "model": "openagents/reason" }` lets the primary thread consult a model mid-turn. The entry has `type` and `model` only. It can be the only roster entry. On `multiagent_20261001` it is the `advisor` member, not a roster entry.

Rules:

- At most one. The reserved name is `openagents.advisor`. A roster that also lists an agent with that name is `400`. In responses the advisor is echoed last.
- The advisor model must be at least as capable as the agent's model. From most capable to least, for this check: `openagents/reason`, `openagents/code`, `openagents`, `openagents/auto`, `openagents/chat`, `openagents/long`, `openagents/fast`, `openagents/classify`. Equals may pair. An invalid pair is `400` `advisor_pairing`. An underlying model id pairs only with an advisor of equal or higher rank on that same list, or is `400` if it is not on the list.
- Only the primary thread consults it. It is not in `list_agents` and not reachable with `send_to_agent`. Roster agents cannot consult it.

A consultation is a thread named `openagents.advisor` that ends itself. Typical order: `session.thread_created`, `session.thread_status_running`, `agent.thread_message_received` (the advice), `session.thread_status_idle` (`end_turn`), `session.thread_status_terminated`. No `agent.tool_use` and no `agent.thread_message_sent` are emitted. The advice can arrive after the idle and terminated events.

Public model ids deliver text content. A redacted advisor delivers `[{ "type": "redacted" }]` to your client; the agent still reads the advice. Advisor thinking is never on the stream. You cannot send a `redacted` block (`400`). A failed consultation, or a `user.interrupt` that names the advisor thread, does not fail the agent's turn. A session-level interrupt halts every thread and delivers no advice.

Advisor threads do not count toward the 25-child limit. They are billed at the advisor model's price and appear in that thread's usage and the session total. To remove the advisor, update the roster without the entry. If it was the only entry, send `"multiagent": null`. On `multiagent_20261001`, send `advisor: { "type": "disabled" }`. Omitting `advisor` on an update keeps it.

### Interrupting and archiving threads

`user.interrupt` with no `session_thread_id` interrupts every non-archived thread, including the primary. Against a child thread blocked on `requires_action`, the interrupt closes each pending tool call with an error result (`Tool execution was interrupted before completion. Please retry.`) and emits `session.thread_status_idle` with `end_turn` without sampling the model. Against a thread already idle with `end_turn` or `budget_reached`, the interrupt does nothing. Naming a terminated thread is `400`. On a self-hosted session whose worker failed the claim, `user.interrupt` queues the work again.

Archive requires the thread to be idle, and `requires_action` counts as idle. A `running` thread must be interrupted first. A workflow run's thread cannot be archived while its run is open.

### Pitfalls

- `multiagent` is on the agent, not on session create and not inside `tools`.
- Threads share the filesystem and do not share history or tools. The delegated message has to say what to do, or the coordinator writes it to disk.
- A second level of delegation is a validation error.
- On `multiagent_20261001`, omitting `workflows` or sending `null` turns workflows on. For a roster only, use `coordinator` or `"workflows": { "type": "disabled" }`.
- A workflow run spends tokens for every agent in it. Set a budget, and say in `system` when a run should start.

## Errors

The body is the [shared error object](design.md). `type` is the class. `code` is the reason. `param` names the field when there is one.

| Status | `type` | `code` examples |
| --- | --- | --- |
| `400` | `invalid_request` | `missing_param`, `invalid_json`, `model_speed_unsupported`, `inference_geo_mismatch`, `advisor_pairing`, `workflow_run_open` |
| `401` | `authentication` | `invalid_key`, `key_revoked` |
| `402` | `payment_required` | `balance_insufficient` |
| `403` | `permission` | `scope_missing`, `computer_not_granted` |
| `403` | `limit_reached` | `limit_reached`, with `param` naming the key limit |
| `404` | `not_found` | `agent_not_found`, `session_not_found`, `environment_not_found`, `vault_not_found` |
| `409` | `conflict` | `version_conflict`, `name_taken`, `memory_path_conflict`, `memory_precondition_failed`, `idempotency_conflict` |
| `410` | `invalid_request` | `cursor_expired` |
| `413` | `invalid_request` | `request_too_large` |
| `429` | `rate_limited` | `rate_limited`, `session_rate_limited` |
| `500` | `server_error` | `internal_error` |
| `503` | `unavailable` | `upstream_unavailable`, `computer_offline` |

`memory_path_conflict` includes `conflicting_memory_id`. A `409` from optimistic concurrency is `version_conflict`, not a generic invalid request.

## What is not built

These routes are not mounted. The inference gateway, account sessions, the website's repository-environment setup, the skill directory, and the sats balance exist and are what this API is meant to sit on. Cloud computers and granted computers exist in part ([glossary](../glossary.md)); this API is the public shape for running a saved agent on them. Nostr stays inside OpenAgents. It does not appear on these routes.

## A short call

```sh
export OA=https://api.openagents.com/v1
export KEY=oak_…
```

Create an environment, an agent, and a session, then read the stream:

```sh
curl -s $OA/environments \
  -H "Authorization: Bearer $KEY" \
  -H "Idempotency-Key: 1b3e7c4a-8f2d-4c5b-9a10-6d0e1f2a3b4c" \
  -H "Content-Type: application/json" \
  -d '{"name":"dev","config":{"type":"cloud","networking":{"type":"limited","allowed_hosts":["api.example.com"],"allow_mcp_servers":true}}}'

curl -s $OA/agents \
  -H "Authorization: Bearer $KEY" \
  -H "Idempotency-Key: 2c4f8d5b-901e-4d6c-ab21-7e1f203b4c5d" \
  -H "Content-Type: application/json" \
  -d '{"name":"Research assistant","model":"openagents","system":"Answer the question. Check your own work.","tools":[{"type":"agent_toolset_20260401","default_config":{"permission_policy":{"type":"auto"}},"configs":[{"name":"web_fetch","enabled":false},{"name":"web_search","enabled":false}]}]}'

curl -s $OA/sessions \
  -H "Authorization: Bearer $KEY" \
  -H "Idempotency-Key: 3d509e6c-a12f-4e7d-bc32-8f20314c5d6e" \
  -H "Content-Type: application/json" \
  -d '{"agent":"agent_…","environment_id":"env_…","budget":{"type":"limit","max_list_cost":{"amount_sats":25000}},"initial_events":[{"type":"user.message","content":[{"type":"text","text":"Summarize the notes in /workspace."}]}]}'

curl -N $OA/sessions/sesn_…/events/stream \
  -H "Authorization: Bearer $KEY" \
  -H "Accept: text/event-stream"
```
