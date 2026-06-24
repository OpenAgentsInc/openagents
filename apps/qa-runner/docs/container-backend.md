# Containerized execution backend

The qa-runner's isolation backends live behind one abstraction (`src/backend.ts`).
The OSS default is `localBackend` (real chromium on this host). For
cross-environment / isolated execution, the seam grows into a typed cross-OS
Cloud-VM provisioner (`CloudVmProvisionerV2` / `CloudVmHandle`) whose **production**
implementation (firecracker / sek8s microVMs) lives owner-gated in the `cloud`
repo.

The **container backend** (`src/container-backend.ts`) is the real, in-repo,
locally-runnable analogue of a cloud VM. It exercises the *same* lifecycle —
provision → exec → teardown → artifact-extraction — against a container engine
(Docker), so the seam is proven end-to-end **without faking a green** and without
requiring the cloud provisioner to be wired.

## Lifecycle

```
provision  start a container from an image (default a headless-browser image);
           the container stays up via a long-lived no-op entrypoint so we can
           exec into it — mirroring a microVM staying up between provision and
           teardown.
exec       run a qa session INSIDE the container. The session writes its
           result.json + artifacts (video / trace / screenshots) to a known
           in-container workdir (/qa/artifacts).
extract    `docker cp <id>:/qa/artifacts/.` copies the artifact dir back OUT to
           the host run dir, so the result + video + trace are dereferenceable
           with NO access to the container.
teardown   stop + remove the container (runs even if exec/extract threw — a
           container is never leaked).
```

Two entry points:

- `runContainerSession(input, options)` — the full provision→exec→extract→teardown
  one-shot. Resolves with the container id, the exec transcript, and the host
  dir the artifacts were extracted into.
- `provisionContainerVm(input, options)` — returns a `CloudVmHandle`
  (`exec` / `teardown`) over the container engine, the bridge between the
  owner-gated cloud seam and a runnable local backend. `acquireBrowser` is
  intentionally **not** supported over this handle (in-container browser
  acquisition over a tunnel is the cloud provisioner's job) and throws rather
  than faking an in-container browser.

## Owner-gated / armed by env (default OFF)

A real isolated execution backend does not turn itself on. The container backend
is **INERT** unless explicitly armed:

- env: `QA_CONTAINER_BACKEND=1` (or `true`), or
- code: `{ armed: true }`.

Un-armed → `ContainerBackendNotArmedError` (the engine is never even touched).

## Honest about Docker

When **armed but Docker is not available** on the host (binary missing or daemon
unreachable), the backend throws `ContainerEngineUnavailableError`. It **never
silently falls back to local** and **never fakes a result**. `available()` on the
real runtime probes `docker info` and reports the true host state.

## Deterministic in CI

The container engine is injected (`ContainerRuntime`, `src/container-runtime.ts`).
Unit tests (`src/container-backend.test.ts`) pass a **fake runtime** that records
the lifecycle and materializes a synthetic artifact set on `copyOut`, proving the
provision/exec/extract/teardown contract plus the armed / un-armed / Docker-absent
branches with **no Docker and no network**. When Docker *is* present, one test
runs a real `alpine:3` container session end-to-end (otherwise it skip-lives and
says so).

## Artifact contract is unchanged

The extracted `result.json` is the same public-safe `QaRunResult`
(`backend = "container"`) the browser/terminal runners emit, so the
brain/target/artifact contracts are unchanged. The public-safety tripwire
(`assertPublicSafeResult`) still applies.

## Relationship to the production cloud provisioner

This is the *local analogue*. The production cross-OS (Linux/macOS/Windows)
provisioner — firecracker microVMs / sek8s confidential runners via
`oa-node` / `oa-workroomd` — lands owner-gated in the `cloud` repo and implements
the same `CloudVmProvisionerV2` / `CloudVmHandle` contract from `src/backend.ts`.
See the follow-up issues referenced from #6186.
