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

## 2026-01-27
- Fixed Adjutant plan mode Send issues by releasing session locks before awaits and running the plan pipeline in a blocking task.
- Updated plan mode metric test data to satisfy substantive-topic scoring.

## 2026-01-28
- Surface optimized plan-mode instructions in the Adjutant signature UI panels.
- Refined plan-mode documentation to match the current pipeline and optimization artifacts.

## 2026-01-29
- Moved Autopilot Desktop backend modules into `apps/autopilot-desktop/src-tauri/src/` and removed the backend crate.
- Extracted AI Gateway server management into the new `crates/ai-server/` crate and updated app wiring.

## 2026-01-30
- Moved DSPy planning/execution/review in Autopilot Core onto a dedicated runtime helper to avoid `block_in_place`.
