# GPT-OSS E2E Test Added - 2025-12-25

## Summary
- Added an ignored end-to-end test for a real GPT-OSS server.
- Updated d-019 directive + status docs to reflect E2E coverage.

## Notes
- Test uses `GPT_OSS_E2E_URL` or `GPT_OSS_URL` and `GPT_OSS_E2E_MODEL`.
- Run with: `cargo test -p gpt-oss --test real_server_e2e -- --ignored`
