# Audit: Coder inference, agent model defaults, and a Rust coder API

Date: 2026-08-27
Status: audit. Not a plan, not dispatch, not a catalog change.
Sources: `OpenAgentsInc/openagents.com` (Phoenix) at the local checkout
`~/work/openagents.com`; this repository's CLI and Coder Mini on
`openagents/main`. Related: issues #245–#249 (Coder Mini landed),
`docs/sol/2026-08-27-task-tool-coder-mini-plan.md`,
`docs/analysis/2026-08-27-claude-sdk-rust-parity-and-coder-harness-imports.md`.

Owner direction recorded here:

1. Built-in delegated agents get **default models**. Explore is
   `gemini-3.7-flash`. Plan stays on the current default,
   `glm-5.3-flash`.
2. Backend inference moves off the Phoenix Elixir monolith onto a
   **separate Rust coder API**: lighter, easy to run locally, **shared
   types** with the CLI.
3. Connecting that API into Phoenix is a later question. This document
   names the seam; it does not pick a date or a reverse-proxy design.

This document is the inventory and the seam. It does not implement the
agent defaults or the new crate.

---

## 1. What Coder already does with models

A Coder session does not hold a provider key. It opens a **thread**
against the account bearer, receives a short-lived **inference grant**,
and every model call goes back through `POST /api/inference/proxy` with
that grant as the bearer. The proxy fans into a vendor. Credentials stay
on the server (`RELEASE-002`). The CLI renders what answered
(`PROVIDER-002`).

The live hop, as `crates/openagents-cli/src/runtime.rs` implements it:

```text
CLI  -- account bearer -->  POST /api/v1/threads
                              {objective, lane:"thread", model?}
                         <--  {thread, grant:{token, url, model}}

CLI  -- grant bearer   -->  POST /api/inference/proxy
                              OpenAI-shaped /chat/completions SSE
                         <--  chunks; x-openagents-model; usage

CLI  -- account bearer -->  POST /api/v1/threads/{id}/events
                            POST /api/v1/threads/{id}/report
                            GET  /api/v1/models
                            GET  /api/v1/credit
```

`--dev` points the same client at `http://localhost:{port}/api/v1` and
today that port is Phoenix. A local Ollama lane is the exception: it
opens a transcript-only thread (`lane: "local"`), mints no grant, and
never touches the proxy.

Delegated Mini already opens a **second thread**.
`HarnessToolRegistry::delegate_to_builtin_agent` builds a fresh
`CoderRuntimeSession` and `execute_turn` calls `create_thread`. A child
can therefore pin a different catalog id than the parent. The Phoenix
thread controller documents this on purpose: a session that wants a
child on Gemini opens a second thread on `gemini-3.7-flash` and gets
authority for that model, with its own budget.

That is the mechanism Explore and Plan will use. No new protocol is
required for the default-model change. The child session already names
`model` at open.

---

## 2. The Phoenix catalog, as of this writing

`config :openagents, :model_catalog` in
`openagents.com/config/config.exs` is the one list. Thread admission,
grant mint, the proxy, and `GET /api/v1/models` all read it
(`OpenAgents.Inference.Models`). Three public ids:

| Public id | Provider lane | Vendor string | Context / max output | Pricing |
|---|---|---|---|---|
| `glm-5.3-flash` | `:vercel_gateway` | `zai/glm-5.3-flash` | 1_000_000 / 131_000 | **declared**. List $0.15 / $0.50 / $0.03 per million in/out/cached. Promotion `declared.glm-5.3-flash.free-through-2026-08-31.v1` zeros those rates until `2026-09-01T00:00:00Z`. |
| `gemini-3.7-flash` | `:vercel_gateway` | `google/gemini-3.7-flash` | 1_048_576 / 65_536 | **placeholder**. $1.25 / $10.00 / $0.10 per million. Nothing may bill from this (`METER-001`). |
| `openrouter/free` | `:openrouter` | `openrouter/free` | 32_768 / 8_192 | **declared** zeros. Coder Free. |

Catalog order puts GLM first, so it is what a caller that names none
gets (`Models.default_id/0`). Gemini is second. It led the list until
GLM was added.

Both paid ids ride the **Vercel AI Gateway**
(`https://ai-gateway.vercel.sh/v1/chat/completions`), OpenAI chat
completions on the wire. Gemini is pinned toward Vertex
(`vercel_gateway_providers: ["vertex"]`) so the call spends Google
credits this account already holds; the adapter's own comment is that
BYOK Vertex answers `"cost":"0"` from the gateway. GLM is resolved to
z.ai with BYOK z.ai credentials on the same gateway.

