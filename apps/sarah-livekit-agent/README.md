# Sarah LiveKit agent

This service is the explicitly dispatched Sarah voice participant. It joins one
admitted LiveKit room as `principal.sarah`, opens one OpenAI Realtime connection
for that exact generation, and exits instead of reconnecting across a failed
provider or room generation.

The worker requires:

- `LIVEKIT_URL`, `LIVEKIT_API_KEY`, and `LIVEKIT_API_SECRET` for the self-hosted
  LiveKit deployment.
- `OPENAI_API_KEY` for `gpt-realtime-2.1`.
- `OPENAGENTS_CONTROL_URL`, an HTTPS OpenAgents API origin.
- `SARAH_LIVEKIT_WORKER_REF`, a stable non-secret deployment identity.
- `SARAH_LIVEKIT_CONTROL_ROOT`, an untrimmed 64–128-character base64url root
  injected from the same Secret Manager version as the API.

Explicit dispatch metadata contains no credential. The API and worker
independently derive the same generation-bound bearer token from the HMAC root
and the canonical immutable dispatch fields. Only its SHA-256 digest is stored.
The raw token exists only while constructing an authorization header for the
claim/event routes. It is never added to a job, body, receipt, or log. The
worker does not record media, retain transcripts, or log raw OpenAI events.

Agents JS 1.6.0 enforces publish, subscribe, publish-data, metadata-update, and
hidden permissions during worker registration. The pinned SDK patch also
serializes `canPublishSources`, and Sarah registers microphone as its only
publishable media source. Sarah enables publish-data only because the pinned SDK
implements disclosed session transcription with
`localParticipant.publishTranscription`. The worker has no generic data-publish
call, does not store transcripts, disables metadata updates, and remains
visible.

The OpenAI plugin is pinned and patched to attach the generation's hashed owner
identifier as `OpenAI-Safety-Identifier`. Response usage comes only from
`response.done.response.usage`; input-transcription usage is recorded
separately from the transcription-completed event. Responses are not terminal
until their status and usage are durable, and every committed input-audio item
remains pending until its transcription usage is durably accepted by the
control plane. Sarah-initiated shutdown cancels an in-flight response, drains
response and transcription accounting, and only then closes the provider and
asks the worker SDK to shut down. A transcription failure, accounting timeout,
or provider/SDK-first disconnect enters `accounting_uncertain`: the recorded
charge is not presented as final, the full credit hold remains reserved, and
room cleanup waits for explicit provider reconciliation.

The control plane does not advertise the session as ready when the worker merely
joins LiveKit. It waits for the owner participant, a completed `AgentSession`
start, and a server-confirmed OpenAI configuration with the exact model, audio
modality and formats, transcription model, voice, and semantic-VAD response and
interruption policy. It persists only bounded SHA-256 provider session and
configuration references before the control socket emits `session_ready`.

Private and community jobs instantiate separate capability profiles. Community
jobs are structurally tool-free and receive no owner memory, workspace, editor
proposal, payment, administration, shell, Git, or credential capability. A
private Omega job has the existing bounded editor command set:
`editor_context_read`, `editor_reveal_range`, `editor_replace_selection`,
`editor_save_document`, and `start_agent_thread`. Targets must already be exact;
the worker cannot discover workspaces or paths. Read and reveal commands execute
without a confirmation prompt, while replace, save, and agent-thread commands
require confirmation. Every tool waits for a typed Omega outcome before it can
return success to OpenAI.

For the Omega client, LiveKit remains the audio transport. The session's
ticketed `gatewayUrl` remains the authoritative control channel. The client
must keep that socket open, render `tool_proposal`, send `tool_decision` with
the exact proposal ref and digest, execute only the `tool_execute` command, and
return `tool_outcome` with the same ref and digest. The API then emits
`tool_outcome_ref`; the worker observes that outcome through its authenticated
control route and only then returns the function result to the Realtime model.
No LiveKit room data packet authorizes a command. Omega issue #185 owns the
client adapter that maps these existing `SarahVoiceServerControl` and
`SarahVoiceClientControl` frames into the desktop command-confirmation UI.

An explicit Omega `interrupt` remains a control-channel request. The API first
increments a durable sequence on the exact room generation, then uses the
LiveKit server API to send a reliable HMAC-authenticated interrupt packet only
to `principal.sarah`. The worker accepts the packet only when its session,
generation, room, epoch, signature, and increasing sequence match dispatch
authority; participant-originated and stale packets are ignored. The existing
worker lease response carries the same sequence as a delivery fallback. An
acknowledgment is emitted only after the durable request and direct LiveKit
delivery succeed. The packet cannot authorize tools and contains no transcript
or media.

New room admission is serialized in Postgres and refuses room 21. Operators can
set `SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED=false` on the API to stop all new
LiveKit admissions and dispatches without preventing existing generations from
reporting usage and closing. Apply it with the deploy
(`SARAH_LIVEKIT_ADMISSIONS=off scripts/deploy-cloudrun.sh production`) so the
state belongs to one revision, and reconcile afterwards with
`scripts/cloudrun/check-livekit-admission-drift.mjs`.

