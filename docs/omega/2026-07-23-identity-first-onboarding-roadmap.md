# Omega identity-first onboarding roadmap

- Class: owner-accepted work-packet ledger
- Date: 2026-07-23
- Revised: 2026-07-24
- Revision: 2
- Status: active plan
- Product: Omega
- Client repository: `OpenAgentsInc/omega`
- Shared contract repository: `OpenAgentsInc/openagents`
- Owner authority: the 2026-07-23 Omega onboarding direction
- OpenAgents base: `ae40fdad85492cb922bc6c11bc9252a728561df4`
- Omega source pin: `30c80504403b7dcb10c7d0a476577014ebc871f6`
- Buzz onboarding study pin: `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`

## 1. Owner outcome

Omega will show a new identity setup before any inherited Zed setup.
The user can create or recover a sovereign identity.
Omega will explain what the identity can and cannot prove.
The user can complete this work without a network connection.

The inherited Zed setup will remain in Omega.
It will become Editor Onboarding.
The user can start it after identity setup.
The user can also reopen it later.

The new journey will use Omega branding and native GPUI components.
It will use the current Zed onboarding structure as its base.
It will preserve the current theme selector exactly.
It will preserve the current agent setup section for later expansion.
It will not port the Buzz React or Tauri user interface.

This roadmap starts the rebrand with a new product surface.
It does not use a repository-wide string replacement as the implementation method.
When a changed journey exposes Zed text, that packet must remove the text.
A release scan remains a final verification gate.

## 2. First value

The first value is not a completed editor preference form.
The first value is a durable local identity that the user controls.

The first successful journey gives the user these facts:

1. Omega has one public person identity for this installation.
2. Omega stores the signing secret in an approved local custody service.
3. The public identity is safe to share.
4. The secret is not a password and has no password-reset path.
5. A signature proves that the key signed exact bytes.
6. A signature does not prove permission, truth, payment, or hosted account ownership.
7. The user can add an OpenAgents account later.

The proposed first-run message is:

> Welcome to Omega. Create an identity that stays with your work.

The supporting text must explain local custody and recovery.
It must not present Nostr terms before the user asks for technical details.

## 3. Current implementation truth

The current Omega source still uses the inherited Zed first-open path.
`FIRST_OPEN` is one key-value flag in `crates/onboarding/src/onboarding.rs`.
Omega writes the flag when it opens the current onboarding page.
It does not wait for successful completion.

The current launch paths are also not consistent.
A restored workspace can bypass the first-open check.
A path-open request can bypass the first-open check.
The `AlwaysNew` path has a second copy of the check.

The current onboarding page is one editor setup page.
It includes theme, keymap, agent, import, Vim, trust, and telemetry controls.
It has no cryptographic identity step.
It can split and serialize as a workspace item.

The screen shell is `Onboarding::render` in
`crates/onboarding/src/onboarding.rs`.
It provides the centered 780-pixel column, header, Finish action, scroll state,
focus traversal, divider, and page body.

The body composition is `render_basics_page` in
`crates/onboarding/src/basics_page.rs`.
Its section order is:

1. Theme.
2. Base keymap.
3. Agent Setup.
4. Editor settings import.
5. Vim mode.
6. Project trust.
7. Telemetry.

The theme section is already a complete live control.
It supports Light, Dark, and System appearance.
It shows One, Ayu, and Gruvbox preview families.
It writes the selected mode and theme to the settings file.
System mode stores the matched light and dark family.

The Agent Setup section uses the shared `AgentSetupButton`.
It reads installed agents from `AllAgentServersSettings`.
It offers `claude-acp`, `codex-acp`, `github-copilot-cli`, and `cursor`.
An Install action writes registry agent settings and selects the agent.

The first tile is different.
`render_zed_agent_button` uses Zed account state, plans, URLs, and sign-in.
Omega cannot rename that tile without replacing its service contract.

The current Omega application identity is still Zed.
The application uses Zed names, application IDs, paths, and credential behavior.
The development credential provider writes unencrypted JSON.
Omega must not put a person signing secret in that provider.

