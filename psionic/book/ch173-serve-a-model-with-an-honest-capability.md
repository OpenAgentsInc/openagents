# 173. Serve a model with an honest capability

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter turns the loopback server of [chapter 20](ch020-serve-systemone.md) into a service that states exactly what it can do. A client reads a versioned manifest and learns which model, backend and numerical profile answer each request, and what evidence backs that profile.

You build the shared System One protocol integration here. Every later serving chapter in Part XIX extends this server, so its contracts come first: request and response schemas, bounds, cancellation and the capability manifest.

## What this chapter covers

- **Schemas in `proto/`.** Request and response messages start as files in `proto/`. The JSON form of `openagents.systemone.v1` and the protobuf form of [chapter 21](ch021-the-protobuf-wire.md) are projections of the same schema.
- **A versioned capability manifest.** One entry per model, backend and profile. Each entry names the authenticated checkpoint digest from [chapter 7](ch007-reading-original-weights.md), the backend (CPU, CUDA or Metal), the numerical profile and the qualification record that supports it.
- **Bounded requests.** Body size, token counts, question counts and deadlines have stated limits. A request outside a limit gets a typed refusal before any model work starts.
- **Cancellation.** A cancelled or expired request stops its model work and releases its arena. The server never publishes a partial answer.
- **Evidence per entry.** An entry without a qualification record reads as unknown, under [chapter 17](ch017-qualify-honestly.md)'s rules. The manifest never claims a backend that the host did not qualify.
- **Coax decides, the host does effects.** Admission, refusal and the manifest are Coax procedures. The host handles sockets and processes, as the [Coaxial dev server](../../coax/web.md) does for the website.

## What it builds

- A planned `psionic/lib/serve/manifest.coax` beside the existing `serve-engine.coax` and `serve-request.coax`.
- Schema files for the manifest and the System One request and answer in `proto/`.

## Dependencies

- [Chapter 20](ch020-serve-systemone.md): the loopback `/v1/systemone` server, bounded NIP-DEC bodies and typed refusals.
- [Chapter 21](ch021-the-protobuf-wire.md): the protobuf projection of the same contract.
- [Chapter 22](ch022-run-it-live.md): golden warm-up, versioned health and restart supervision.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the service benchmark that measures every later change.
- A qualified Linux target and asynchronous I/O with deadlines in Coax, which the [Coax roadmap](../../coax/ROADMAP.md) lists as remaining work.

## Done means

- Every manifest entry resolves to a checkpoint digest, a backend, a profile and a qualification record. A test removes a record and the entry reads as unknown.
- A request outside each declared bound gets the documented refusal, and no model work runs.
- A cancelled request releases its arena within a stated time, measured by [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s harness.
- The JSON and protobuf forms of one request produce byte-identical answers.
- Answers carry the same served model identity that the manifest advertises.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [NIP-DEC](../../nips/openagents/NIP-DEC.md)
