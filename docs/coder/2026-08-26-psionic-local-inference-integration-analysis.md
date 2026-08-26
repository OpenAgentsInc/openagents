# Psionic local inference integration analysis

## Executive summary

OpenAgents can replace Ollama as Coder's local inference runtime, but the current
repositories do not support that replacement yet.

The shortest sound architecture keeps Psionic in its own repository and ships
its OpenAI-compatible server as a separately versioned engine. The `openagents`
CLI should gain an `inference` namespace that installs, configures, starts,
stops, inspects, and diagnoses that engine. Coder should call the managed local
endpoint through a provider-neutral client. It should not link the complete
Psionic serving dependency graph into the `openagents` executable.

This approach preserves one user-facing CLI while maintaining a process boundary
between product control and model execution:

```text
openagents inference ── owns lifecycle, configuration, artifacts, and status
         │
         ▼
psionic-openai-server ── owns model loading, backend execution, and serving truth
         │
         ▼
Qwen model artifact + CPU, CUDA, or Metal backend

openagents coder ── calls the same loopback endpoint through a local provider
```

Four gaps block the requested Qwen 3.8 outcome:

1. OpenAgents has no `openagents inference` command. Its only local Coder path
   is an Ollama-specific branch in `crates/openagents-cli/src/runtime.rs` that
   calls `/api/tags` and `/api/chat`.
2. Psionic has no Qwen 3.8 model family. The pinned Psionic tree has native
   Qwen3.5 CPU, CUDA, and Metal services, but a search of its source and
   canonical inference documentation finds no `qwen3.8` implementation.
3. Psionic's current Qwen streaming executor completes generation before it
   publishes the response stream. Coder would receive one completed delta
   instead of live tokens.
4. Psionic's recorded Qwen3.5 27B Metal result is slower than Ollama on the
   same host and prompt: `9.16` decode tokens per second compared with
   `21.27`. Qwen 3.8 cannot replace Ollama by default until correctness,
   latency, memory, and tool-use gates pass on the release hardware.

The work should proceed in two independent tracks. First, build the generic
OpenAgents-to-Psionic lifecycle and protocol integration against an already
supported small Qwen3.5 artifact. Second, add Qwen 3.8 support inside Psionic.
Join the tracks only after each has its own tests. This separation prevents
model bring-up defects from being misdiagnosed as CLI lifecycle or protocol
defects.

## Scope and revisions

This analysis compares these local trees on August 26, 2026:

- OpenAgents at `e1db3b40d72c1d757138976bc9df86446e8ec829`, principally
  `crates/openagents-cli`.
- Psionic at `54201484bb8eb11b528f7038922db02724864523`, principally
  `psionic-serve`, `psionic-models`,
  `psionic-catalog`, and the CPU, CUDA, and Metal backends.

The proposed first product target is Apple Silicon because the requested local
model has already been exercised through Ollama there and Psionic has a native
Metal backend. The architecture remains portable, but local inference support
does not need to match the OpenAgents CLI's seven-target release matrix on its
first release.

This document uses **Qwen 3.8** to mean the exact artifact currently recorded by
OpenAgents as `qwen3.8:27b-mtp-q8_0`. That string is an Ollama model identity,
not a portable Psionic artifact reference. An implementation must bind the model
to an upstream artifact, digest, architecture facts, tokenizer, chat template,
quantization, and license before it can claim equivalence.

## Current OpenAgents local inference path

### Command surface

`crates/openagents-cli/src/cli.rs` exposes forge, auth, issues, projects,
milestones, repositories, Coder, delegation, deployment, providers, Boxes,
computers, forum, memory, API, plugins, traces, and updates. It does not expose
an `inference` namespace.

Coder has three ways to select the same local lane:

- `openagents coder --local`
- `openagents coder --lane local`
- `openagents coder --model ollama:<model>`

`CoderArgs::lane_name` resolves those flags into `Lane::Local`. `Lane::Local`
means Ollama in code, copy, environment variables, model identity, tests, and
errors. `OPENAGENTS_OLLAMA_HOST` defaults to the local Ollama service.

### Runtime coupling

The local branch in `crates/openagents-cli/src/runtime.rs` performs all Ollama
protocol work directly:

1. `GET /api/tags` discovers installed model names.
2. Prefix matching resolves a shorthand such as `qwen3.8` to an installed
   Ollama tag.
