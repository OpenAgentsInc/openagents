# Omega release-readiness brand audit

- Date: 2026-07-24
- Class: current-status and release roadmap
- Owner: OpenAgents
- Scope: Omega application identity, public brand, services, package, and release proof
- Immediate target: `v0.2.0-rc1`
- Omega source: `4c117bfb1eb60efe67e45471b501b305c290cbd0`
- OpenAgents source: `732579f68b0239e36a285cd16b09cd908d82ad82`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. Question

What work must finish before OpenAgents can release an Omega candidate that
does not present Zed as the product or use Zed production services?

This audit does not repeat the identity feature backlog.
It records the separate release work that the current issue list does not own.

## 2. Result

Omega is not ready for release.

The application identity work is a good base.
Omega now has separate display names, application identifiers, data roots,
credential namespaces, protocol schemes, and a primary `omega` executable.
The first-run header and identity fixture section also use Omega text.

The release package and the main editor still have Zed brand and service
dependencies.
The installed icon is still a Zed icon.
The welcome screen, application menu, About window, settings, update UI, and
many agent surfaces still show Zed text.
The default client server is still `https://zed.dev`.

The update code still asks for Zed assets.
The macOS package still uses Zed signing data, Zed artifact names, and Zed
commercial terms.
The inherited release workflow cannot publish an OpenAgents release.

The release team must complete six gates:

1. Install the OpenAgents Desktop icon family.
2. Remove Zed from all public Omega surfaces.
3. Disable or replace each Zed production service.
4. Create an OpenAgents package and release path.
5. Keep only reviewed compatibility identifiers.
6. Prove the exact installed candidate.

## 3. Sources

The audit used these current sources on 2026-07-24:

- All files in `docs/omega/`.
- Omega commit `4c117bfb1eb60efe67e45471b501b305c290cbd0`.
- OpenAgents commit `732579f68b0239e36a285cd16b09cd908d82ad82`.
- The current Omega GitHub issue list.
- The OpenAgents Desktop icon files.
- The Omega Rust source, assets, package scripts, and GitHub workflows.

The current issue state is:

| Issue | State | Relation to this audit |
| --- | --- | --- |
| `#1` Isolate Omega application identity and first-run data | Closed | It supplies the identity base. |
| `#2` Add fixture identity states to the Omega onboarding screen | Closed | It supplies the Omega first-run header and fixture section. |
| `#3` through `#10` | Open | These issues primarily own identity behavior and proof. |

The open issues do not form a release rebrand backlog.
Issue `#8` includes package proof, but it does not own all work in this audit.

## 4. Method

The audit used these steps:

1. Read all current Omega documents.
2. Read the current issue titles, states, and bodies.
3. Inspect the changes from issues `#1` and `#2`.
4. Inspect the application identity and path code.
5. Inspect the package icons and the OpenAgents Desktop icons.
6. Search Rust string literals for public Zed text.
7. Search code and release files for Zed URLs and package names.
8. Inspect the default server, updater, menus, About window, and package scripts.
9. Compare the result with the `v0.2.0-rc1` release contract.

The text scan is a discovery aid.
It includes tests, examples, internal names, and compatibility data.
It is not a count of unique user defects.
An installed application scan is still required.

## 5. Current status

