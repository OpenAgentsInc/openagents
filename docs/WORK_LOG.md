# Work Log

## 2025-01-15
- Installed the Rustls ring crypto provider at startup to avoid the ambiguous provider panic when both ring and aws-lc features are present.
- Disabled provider startup when no inference backends are available so the local Codex bridge can run in host-only mode.