The Omega workspace has no Nostr or secp256k1 dependency.
It has the `zeroize` dependency.
The new contract must select and admit the cryptographic implementation.

The current `@openagentsinc/sovereign-identity` package is not ready for Omega.
It exports TypeScript source and uses a workspace-only dependency.
Omega needs a compiled, versioned, and digested contract artifact.

## 4. Source study

### 4.1 Buzz patterns to adapt

Buzz gives useful patterns for machine identity.
Omega will adapt these patterns:

- Keep machine identity separate from community and editor setup.
- Version and scope completion state to a public identity.
- Serialize identity mutations and make them idempotent.
- Read back each write before success.
- A migration marker distinguishes first use from identity loss.
- Locked, lost, reset-failed, and relaunch states are different.
- A locked or lost state denies all signing.
- Quarantine a corrupt legacy file.
- A reset uses a restart-safe transaction.
- An import stays masked and shows the derived public identity.
- Stale asynchronous results cannot update a newer transaction.
- An early launch intent waits for the final identity.
- A timer can release a stalled view.
- A timer cannot decide durable truth.

The retained Buzz documents are historical and superseded.
This plan uses them as evidence only.
It does not authorize a Buzz runbook action.

### 4.2 Buzz patterns to reject

Omega will not copy these Buzz behaviors:

- Buzz creates a key during startup before the Create action.
- Buzz can show and copy a raw `nsec` during normal backup.
- Buzz can let a backup error become a skip path.
- Buzz mixes identity setup with agent harness setup.
- Buzz can make relay or community state part of onboarding.
- Buzz uses React, browser storage, Tauri commands, and browser confirmations.
- Buzz can use a plaintext secret file as a fallback.
- Buzz can use a human key from an environment variable.

Omega will also reject Buzz authority assumptions.
A relay cannot become OpenAgents command authority.
A profile event cannot prove onboarding completion.
A Nostr signature cannot settle an OpenAgents action.

### 4.3 Later pattern

NIP-AB device pairing is a useful later recovery method.
It avoids a raw secret in a QR code.
It uses an ephemeral exchange and a short authentication string.
It is not part of the first onboarding packet.
It needs a separate relay, metadata, and recovery decision.

### 4.4 Inputs reviewed

This plan used these OpenAgents documents:

