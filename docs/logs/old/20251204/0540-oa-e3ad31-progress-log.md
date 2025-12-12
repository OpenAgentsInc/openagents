# 0540 Work Log

Progress oa-e3ad31: planning.

- Surveyed provider clients: OpenRouter, Anthropic, OpenAI, Gemini lack retries.
- Plan: add shared retry/backoff helper (pi-mono style defaults), extend ChatRequest with retry overrides, wrap provider calls with HttpError + retryable classification, add tests for helper.