Availability is `configured?/0` on the adapter: a missing
`VERCEL_GATEWAY_API_KEY` (runtime `vercel_gateway_api_key`) lists both
GLM and Gemini as `unavailable` rather than omitting them. Selecting an
unavailable id is refused with `model_unavailable`. Gym evidence that
the Gemini id actually answers: the 2026-08-24 `fix-git` trial
`gemini-3.7-flash` on the proxy **passed** (17 steps, ~125k in / 640
out). That is a live-lane receipt, not a catalog comment.

### 2.1 Gateway fallback vs a pinned Explore model

This is the load-bearing defect for “Explore is Gemini.”

`vercel_gateway_fallback_models` is:

```text
zai/glm-5.3-flash, zai/glm-5.3, zai/glm-5.2, openai/gpt-5.6-luna
```

The Vercel adapter reports `substitutable?/0 == true` while that list is
non-empty. A call for `google/gemini-3.7-flash` can be answered by
`zai/glm-5.3-flash` and still return 200. `PROVIDER-002` was amended
(#250) so the proxy **attributes** the serving model (`x-openagents-model`
and each chunk's `model`) rather than pretending the request was
honored. Attribution is honest. Pinning is not.

Explore defaulting to Gemini is a pin. A 200 from GLM on that grant is
a miss, even if the header says GLM. A later coder API, and any Phoenix
change that precedes it, has to refuse or disable fallback **when the
grant named a model**, or Explore is GLM with extra steps.

Luna is not in the selectable catalog. It remains last in the gateway
chain as an automatic backstop. A rescued call that lands off-catalog
is attributed `unpriced`, not charged at the requested model's rates.

### 2.2 GLM promotion clock

On 2026-08-27 the GLM lane is still in the free promotion window. At
`2026-09-01T00:00:00Z` `OpenAgents.Inference.Pricing` returns to the
declared list table without another deploy. Plan-on-GLM after that date
is a paid lane at list rates, metered against account credit. That is
expected; it is not a reason to move Plan onto Gemini.

### 2.3 Credit

Account credit is the bound (`OpenAgents.Inference.Credit`). A signed-in
user holds `users.credit_allowance_microusd` (new accounts default
$20). A visitor holds a smaller figure. Spend is the sum of
`inference_grants.usage`, not a second counter. The CLI only **reads**
`GET /api/v1/credit`; it never subtracts locally.

A Gemini call priced as `placeholder` records **no billable cost**.
Explore-on-Gemini therefore does not draw the account down until an
operator replaces those rates with `source: :declared`. Token and call
ceilings on a thread are currently unbounded
(`thread_grant_max_*` are `nil`); credit is the bound, so an unpriced
Explore lane is bounded only by revocation and by whatever call/token
ceilings a future API restores.

---

## 3. CLI lanes vs catalog ids

The CLI has two switchable **lanes**, resolved against the live catalog
at thread open (`LANES` in `runtime.rs`):

| `--lane` | Label | Preference order |
|---|---|---|
| `flash` (default) | Coder Flash | `glm-5.3-flash`, `zai/glm-5.3-flash`, `gemini-3.7-flash` |
| `free` | Coder Free | `thinkingmachines/inkling`, `openrouter/free` |

A typed catalog id (`--model gemini-3.7-flash`, or `Lane::Named`) is
not a lane alias. It pins that id. `--lane gemini` used to be an alias
and was removed so a reader who types a model id cannot receive a
different model when the alias moves.

Coder Mini today does **not** use that pin. `delegate_to_builtin_agent`
sets:

- `coder-mini` with `model` in the tool call → `Lane::from_str(model)`
- everything else, including `explore` and `coder` → `Lane::from_str(&gate.lane)`

`explore` and `coder-mini` share the same read-only pool and the same
report-back prompt (`crates/openagents-cli/src/coder/agents.rs`). There
is no `plan` built-in. There is no default-model field on
`AgentDefinition`.

The owner defaults therefore are a small, local change on that struct
and on the Mini open path:

| Agent | Pool | Default catalog id | Notes |
|---|---|---|---|
| `explore` | read-only | `gemini-3.7-flash` | Pin, do not inherit the parent lane. Distinct prompt (investigator, no edits, no project-memory load). Cheap/fast is the point. |
| `plan` (new) | read-only | `glm-5.3-flash` | Distinct planning prompt. Report is a plan the parent can accept or hand to `coder`. |
| `coder-mini` | read-only, overridable | inherit parent / session flash (GLM) unless `model` is passed | General-purpose helper. |
| `coder` | read-write | inherit parent | Same as today, write-capable. |

`model` on the tool call already overrides Mini. Keep that. The default
is what runs when the parent names no model.

Opening the child thread already sends `body["model"]`. Explore's
session should send `gemini-3.7-flash` there. If the catalog lists that
id `unavailable`, the child should fail by name (`model_unavailable`),
not fall through to GLM.

---

## 4. What Phoenix owns that Coder actually uses

Approximate Elixir size of the inference slice (implementation, not
tests):

| Module | Lines | Role |
|---|---|---|
| `OpenAgents.Threads` | ~1,350 | Open, cap, generation fence, events, report, cancel, local-lane exception |
| `InferenceProxyController` | ~630 | Grant auth, model pin, provider fan-in, SSE, usage, attribution |
| `OpenAgents.Inference` | ~580 | Mint / resolve / revoke / record_usage |
| `Inference.Models` | ~260 | Catalog, availability, `GET /api/v1/models` projection |
| `Inference.Pricing` | ~260 | Declared vs placeholder vs promotion |
| `Inference.Credit` | ~245 | Account allowance minus grant usage |
| `Providers.OpenRouter` | ~240 | Chat-completions POST + stream decode (also used by the gateway) |
| `Inference.Grant` | ~140 | Schema |
| `Providers.VercelGateway` | ~130 | Endpoint, Vertex pin, fallback list, `substitutable?` |
| `Inference.Health` | ~115 | Degraded-lane status |
| Thread / catalog / credit controllers | small | HTTP doors |

Plus `ThreadController` (the HTTP envelope around `Threads`), grant
migrations, and a dense test set under
`test/openagents/inference/` and the matching controller tests.

The **invariants** this slice currently carries, which a Rust API has to
keep if Coder is to keep working:

- **PROVIDER-002** — no silent substitution. Catalog is the served set.
  Unavailable is listed, not hidden. Grant pins the model. Body mismatch
  is `model_mismatch`. Successful responses name what answered.
  Provider-side fallback is the one exception, and it must disclose.
- **METER-001** — only `:declared` rates are billable. Placeholder and
  off-catalog rescue are `unpriced`, never `$0.00`.
- **THREAD-001** — a grant names exactly one fence (`thread_id` xor
  `conversation_id`). A thread has at most one live grant. Mint revokes
  the previous generation in the same transaction.
- **RELEASE-002** — provider credentials never leave the server. The
  client holds a grant token.
- Local lane mints nothing. Resume is `POST /threads/{id}/grants`,
  because the plaintext token exists only at mint.
- Finish vs cancel are different acts (`POST /report` vs `DELETE`).
- Credit is the account's, read from the server.

Auth for these routes is the account API token / session bearer
(`api_route_authority.ex`: models, threads, credit). The proxy is
classified `internal_service` and authenticates the **grant**, not the
account.

Identity, GitHub OAuth, API-token issuance, forum, forge, issues, and
the rest of Phoenix are **not** in this slice. A coder API that tried to
absorb them would recreate the monolith.

---

## 5. Why a separate Rust coder API

The CLI is already Rust. The types it uses to talk to inference —
`ServedModel`, `InferenceGrant`, `TurnUsage`, `ThreadRecord`, `Credit`,
`Lane` — live in `crates/openagents-cli` and are hand-decoded from JSON
Phoenix emits. Every catalog field the server adds is a second edit.
Every envelope rename is a silent CLI break.

A dedicated coder API in this workspace, next to the CLI, buys:

- **One compile.** Shared types are a crate both binaries depend on.
  A grant that does not deserialize does not ship.
- **A local server the CLI can start.** `--dev` today looks for Phoenix
  (`start_server.sh` in `OPENAGENTS_WEB_REPO`). Phoenix is the whole
  product: LiveView, forge, forum, Postgres. A coder API that is “catalog
  + threads + proxy + credit” can boot with SQLite (or in-memory) and
  the two gateway env vars.
- **An inference process that is not the web app.** Restarts, deploys,
  and schema work on forum/forge stop being inference incidents.
- **The same process locally and in the cloud.** The catalog, the
  fallback policy, and the Explore pin are one binary, not “Phoenix
  config.exs plus a CLI table that tries to track it.”

This is not a rewrite of OpenAgents.com. It is extracting the Coder
inference door.

---

## 6. Proposed crate split (this repo)

Three crates. Names are indicative.

### 6.1 `crates/openagents-coder-contract`

Pure types and JSON envelopes. No HTTP server, no vendor clients, no
CLI. Both the CLI and the API depend on it.

Minimum surface, matching what the CLI already parses:

- **Model catalog.** `{ models: [ {id, provider, context_window,
  max_output, availability, pricing_basis, default, pricing?,
  pricing_promotion?} ], default }` — today's `GET /api/v1/models`.
- **Thread open.** Request: `{ objective, lane: thread|local, model?,
  reasoning?, repository?, visibility? }`. Response:
  `{ thread: {id, …}, grant?: { token, url, model } }`.
- **Grant.** `{ token, url, model }` plus the usage/ceiling fields the
  thread show page already publishes.
- **Proxy request.** The OpenAI `/chat/completions` subset the CLI
  actually sends (messages, tools, stream, `tool_choice`). Not a
  complete OpenAI clone.
- **Proxy stream events.** The SSE the CLI already consumes, including
  `model`, `usage`, and `x-openagents-model`.
- **Credit.** `{ allowance_microusd, spent_microusd, remaining_microusd,
  unpriced_calls, complete }`.
- **Typed errors.** `model_unavailable`, `model_not_served`,
  `model_mismatch`, `thread_quota_reached`, `thread_lane_local`, grant
  expired/revoked. Stringly errors are how the CLI currently recovers;
  the contract should make the class a field.
- **Built-in agents.** `{ id, pool, default_model, system_prompt }` for
  `explore` / `plan` / `coder-mini` / `coder`. This is how the Explore
  Gemini pin stays one value in CLI and API.

The CLI's current structs (`ServedModel`, `InferenceGrant`, `Credit`,
`ThreadRecord`, `TurnUsage`) move here. Phoenix, later, serializes the
same JSON.