Build and test from the repository root:

```sh
pnpm --dir apps/sarah-livekit-agent typecheck
pnpm --dir apps/sarah-livekit-agent test
pnpm --dir apps/sarah-livekit-agent build
docker build -f apps/sarah-livekit-agent/Dockerfile -t sarah-livekit-agent .
```

The production image is built by Cloud Build and published only to the existing
`us-central1-docker.pkg.dev/openagentsgemini/oa-cloud` repository:

```sh
bash scripts/cloud/build-sarah-livekit-agent.sh
bash scripts/cloud/build-sarah-livekit-agent.sh --apply
```

The first command prints the immutable source-tag build plan without changing
cloud state. The second requires a clean Git worktree, submits the repository
root with the committed Cloud Build configuration, waits for success, and
prints the resulting digest-only image reference. It does not deploy the image.
Use that reference in the LiveKit manifest and deployment bundle by following
`infra/livekit/README.md`.

The production acceptance command resolves its source revision from the
converged Kubernetes Deployment image digest and the image's unique
`source-<revision>` Artifact Registry tag; it does not accept an operator-entered
revision. Its session-scoped settlement evidence requires literal
`principal.sarah`, distinct job/provider/configuration/context/capability/hold/
usage/settlement digests, one worker and provider session, nonzero exact usage,
an explicit ticketed control-channel interrupt with an audio tail no longer
than 750 ms, at least one exactly accounted cancelled response, and a settlement
charge equal to recorded provider usage. The receipt retains only booleans,
counts, timings, totals, and digests. Failure injection, reconnect, and
packaged-client privacy collection remain separate live operator actions.

The terminal-failure matrix command is non-mutating by design:

```sh
pnpm --dir apps/sarah-livekit-agent failure-matrix
```

Its default mode prints the seven required scenarios and executes no network
request. With `--apply`, a separate owner gate permits it to validate a private
production observation that remains outside the repository and to write one
public-safe receipt. It does not inject a fault. Each scenario must reconcile
exact provider usage, the reserved/charged/released hold, and terminal
settlement; show one terminal event; show at most one worker generation and one
provider session; require fresh admission; and carry zero secret, raw-media,
and transcript findings. Reconnect must begin after the previous terminal
boundary with a different generation digest and must not revive the settled
generation. The receipt contains only aggregate counts, amounts, durations,
and SHA-256 evidence projections. See the production runbook before collecting
or accepting live drill evidence.

The provider-disconnect row has a separate, deliberately narrow live control at
`POST /api/operator/sarah/livekit/provider-disconnect`. It is absent from the
effective API surface unless
`SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED=true`; the checked-in
production environment keeps it `false`. When armed for a bounded acceptance
window, the request still requires an authenticated administrator, the exact
header
`x-openagents-livekit-owner-gate: I_ACCEPT_EXACT_SARAH_PROVIDER_DISCONNECT`,
and the body acknowledgement `disconnect_exact_provider_socket`.

The body identifies one alpha-cohort `sessionRef`, its active `generation`, and
the SHA-256 `providerSessionRefDigest` that the worker durably admitted. The
control plane rejects a stale generation, a mismatched digest, a session that is
not connected, a worker that is stopping or closed, and any second directive
for the generation. It persists the directive before returning it only to that
generation's authenticated worker lease. The worker durably acknowledges the
same request and provider digest, fences the generation as
`provider_disconnect`, drains available terminal usage, closes its
`AgentSession`, and shuts down. Its provider connection has `maxRetry: 0` and
`retryIntervalMs: 0`; the directive cannot reconnect or revive the settled
generation. A later conversation requires a fresh admission and generation.

This control never changes a firewall, route, NetworkPolicy, Secret, LiveKit
server, or shared Deployment. If complete terminal response and transcription
usage reaches the control plane during the bounded drain, normal exact
settlement applies. If the socket closes before completeness can be proven, the
session becomes `accounting_uncertain`, keeps its full hold, and requires the
provider-export reconciliation procedure. The acceptance response is
`Cache-Control: no-store`; keep its session and provider identifiers in the
private drill record rather than a public receipt.

Both Docker stages pin the Linux AMD64 manifest digest for the official
`node:24.13.1-bookworm-slim` image. To refresh it, inspect the authoritative
Docker Hub manifest index with:

```sh
docker buildx imagetools inspect node:24.13.1-bookworm-slim \
  --format '{{json .Manifest}}'
```

Select the `linux/amd64` child manifest digest, update both `FROM` lines, and
rerun the worker image policy test before publishing. Do not replace the digest
with a mutable tag.

Production deploys three replicas on the dedicated GKE application pool.
External Secrets injects all credentials at runtime, the pod UID supplies a
unique `SARAH_LIVEKIT_WORKER_REF`, and the 90-second pod termination allowance
covers the process's 30-second worker drain and 35-second child shutdown bound
with additional time for final provider accounting and the durable close event.
