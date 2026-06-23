# Khala Modality-Specific Cloud Primitive Contracts

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6094

Status: P2 study. No production serving, routing, WebSocket, batch-worker,
image/video worker, billing, or product-promise state changed.

## Recommendation

Do not reuse the Khala chat telemetry contract as the default contract for every
future Agent Cloud primitive.

Khala chat is optimized around an OpenAI-compatible request/response or
streaming text completion. Its most visible metrics are TTFT, inter-token
latency, perceived TPS, total wall-clock time, token counts, route, verification,
and cost. Embeddings, live voice, image generation, and video generation have
different bottlenecks and different customer-visible outcomes. A future primitive
may share account, balance, receipt, referral, and settlement rails, but each
primitive must define its own request shape, receipt fields, scaling lane, and
product-promise gate before public copy can claim it is live.

This keeps the Agent Cloud honest: one balance and one receipt spine, but not one
metric vocabulary pretending to fit every modality.

## Shared Contract Rules

Every Cloud primitive should keep these shared fields because they are the
economic and authority spine:

- `schemaVersion`;
- `primitive`;
- request id and dereferenceable public-safe receipt ref;
- account/referrer/public product refs where safe;
- route, provider, served model or engine, version, region, and fallback reason;
- request class and transport class;
- public-safe status: `accepted`, `queued`, `running`, `completed`, `failed`,
  `rejected`, or `not_measured`;
- cost basis, price, margin bucket, settlement state, and blocker refs;
- product-promise ids and evidence refs for any public claim.

Each primitive must also keep the existing privacy discipline:

- no raw prompts, documents, transcripts, audio, images, videos, private repo
  material, invoices, preimages, wallet material, or provider credentials in
  public receipts;
- no `0` as a stand-in for unknown values;
- `not_measured` is explicit and carries a blocker ref when the field matters;
- public projections expose neutral classifiers, dimensions, durations, counts,
  hashes, and dereferenceable refs only.

## Primitive Matrix

| Primitive | Request Shape | Primary User-Visible Outcome | Scaling Lane | Do Not Inherit From Chat |
| --- | --- | --- | --- | --- |
| Chat / coding completion | Interactive request/response or streaming text | Accepted answer or verified coding artifact | Decode-heavy LLM serving, prefix cache, verifier path | This is the baseline only for text completion. |
| Embeddings / bulk documents | Async or batch-first document set | Vector set and corpus/job receipt | Batch queues, document chunkers, embedding workers, storage/index writers | TTFT, inter-token latency, perceived TPS. |
| Live voice ASR/TTS/session | Bidirectional streaming or durable WebSocket session | Transcript, synthesized audio, turn latency, approval/action receipt | Low-latency streaming workers, session state, audio buffers, VAD/endpointing | Token TPS as the primary metric. |
| Image generation/editing | Job or interactive artifact request | Image artifact plus safety/watermark/seed receipt | Compute-bound accelerator lane, artifact storage, safety checker | Chat TTFT/TPS and text verifier semantics. |
| Video generation/editing | Async job with progress and artifact closeout | Video artifact, duration, frames, codec, render receipt | Long-running accelerator lane, queue, checkpoint/progress, storage | Interactive edge deadline and token metrics. |

## Embeddings And Bulk Documents

Request shape:

- batch or async job submission;
- multiple documents or chunks per job;
- optional corpus/ref namespace;
- optional index/store target;
- explicit retention and deletion policy refs.

Distinct receipt and metric fields:

- input document count and chunk count;
- total input bytes and tokenized input count when measured;
- embedding model, dimension, dtype, and normalization mode;
- chunking policy ref and overlap/window parameters;
- accepted chunk count, rejected chunk count, and rejection reasons;
- queue wait, processing time, index write time, and total closeout time;
- storage/index target ref, index-write status, and replayable manifest hash;
- privacy/redaction policy ref and deletion/export ref when the data belongs to
  a customer workspace;
- cost per document, cost per chunk, cost per vector, and total job cost.

Scaling lane:

- queue-first, not edge-blocking;
- high-throughput batch workers;
- scale by document volume, embedding model throughput, index write pressure,
  and storage throughput;
- scale-to-zero is acceptable for dev or predictable offline jobs when the job
  receipt is honest about queue wait and cold start.

Product-promise gate:

- no green embedding, corpus, or retrieval promise until a paid batch receipt
  proves submission, chunking, embedding, index/write closeout, cost, and
  retention/deletion policy for the exact promised scope.

## Live Voice

Live voice covers ASR, TTS, and bidirectional voice sessions.

Request shape:

- bidirectional streaming or hibernatable WebSocket session;
- audio frames in, transcript / model events / synthesized audio frames out;
- explicit session start, turn, interruption, endpoint, and closeout events;
- approval/action receipts separate from raw transcript capture.

Distinct receipt and metric fields:

- transport: WebSocket, WebRTC bridge, or provider stream;
- codec, sample rate, channel count, and audio duration;
- first-audio-in to first-partial-transcript latency;
- endpointing/VAD latency;
- user turn duration and assistant turn duration;
- interruption/barge-in count and recovery status;
- ASR model, TTS voice/model, and session model/route;
- transcript token count only as a secondary field;
- synthesized audio duration, bytes, and first-audio-out latency;
- approval/action refs when a voice command triggers work;
- privacy boundary: transcript retention, redaction, and user consent refs.

Scaling lane:

- session-oriented low-latency workers;
- durable connection management rather than plain request/response;
- autoscale on active sessions, audio seconds per second, buffer pressure,
  endpointing latency, and provider stream health;
- isolate voice from chat decode saturation so text batch pressure cannot starve
  live audio.

Product-promise gate:

- no green voice companion, voice approval, ASR, or TTS promise until a paid
  session receipt proves the live transport, transcript/audio closeout, user
  consent/retention boundary, and any approval/action side effect.

## Image And Video

Image and video are artifact-generation primitives. They may accept text prompts
or reference assets, but their product is not a chat answer.

Request shape:

- image: interactive job or short async job with one or more generated/edited
  image artifacts;
- video: async job with progress, frames/segments, and terminal artifact
  closeout;
- optional reference asset refs, masks, storyboard refs, or seed controls;
- explicit safety/copyright/watermark policy refs where applicable.

Distinct receipt and metric fields:

- modality: `image`, `image_edit`, `video`, or `video_edit`;
- artifact count and artifact refs;
- requested and produced dimensions;
- output format, codec/container for video, frame count, fps, and duration;
- seed, sampler/scheduler, step count, guidance/strength when the provider
  exposes them and they are safe to disclose;
- model/engine/version, accelerator class, and warm/cold state;
- queue wait, render time, safety-check time, upload/storage time, and total
  closeout time;
- safety classifier result, blocked/rejected reason, and human-review ref when
  applicable;
- cost per artifact, cost per megapixel, cost per generated second, and total
  job cost;
- retry count, partial-artifact status, and expiration/lifecycle policy.

Scaling lane:

- compute-bound accelerator workers scaled independently from LLM decode-heavy
  lanes;
- image lanes scale on concurrent renders, accelerator utilization, and queue
  depth;
- video lanes scale on queued render seconds, checkpoint/progress throughput,
  artifact storage pressure, and long-job reliability;
- do not let image/video queues compete with chat TTFT capacity unless a shared
  provider route has explicit priority/cap controls.

Product-promise gate:

- no green image or video primitive promise until a paid artifact receipt proves
  accepted request, produced artifact, safety policy, storage/dereference path,
  cost, and any public claim about quality, duration, or delivery time.

## Public Copy And Promise Discipline

The product-promise rule is per modality:

- A primitive can be documented as planned or experimental before it is paid.
- A primitive can be yellow only when a scoped lane exists and blockers are
  named.
- A primitive turns green only for the exact scope proven by paid,
  dereferenceable receipts and any required owner/legal/safety review.
- A green Khala chat or coding receipt does not make embeddings, voice,
  image, or video green.
- A free/internal smoke may support readiness, but it does not prove the paid
  customer promise unless the promise explicitly allows that narrower scope.

The shared balance, referral, and settlement rail can be reused across the Agent
Cloud, but public claims remain primitive-specific. Each primitive should point
to its own product-promise id and evidence refs rather than hiding behind a broad
"Cloud primitives" statement.

## Closeout

#6094 is resolved by this study because it defines, for embeddings/bulk
documents, live voice, and image/video:

- request shape;
- distinct receipt and metric fields;
- scaling lane;
- the explicit rule that chat TTFT/TPS semantics do not automatically apply;
- the paid-receipt gate required before any per-modality product promise turns
  green.

No production serving is required to close this issue.