### 6.2 `crates/openagents-coder-api`

The process. Axum (or equivalent already in-tree) over the contract.

Responsibilities:

1. Catalog from config / env. Same three ids to start. Availability from
   adapter `configured?`.
2. Account-scoped threads and generation-fenced grants. SQLite locally;
   Postgres when someone points it at one. The fence is the behavior,
   not the database brand.
3. Inference proxy: grant auth, pin, fan-in to Vercel Gateway and
   OpenRouter, SSE out, usage in.
4. Credit: sum of grant usage against an allowance. Locally the
   allowance can be a config figure so a signed-out `--dev` still runs.
5. Health: mark a lane degraded after consecutive provider failures so
   `GET /models` can say `degraded`.

Out of process for v1: GitHub OAuth, API-token minting, forum, forge,
issues, LiveView, voice, box. A local run accepts a
pre-issued bearer (the existing CLI credential, or a dev token in env).

`--dev` change in the CLI: start this binary if nothing is on the port,
instead of (or before) Phoenix. Keep talking to production
`openagents.com` when `--dev` is off.

### 6.3 CLI remains `crates/openagents-cli`

Depends on the contract crate. Keeps TUI, tools, Mini, ACP, gym. Stops
owning the JSON shape of grants.

Built-in agent defaults live in the contract so a Mini child and a
future server-side dispatcher cannot disagree about Explore's model.