3. `POST /api/chat` sends Ollama messages and tool declarations.
4. A newline-delimited JSON parser extracts thinking, content, tool calls, and
   token counts.
5. Tool results are encoded back into Ollama's message shape.
6. The session records model identity as `ollama:<model>`.

This code combines five responsibilities: provider discovery, provider wire
format, model resolution, the tool loop, and local-lane policy. Replacing the
HTTP URLs without separating those responsibilities would produce a Psionic
branch beside the Ollama branch and duplicate most of the tool loop.

### Existing strengths to preserve

The current local path already provides behavior that the Psionic integration
must retain:

- Nothing leaves the machine.
- No server thread opens and no inference credit is metered.
- Ambiguous model prefixes fail instead of selecting an arbitrary artifact.
- The complete conversation returns to the model after each tool result.
- The final assistant answer joins local conversation history.
- Tool calls preserve names, JSON arguments, call IDs, and result association.
- Streaming errors, provider refusals, and unavailable models produce distinct
  failures.
- ATIF records the actual local model identity.
- Local turns run in interactive, plain, and headless Coder modes.

## Current Psionic serving path

### Reusable boundary

`psionic-serve` already contains the right external boundary. The
`psionic-openai-server` binary:

- accepts one or more `--model <artifact>` paths;
- selects `--backend cpu|cuda|metal`;
- binds to a configurable host and port;
- loads `OpenAiCompatServer`;
- serves `/health`, `/v1/models`, `/v1/chat/completions`, and
  `/v1/responses`;
- publishes backend, execution mode, and execution engine truth;
- supports OpenAI-shaped tools, tool choice, parallel tool calls, usage, and
  response-state flows on admitted model families.

The server also has routing, mesh, management, response-state, and debug
surfaces. A local CLI integration needs only a bounded subset. It should not
make the mesh control plane a requirement for one laptop.

### Qwen support

Psionic's native Qwen path is specifically `GgufDecoderFamily::Qwen35`.
`psionic-serve/src/qwen35.rs` defines separate CPU, CUDA, and Metal text
generation services. The generic OpenAI server admits a model only after GGUF
inspection identifies a supported decoder family and the chosen backend can
load it.

The current tree does not contain a Qwen 3.8 decoder family, prompt renderer,
tokenizer admission contract, GGUF layout, or backend service. Qwen3.5 support
does not prove Qwen 3.8 support. The name suggests a related family, but family
reuse must come from inspected architecture and tensor facts, not a string
prefix.

The exact requested Ollama artifact also includes `mtp` in its tag. Psionic
must determine whether that means a multi-token-prediction head, extra tensors,
a decoding mode, or packaging metadata. Ignoring those tensors or running a
base decoder under the same model name would create a false equivalence.

### Streaming limitation

The Qwen3.5 implementations satisfy `StreamingTextGenerationExecutor`, but each
calls `generate` first and wraps the completed response in
`CompletedQwen35Stream`. The generic HTTP handler therefore cannot provide true
token-time streaming for Qwen through this executor. It can format an
OpenAI-compatible server-sent event response, but generation has already
finished before those events reach the client.

Coder depends on progressive output for visible activity, waiting-state
transitions, cancellation, and time-to-first-token behavior. Protocol-compatible
completed streaming is insufficient.

### Performance limitation

Psionic's committed `docs/audits/2026-04-05-qwen35-27b-metal-gap-audit.md`
records native Metal Qwen3.5 27B at `9.16` decode tokens per second and Ollama
at `21.27` on the same raw prompt and host. The document attributes the gap to
host-owned hidden and recurrent state, scalar prompt replay, and incomplete
backend ownership.

That evidence applies to Qwen3.5, not Qwen 3.8. It still establishes a release
rule: do not remove the Ollama path or make Psionic the default based only on
correct output from one prompt. Measure the requested model on the hardware
that will receive the release.

## Architecture decision

### Recommended: managed sidecar

Ship Psionic as a separately versioned local inference engine and make
`openagents inference` its lifecycle controller. Coder talks to its loopback
OpenAI-compatible endpoint through a provider-neutral local client.

This design has these properties:

- The user learns one command surface: `openagents`.
- Psionic continues to own model and backend truth.
- OpenAgents continues to own product configuration, Coder behavior, release
  selection, and user-facing errors.
- A Psionic crash cannot corrupt Coder's process or terminal cleanup.
- The CLI can update without rebuilding large backend crates.
- The engine can update for a model or kernel fix without changing unrelated
  forge and issue commands.