- [transcript 259](../transcripts/259.md)
- [transcript 260](../transcripts/260.md)
- [transcript 261](../transcripts/261.md)
- [transcript 262](../transcripts/262.md)
- [Buzz status](../buzz/README.md)
- [retained Buzz runbook](../buzz/2026-07-22-buzz-self-host-and-sarah-runbook.md)
- [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md)
- [accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
- [master roadmap](../sol/MASTER_ROADMAP.md)
- [identity and Sync contract](../sol/2026-07-10-r1-r2-identity-sync-contract.md)
- [identity recovery audit](../sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md)

The Omega source review used these paths at the Omega source pin:

- `crates/onboarding/src/onboarding.rs`
- `crates/onboarding/src/basics_page.rs`
- `crates/workspace/src/welcome.rs`
- `crates/zed/src/main.rs`
- `crates/zed/src/zed/open_listener.rs`
- `crates/paths/src/paths.rs`
- `crates/release_channel/src/lib.rs`
- `crates/zed_credentials_provider/src/zed_credentials_provider.rs`
- `crates/ui/src/components/ai/agent_setup_button.rs`

The Buzz source review used these paths at the Buzz study pin:

- `desktop/src/features/onboarding/machineOnboarding.ts`
- `desktop/src/features/onboarding/communityOnboarding.tsx`
- `desktop/src/features/onboarding/ui/MachineOnboardingFlow.tsx`
- `desktop/src/features/onboarding/ui/BackupStep.tsx`
- `desktop/src/features/onboarding/ui/NostrKeyImportForm.tsx`
- `desktop/src-tauri/src/app_state.rs`
- `desktop/src-tauri/src/commands/identity.rs`
- `desktop/src-tauri/src/app_state_tests.rs`

## 5. Identity contract

Omega must keep three identity roles separate.

| Role | Purpose | Authority |
| --- | --- | --- |
| Device-local identity | Owns local rows and local continuity | Local device state |
| Sovereign person identity | Supplies the public `npub` and isolated signing | Local custody and admitted signer |
| Hosted owner identity | Owns hosted OpenAgents account authority | Server-derived OpenAuth user ID |

An `npub` is not an OpenAuth owner ID.
An OpenAuth link does not replace the local identity.
A provider token is not an OpenAgents identity.
A wallet key is not a person identity.
An agent key is not a person identity.

The owner direction advances local sovereign identity.
It does not advance relay use or Nostr command authority.
Optional relay interoperability remains in OMEGA-BZ-07.

The first contract packet must define the binding map.
It must also define removal and replacement behavior.

### 5.1 Fresh identity profile

The recommended fresh profile is Nostr-only.
It must not derive Spark or other wallet material.
The legacy profile `openagents.legacy_unified_nostr_spark.v1` controls identity and money.
Omega can use that profile only for exact recovery or migration.
New use of the legacy profile needs a separate owner decision.

The ProductSpec must freeze the fresh profile before real creation.
The shared contract must add the required public manifest schema.
The manifest must not require a wallet fingerprint for a Nostr-only profile.

### 5.2 Secret boundary

The normal Omega interface can receive only:

- the identity reference
- the public key
- the `npub`
- the custody state
- the recovery state
- the profile state
- public-safe receipt references

The normal interface cannot receive:

- a mnemonic
- an `nsec`
- a raw private key
- a seed
- a wallet secret

A separate custody-only port can accept an import secret.
It can also create an encrypted recovery artifact.
It must not return the plain secret to the GPUI view.

An advanced import view is a custody view.
It sends a zeroizing input buffer directly to the custody port.
It clears the input after success, error, cancellation, or view close.

The signer must accept an admitted signing request.
It must return the signature and public-safe status.
Relay, search, archive, and index code cannot access the signing secret.

### 5.3 Storage decision

Version 1 uses approved operating-system secret storage.
It has no cloud custody or cloud recovery.
It does not use the inherited development credential file.

The public manifest can use the isolated Omega data root.
It contains no secret.
The secret store and manifest must agree on the public key.

Creation is complete only when:

1. The secret write succeeds.
2. A read-back check succeeds.
3. The public key from the stored secret matches the manifest.
4. The public manifest commits atomically.
5. The completion record names the same identity and contract version.

A local Boolean flag cannot prove completion.

### 5.4 RC and stable custody

The contract packet must settle RC and stable custody before a real key write.
A throwaway RC key creates a second person identity.
A shared stable slot changes durable user state.

The recommended rule is:

- Fixture-backed development uses no real person secret.
- An explicit secure-development mode uses a development-only slot.
- RC can use canonical custody only after migration and rollback rules pass.
- Stable uses the admitted canonical locator.

The exact locator names belong in ProductSpec.
UI code must not select them.

## 6. User journey

Keep the current vertically scrolling onboarding structure.
Do not replace it with a modal or a multi-page wizard.
Add identity as the first section in that structure.

The first-run section order is:

1. Omega identity.
2. Theme.
3. Base keymap.
4. Agent Setup.
5. Editor settings import.
6. Vim mode.
7. Project trust.
8. Telemetry.

The identity section can show an inline substate.
Recovery and blocked states replace only that section.
They do not replace the complete page.

### 6.1 Omega identity section

Show the Omega mark, product name, and one clear value statement.
Show these primary actions:

- Create my identity
- Use an existing identity

The Create action is the first allowed key-generation trigger.
Page construction, startup inspection, and hover cannot generate a key.

An explanation link answers:

- What is an identity key?
- What is safe to share?
- What happens if I lose access?
- Is this my OpenAgents account?

The create path makes one Nostr-only person key pair.
It makes no wallet.
It makes no network request.
It publishes no profile.

The recovery path prefers an encrypted recovery artifact.
An advanced import can accept an existing Nostr secret.
The input stays masked.
Omega validates the input before it changes custody.
Omega shows the derived public identity before final import.

Discovery happens before create.
Omega does not automatically read candidate secrets.
The user authorizes each candidate read.
Multiple valid identities require an explicit choice.

After success, show the public `npub` and a short fingerprint.
Let the user add an optional local display name and avatar.
Do not publish a kind `0` event.

### 6.2 Recovery protection

Show recovery protection inside the identity section.
Omega offers an encrypted recovery artifact.
The custody service creates the artifact.
The GPUI layer never receives the plain signing secret.

The recommended default is Back up now.
ProductSpec must settle whether the user can defer this action.
If ProductSpec permits defer, the user must accept the risk.
Omega must keep a visible `Recovery needed` state.

Normal onboarding does not show or copy a raw `nsec`.
Manual raw export is outside this roadmap.

### 6.3 Theme section

Preserve the current theme section exactly.
Do not change its layout, theme families, controls, preview rendering, or settings behavior.

The preserved contract includes:

- Light, Dark, and System controls
- One, Ayu, and Gruvbox families
- matched light and dark themes in System mode
- the current preview tile rendering
- immediate settings-file updates
- current selection and focus styling

Move the function only if reuse requires an extraction.
If it moves, require a byte-equivalent behavior test.
Do not add Omega brand colors to the editor theme choices.

### 6.4 Agent Setup section

Preserve the current Agent Setup structure and registry-agent behavior.
Keep `AgentSetupButton` and the current install interaction.
Keep the four current featured registry IDs.
Do not reduce the section to one OpenAgents agent.

The Zed Agent tile is a separate compatibility problem.
It uses Zed account, plan, trial, URL, and sign-in services.
Do not relabel this tile as Omega Agent without an Omega service.
Hide it or show an honest unavailable state when Omega disables Zed services.
Keep that disposition narrow to the Zed-hosted tile.

Later work can expand this section with more agents and capability truth.
This onboarding packet must not redesign the registry-agent flow.

### 6.5 Remaining editor setup

Preserve the current base keymap, settings import, Vim, trust, and telemetry structure.
Change Zed text only when the new Omega journey exposes it.
Do not change the editor settings behavior in the identity packet.

The Finish action requires a durable identity.
It also records the independent Editor Onboarding completion.
If identity is incomplete, keep Finish disabled and explain the required action.

Reopening Editor Onboarding uses the same screen.
It shows the completed identity as a compact status section.
It keeps Theme and Agent Setup available.

## 7. Blocked and recovery states

Startup must use this precedence:

```text
reset-failed
  > keychain-locked
  > relaunch-required
  > identity-conflict
  > identity-lost
  > incomplete transaction
  > identity absent
  > ready
```

Each state has a different user action.
Omega must not use one generic error screen.

| State | Required behavior |
| --- | --- |
| `reset-failed` | Block signing and continue the restart-safe reset |
| `keychain-locked` | Explain how to unlock custody and offer retry |
| `relaunch-required` | Explain the restart requirement and preserve intent |
| `identity-conflict` | Show public fingerprints and require owner selection |
| `identity-lost` | Offer recovery and never mint a replacement silently |
| `incomplete transaction` | Resume or roll back the exact transaction |
| `identity absent` | Show the Welcome stage and do not create yet |
| `ready` | Resume the saved launch intent |

An unavailable keychain is not an absent identity.
A corrupt value is not proof of a fresh installation.
A prior marker prevents automatic replacement.

## 8. New native architecture

The implementation uses new logical components.

### 8.1 `omega_identity`

Add a new Rust crate with a descriptive library root.
The crate owns:

- identity inspection
- Nostr key generation and validation
- public identity derivation
- the secure secret-store interface
- the isolated signer interface
- public manifest consistency
- recovery transactions
- reset transactions
- public-safe status

The production secret store must use secure operating-system storage.
Tests use a deterministic fake store.
Secret values use zeroizing containers.

The crate consumes an admitted shared contract.
It must not use a relative path into the OpenAgents monorepo.
It uses a released version, digest, and conformance vectors.

### 8.2 Existing `onboarding` crate

Keep the current `onboarding` crate and screen structure.
Do not add an `omega_onboarding` crate.

Add the identity state machine as a new logical component.
The expected source seam is `crates/onboarding/src/identity_section.rs`.
Compose it before `render_theme_section` in the existing page body.

Keep these source responsibilities:

- `onboarding.rs` owns the page shell, actions, focus, scroll, and Finish behavior.
- `basics_page.rs` owns the ordered editor setup sections.
- `theme_preview.rs` owns the existing theme preview rendering.
- `agent_setup_button.rs` owns the shared agent tile.
- `identity_section.rs` owns identity presentation and safe input.

Add an explicit view mode:

- `FirstRun` shows the full identity section first.
- `EditorSetup` shows compact identity status and the same editor sections.

The view is an application singleton.
Make it non-splittable while it contains identity controls.
It does not serialize a secret or a pending secret.
It rebuilds its identity state from public facts after restart.

Background tasks stay in owned task fields.
Dropping a view must not produce a half-committed identity.
Each transaction has a generation ID.
Ignore late work for an old generation.

### 8.3 Initial surface router

Replace the duplicated `FIRST_OPEN` checks with one router.
The router checks identity before workspace restoration and path handling.

The router preserves:

- local path-open intent
- remote-open intent
- CLI wait behavior
- empty-window intent
- workspace restore intent
- deep-link intent

It resumes the exact intent after identity completion.
It does not sign, claim, or publish the intent before completion.

The router must not mark Editor Onboarding complete.
Identity setup and editor setup use independent versioned state.

### 8.4 Editor Onboarding

Keep the current screen as the Editor Onboarding base.
Keep its settings behavior and section order.
Use `OpenEditorOnboarding` for the replay action.

The FirstRun mode adds identity before the preserved editor sections.
The EditorSetup mode reuses those sections after first run.
Do not create a second copy of Theme or Agent Setup.

Rename public Zed copy when this path exposes it.
Do not rename internal compatibility identifiers without review.
The wider editor setup redesign can occur in a later packet.

## 9. Omega visual direction

Preserve the current Zed onboarding proportions and section rhythm.
Keep the centered column, header row, divider, vertical spacing, and scrolling page.
Use the current GPUI component vocabulary.

Replace the Zed logo, product heading, and product subtitle with Omega assets and copy.
Use the OpenAgents energy-blue accent only for Omega identity and primary state.
Do not force a dark brand surface over the selected editor theme.

Preserve the Theme section exactly.
The theme choice must continue to change the live onboarding appearance.
Do not add decorative Omega effects to its preview tiles.

The identity section uses:

- one Omega mark
- one clear identity heading
- one short explanation
- one dominant Create action
- one quiet recovery action
- one public fingerprint after success

It does not use decorative card grids.
It does not use continuous animation.
It does not use large marketing copy inside the product.

Use motion only for state feedback.
Reduced-motion mode removes nonessential movement.
Light and dark editor themes must not reduce onboarding contrast.

The interface must work at 360 pixels wide.
It must also work with a larger UI font.
Long public values wrap safely.
Secret values cannot enter accessible labels.

## 10. Rebrand method

The onboarding work uses a vertical rebrand.
Each new surface uses Omega names, assets, URLs, and telemetry names.

The implementation order is:

1. Isolate the application and data identity.
2. Add the Omega brand assets used by the existing shell.
3. Add identity as the first section in the existing composition.
4. Preserve Theme exactly.
5. Preserve the registry-agent setup section.
6. Disposition the Zed-hosted agent tile honestly.
7. Add the Editor Onboarding replay mode.
8. Replace other inherited surfaces when their native Omega replacement starts.

Do not run an automatic Zed-to-Omega text replacement.
Do not rename internal compatibility identifiers without review.

The RC release gate still scans all public surfaces.
An exposed Zed product label blocks the RC.
The scan is a falsifier, not the implementation plan.

## 11. Suggested GitHub issue sequence

This roadmap suggests these issues.
They are not created or dispatched by this roadmap.

| ID | Suggested issue title | Repository | Depends on |
| --- | --- | --- | --- |
| OMEGA-OID-00 | Freeze the Omega identity-first onboarding contract | `openagents` | none |
| OMEGA-OID-01 | Isolate Omega application identity and first-run data | `omega` | OMEGA-OID-00 |
| OMEGA-OID-02 | Publish the sovereign identity contract for Omega | `openagents` | OMEGA-OID-00 |
| OMEGA-OID-03 | Add fixture identity states to the Omega onboarding screen | `omega` | OMEGA-OID-00 |
| OMEGA-OID-04 | Add the isolated Omega signer and secure custody | `omega` | OMEGA-OID-01, OMEGA-OID-02 |
| OMEGA-OID-05 | Add identity recovery and explicit creation | `omega` | OMEGA-OID-04 |
| OMEGA-OID-06 | Connect live identity state to the GPUI journey | `omega` | OMEGA-OID-03, OMEGA-OID-05 |
| OMEGA-OID-07 | Route every first launch through Omega onboarding | `omega` | OMEGA-OID-06 |
| OMEGA-OID-08 | Add the Editor Onboarding replay mode | `omega` | OMEGA-OID-07 |
| OMEGA-OID-09 | Prove the packaged identity-first journey | both | OMEGA-OID-08 |

### 11.1 OMEGA-OID-00: freeze the contract

Work:

- Add the ProductSpec delta.
- Add the AssuranceSpec delta.
- Freeze the three-role identity map.
- Freeze the Nostr-only fresh profile.
- Freeze RC and stable custody locators.
- Freeze recovery, defer, reset, and deletion policy.
- Freeze the exact first-run acceptance criteria.

Exit:

- No implementation must select an unstated identity policy.
- The owner accepts the user journey.
- Assurance owns the failure matrix before implementation.

Falsifier:

- One key implicitly owns person, wallet, device, agent, and hosted authority.

### 11.2 OMEGA-OID-01: isolate application state

Work:

- Set Omega application names and IDs.
- Set separate data, cache, log, and credential roots.
- Add development, RC, and stable namespace rules.
- Prove that Omega does not read or mutate Zed state.
- Add the new Omega mark used by first run.

Exit:

- A secure identity cannot enter a Zed or plaintext development store.
- Omega and Zed can run beside each other.

Falsifier:

- A clean Omega start reads or changes a Zed file or credential.

### 11.3 OMEGA-OID-02: publish the shared contract

Work:

- Add the Nostr-only public manifest profile.
- Publish a compiled and versioned package.
- Remove workspace-only dependencies from the release artifact.
- Publish the artifact digest.
- Publish Rust and TypeScript conformance vectors.
- Keep secret export outside the normal interface.

Exit:

- Omega can consume immutable contract bytes.
- Rust and TypeScript produce the same public identity results.

Falsifier:

- Omega needs a relative monorepo path or a normal API returns a secret.

### 11.4 OMEGA-OID-03: adapt the existing screen

Work:

- Keep `Onboarding::render` as the screen structure.
- Add the identity section before Theme.
- Add Omega header copy, assets, and semantic roles.
- Use fixture identity states only.
- Add keyboard, focus, narrow-window, and visual tests.
- Add blocked, empty, loading, and error views.
- Add a preservation test for the exact theme behavior.
- Add a preservation test for registry-agent setup.
- Add the accepted Zed-hosted tile disposition.

Exit:

- The owner can review the complete journey without a real key write.
- Theme behavior matches the current source.
- Registry-agent setup behavior matches the current source.
- The Omega header and identity section contain no Zed product text.

Falsifier:

- The packet ports React or changes Theme or registry-agent behavior.

### 11.5 OMEGA-OID-04: add custody

Work:

- Add the secure-only secret-store boundary.
- Add the isolated signer.
- Add zeroizing secret handling.
- Serialize mutations across tasks and processes.
- Verify each write with a read-back public-key check.
- Add atomic public manifest writes.

Exit:

- A stored identity restarts with the same public key.
- Normal Omega code cannot retrieve the secret.

Falsifier:

- A secret enters app data, SQLite, logs, telemetry, arguments, or crash output.

### 11.6 OMEGA-OID-05: add recovery and create

Work:

- Add existence-only discovery.
- Add visible candidate authorization.
- Add conflict resolution by public identity.
- Add encrypted recovery import and export.
- Add advanced masked Nostr-secret import.
- Add one explicit and idempotent create transaction.
- Add lost, locked, corrupt, reset, and relaunch recovery.

Exit:

- Open and create are separate operations.
- A crash or retry cannot rotate a known identity.
- Create performs no network request.

Falsifier:

- Inspection, page load, or recovery failure silently creates a key.

### 11.7 OMEGA-OID-06: connect live state

Work:

- Connect the GPUI state machine to public custody facts.
- Add recovery progress and retry.
- Add the encrypted backup choice.
- Add the optional local profile.
- Add public fingerprint confirmation.
- Fence stale asynchronous results.

Exit:

- Every visible state follows from a durable or in-flight fact.
- No view state contains a plain secret after mutation.

Falsifier:

- A timer or local completion flag becomes identity authority.

### 11.8 OMEGA-OID-07: route first launch

Work:

- Replace both `FIRST_OPEN` checks.
- Gate restore, path open, remote open, and new-window paths.
- Preserve and resume the exact launch intent.
- Add concurrent-launch and CLI-wait tests.
- Mark completion only after custody and manifest consistency.

Exit:

- Every clean launch focuses identity before an editor setup interaction.
- Every completed launch resumes the requested work.

Falsifier:

- A path, restored workspace, or second process bypasses identity setup.

### 11.9 OMEGA-OID-08: add editor replay mode

Work:

- Give editor setup an independent completion version.
- Add FirstRun and EditorSetup modes to the shared screen.
- Show compact identity status in EditorSetup mode.
- Add the editor-to-home handoff.
- Add a command and menu action to reopen Editor Onboarding.
- Keep Theme and registry-agent setup available in both modes.

Exit:

- Identity completion cannot remove Editor Onboarding.
- Editor completion cannot suppress identity recovery.
- The implementation has one Theme section and one Agent Setup section.

Falsifier:

- One Boolean controls both journeys.

### 11.10 OMEGA-OID-09: prove the package

Work:

- Test the exact installed candidate.
- Test offline create and recovery.
- Test keychain locked and unavailable states.
- Test crash points in each custody transaction.
- Test update, rollback, uninstall, and reinstall.
- Run secret tripwire scans.
- Run the GPUI accessibility matrix.
- Run the Zed data-isolation audit.
- Record owner and independent review.

Exit:

- The exact candidate passes the AssuranceSpec.
- The RC release record binds the identity contract and artifact digests.

Falsifier:

- Unit tests or screenshots are the only evidence.

## 12. Dependency and parallel work

The contract packet is first.
No packet can write a real person key before OMEGA-OID-00.

After OMEGA-OID-00:

- OMEGA-OID-01 can isolate application state.
- OMEGA-OID-02 can publish the shared contract.
- OMEGA-OID-03 can adapt the existing screen with fixtures.

OMEGA-OID-04 waits for state isolation and the released contract.
OMEGA-OID-05 waits for custody.
OMEGA-OID-06 joins the live service and the fixture-approved shell.
OMEGA-OID-07 and OMEGA-OID-08 then change launch order.
OMEGA-OID-09 closes the installed journey.

One coordinator owns launch routing and identity completion.
Only one packet can change the custody contract at a time.
Only one packet can change application identity at a time.

## 13. ProductSpec delta

ProductSpec must define:

- the first-launch route
- the three identity roles
- the fresh identity profile
- explicit Create intent
- create, existing, recovery, conflict, locked, and lost behavior
- RC and stable custody
- backup and defer policy
- no-login local use
- optional OpenAuth linking
- local profile behavior
- no automatic publication
- restart stability
- import, archive, delete, and reset behavior
- FirstRun and EditorSetup modes
- the Editor Onboarding reopen path

ProductSpec must state that a person identity is not a wallet.
It must state that a Nostr signature is not command admission.

## 14. AssuranceSpec delta

AssuranceSpec must define tests for:

- clean identity absence
- one existing identity
- multiple identity candidates
- symlink and weak-permission refusal
- keychain locked and unavailable
- corrupt keychain data
- concurrent starts
- double Create input
- crash at every custody transition
- corrupt recovery artifact
- wrong recovery password
- restart, update, downgrade, uninstall, and reinstall
- signer crash
- forged signing requests
- no-network operation
- logs, telemetry, clipboard, UI tree, and crash redaction
- keyboard and focus
- screen readers
- 360-pixel width
- larger UI fonts
- light and dark appearance
- high contrast
- reduced motion

The independent verifier must use the exact installed candidate.
Owner acceptance does not replace independent verification.

## 15. Required first-run tests

The implementation must include these named journeys:

1. No secret exists before the user selects Create.
2. One Create action produces one identity.
3. A double action cannot produce a second identity.
4. A failed create leaves the state absent and retryable.
5. Restart shows the same public identity.
6. Invalid import cannot change custody.
7. A read-back mismatch cannot mark completion.
8. A lost identity cannot mint a silent replacement.
9. A locked identity cannot sign.
10. An incomplete reset resumes before normal use.
11. An offline user can complete local identity setup.
12. A stale task cannot update a newer transaction.
13. A saved launch intent resumes after completion.
14. Identity and Editor Onboarding modes have separate completion.
15. No test tripwire finds a plain secret.
16. Theme selection matches the current theme behavior.
17. Registry-agent setup matches the current behavior.
18. No test finds a Zed product label in the Omega header or identity section.

## 16. Non-goals

This roadmap does not:

- port Buzz
- deploy a Buzz relay
- add community onboarding
- expand the current agent registry
- add a wallet
- publish a Nostr profile
- add Nostr direct messages
- make Nostr command authority
- add cloud custody
- add NIP-AB pairing
- finish the complete Omega rebrand
- redesign the preserved editor settings sections
- migrate Zed or Electron secrets

## 17. RC integration

This roadmap specializes the Omega RC1 brand and identity lane.
It does not remove the other RC1 gates.

OMEGA-RC1-01 can continue as a parallel legal and source packet.
OMEGA-RC1-03 must finish state and endpoint isolation.
OMEGA-RC1-04 must supply the owned build.
OMEGA-RC1-05 through OMEGA-RC1-07 still control package and publication.

OMEGA-OID-09 joins those gates.
The RC cannot ship only because the onboarding UI looks complete.
The RC cannot ship with an exposed Zed product label.

## 18. Rollback

Before launch routing changes, rollback removes the fixture identity section.
It does not touch a real identity.

After custody starts, rollback must preserve:

- the same identity reference
- the same public key
- the same secret-store record
- the same recovery state
- a readable prior public manifest

Rollback cannot silently generate a second key.
An older build must fail clearly on an unsupported manifest.

## 19. Completion rule

This roadmap is complete when OMEGA-OID-09 passes.
Completion needs code, tests, installed evidence, and review.
A plan, fixture, or screenshot is not completion.

The next ready packet is OMEGA-OID-00.
OMEGA-OID-01 and OMEGA-OID-03 can start after its contract decisions.

## 20. Plan claim

- Claim actor: `codex-root-omega-identity-onboarding-r2-20260724`
- Claim time: `2026-07-24T05:06:27Z`
- Repository: `OpenAgentsInc/openagents`
- Branch: `main`
- Base commit: `ae40fdad85492cb922bc6c11bc9252a728561df4`
- Scope: the identity-first plan, Zed structure clarification, and Omega roadmap index
- Owned paths:
  - `docs/omega/2026-07-23-identity-first-onboarding-roadmap.md`
  - `docs/omega/README.md`
  - `docs/omega/ROADMAP.md`
  - required STE profile and generated inventory records
- Hot contract: `docs/omega/ROADMAP.md`
- ProductSpec state: proposed delta, not admitted
- AssuranceSpec state: proposed delta, not admitted
- Suggested issues: not created

Verification:

- `pnpm run generate:ste-final-inventory`
- `pnpm run generate:ste-ledger`
- `pnpm run generate:ste-final-inventory`
- `pnpm run check:ste`
- `pnpm run check:ste-control-semantics`
- `pnpm run check:sol-docs`
- `pnpm run test:sol-docs`
- `git diff --check`