| Area | State | Release result |
| --- | --- | --- |
| Repository name and product direction | Complete | The root README presents Omega and explains the Zed fork. |
| Application identity table | Complete | The four channels have separate Omega names and identifiers. |
| Data and credential roots | Complete for the current identity seam | Omega does not select Zed roots for normal channel data. |
| Primary executable and CLI | Mostly complete | The primary executable and installed command are `omega`. |
| First-run header | Complete for the changed header | The header uses the Omega mark and Omega text. |
| Identity fixture section | Complete for fixture scope | The new section uses Omega text and has no real secret write. |
| Operating-system app icon | Not started | All package icon inputs are still Zed artwork. |
| Main welcome screen | Not started | It shows the Zed logo and Zed welcome text. |
| Application menu and About window | Not started | They show `Zed`, `About Zed`, `Hide Zed`, and `Quit Zed`. |
| Public editor and settings text | Not complete | Many settings, notifications, errors, and agent surfaces show Zed. |
| Zed account and hosted agent | Enabled in public code | The onboarding agent tile still uses Zed account and plan services. |
| Default server and production services | Not isolated | The default server is `https://zed.dev`. |
| Updates | Not owned | The client asks for Zed assets and uses Zed package names. |
| Help, support, and feedback links | Not owned | Public actions open Zed sites and repositories. |
| macOS package | Blocked | The script has Zed signing, terms, volume, and artifact data. |
| Linux metadata | Blocked | AppStream metadata names Zed Industries and links to Zed. |
| Windows package | Partial | Names changed, but Zed terms and inherited update paths remain. |
| Release workflow | Blocked | Zed release systems control the workflow, and it runs only for Zed owners. |
| Candidate version | Blocked | The main package version is still `1.14.0`, and the channel file is `dev`. |
| Installed proof | Absent | No exact Omega candidate exists for installed tests. |

## 6. Work that is complete

### 6.1 Application identity

`crates/app_identity/src/app_identity.rs` defines these values:

| Channel | Display name | Application identifier | Credential namespace | Scheme |
| --- | --- | --- | --- | --- |
| Development | Omega Dev | `com.openagents.omega.dev` | `com.openagents.omega.credentials.dev` | `omega-dev` |
| Nightly | Omega Nightly | `com.openagents.omega.nightly` | `com.openagents.omega.credentials.nightly` | `omega-nightly` |
| RC | Omega RC | `com.openagents.omega.rc` | `com.openagents.omega.credentials.rc` | `omega-rc` |
| Stable | Omega | `com.openagents.omega` | `com.openagents.omega.credentials` | `omega` |

The current path code uses these values for application support, config, cache,
state, logs, and browser data.
The current credential provider has a secure-only system keychain seam.
The primary binary name is `omega`.
Linux and Windows package templates now use Omega application names.

The focused `app_identity` test passes.
It proves that the four identity sets are unique and contain no Zed name.

### 6.2 First-run surface

The onboarding header now uses `VectorName::OmegaLogo`.
It shows `Welcome to Omega` and `Your last IDE.`
The identity section is the first section.
The section has fixture states for absent, create, ready, recovery, lock,
conflict, incomplete, reset, and relaunch conditions.

This work does not complete the rebrand.
The same page still has Zed telemetry text and a Zed Agent tile.

## 7. Release blockers

### 7.1 Canonical app icon

The current OpenAgents Desktop app supplies the canonical icon source:

- PNG source:
  `apps/openagents-desktop/resources/openagents-icon.png`
- PNG SHA-256:
  `4dc74a8507669f77118a82791a8bf6e85773e1d31a4b16816b46bc2066e452d8`
- PNG size: 1024 by 1024 pixels
- PNG source commit:
  `b9f41580fb2a1d536a67d970e08dec9125612931`
- macOS source:
  `apps/openagents-desktop/resources/openagents-icon.icns`
- ICNS SHA-256:
  `779a545acbf626c7e319d0dacf106e2e69eb0caf2d506dabe14c343a1925563f`
- ICNS source commit:
  `e3600a5c326fa1e33e5be9d13363ff902ea57f4d`

Omega has 13 Zed package icon inputs:

- Eight PNG application icons for four channels.
- Four Windows ICO application icons.
- One macOS document icon.

The package scripts consume these files.
Changing only `assets/images/omega_logo.svg` does not change the installed app
icon.

Required work:

1. Copy the pinned OpenAgents Desktop icon bytes into Omega.
2. Generate the required 512-pixel, 1024-pixel, ICNS, and ICO files.
3. Define channel badges only if OpenAgents approves them.
4. Replace `Document.icns` with an Omega document icon.
5. Replace Zed logos on the welcome and other public product surfaces.
6. Add digest tests for every generated icon.
7. Check Finder, Dock, Launchpad, Windows Explorer, taskbar, Linux launchers,
   installers, and file associations.