- Platform-specific backends do not enter every OpenAgents CLI artifact.
- Logs and resource usage remain attributable to a distinct process.
- The protocol boundary can be tested with a stub and with the real engine.

### Rejected: link Psionic into `openagents`

Adding `psionic-serve` as a dependency of `openagents-cli` would pull its broad
serving graph into the released executable. The dependency list includes data,
adapters, CPU, CUDA, Metal, catalog, cluster, compiler, core, evaluation, IR,
models, networking, observation, research, router, runtime, transformer, and
training crates.

This approach creates several problems:

- Every normal CLI install carries inference code even when the user never runs
  a local model.
- The seven-target release must compile platform-specific backend code into one
  product artifact.
- A model runtime defect can terminate the terminal process.
- Backend initialization, memory, and threads share Coder's process.
- Psionic and OpenAgents releases become lockstep.
- The CLI's MIT distribution must carry Apache 2.0 notices and review the full
  transitive native dependency and binary-redistribution surface.
- Updating the engine requires replacing the complete CLI.

An in-process library could become useful for a small, portable CPU model after
Psionic provides a narrow client library with a bounded dependency graph. It is
the wrong first integration for a 27B Apple Metal model.

### Rejected: make Coder spawn Cargo

Coder must not run `cargo run -p psionic-serve`. That would require a Psionic
source checkout, a Rust toolchain, long compilation, repository-relative paths,
and developer-only environment state. Development mode can accept an explicit
engine path, but production must run a signed release artifact.

### Transitional: retain explicit Ollama support

Do not delete Ollama in the first Psionic release. Keep it behind explicit
`ollama:<model>` selection while `local` and `psionic:<model>` exercise Psionic.
Remove Ollama only after Psionic passes the replacement gates. Do not silently
fall back from Psionic to Ollama, because the UI would claim one runtime while
another executed the turn.

## Proposed command surface

Add `Inference(InferenceArgs)` to the root `Commands` enum. The namespace should
own engines and model artifacts, not Coder conversations.

```text
openagents inference install
openagents inference models
openagents inference add <artifact-or-approved-reference>
openagents inference remove <model>
openagents inference serve [--model <model>] [--backend auto|metal|cuda|cpu]
openagents inference status
openagents inference stop
openagents inference doctor
openagents inference logs [--follow]
```

### `install`

Resolve the engine release compatible with the running CLI and target. Download
it from the OpenAgents release origin, verify its digest, verify its platform
identity, and install it under `~/.openagents/inference/engines/<version>/`.
On macOS, verify its code signature and notarization result. Installation must
not download model weights without a separate command and license disclosure.

### `models`

List registered model artifacts and active engine inventory. Each row should
show:

- local model ID and aliases;
- canonical artifact path;
- content digest and size;
- family reported by Psionic inspection;
- quantization and context limit;
- compatible backends;
- license acknowledgement state;
- loaded, warm, or stopped state;
- execution refusal reason, when present.

This command must distinguish an artifact present on disk from a model the
active engine successfully admitted.

### `add`

Register a local artifact only after Psionic inspection succeeds. Store a
manifest, not a mutable name-to-path shortcut. The manifest should bind the
source reference, resolved file, digest, bytes, family, quantization, tokenizer,
chat template, license metadata, and inspection report.

Remote acquisition should accept only an approved HTTPS or Hugging Face source
with an expected digest. Avoid an unconstrained “pull arbitrary code and model”
operation. Download to a temporary file, verify, then atomically move it into
the content-addressed model store.

### `serve`

Select the artifact and backend, choose an unused loopback port, generate a
session secret, spawn the pinned Psionic engine, and wait for both `/health`
and `/v1/models` to confirm the expected model digest and backend. Report load
progress during large-model startup.

Foreground mode is useful for development and logs. Coder should request a
managed background instance through the same lifecycle module instead of
implementing a second spawn path.

### `status`, `stop`, `doctor`, and `logs`

Store process state under `~/.openagents/inference/run/` with restrictive file
permissions. State should include PID, process start identity, engine version,
endpoint, model digest, backend, log path, and the last successful health
observation. Never trust a PID alone; verify the process identity before sending
a signal.

`stop` should request graceful shutdown, wait for model cleanup, then terminate
only the verified owned process. `doctor` should compare installed artifacts,
signatures, backend availability, memory, model admission, endpoint security,
and Coder protocol compatibility. `logs` should read the owned log without
printing authentication secrets.

