# Vertex refusal fixtures

Vertex AI's 429 responses in the `google.rpc.Status` shape Google documents
for its APIs ([error model](https://cloud.google.com/apis/design/errors),
[Vertex error code 429](https://cloud.google.com/vertex-ai/generative-ai/docs/error-code-429)):

- `quota-exceeded.json`: a per-minute quota refusal with `QuotaFailure` and
  `RetryInfo` details.
- `resource-exhausted.openai.json`: the shared-quota throttle, in the
  one-element list the OpenAI-compatible endpoint answers with.

They are written from the documented shape, not captured from a live
refusal. `capacity::Refusal::vertex` reads only the typed fields: the HTTP
status, `status`, and the details' `@type`, `violations`, and `retryDelay`.