Acceptance evidence:

- Each package icon derives from the pinned OpenAgents source.
- No installed surface shows a Zed app icon.
- Each expected icon size is present and renders clearly.
- The release record contains the source and output digests.

### 7.2 Main shell and public text

The source scan found 344 Rust string-literal lines in 135 Rust files that
contain the word `Zed`.
Many are tests or technical text.
Many others are public product text.

Confirmed public examples include:

- `crates/workspace/src/welcome.rs`
  shows `Welcome to Zed`, the Zed logo, and the Zed subtitle.
- `crates/zed/src/zed/app_menus.rs`
  defines the Zed application menu and Zed Help links.
- `crates/zed/src/zed.rs`
  opens a window with the title `About Zed`.
- `crates/zed/src/main.rs`
  shows Zed launch-failure text.
- `crates/onboarding/src/basics_page.rs`
  shows Zed telemetry text and the Zed Agent.
- `crates/settings_ui/src/page_data.rs`
  has many public setting descriptions that name Zed.
- `crates/ai_onboarding/src/`
  presents Zed AI plans and Zed account flows.
- `crates/extensions_ui/src/extensions_ui.rs`
  describes features as part of Zed.
- `crates/workspace/src/notifications.rs`
  shows Zed update and service notices.
- `crates/title_bar/src/`
  shows Zed update and collaboration text.

Required work:

1. Replace product nouns with Omega on retained local editor features.
2. Remove Zed account, plan, trial, and hosted-model text from the default UI.
3. Hide a service surface when Omega has no replacement service.
4. Show an honest unavailable state when a visible placeholder gives value.
5. Replace the main welcome logo, title, and subtitle.
6. Replace menu, About, update, notification, settings, and error text.
7. Replace accessibility labels and window titles.
8. Add tests for the highest-risk public strings.

Do not rename an internal identifier only because it contains `zed`.
Use the compatibility review in section 9.

Acceptance evidence:

- A clean installed journey shows no Zed product or publisher text.
- Screen-reader output shows no Zed product or publisher text.
- Disabled hosted features do not claim to be Omega services.

### 7.3 Services and network boundaries

`assets/settings/default.json` sets `server_url` to `https://zed.dev`.
The client uses this value for account, credentials, collaboration, release,
and hosted service paths.
The code also contains Zed documentation, status, feedback, update, remote
server, telemetry, and crash paths.

The source scan found 95 non-document files with `zed.dev`.
Some uses are schema or test compatibility data.
The current default server is a release blocker.

Required work:

1. Set a safe Omega default that does not contact a Zed production service.
2. Disable Zed account and collaboration connections by default.
3. Disable the Zed hosted agent, plans, trials, and Zed cloud models.
4. Disable Zed telemetry and crash upload.
5. Disable Zed update checks until an Omega update feed exists.
6. Disable Zed remote-server downloads or publish owned compatible artifacts.
7. Replace public documentation, status, feedback, privacy, and support links.
8. Decide the extension registry compatibility boundary.
9. Record each allowed Zed network host and its exact purpose.
10. Run a network capture from first start through shutdown.

Acceptance evidence:

- A clean offline start completes without a Zed request.
- A connected start makes no Zed request unless an approved compatibility
  action requires it.
- Each disabled feature has an honest state.
- The candidate has an endpoint allow-list with an owner.

### 7.4 Update behavior

The updater polls for all non-development channels.
It requests asset type `zed`.
It expects files such as `Zed.dmg` and `Zed.exe`.
The helper UI also says `Updating Zed`.

The RC must use one of these dispositions:

1. Disable update polling and remove update actions for `v0.2.0-rc1`.
2. Add an owned, signed Omega update feed and Omega package format.

The first disposition is the smaller safe RC scope.