## Provider-neutral Coder integration

### Separate the local provider from the tool loop

Refactor `runtime.rs` around a local inference interface before adding Psionic:

```rust
trait LocalInferenceProvider {
    async fn models(&self) -> Result<Vec<LocalModel>, Failure>;
    async fn stream_chat(
        &self,
        request: LocalChatRequest,
        events: Sender<LocalChatEvent>,
    ) -> Result<LocalChatCompletion, Failure>;
    async fn cancel(&self, request_id: &str) -> Result<(), Failure>;
}
```

The shared Coder loop should own conversation history, maximum tool steps, tool
execution, ATIF recording, final-answer ordering, and retry policy. Provider
adapters should own only discovery, request encoding, stream decoding, usage,
model identity, and provider-specific errors.

Implement these adapters during migration:

- `PsionicLocalProvider` for `/v1/models` and `/v1/responses` or
  `/v1/chat/completions`.
- `OllamaLocalProvider` containing the existing `/api/tags`, `/api/chat`, and
  newline-delimited JSON behavior.

Delete the Ollama adapter after the replacement gate and deprecation window.

### Prefer `/v1/responses`

Psionic already exposes `/v1/responses` with response-state and tool-result
continuation. This surface aligns with OpenAgents' hosted inference direction
better than an Ollama-shaped chat protocol. Use it if the Qwen 3.8 lane supports
all required streaming events and tool-result replay. Use
`/v1/chat/completions` as the first compatibility path only if it reaches a
working tool loop sooner.

OpenAgents should define one internal event model and map both surfaces into it:

- response started;
- reasoning delta;
- output text delta;
- tool call started;
- tool argument delta;
- tool call completed;
- usage completed;
- response completed;
- response failed;
- response canceled.

Do not parse rendered tool-call text. Consume structured call IDs, names, and
arguments.

### Model selection and compatibility

Change local model syntax without making `local` mean a vendor:

- `--local` selects the configured default local engine and model.
- `--model local:<model>` selects a model from the configured local engine.
- `--model psionic:<model>` explicitly selects Psionic during migration.
- `--model ollama:<model>` explicitly selects the legacy adapter.

After Psionic becomes the sole local engine, `local:<model>` remains stable and
`psionic:<model>` can remain an explicit alias. Avoid writing Psionic into the
generic `Lane::Local` variant or every local abstraction will need another
rename later.

ATIF should record:

- canonical model ID;
- artifact digest;
- engine name and version;
- backend and execution mode;
- local execution locality;
- prompt, completion, and cached token counts;
- finish reason;
- tool calls;
- timing and cancellation state.

### Lifecycle from Coder

When Coder selects a Psionic local model:

1. Check the managed endpoint and expected model.
2. If no engine runs, start the installed compatible engine.
3. Display model loading and health progress instead of showing a generic
   waiting message.
4. Refuse with an actionable `openagents inference install` or `add` command if
   the engine or model is missing.
5. Keep a session lease while Coder uses the engine.
6. On exit, release the lease. Stop immediately only under an explicit
   `--stop-local-engine-on-exit` policy; otherwise apply an idle timeout.

Do not start or replace an engine that another OpenAgents process actively
leases. Do not attach to an endpoint whose process identity or session secret
does not match managed state.

## Qwen 3.8 work required in Psionic

### Establish artifact identity

Before code changes, resolve `qwen3.8:27b-mtp-q8_0` to the exact upstream
artifact and record:

- repository and revision;
- file names, bytes, and SHA-256 digests;
- model and tokenizer licenses;
- GGUF version and architecture name;
- tensor inventory, dimensions, and quantization types;
- tokenizer pre-tokenizer, special tokens, and vocabulary digest;
- chat template and reasoning controls;
- context length and rope or MRoPE facts;
- MTP tensors and intended decode behavior.

An Ollama manifest may help locate blobs, but it cannot remain the authoritative
artifact contract after Ollama is removed.

### Add a model-family admission report

Extend Psionic's GGUF inspection before writing kernels. The report should
classify every metadata key and tensor as required, optional, ignored with a
reason, or unsupported. It should compare Qwen 3.8 with Qwen3.5 and state which
existing parser, prompt, tokenizer, projection, attention, recurrent, and MTP
paths can be reused.

Fail if unknown required tensors remain. Do not map the artifact to
`GgufDecoderFamily::Qwen35` because its shape looks close.

### Implement reference correctness first