---

## 7. Phoenix connection (later, not this extract)

Do not block the Rust API on a Phoenix cutover. Three later options,
in increasing coupling:

1. **CLI dual-origin.** Production Coder still hits Phoenix
   `/api/v1/threads` and `/api/inference/proxy`. `--dev` hits the Rust
   API. Phoenix unchanged. This is enough to implement and dogfood the
   API.
2. **Phoenix reverse-proxy.** Phoenix keeps account auth and the public
   `/api/v1` host. Thread and proxy routes are forwarded to the Rust
   process. The browser thread UI keeps working. Grant rows either stay
   in Phoenix Postgres (Rust talks to the same DB) or Phoenix writes a
   stub and the Rust API owns the ledger — that choice is the real
   design review, and it is not free.
3. **Phoenix stops serving inference.** Catalog, threads, proxy, credit
   live only on the coder API. Phoenix becomes identity + product web,
   and the CLI's production origin for those routes moves.

Option 1 is the extract. Option 2 is the product-web compatibility
layer. Option 3 is a cutover with a published origin change. None of
them are implied by standing up the crate.

Identity is the awkward shared object. A grant has to name an account
(`owner_visitor_id` today). Locally, one dev principal is enough.
In production, either the CLI still authenticates to Phoenix and
presents that token to the coder API (Phoenix-issued, coder-API
validated via JWKS or introspection), or the coder API learns to
validate the same bearer Phoenix already issues. That adapter is the
whole of “connect it into the monolith.” It can wait until the API
answers `--dev` sessions.

---

## 8. What has to be true for Explore = Gemini, Plan = GLM

Independent of where the proxy runs:

1. **`gemini-3.7-flash` stays in the catalog and stays `available`.**
   Vertex BYOK on the Vercel gateway is the current path. Gym has a
   passing proxy trial. If the gateway credential is missing, Explore
   must fail `model_unavailable`, not inherit Flash.