Acceptance evidence:

- The RC does not query a Zed release endpoint.
- A manual update action cannot install Zed over Omega.
- The UI states that this candidate has no automatic updates if updates are
  disabled.

### 7.5 Package, signing, and legal material

`script/bundle-mac` still contains:

- `Zed Industries, Inc.` as the signing identity.
- Apple team `MQ55VZLNZQ`.
- Zed keychain and certificate file names.
- Zed Sentry project names.
- `Zed-<architecture>.dmg` artifact names.
- `Zed` as the disk image volume name.
- `script/terms/terms.json` as the disk image license.

`script/terms/terms.rtf` is the Zed commercial service agreement.
The Windows installer also includes this agreement.
Omega must not present this agreement as OpenAgents terms.

The Linux AppStream file still names Zed Industries.
It also uses Zed product text, screenshots, and public URLs.

Required work:

1. Create an Omega-owned macOS bundle command.
2. Use the OpenAgents signing identity and team `HQWSG26L43`.
3. Add valid Omega provisioning and entitlement inputs.
4. Name the RC artifact
   `Omega-v0.2.0-rc1-macos-arm64.dmg`.
5. Use `Omega RC` as the disk image volume name.
6. Remove the Zed commercial agreement from all Omega installers.
7. Keep GPL, Apache, copyright, source, and third-party notices.
8. Replace Linux AppStream data with Omega data and Omega screenshots.
9. Replace Windows public installer data and legal text.
10. Check package contents for Zed publisher and product claims.

Acceptance evidence:

- `codesign`, Gatekeeper, notarization, and staple checks pass.
- The installed bundle has `com.openagents.omega.rc`.
- The package contains OpenAgents terms or no click-through terms.
- The package keeps all required open-source notices.
- A legal review accepts the exact package.

### 7.6 Version and release system

The main application package version is `1.14.0`.
The release channel file contains `dev`.
The inherited GitHub release workflow runs only for Zed repository owners.
It uploads Zed artifacts to `zed-industries/zed`.
It also depends on Zed secrets, runners, services, and compliance jobs.

Required work:

1. Set the exact Omega candidate version.
2. Set the candidate channel to the RC mapping.
3. Add one owned build command from a clean checkout.
4. Add an OpenAgents release workflow or a recorded local release procedure.
5. Record the Omega commit, upstream commit, toolchains, lock digest, package
   digest, signing identity, and source archive digest.
6. Publish only to `OpenAgentsInc/omega`.
7. Mark `v0.2.0-rc1` as a prerelease.
8. Do not publish it as `latest`.

Acceptance evidence:

- One command produces the expected candidate from a clean checkout.
- A dirty checkout or absent identity input stops the command.
- The release record binds all source and package digests.

## 8. Ordered release roadmap

This roadmap is separate from issues `#3` through `#10`.
Identity work can continue in parallel where the source paths do not overlap.

### OMEGA-BRAND-01: install the icon family

Scope:

- Pin the current OpenAgents Desktop icon inputs.
- Generate all Omega app and document icon formats.
- Connect each package target to the new files.
- Add digest and dimension tests.

Exit:

- No package target uses a Zed icon.

### OMEGA-BRAND-02: replace the main product shell

Scope:

- Replace the welcome page, application menu, About window, titles, and
  accessibility labels.
- Use Omega names and OpenAgents publisher text.
- Remove public Zed social and company actions.

Exit:

- The empty start, project start, menu, and About journeys contain no Zed
  product text.

### OMEGA-BRAND-03: isolate services

Scope:

- Remove the `https://zed.dev` default.
- Disable Zed account, collaboration, hosted AI, telemetry, crash, and update
  services.
- Define the extension and remote-server compatibility boundary.

Exit:

- A network capture has no unexpected Zed request.

### OMEGA-BRAND-04: clean all reachable copy

Scope:

- Audit settings, onboarding, notifications, agent UI, extensions, errors,
  dialogs, tooltips, and screen-reader labels.