Add the Qwen 3.8 family to the smallest reference backend that can establish
correctness. CPU is suitable for tiny fixtures and short deterministic decode;
the full 27B artifact may make complete CPU acceptance impractical. Required
evidence includes:

- tokenizer parity on a fixed corpus;
- prompt token parity for system, user, assistant, and tool messages;
- tensor and quantization load receipts;
- fixed-token logits or top-k agreement for initial positions;
- greedy token parity for short prompts;
- stop-token and context-limit behavior;
- reasoning boundary parsing;
- single, required, named, and parallel tool-call behavior;
- tool-result continuation;
- malformed artifact and unsupported feature refusals.

Compare against at least one independent reference. Ollama can serve as a
temporary reference, but the final fixtures should not require Ollama to run.

### Add native Metal execution

Implement a Qwen 3.8 Metal service that keeps weights, hidden state, cache,
recurrent state, MTP state, scratch buffers, and output selection under backend
ownership. Do not repeat the Qwen3.5 bring-up design that aliases Metal state to
CPU containers and steps major operations through host `Vec<f32>` values.

For a 27B Q8 artifact, admission must measure model bytes, runtime buffers,
cache growth, unified memory pressure, and safe headroom before loading. Refuse
with required and available memory instead of allowing macOS to enter sustained
swap pressure.

The performance plan should include:

- chunked prompt prefill;
- backend-owned KV, recurrent, and MTP state;
- fused quantized projections and normalization where supported;
- device-side sampling or bounded candidate readback;
- prefix reuse across tool-loop rounds;
- cancellation checks between bounded submissions;
- deterministic cleanup after refusal, cancellation, and process termination.

### Implement true streaming and cancellation

Replace the completed-response stream with a bounded channel fed by the decode
loop. Publish the first token as soon as decoding produces it. Propagate
backpressure rather than accumulating unbounded text. Ensure cancellation stops
decode and releases per-request buffers without unloading the shared model.

Streaming acceptance must measure:

- time to first event;
- time to first content token;
- inter-token cadence;
- cancellation latency;
- memory retained after cancellation;
- behavior when the client disconnects;
- tool argument streaming and final JSON validity;
- exactly one terminal completion or failure event.

### Publish honest served capability

`/v1/models` and `/health` need enough Psionic fields for OpenAgents to verify
the active artifact and runtime. At minimum, publish canonical model ID,
artifact digest, family, quantization, backend, engine version, execution mode,
context limit, tool support, response-state support, streaming mode, and current
warm state.

Do not advertise token streaming while the implementation emits a completed
response as one delta. A capability value such as `completed` versus `token`
lets Coder refuse or explain the degraded mode.

## Engine distribution and updates

### Keep CLI and engine manifests separate

The current OpenAgents release script publishes one CLI artifact for seven
targets. Do not append Psionic bytes to those artifacts. Publish a second
manifest:

```json
{
  "schema": "openagents.inference-engine-release.v1",
  "engine": "psionic",
  "version": "<version>",
  "protocol": "openagents.local-inference.v1",
  "artifacts": [
    {
      "platform": "macos-aarch64",
      "sha256": "<digest>",
      "bytes": 0,
      "signature": "apple-developer-id"
    }
  ]
}
```

The CLI release or a compatibility catalog should map its supported local
protocol version to an engine version range. Pin the selected engine digest in
managed state. Do not resolve an unbounded `latest` during Coder startup.

### Start with Apple Silicon

The OpenAgents CLI can continue shipping to seven targets while
`openagents inference install` supports only `macos-aarch64` in the first
milestone. On other targets, return an explicit unsupported-target result and
leave hosted Coder behavior intact.

Expand only after Psionic produces native engine artifacts and backend evidence
for:

- macOS x86-64 CPU, if the model can run within useful limits;
- Linux x86-64 GNU with CUDA and CPU variants;
- Linux x86-64 musl only if native dependencies support static distribution;
- Linux AArch64 GNU and musl with an admitted backend;
- Windows x86-64 with a tested service lifecycle and backend.

Backend variants may require distinct engine artifacts even when the CLI target
is the same. A single Linux executable that assumes CUDA can fail on a CPU-only
machine before it can report a useful refusal.

### Model artifacts remain separate

Do not package 27B weights with the engine or CLI. The model store should be
content-addressed and reusable across engine versions. Upgrades must not delete
weights. Garbage collection should remove only unreferenced artifacts after an
explicit command or policy, and it should report reclaimable bytes before
deletion.

