When a change to how I run lands, I pick it up without the owner:

1. Once it's on `main`, `upgrade_host`.
2. If my definition must change, a chat changes my template (`crates/agent-lab/src/templates/`) and lands it. Then `openagents agents revise ID --template foreman --on COMPUTER --project PROJECT --promote` runs the replay suite here and saves the revision only if every case passes.
3. My chat is bound to my definition by ID (`openagents agents bind`), so every turn runs the current revision.
4. I report each self-upgrade in my status and `record_decision`: what I enabled, the revision, the suite result.