- Replace local editor product nouns with Omega.
- Hide Zed-only commercial features.

Exit:

- The public-text scan has no unapproved Zed result.

### OMEGA-BRAND-05: own the package

Scope:

- Replace signing, artifact, volume, installer, and metadata inputs.
- Remove Zed commercial terms.
- Keep required open-source notices.
- Set version `v0.2.0-rc1` and the RC channel.

Exit:

- One clean command produces the named OpenAgents candidate.

### OMEGA-BRAND-06: prove the installed candidate

Scope:

- Install Omega beside Zed in a clean user profile.
- Test offline first start, open, edit, save, Git, terminal, close, and restore.
- Inspect the file system, keychain namespace, process tree, and network.
- Inspect all visible product text and package metadata.
- Remove Omega and confirm that Zed data did not change.

Exit:

- The exact signed candidate passes owner and independent review.

## 9. Compatibility review

Some Zed identifiers can remain because Omega is a tracked fork.
Each retained item needs a reason, an owner, and a test.

Likely allowed compatibility classes:

- Rust crate names such as `zed`.
- Existing action namespaces.
- Selected `ZED_*` build variables.
- Project `.zed` configuration folders.
- Public Zed extension schemas when Omega keeps format compatibility.
- Upstream source links in comments, license notices, and test fixtures.
- Protocol or remote-server names that an explicit compatibility contract
  requires.

Not allowed in an Omega release:

- Zed app icons or logos on public surfaces.
- Zed as the visible product or publisher.
- Zed artifact, installer, or disk image names.
- Zed commercial terms presented to an Omega user.
- Zed account, plan, trial, or hosted-agent claims.
- An unexpected request to a Zed service.
- A Help action that presents Zed support as Omega support.
- A Zed update installed into Omega.

Create a machine-readable allow-list before the final scan.
The allow-list must identify the exact path or match, reason, owner, and expiry
condition.

## 10. Required release scans

Run these scans against source and installed files:

```sh
rg -n -i '\bzed\b|zed\.dev|zed-industries' \
  Omega\ RC.app package-root release-record
```

Also inspect:

- Application menu text.
- Window titles.
- About and Welcome.
- Settings descriptions.
- Notifications and error dialogs.
- Tooltips and accessibility labels.
- Icon resources.
- Bundle metadata.
- File associations.
- Shell integration.
- Installer and disk image text.
- License and notice files.
- Network destinations.
- Data, cache, log, and credential roots.
- Update metadata and update behavior.

Classify each scan result as:

- release blocker
- approved compatibility identifier
- required attribution
- test or source-only data

An unclassified public result blocks release.

## 11. Release completion rule

Omega `v0.2.0-rc1` is brand-ready only when all conditions are true:

1. The installed product shows the OpenAgents icon and Omega name.
2. No public surface presents Zed as the product or publisher.
3. No unexpected Zed service receives a request.
4. Omega and Zed use separate data and credential roots.
5. The package uses OpenAgents signing and correct legal material.
6. The version, channel, artifact name, and bundle identifier match the release
   record.
7. Each retained Zed identifier is in the reviewed compatibility allow-list.
8. The exact installed candidate passes the clean-user journey.
9. The release record contains source, package, icon, and notice digests.
10. An independent reviewer accepts the evidence.

The current source passes none of the complete package gates.
Application identity and the changed first-run header are the completed base.

## 12. Next actions

1. Start `OMEGA-BRAND-01`.
   Copy and pin the OpenAgents Desktop icon family.
2. Start `OMEGA-BRAND-03` in a separate source lane.
   Disable inherited Zed services before more public UI uses them.
3. Start `OMEGA-BRAND-02` after the icon paths are stable.
   Replace the main shell and public product controls.
4. Build the compatibility allow-list during each packet.
5. Do not start a release build until `OMEGA-BRAND-01` through
   `OMEGA-BRAND-05` pass.
6. Use `OMEGA-BRAND-06` as the final release gate.
