# Omega Agent cloud-coupling severability trace

- Status: severability trace for `OMEGA-AGENT-00`, 2026-07-25
- Owner: OpenAgents
- Issue: omega#74, under epic omega#73. Answers the trace request in omega#72
- ProductSpec: [Omega Agent product contract](../../specs/omega/omega-agent.product-spec.md)
  at `spec_revision: 1`
- Companion: [Omega Agent shape record](./2026-07-25-omega-agent-shape-record.md)
- Source pin: `OpenAgentsInc/omega` `b768854c56` (`0.2.0-rc10`)
- Method: static read of the source at the pin. This document records no
  runtime capture.

This document answers one question.
What is the Omega Agent when nothing external answers?

Issue omega#72 asked whether `cloud_api_types` and `cloud_llm_client` in
`crates/agent` and `crates/agent_ui` are load-bearing at runtime or type-level
only. Section 3 answers that question with file evidence.

## 1. The isolation posture, as the code sets it

Omega does not contact Zed production hosts on a normal start.

| Fact | Evidence |
| --- | --- |
| `ZED_PRODUCTION_SERVICES_ENABLED` is a `const false`. | `crates/app_identity/src/app_identity.rs:9` |
| `OMEGA_ALLOW_ZED_SERVICES=1` is the only override. | `crates/app_identity/src/app_identity.rs:11`-`:15` |
| The configured host is a reserved non-resolvable name. | `assets/settings/default.json:2684`, value `https://services.openagents.invalid` |
| `.invalid` is reserved by RFC 2606 and does not resolve. | RFC 2606 |
| A test asserts that the setting is not the upstream host. | `crates/app_identity/src/service_isolation.rs:105`-`:114` |
| `ZED_SERVER_URL` still overrides the setting. | `crates/client/src/client.rs:63`-`:64` and `:119`-`:122` |

The cloud client base address comes from the same setting.

| Fact | Evidence |
| --- | --- |
| `Client::production` builds the HTTP client from `ClientSettings::server_url`. | `crates/client/src/client.rs:583`-`:591` |
| `CloudApiClient::new` takes that HTTP client. | `crates/client/src/client.rs:567` |
| `zed_urls` helpers also derive from the same setting. | `crates/client/src/zed_urls.rs:13`-`:14` |

Result. Every inherited cloud address in the fork points at a name that does
not resolve, unless an operator sets `OMEGA_ALLOW_ZED_SERVICES=1` or
`ZED_SERVER_URL`.

## 2. What the isolation switch turns off

| Site | Behavior when the switch is off |
| --- | --- |
| `crates/language_models/src/language_models.rs:223`-`:236` | The cloud language-model provider is never registered. The code writes a log line and continues. A user cannot select a cloud model. |
| `crates/zed/src/main.rs:1341`-`:1346` | `authenticate` returns `Ok(())` at once. Omega performs no account sign-in and reads no account credential. |
| `crates/extension_host/src/extension_host.rs:732` | Gated. |
| `crates/auto_update/src/auto_update.rs:572` and `:634` | Gated. |

`crates/language_models_cloud` is reachable only through the registration at
`language_models.rs:223`. With the switch off, that crate is compiled and
never constructed.

## 3. The omega#72 question, answered

### 3.1 Declared dependencies

| Crate | Dependency | Line |
| --- | --- | --- |
| `crates/agent` | `client` | `Cargo.toml:29` |
| `crates/agent` | `cloud_api_types` | `Cargo.toml:30` |
| `crates/agent` | `cloud_llm_client` | `Cargo.toml:31` |
| `crates/agent_ui` | `client` | `Cargo.toml:45` |
| `crates/agent_ui` | `cloud_api_types` | `Cargo.toml:46` |
| `crates/agent_ui` | `cloud_llm_client` | not declared |
| `crates/acp_thread` | `client` or any cloud crate | not declared |

The seam crate `crates/acp_thread` carries no cloud dependency at all.

### 3.2 Use sites in `crates/agent`

| Site | Class | Detail |
| --- | --- | --- |
| `crates/agent/src/thread.rs:27` `use cloud_api_types::Plan;` | type-level only | The single use is the parameter `plan: Option<Plan>` at `thread.rs:3262`. The read at `:3273` is `if model.provider_id() == ZED_CLOUD_PROVIDER_ID { plan.is_some() } else { true }`. For a direct provider the value is never consulted. |
| `crates/agent/src/thread.rs:26` `use client::UserStore;` | type-level and local read | The field reads an entity the project already holds. The only read is `user_store.plan()` at `:3070`, on a completion error path. |
| `crates/agent/src/tools/web_search_tool.rs:6` `use cloud_llm_client::WebSearchResponse;` | type-level only | A response shape. The tool refuses a non-cloud provider at `web_search_tool.rs:66`-`:68`, `provider == &ZED_CLOUD_PROVIDER_ID`. |

`crates/agent` issues no request to a cloud endpoint. The one HTTP reach in
the turn path is `crates/agent/src/thread.rs:2122`, which gives the fetch tool
a generic HTTP client for a user-supplied address. That is not a cloud
endpoint.

The remaining `client::Client` uses in `crates/agent/src` are in test and
evaluation code, at `src/tests/mod.rs:9` and under `src/tools/evals/`.