### Licensing

Psionic uses Apache 2.0 while the OpenAgents workspace uses MIT. A separate
binary requires its own license and notices in the engine distribution. The
installer and model registration flow must also expose the Qwen artifact's
license and any use restrictions. Code license compatibility does not grant a
right to redistribute model weights.

## Security and privacy

Local execution must be true in transport and operation:

- Bind to `127.0.0.1` or an owned local socket by default.
- Refuse `0.0.0.0` unless the user explicitly selects a server mode and
  configures authentication.
- Generate a per-engine secret and require it on every inference and management
  request. Loopback alone does not protect against other local processes.
- Store the secret in a user-only state file and never include it in logs,
  process arguments, ATIF, or diagnostics.
- Verify engine path, signature, digest, PID start identity, and endpoint before
  attaching or stopping.
- Pass model paths as direct process arguments without a shell.
- Treat chat templates and tokenizer metadata as data, not executable code.
- Disable mesh coordination for the default laptop mode.
- Never proxy to a remote bootstrap server under a command that claims local
  inference.
- Record execution locality and refuse if Psionic reports `remote` or `proxy`.

The model sees local files only through Coder's declared tools. Psionic should
receive conversation and tool schemas, not ambient filesystem authority.

## Observability and support

The engine and CLI should share correlation IDs without sharing internal state.
For each turn, record:

- Coder turn and ATIF identifiers;
- local request and response IDs;
- engine version and process identity;
- model and artifact digest;
- backend and execution mode;
- load, queue, prefill, time-to-first-token, and decode timing;
- prompt, cached, reasoning, and completion tokens;
- peak backend and host memory;
- tool-loop round count;
- cancellation or failure code.

Default logs should omit prompt, response, tool arguments, local paths beyond
the registered artifact, environment variables, and secrets. A debug mode may
include content only after an explicit warning.

Coder should translate Psionic failures into actionable messages:

| Psionic condition | Coder action |
| --- | --- |
| Engine missing | Show `openagents inference install`. |
| Model missing | Show `openagents inference add <artifact>`. |
| Artifact unsupported | Name the rejected family or tensor fact. |
| Insufficient memory | Report required, available, and selected context. |
| Backend unavailable | List admitted backends from `doctor`. |
| Engine starting | Show model load progress and elapsed time. |
| Completed-only streaming | Refuse by default or label the degraded behavior. |
| Tool calling unsupported | Refuse Coder mode before sending a prompt. |
| Engine crashed | Preserve transcript, show log path, and offer one bounded restart. |
| Client disconnected | Cancel the request and keep engine health observable. |

## Test plan

### OpenAgents unit and stub tests

- Parse `local:`, `psionic:`, and explicit legacy `ollama:` model names.
- Keep `--local` compatible with one selected explicit local model.
- Resolve models by exact ID and reject ambiguous aliases.
- Map Chat Completions and Responses events into the internal local event model.
- Preserve reasoning, text, tool calls, IDs, arguments, usage, and finish reason.
- Run multiple tool rounds without duplicating assistant or tool messages.
- Cancel a request and ignore late events.
- Refuse remote execution truth under the local lane.
- Export engine and artifact identity to ATIF.

### Lifecycle integration tests

- Install a signed fixture engine after digest verification.
- Reject a wrong digest, wrong platform, unsigned macOS artifact, and incompatible
  protocol.
- Start one engine concurrently from two CLI processes without duplication.
- Recover from stale state, PID reuse, port collision, and engine crash.
- Keep one engine alive across sequential Coder sessions according to idle
  policy.
- Stop only the verified owned process.
- Preserve and rotate logs with restrictive permissions.
- Leave raw terminal state intact when engine startup fails.

### Psionic protocol tests

- Confirm `/health` and `/v1/models` bind the expected model digest and backend.
- Compare Chat Completions and Responses behavior for the same conversation.
- Stream multiple token events before completion.
- Stream ordered tool-call argument deltas that form valid JSON.
- Continue after tool results without replay drift.
- Cancel during load, prefill, decode, and tool-call generation.
- Reject unsupported tools, structured output, context, and sampling options before
  expensive generation.
- Require the local session secret.

### Qwen 3.8 conformance tests

