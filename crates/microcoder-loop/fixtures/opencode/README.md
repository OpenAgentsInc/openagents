# OpenCode error fixtures

`model-access-disabled.error.json` is the `error` of a real `opencode run
--format json` turn on OpenCode 1.18.26, captured on 2026-09-28: the OpenCode
Zen provider refused the model with HTTP 403. It is not a capacity refusal.

`rate-limited.error.json` is the same recorded error with its status set to
429, its message and body set to a rate limit, and a `retry-after` header
added. No live 429 was captured: OpenCode retries a 429 itself, honoring
`retry-after`, and reports it only after five attempts.
