# Coder attempt observations v2

`proto/openagents/coder/observability/v2/events.proto` declares the new
`kind: "attempt"`, `schema: "coder.observability.v2"` event. Existing v1
events and snapshots retain their identity. This event records one finished
Microcoder request attempt, including fallback failures and skipped attempts;
it is neither a cumulative snapshot nor an invoice.

`scope` is `request_attempt`. `gateway` and `model` identify the configured
attempt, with `model_source: "attempt_configuration"`. They do not establish
which underlying provider or alias actually served it. `requested_effort`
records the request setting, or null; it is not an observed effective effort.
`engine` is filled by Coder as `micro` or `psionic`. The worker emits null
until that session context is attached. Outcome and HTTP status describe the
gateway attempt, not task acceptance.

Token keys are `input_tokens`, `output_tokens`, `cached_tokens`,
`reasoning_tokens` and `cache_write_tokens`. Missing or invalid counts are
null, including cache details omitted by the provider. Explicit zero remains
zero. Counts follow the existing gateway normalization: Anthropic input
includes cache reads and writes, Gemini output adds reported thought tokens,
and Chat Completions can derive output from total minus prompt. Cache and
reasoning counts are subsets; do not add them to input/output totals.

`provider_cost_usd_micros` contains a provider-reported amount only, otherwise
null. `card_estimate_usd_micros` is the existing static rate-card estimate,
otherwise null. It may be stale and is not a provider charge. The worker's
legacy `usd_micros` field remains the card estimate for existing status bars.
No prices, payment decisions or arithmetic were changed by this logging path.

The session writer accepts the exact version and rebuilds a fixed whitelist
of metadata fields. It drops prompts, responses, headers and arbitrary extra
payloads. Normal session credential redaction still applies. Old workers
without this schema can still drive the UI, but cannot manufacture v2 evidence.
Provider totals do not account for unreported retries hidden inside a gateway.
