# Dev Team Slack Memo — GitBook v0.1.13 Verification Asks

> Drop this in `#dev` (or wherever you prefer). It's a list of items I couldn't verify locally without a v0.1.13 binary or release-manifest access. Items are ordered by blast radius — #1 is the only one that blocks merge of the GitBook PR.

---

Hey team — I just landed the parity-review fixes and the developer-pathway fills on `gitbook/vegas-btc-2026` ([PR #4428](https://github.com/OpenAgentsInc/openagents/pull/4428)). Most of the review-flagged drift is now repaired. A few items need someone with binary/release access to confirm so the GitBook ships honest. Numbered by priority:

**1. CRITICAL — Canonical v0.1.13 CLI surface.**
The User Pathway documents `cargo pylon` (no init/online/serve subcommands — bootstraps everything). My new `developers/quickstart.md` matches that. The previous quickstart stub used `pylon init` / `pylon config set …` / `pylon serve --online`, which contradicts the user-facing surface. Before merge can someone confirm: is the canonical v0.1.13 surface `cargo pylon` (with cargo aliases in `.cargo/config.toml`), or is there still a `pylon init/config/serve` lane I should document for an audience that wants explicit lifecycle control? I assumed the former based on the proof receipts; flag if that's wrong.

**2. Cargo aliases in `.cargo/config.toml`.**
I documented three aliases: `cargo pylon`, `cargo pylon-tui`, `cargo pylon-headless`. Confirm they exist exactly as named at the v0.1.13 tag, and that `cargo pylon-headless online` is a real subcommand (per the headless / agentic section).

**3. Literal log strings.**
The Quickstart and Go-online pages say to watch for these three lines in this order:

```
identity loaded
provider presence published
intake online
```

Confirm these are the exact literals emitted by `cargo pylon` at v0.1.13. If they've drifted, point me at the right strings and I'll patch.

**4. Line ranges for `crates/nostr/core/src/identity.rs`.**
Two pages cite ranges:
- `users/troubleshooting.md`: `identity.rs:55-60` (refusal-to-load on empty mnemonic)
- `users/sovereignty.md` (linked indirectly): `identity.rs:44-46`

Confirm these are still correct at the v0.1.13 tag; the audit-cited line ranges have drifted before.

**5. Credentials vault metadata file extension.**
`docs/CREDENTIALS.md` was updated last month — confirm the metadata file is `autopilot-credentials-v1.conf` (what I have in mind) versus a `.toml` / `.json` extension. None of my pages cite the literal filename today; raising this so future fills don't miscite it.

**6. SHA-256 hashes for non-darwin-arm64 binaries.**
Quickstart documents the published darwin-arm64 SHA: `de995efc90675d90108785a2790e0c2bc4099cd0ef6eaff2d8ae58fccc234a66`. If we have hashes for darwin-x86_64, linux-x86_64, linux-arm64, win-x86_64, drop them and I'll add a hash table. (Alternative: I can drop the table entirely and just point at the release manifest — your call.)

**7. Payout id receipt artifact path.**
The earn-loop receipt id `019db8a2-98d2-7890-95e4-6a1d78709a3c` shows up in three places in the GitBook. Where is the canonical artifact (signed delivery bundle event, kernel state dump, a `docs/reports/` file)? I'd like to link to it directly from `developers/quickstart.md` step 5.

**8. NIP-99 / NIP-90 bounty mechanics.**
`developers/bounties.md` says the current settlement lane is direct Lightning, with NIP-99 classifieds + NIP-90 fulfillment as the "as those NIPs come fully online" target. Confirm that's the right framing, or correct it. The bounty page is otherwise written from the OAPN #6 framing plus what's in the repo.

**9. Repo path before merge.**
The GitBook draft is on `OV1-Kenobi/openagents:gitbook/vegas-btc-2026` and rendered by [PR #4428](https://github.com/OpenAgentsInc/openagents/pull/4428). I called this out in `developers/README.md` so external readers aren't confused. Confirm: do we land the GitBook on `OpenAgentsInc/openagents` before the Vegas event, or stay on the fork through Vegas? The "repo path" line in `developers/README.md` will need a one-line edit either way.

**10. WGPUI Go Online pane simulation status.**
I documented the v0.1 desktop Go Online pane as a UI simulation (not yet wired to a live Pylon backend); the live earning lane is the packaged-app `autopilotctl` surface plus `cargo pylon`. This shows up in `users/README.md`, `users/go-online.md`, and `users/troubleshooting.md`. If that mischaracterizes where the desktop pane is at v0.1.13, flag it — I'd rather correct it now than have the docs lie about parity.

---

Replies in thread please. Anything I don't hear back on by `[your deadline]`, I'll leave as-is in the GitBook and we can patch post-Vegas.

— David
