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
- `SARAH_LIVEKIT_CONTROL_ROOT`, the high-entropy root used to authenticate
  deterministic generation control credentials.

The API supplies the generation-bound control token only inside explicit
dispatch metadata. The worker sends it only as a bearer credential to the
claim/event routes. It does not record media, retain transcripts, or log raw
OpenAI events.

The OpenAI plugin is pinned and patched to attach the generation's hashed owner
identifier as `OpenAI-Safety-Identifier`. Response usage comes only from
`response.done.response.usage`; input-transcription usage is recorded
separately from the transcription-completed event.

Private and community jobs instantiate separate capability profiles. Community
jobs receive no owner memory, workspace, editor proposal, payment,
administration, shell, Git, or credential capability. The private editor tool
returns a confirmation-required proposal and cannot execute it.

New room admission is serialized in Postgres and refuses room 21. Operators can
set `SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED=false` on the API to stop all new
LiveKit admissions and dispatches without preventing existing generations from
reporting usage and closing.

Build and test from the repository root:

```sh
pnpm --dir apps/sarah-livekit-agent typecheck
pnpm --dir apps/sarah-livekit-agent test
pnpm --dir apps/sarah-livekit-agent build
docker build -f apps/sarah-livekit-agent/Dockerfile -t sarah-livekit-agent .
```

Production deploys three replicas on the dedicated GKE application pool.
External Secrets injects all credentials at runtime, the pod UID supplies a
unique `SARAH_LIVEKIT_WORKER_REF`, and the 60-second pod termination allowance
covers the process's 30-second worker drain and 35-second child shutdown bound.
