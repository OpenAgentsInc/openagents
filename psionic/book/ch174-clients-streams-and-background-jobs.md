# 174. Clients, streams and background jobs

> **Status: unimplemented — design draft.** Nothing here runs yet.

A served model needs clients that do more than one request and one answer. This chapter adds streamed tokens and audio, reused sessions, cancellation that reaches the scheduler, and background jobs that finish after the caller disconnects.

It also adds an OpenAI-compatible Responses edge. That edge translates into the System One contract. The System One contract never bends to match the edge.

## What this chapter covers

- **Bidirectional streams.** One connection carries tokens out and audio in or out. Each frame has a sequence number, and a stream that ends without a finish event is reported incomplete, as in [chapter 89](ch089-deepseeks-front-door.md).
- **Session reuse.** A client keeps one connection and one authenticated session across requests, and the server keeps that session's resident state.
- **Cancellation that reaches the scheduler.** A client cancel removes the request from [chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md)'s batch on the next step, not after the stream drains.
- **Background jobs.** The server acknowledges a job at once and returns an identifier. The job runs under its own deadline, and the client fetches the result later or receives it on delivery.
- **A Responses edge.** Requests in the Responses shape from [chapter 79](ch079-serve-responses.md) translate into System One requests. Fields with no System One meaning get an explicit unsupported-feature error.
- **Client code.** A small reference client shows session reuse, asynchronous jobs and streaming, following section 7.5 of *Inference Engineering*.

## What it builds

- Planned stream framing and job records beside the [chapter 173](ch173-serve-a-model-with-an-honest-capability.md) schemas in `proto/`.
- A reference client for tests and for the harness of Part XXI.

## Dependencies

- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md): the server, its schemas and its cancellation contract.
- [Chapter 79](ch079-serve-responses.md): the Responses-native HTTP and semantic SSE profile.
- [Chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md): the scheduler that cancellation must reach.
- Voice integration uses `verse/platform/voice.h`, as in [chapter 154](ch154-transcribe-while-someone-speaks.md).

## Done means

- A cancelled stream frees its batch slot within one scheduler step, checked with an instrumented scheduler.
- A background job completes after its client disconnects and returns the same answer as the same request run synchronously.
- A stream truncated at every frame boundary is reported incomplete, never as a finished answer.
- A Responses request and its translated System One request give the same answer, and each unsupported field gets its named error.
- Session reuse lowers per-request overhead by a measured amount in [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s benchmark.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
