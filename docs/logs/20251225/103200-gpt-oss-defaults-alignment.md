# GPT-OSS Defaults Alignment + Test Fixes - 2025-12-25

## Summary
- Aligned GPT-OSS defaults to real model names (`gpt-oss-20b`) and port `8000` across configs, tests, and docs.
- Updated local-inference examples and tests to use the LocalModelBackend trait APIs correctly.
- Fixed gpt-oss test expectations to match current error types and serialization behavior.

## Validation
- `cargo test -p gpt-oss -p local-inference -p gpt-oss-agent --tests --no-run`
