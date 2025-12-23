# Deeper Findings (Pass 7)

## High
- D7-H-1 Compute relay integration is stubbed: `RelayService::connect` only marks relays as connected and both `subscribe_job_requests` and `publish` always return errors, so `DvmService::start` fails and the compute provider cannot go online. Evidence: `crates/compute/src/services/relay_service.rs:62`, `crates/compute/src/services/relay_service.rs:81`, `crates/compute/src/services/dvm_service.rs:120`.

## Medium
- D7-M-1 Ollama integration is stubbed and always reports unavailable, so DVM inference will never run even if relay wiring existed. Evidence: `crates/compute/src/services/ollama_service.rs:1`, `crates/compute/src/services/ollama_service.rs:47`.
- D7-M-2 Marketplace compute consumer job submission is unimplemented (`submit_job` always errors), so CLI/consumer-side compute requests cannot be sent. Evidence: `crates/marketplace/src/compute/consumer.rs:258`.
- D7-M-3 Marketplace skill and dataset discovery are unimplemented (`SkillBrowser` and `DatasetBrowser` always error), so browse/search/get flows never return results. Evidence: `crates/marketplace/src/skills/browse.rs:240`, `crates/marketplace/src/data/discover.rs:281`.
- D7-M-4 Compute app falls through to generating a new identity when an encrypted identity exists but no password is provided, silently switching pubkeys and storing a plaintext seed without user confirmation. Evidence: `crates/compute/src/app.rs:112`, `crates/compute/src/app.rs:143`.

## Low
- D7-L-1 Autopilot GUI permissions handlers open `PermissionStorage` with a hardcoded path on every request instead of using shared app state, adding redundant file opens and bypassing configuration. Evidence: `crates/autopilot-gui/src/server/routes.rs:245`, `crates/autopilot-gui/src/server/routes.rs:281`.