- Tokenizer and chat-template corpus parity.
- GGUF metadata and tensor admission snapshots.
- Quantized tensor decode tests for every used quantization.
- Reference logits and greedy-token fixtures.
- Reasoning and answer boundary fixtures.
- Single, named, required, and parallel tool calls.
- Multi-round Coder tool loop.
- Long-context boundary and overflow policy.
- MTP enabled, disabled, and unsupported-artifact behavior.
- Deterministic seed and greedy decode.
- Malformed, truncated, mismatched, and unsupported artifacts.

### Real Apple Silicon acceptance

Run on the oldest and smallest supported Apple Silicon class and on the primary
development Mac. Record:

- cold and warm load time;
- resident, peak, and post-unload memory;
- prompt and decode tokens per second;
- time to first token;
- cancellation latency;
- sustained memory pressure and swap;
- ten sequential Coder turns with tool calls;
- concurrent non-inference CLI commands while the engine runs;
- sleep, wake, terminal exit, and process crash recovery;
- output parity and task effectiveness against the current Ollama baseline.

## Replacement gates

Psionic can become the default local runtime only when all of these statements
are true for the exact Qwen 3.8 artifact:

1. Psionic admits the model under a specific Qwen 3.8 family and publishes its
   artifact digest.
2. Tokenizer, prompt, deterministic decode, reasoning, and tool-use conformance
   pass against independent reference fixtures.
3. `/v1/responses` or `/v1/chat/completions` provides true token-time streaming,
   structured tool calls, usage, cancellation, and multi-round continuation.
4. The Metal path retains backend-owned execution state and does not enter
   unacceptable swap pressure on supported Macs.
5. Time to first token and decode throughput meet an explicit product threshold.
   A reasonable initial bar is no worse than 15% below Ollama on the same
   artifact and hardware, with no effectiveness regression.
6. `openagents inference` can install, verify, start, inspect, stop, update, and
   diagnose the engine without a source checkout or Rust toolchain.
7. Coder interactive, plain, and headless modes pass the same local tool-loop and
   ATIF tests.
8. The engine distribution is signed, notarized where applicable, licensed, and
   pinned by digest.
9. A fallback never occurs silently. Every turn reports the engine and model it
   actually used.
10. The explicit Ollama path remains available for one deprecation release and
    can be removed without losing a supported behavior.

## Staged implementation plan

### Phase 0: Freeze contracts

1. Define `openagents.local-inference.v1`, including model inventory, health,
   execution truth, stream events, errors, and cancellation.
2. Resolve the Qwen 3.8 artifact and license.
3. Pin a small supported Qwen3.5 GGUF fixture for CLI integration.
4. Record Ollama baselines for the exact Qwen 3.8 artifact and Apple hardware.

**Exit:** OpenAgents and Psionic can test the protocol independently, and the
requested model has a content identity.

### Phase 1: Provider abstraction

1. Extract the existing Ollama wire code from the shared local tool loop.
2. Add the internal local request and event types.
3. Implement the Psionic adapter against a manually started Qwen3.5 server.
4. Preserve existing Ollama tests and add protocol-stub Psionic tests.

**Exit:** Coder completes a multi-round local tool turn against a manually
started Psionic server without changing engine lifecycle.

### Phase 2: `openagents inference`

1. Add the root namespace and typed subcommands.
2. Implement engine manifest verification and target resolution.
3. Add the content-addressed model registry and inspection handoff.
4. Add lifecycle ownership, leases, health, logs, doctor, and stop.
5. Integrate automatic startup into Coder.

**Exit:** A clean Apple Silicon machine can install the engine, register the
small fixture, and run Coder without Ollama or a Psionic checkout.

### Phase 3: Qwen 3.8 correctness

1. Add artifact inspection and a dedicated family.
2. Add tokenizer, prompt, GGUF, quantization, and reference decode support.
3. Add reasoning, structured tool calls, and response continuation.
4. Publish capability and refusal truth.

**Exit:** Deterministic CPU or reference tests and short native Metal decode
match the approved fixtures.

### Phase 4: Qwen 3.8 Metal product lane

1. Implement backend-owned state and chunked prefill.
2. Add true streaming and cancellation.
3. Add memory admission, prefix reuse, and cleanup.
4. Optimize against the recorded Ollama matrix without weakening correctness.

**Exit:** The exact 27B artifact passes correctness, memory, latency, throughput,
and Coder tool-loop gates on supported Apple Silicon.

### Phase 5: Default and deprecation