**Answer for `crates/agent`: type-level only.**

### 3.3 Use sites in `crates/agent_ui`

| Site | Class | Detail |
| --- | --- | --- |
| `crates/agent_ui/src/conversation_view/thread_view.rs:105`-`:115` | runtime-network-bearing | `client.cloud_client().submit_agent_feedback(...)`. |
| `crates/agent_ui/src/conversation_view/thread_view.rs:149`-`:158` | runtime-network-bearing | `submit_agent_feedback_comments(...)`. |
| `crates/agent_ui/src/agent_registry_ui.rs:596` | address open only | `cx.open_url(&zed_urls::acp_registry_blog(cx))` opens a browser. It issues no request from Omega. |
| `crates/agent_ui/src/agent_panel.rs:6199` | type-level only | A provider identifier comparison. |

**Answer for `crates/agent_ui`: type-level only, with one exception.** The
exception is the thread-feedback control.

## 4. The thread-feedback finding

The thread-feedback control is reachable, and it cannot succeed.

| Fact | Evidence |
| --- | --- |
| The control needs `AgentSettings::enable_feedback` and a connection that reports telemetry. | `crates/agent_ui/src/conversation_view/thread_view.rs:6872`-`:6884` |
| `enable_feedback` defaults to `true`. | `assets/settings/default.json:1187` |
| The native connection reports telemetry. | `crates/agent/src/agent.rs:2711`-`:2713` |
| The submit target is the cloud client. | `thread_view.rs:105` and `:149` |
| The cloud client address is the non-resolvable host. | Section 1 |

Result. On a default Omega install, a native thread shows a feedback control.
A user action on that control sends to a host that does not resolve. The task
uses `detach_and_log_err`, so the failure reaches the log and not the user.

This is a user-visible control that cannot complete its stated purpose.
This lane reports the finding and changes no omega file.
The finding belongs to the brand and surface lanes, not to `OMEGA-AGENT-00`.

## 5. The severability table

This table states what an Omega Agent does when nothing external answers.
Two conditions are separate.
Condition A is that no OpenAgents cloud service answers.
Condition B is that no network answers at all, including the model provider.

| Function | Condition A, no OpenAgents cloud | Condition B, no network |
| --- | --- | --- |
| Application start, window, editor, project, buffers | Works | Works |
| Agent panel, thread user interface, thread history | Works | Works |
| Local thread persistence | Works | Works |
| Native turn with a direct provider key | Works | Stops. The provider is remote. |
| Native turn with a local model server | Works | Works, if the server is local |
| Tool execution, permission model, diff review | Works | Works |
| Worktree instruction files and skills | Works | Works |
| External ACP agent over `crates/agent_servers` | Works, if the agent process runs locally | Depends on the agent. A local process starts. Its own provider call fails. |
| Router decision and executor selection | Works | Works. The decision is in-process. |
| Executor disclosure line | Works | Works. The values are local. |
| `omega-effectd` engine start and supervision | Works. The engine is a local packaged process. | Works |
| Full Auto run lifecycle inside the engine | Works | Works for the lifecycle. Stops for any lane that needs a provider. |
| Agent Computer cloud session | Stops | Stops |
| Khala outbound lanes, hosted and cloud | Stops | Stops |
| Khala Sync projection | Stops. Full Auto continues locally and reports the reason `omega_khala_sync_session_unavailable`, which the engine fixture at `crates/omega_effectd/fixtures/fake_effectd.mjs:729` and `:737` exercises. | Stops, same reason |
| Nostr workroom record on `relay.openagents.com` | Stops | Stops |
| Receipt render for a run already recorded locally | Works | Works |
| Thread feedback submission | Already fails. See section 4. | Already fails |
| Zed account sign-in | Not attempted. See section 2. | Not attempted |
| Zed cloud model provider | Not registered. See section 2. | Not registered |

## 6. What the reader should take from this

The Omega Agent, with no external OpenAgents service, is a local coding agent.
It starts, opens a thread, routes between local executors, states which
executor ran the work, runs tools, and writes local thread history.
It reaches one remote system only, and that system is the model provider the
user configured.

The Omega Agent, with no external OpenAgents service, is not a cloud lane.
It is also not a Khala route, not a cross-device record, and not a signed
workroom history.
Those four functions stop, and the product must say so rather than appear to
continue.

The severable part is larger than omega#72 assumed. The inherited cloud
dependencies in `crates/agent` are type-level only. The seam crate
`crates/acp_thread` has no cloud dependency. One user-visible control in
`crates/agent_ui` reaches a cloud endpoint, and section 4 records that it
already fails.

## 7. Limits of this trace

This trace is a static read. It does not prove a zero-request result.

The following remain unverified by this method.

1. Telemetry transport. Both crates depend on `crates/telemetry`. The
   settings set `diagnostics: false` and `metrics: false` at
   `assets/settings/default.json:1569`-`:1571`. This lane did not trace
   whether those settings close the transport completely.
2. The `crates/client` connection machinery. This lane did not confirm that no
   connection attempt starts independently of `authenticate`.
3. `crates/omega_effectd` and `crates/omega_identity` outbound behavior.
4. Extension host and auto-update paths beyond the gate lines in section 2.

A claim of zero outbound requests needs a runtime packet capture on an
installed candidate. This document makes no such claim.