2. **A grant pinned to Gemini cannot be answered by GLM.** Disable
   gateway fallback for pinned grants, or fail the proxy call when the
   serving model is not the grant's model. Attribution-only is not
   enough for this default.
3. **`AgentDefinition` grows `default_model`.** `explore` →
   `gemini-3.7-flash`. New `plan` → `glm-5.3-flash`. Mini/coder inherit
   the parent unless `model` is passed. Distinct prompts; Explore does
   not load project skills/memory.
4. **Gemini pricing remains placeholder until an operator declares
   rates.** Explore will not draw account credit until then. That is
   acceptable for a Vertex-BYOK lane whose gateway cost is zero; it is
   not acceptable to display as `$0.00`. Keep `unpriced`.
5. **Plan on GLM is already the catalog default.** After 2026-09-01 it
   is list-priced and metered. No extra work unless Plan should ride
   the remaining free window harder before that instant.

Items 2 and 3 are the implementation. Item 2 belongs in whichever
process runs the proxy (Phoenix today, Rust API once it exists). Item 3
belongs in the CLI now, and in the contract crate once that exists so
the default cannot drift.

---

## 9. Suggested order

This is sequencing for later work, not admission.

1. **Agent defaults in the CLI** (`agents.rs` + Mini open path). Explore
   pins Gemini, Plan exists on GLM, prompts actually differ. Works
   against current Phoenix. Independent of the Rust API.
2. **Pin integrity on the proxy** (Phoenix, small): a grant that named
   a model is not eligible for gateway fallback. Unblocks Explore
   meaning Gemini while production still is Phoenix.
3. **`openagents-coder-contract`.** Move the JSON types. CLI compiles
   against them, still talking to Phoenix. This is the shared-types
   deliverable with no new process.
4. **`openagents-coder-api` v1**, SQLite, Vercel + OpenRouter adapters,
   catalog + threads + proxy + credit. CLI `--dev` can target it.
   Production origin unchanged.
5. **Phoenix connection**, when the API has answered real `--dev`
   sessions: bearer validation and, if wanted, reverse-proxy of the
   thread/proxy routes.

Do not wait for 4–5 to do 1–2. Do not wait for #232 (Claude SDK
parity) or swarm issues; those are a different transport. ACP remains
how `agent: "claude"` runs.

---

## 10. Risks and non-goals

**Risks**

- Gateway fallback silently serving GLM for a Gemini grant (section 2.1).
- Gemini placeholder pricing making Explore look free in credit UI.
- GLM promotion ending 2026-09-01 changing Plan from free to list
  without a deploy.
- Dual-writing grants if option 2 (Phoenix reverse-proxy) is attempted
  before one ledger is chosen.
- Recreating Phoenix (OAuth, forge, forum) inside the coder API.
- `--dev` starting two processes (Phoenix and coder-api) on the same
  port. The CLI has to pick one.

**Non-goals for the extract**

- Replacing OpenRouter or Vercel with a first-party provider SDK.
- Native Claude via `crates/claude_agent_sdk` (#232). Explore is Gemini;
  Claude remains ACP.
- Background Mini / swarm mailboxes.
- Changing production DNS or Cloud Run topology.
- Declaring Gemini rates. That is an operator edit of the catalog.

---

## 11. References

- Phoenix catalog and commentary: `openagents.com/config/config.exs`
  (`:model_catalog`, `:vercel_gateway_fallback_models`, thread grant
  ceilings, credit defaults).
- Catalog / mint / proxy: `lib/openagents/inference/{models,grant,pricing,credit,health}.ex`,
  `lib/openagents/inference.ex`,
  `lib/openagents_web/controllers/{inference_proxy,model_catalog,thread,credit}_controller.ex`.
- Gateway: `lib/openagents/providers/vercel_gateway.ex`.
- Invariants: `openagents.com/INVARIANTS.md` PROVIDER-002, METER-001,
  THREAD-001, RELEASE-002.
- Public copy: `openagents.com/priv/docs/models.md`.
- CLI hop: `crates/openagents-cli/src/runtime.rs` (`Lane`, `LANES`,
  `create_thread`, `InferenceGrant`, `ServedModel`).
- Mini: `crates/openagents-cli/src/coder/agents.rs`,
  `crates/openagents-cli/src/tools.rs` (`delegate_to_builtin_agent`).
- Gemini live trial: `openagents.com/docs/terminalbench/2026-08-24-fix-git-run-analysis.md`.
- Product context: `openagents.com/docs/2026-08-24-coder-first-cloud-complements.md`.