1. Make `--local` resolve to Psionic.
2. Keep explicit `ollama:<model>` and publish migration copy.
3. Measure real failures and effectiveness for one release.
4. Remove Ollama code, environment variables, copy, and tests only after the
   deprecation gate passes.

**Exit:** Psionic is the sole supported local runtime and the provider-neutral
local abstraction remains.

### Phase 6: Additional platforms

Build and qualify engine artifacts by backend and target. Do not block the
Apple Silicon release on all seven CLI targets, and do not mark an untested
engine target supported because the main CLI cross-compiles there.

## Deletion and simplification opportunities

After the replacement gate, delete:

- `OPENAGENTS_OLLAMA_HOST` handling;
- `/api/tags` discovery;
- `/api/chat` request construction;
- newline-delimited Ollama stream parsing;
- Ollama message and tool-result conversion;
- model-prefix logic tied to Ollama tags;
- `ollama:` user-facing copy and legacy tests;
- TypeScript Ollama implementation and tests if the TypeScript CLI remains
  retired;
- benchmark instructions that require Ollama after Psionic fixtures replace the
  reference dependency.

Keep:

- the provider-neutral local request, event, and tool loop;
- `local:<model>` identity;
- engine and artifact manifests;
- ATIF local execution evidence;
- independent reference fixtures and historical Ollama receipts;
- Psionic as a separate repository and release artifact.

Avoid adding:

- a second model registry inside Coder;
- separate engine spawn logic for `openagents inference` and Coder;
- a hidden remote fallback;
- shell-script process supervision;
- a Qwen-specific branch in the shared Coder tool loop;
- Psionic source or model weights inside the OpenAgents repository;
- a seven-platform promise based only on the existing CLI release matrix.

## Recommended first implementation slice

The first slice should not attempt Qwen 3.8. It should prove the architecture
with a supported, small Qwen3.5 artifact:

1. Add a minimal `openagents inference serve`, `status`, and `stop` lifecycle.
2. Add `PsionicLocalProvider` over `/v1/chat/completions`.
3. Refactor the shared local tool loop without changing existing Ollama behavior.
4. Run a multi-round shell-tool fixture through a manually built Psionic engine.
5. Package one Apple Silicon engine artifact and run the same fixture on a clean
   install.

That slice exposes lifecycle, protocol, tool-call, release, and terminal defects
before Qwen 3.8 model work adds another source of failure. It also creates a
usable integration for every Psionic model family that satisfies the same local
protocol.

## Evidence map

### OpenAgents

- `crates/openagents-cli/src/cli.rs`: root commands and Coder local flags.
- `crates/openagents-cli/src/runtime.rs`: lanes, local model resolution, Ollama
  discovery, chat streaming, tool calls, and usage.
- `crates/openagents-cli/src/coder/runtime.rs`: interactive Coder integration.
- `crates/openagents-cli/src/tools.rs`: client-owned tool runtime.
- `crates/openagents-cli/tests/runtime_test.rs`: Ollama local-lane protocol tests.
- `crates/openagents-cli/tests/flags.rs`: local flag and routing tests.
- `crates/openagents-cli/tests/autonomous_test.rs`: autonomous local tool loop.
- `ops/release-cli.sh`: seven-target CLI release and artifact verification.

### Psionic

- `README.md`: workspace and current capability map.
- `docs/ARCHITECTURE.md`: canonical system architecture.
- `docs/INFERENCE_ENGINE.md`: canonical inference readiness and bounded lanes.
- `docs/audits/2026-04-05-qwen35-27b-metal-gap-audit.md`: Metal performance
  and execution-shape evidence.
- `docs/QWEN35_OLLAMA_COMPARISON.md`: native CUDA and Ollama comparison contract.
- `crates/psionic-serve/src/bin/psionic-openai-server.rs`: current server CLI.
- `crates/psionic-serve/src/openai_http.rs`: routes, model inventory, tools,
  Responses, Chat Completions, routing, and streaming format.
- `crates/psionic-serve/src/qwen35.rs`: native Qwen3.5 CPU, CUDA, and Metal
  implementations and completed-response stream adapter.
- `crates/psionic-serve/src/gguf.rs`: GGUF artifact inspection and decoder-family
  facts.
- `crates/psionic-catalog`: artifact catalog and model discovery.
- `crates/psionic-backend-cpu`, `psionic-backend-cuda`, and
  `psionic-backend-metal`: execution backends.

Refresh the pinned revisions, model artifact, and benchmark receipts before
using this document to approve implementation or remove Ollama.
