# Work Log

## 2025-01-15
- Installed the Rustls ring crypto provider at startup to avoid the ambiguous provider panic when both ring and aws-lc features are present.
- Disabled provider startup when no inference backends are available so the local Codex bridge can run in host-only mode.

## 2025-01-16
- Constrained the local bridge TLS cert validity window and added validity checks so stale long-lived certs are regenerated.
- Prefer Herd TLS certs (or env-provided cert/key) for the local bridge to unblock Chrome TLS handshakes.

## 2026-01-26
- Added plan mode training storage and optimization loop for Autopilot Desktop signatures, including benchmark logging and manifest persistence.
- Wired plan mode pipeline to record valid signature outputs, apply optimized instructions, and trigger background optimization runs.
