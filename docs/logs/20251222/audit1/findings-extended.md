# Extended Findings

## High
- E-H-1 Compute identity bootstrapping can generate a brand-new identity even when an encrypted identity exists but no password is provided, which risks orphaning funds and confusing users. Evidence: `crates/compute/src/app.rs:112`, `crates/compute/src/app.rs:137`, `crates/compute/src/app.rs:143`.
- E-H-2 Compute stores plaintext mnemonics on disk (`.seed`) without permission hardening or automatic deletion, creating a high-risk secret exposure path if the directory is backed up or world-readable. Evidence: `crates/compute/src/storage/secure_store.rs:92`, `crates/compute/src/storage/secure_store.rs:102`, `crates/compute/src/storage/secure_store.rs:121`.
- E-H-3 AgentGit uses repository identifiers from the HTTP path directly in filesystem joins, enabling path traversal (e.g., `../`) to read/write outside the workspace, including clone targets. Evidence: `crates/agentgit/src/server.rs:1725`, `crates/agentgit/src/git/clone.rs:109`.

## Medium
- E-M-1 DVM startup relies on relay subscriptions that are permanently unimplemented; `RelayService::subscribe_job_requests` always errors, so `DvmService::start` cannot succeed in real use. Evidence: `crates/compute/src/services/dvm_service.rs:120`, `crates/compute/src/services/relay_service.rs:81`.
- E-M-2 Relay connection logic is a stub that marks relays "connected" without network I/O, which can mask connectivity failures and mislead higher-level services. Evidence: `crates/compute/src/services/relay_service.rs:61`.
- E-M-3 Ollama integration is explicitly disabled and always returns NotAvailable, so compute jobs can never execute locally; this breaks NIP-90 DVM flows. Evidence: `crates/compute/src/services/ollama_service.rs:1`.
- E-M-4 Nostr relay queue task dequeues events but never sends them, logging "needs manual retry"; queued events can be dropped or never delivered while connected. Evidence: `crates/nostr/client/src/relay.rs:631`, `crates/nostr/client/src/relay.rs:656`.
- E-M-5 OutboxModel uses std::sync::RwLock in async-facing code paths, which can block async tasks and panic on poisoning; prefer tokio::sync::RwLock. Evidence: `crates/nostr/client/src/outbox.rs:16`, `crates/nostr/client/src/outbox.rs:96`.
- E-M-6 Marketplace payments/pricing/skill install modules are empty placeholders, leaving core marketplace workflows unimplemented. Evidence: `crates/marketplace/src/core/payments.rs:1`, `crates/marketplace/src/compute/pricing.rs:1`, `crates/marketplace/src/skills/install.rs:1`.

## Low
- E-L-1 Recorder parsing reads entire log files into memory, which can blow memory for large sessions; streaming would be safer. Evidence: `crates/recorder/src/lib.rs:355`.
- E-L-2 SecureStore uses Argon2::default without persisting parameters and relies on a custom base64 implementation; future parameter changes or encoding bugs can make stored data unreadable. Evidence: `crates/compute/src/storage/secure_store.rs:221`, `crates/compute/src/storage/secure_store.rs:261`.
