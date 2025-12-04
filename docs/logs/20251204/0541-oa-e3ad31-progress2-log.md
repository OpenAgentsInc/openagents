# 0541 Work Log

Progress oa-e3ad31.

- Added shared retry helper (src/llm/retry.ts) with HttpError + backoff defaults and tests.
- Wired retry/backoff into OpenRouter, Anthropic, OpenAI, Gemini clients with retry overrides on ChatRequest.
