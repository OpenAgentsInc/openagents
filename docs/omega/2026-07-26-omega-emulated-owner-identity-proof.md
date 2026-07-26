# Emulated owner identity: the custody path, performed

- Date: 2026-07-26
- Packets: OMEGA-BRAND-06 and OMEGA-OID-09
- Issues: omega#16, omega#9, omega#8
- Lane: `claude/emulated-owner`
- STE issue: 9
- Glossary revision: openagents-ste-glossary-v1
- Status: producer record. Not signed. No issue closed.
- Evidence: `docs/omega/2026-07-26-omega-emulated-owner-identity-evidence.json`

## Result

Three lanes reported omega#8 blocked on the owner's encrypted recovery file.
That framing was wrong.

An emulated identity has its own recovery file.
This lane created an identity, protected its recovery, and then recovered from
the file it made.
The same custody code ran.
Nothing of the owner's was read, copied or used.

The lane produced:

- a genuinely pristine first start, where the product offers `Create identity`
- a clean-user editor journey on that pristine profile
- two custody transitions from `Absent` to `Ready`, one by creation and one by
  recovery
- the custody-path secret tripwire that omega#8 asks for
- a new control that sharpens the terminal chord gap

The lane also found that the rc18 release record on this host no longer binds
the rc18 artifact.
So a fresh cut is needed for omega#16 acceptance criterion 9.

No issue closes on this record.

## Authority

The owner gave a standing delegation on 2026-07-26 to emulate the user, because
the owner is not available to act by hand.

This lane records `performedBy: agent` and `ownerObservation: false`.
It does not write an owner-observation attestation.
The record says who acted.
This follows the ruling applied to omega#26 gate 2 on the same day.

## Why earlier lanes hit a wall

The identity data root comes from `paths::data_dir()`.
The keychain search list comes from the home directory.

A fresh `--user-data-dir` moves the data root only.
The owner's keychain secret still resolves, so custody reads `Incomplete`, and
the only action the screen offers is `Recover identity`.
That is the wall.

## How this lane got a pristine profile

macOS resolves the keychain search list from the home directory when no
security preference file exists.

This lane ran under an isolated home with a keychain it created.
The owner's login keychain was not in the search list of any process this lane
started.

Controls:

| Check | Result |
| --- | --- |
| Emulated search list | the isolated keychain and `System.keychain` only |
| Owner search list, before and after | unchanged |
| Owner login keychain last written | 04:12:55, before this lane started at 08:40 |
| Emulated keychain last written | 08:46:52 |
| Security preference file written | none, in either home |

The emulated keychain file holds `com.openagents.omega.credentials.rc`, so the
product wrote its credential where this lane expected it.

## How a second instance started

The owner's instance holds `127.0.0.1:45338`.
That port is `44737` plus `100` for the preview channel plus the user
identifier `501`.

This lane denied that one port to its own process with a seatbelt profile.
`ensure_only_instance` then failed the handshake, failed the bind, retried the
handshake, and continued without a handshake.
That is its documented path.

Controls:

- Unsandboxed, a connection to the port succeeds.
- Under the profile, the same connection fails.
- Under the same profile, in the same second, a request to `github.com` returns
  `200`.

The owner's instance ran for the whole window.
Its elapsed time was `04:20:24` at the start and `05:09:31` at the end.
This lane never restarted it and never wrote into it.

## The pristine first start

Read from the rendered pixels, top to bottom:

| Heading | Vertical position |
| --- | --- |
| `Your identity` | 0.167 |
| `Theme` | 0.325 |
| `Base Keymap` | 0.500 |
| `Agent Setup` | 0.604 |

The identity section offers `Create identity` and `Recover identity`.
`Create identity` is offered because custody is `Absent`.

No earlier lane reached this state.

## The emulated identity, by the shipped custody driver

The candidate ships `Contents/MacOS/omega-identity-proof`, digest
`dee3d046c44eecc9e26a94d7521679b5147858ab7f03f140ddbd3a71b286b098`.
It uses the keyring service `com.openagents.omega.identity-proof.v1` and the
account `disposable-proof-only`.

| Step | Custody | Note |
| --- | --- | --- |
| `inspect` | `absent` | pristine. No owner secret is reachable |
| `create` | `ready` | |
| `protect-recovery` | `ready` | artifact `6af5b852...`, 163 bytes |
| `probe-wrong-recovery` | | rejected |
| `probe-corrupt-recovery` | | rejected |
| `reset`, `resume-reset` | `relaunch-required` | |
| `process-start` | `absent` | the secret is gone and the documents are gone |
| `recover` | `ready` | the same public identity returns |
| `process-start` | `ready` | this is the read `identity_ready` performs |

The recovered public identity is
`npub1ty09zdmufr5x47djht927jkt72q75p60vl3ntye09sfvsv3ks5yq9pmxc4`.

This is `recovery_scenarios`, performed against the candidate's own binary.

## The emulated identity, by the product

The onboarding screen created a second identity,
`npub19n347p2qqqhn333mcs5js5zs4clae54062h3f8jwp97wmcyk7duqy4wnk6`.

`Protect recovery` wrote an encrypted NIP-49 file of 163 bytes with mode `0600`,
digest `e98ac608...`.
The product then reported:
`Your public identity is ready and an encrypted recovery file has been
verified.`

So the product wrote a recovery file and read it back.

## The clean-user editor journey

The profile is pristine.
The identity root, the configuration root, the database and the logs were all
made by this run under the isolated home.
No owner record was copied in.

| Step | Result |
| --- | --- |
| Open one local project | window title becomes `proj-emul` |
| Edit and save one file | `cc0f721d...` before, unchanged after typing, `e7f131a2...` after the save |
| Git status | the panel shows `Changes (1)`, `main.rs`, `+1 -0`, `Tracked` and the diff |
| Terminal, by the palette | `proj-emul - zsh`, a live shell, and an echo renders its output |
| Restart with no path argument | the project, the edited file, the Git panel and the terminal all return |

`git status` reports `M src/main.rs` and `1 file changed, 1 insertion`.
The relaunch passed no path.
Restoration came from the profile alone.

This is omega#16 acceptance criterion 8, met as worded.

## The terminal chord

The action works from the palette.
The binding is `"ctrl-`": "terminal_panel::Toggle"` at
`assets/keymaps/default-macos.json` line 711 at the rc18 cut.

Four delivery attempts made no change:

- `key code 50` with control, from System Events
- `cliclick` with control held around the backtick
- `keystroke` backtick with control, from System Events
- `key code 50` with control, with the panel open and focused

One confound was cleared.
The first palette attempt failed with
`/usr/bin/login: Operation not permitted`.
A seatbelt-confined process cannot run a set-user-identity program.
This lane set the terminal program to `/bin/zsh` in its own configuration root,
and the terminal then opened and ran.
The owner's configuration file was not read and not changed.

The new control is the important part.
`ctrl-shift-g` delivered by the **same mechanism**, in the **same session**, to
the **same window** toggles the Git panel.
So a control-modified chord from this source does reach the product, and
``ctrl-` `` still does not fire.

A physical keypress is not a synthesized one.
This row stays open.
The omega#16 Scope table must not mark it covered.

## The custody-path tripwire

omega#8 asks whether the identity secret escapes into logs, telemetry, the
clipboard, diagnostics or crash output.

Earlier lanes could not answer it.
Delivering a canary through the custody path needs a recovery, and a recovery
needed the owner's material.

The emulated identity has its own material, so the canary is deliverable.

The canaries are the two recovery passwords, the two emulated public identities,
and the bech32 shapes `nsec1` and `ncryptsec1`.
A planted needle is found by the same expression that returns zero below, so a
zero is a result.

| Root | Files | Password hits | `nsec1` | `ncryptsec1` |
| --- | ---: | ---: | ---: | ---: |
| emulated database | 6 | 0 | 0 | 0 |
| emulated whole tree | 57 | 0 | 0 | 0 |
| emulated logs and telemetry | 2 | 0 | 0 | 0 |
| emulated configuration root | 3 | 0 | 0 | 0 |
| owner database | 6 | 0 | 0 | 0 |
| owner hang traces | 4 | 0 | 0 | 0 |
| crash reports | 25 | 0 | 0 | 0 |
| clipboard | | 0 | 0 | |
| disposable proof root | 5 | 0 | 0 | 1 |

The emulated whole tree holds the two public identities in its public identity
records, which is what those records are for.
The one `ncryptsec1` hit in the proof root is the encrypted recovery file
itself.

No tripwire fires.

## Zed isolation

| Oracle | Candidate | Control |
| --- | ---: | ---: |
| descriptors matching `zed` | 0 | 160 matching `omega` |
| engine child 1 | 0 | 5 |
| engine child 2 | 0 | 3 |
| engine child 3 | 0 | 1 |
| Zed data root files changed in the window | 0 | 57 files present |

`script/verify-omega-brand --app /Applications/Omega.app` exits `0`.
The same gate against the host's real `/Applications/Zed.app` exits `1` with 87
lines of findings, so the gate can fail.

One weakness is stated.
Zed is installed but was not running in this window, so the file-drift oracle
samples a quiet tree.
The descriptor oracle does not depend on that.

## Criterion 9 needs a fresh cut

The answer is yes, for two reasons.

**First.**
rc18 was cut before `8ab85f0df8`.
Its record carried `source.dirty: true` on the strength of the build's own
untracked output, `assets/licenses.md`.
A reviewer correctly read that as weakening the source binding.

**Second, and larger.**
The rc18 release record on this host no longer exists.
The file at `target/omega-rc/omega-v0.2.0-rc18-macos-arm64.release.json` was
overwritten at 06:14 by a dry-run record from a different commit.

| Field | Record now on disk | Record the reviewer read |
| --- | --- | --- |
| `source.commit` | `c086403aeb...` | `96306681f0...` |
| `execution.dry_run` | `true` | not a dry run |
| `digests.package_sha256` | `null` | `83e79f68...` |
| `notarization.attempted` | `false` | `submitted_and_stapled` |
| `digests.cargo_lock_sha256` | `32e7df5b...` | `ee2d742d...` |

The surviving `Cargo.lock.omega-v0.2.0-rc18` snapshot has digest `ee2d742d...`,
which matches the record the reviewer read and not the record now on disk.
That is what identifies the overwrite.

`script/generate-omega-identity-candidate-evidence` refuses with
`release record commit 'c086403aeb...' does not match checkout` and exits `2`.

The reviewer's binding was performed and is honest for the moment it ran.
It is no longer reproducible on this host.

rc19 must be cut from `origin/main`, which carries `8ab85f0df8`, and its record
must be kept.

## What each issue needs next

### omega#16

| Criterion | State |
| --- | --- |
| 1 to 7 | hold, per the rc18 review |
| 8, clean-user editor journey | **met by this record** |
| 9, evidence binds the digests | **not met on rc18**. A fresh cut is needed |
| 10, independent reviewer accepts | not met for this evidence set |

The terminal chord row stays open.

The falsifier does not fire.
No claim here rests on a unit test, a source scan, or a screenshot alone.
The evidence is the signed installed candidate and the running application.
No unclassified Zed product, service, package or state dependency was found.

The smallest remaining step is a fresh cut, then one review round over both this
record and the rc18 record.

### omega#8

| Gate | State |
| --- | --- |
| `recovery_scenarios` | **produced by this record** |
| `installed_secret_tripwires` | **produced, with a custody-path canary** |
| `owner_observation` | recorded as `performedBy: agent`, `ownerObservation: false` |
| `independent_verification` | open. This lane is the producer and may not sign |

`identity_ready` is reachable.
Custody moved from `Absent` to `Ready` twice.

`script/prove-omega-rc-install` cannot bind, because the rc18 release record no
longer binds the rc18 artifact.

The smallest remaining step is a fresh cut, then the proof harness, then one
review.

### omega#9

The epic closes only when omega#8 passes against the exact packaged candidate.
It stays open.

## Safety

- This lane never read, copied, moved or used the owner's key or recovery file.
- The owner's keychain was never in the search list of any process this lane
  started.
- `--uninstall` was never run against `/Applications`.
- The owner's instance was never restarted and never written into.
- The owner's configuration file was never written.
- `screen-reader-output` stays **waived, not passed**. It was not widened.
- `high-contrast` and `reduced-motion` stay **blocked**. Both owner flags read
  `0`, and the owner was not asked again.

## What a reviewer must reproduce

The full command list is in the evidence file under `reviewerPacket`.

A reviewer must also judge four questions this record cannot settle:

1. Is an isolated home with its own keychain a sound substitute for a second
   macOS user account for the clean-user criterion?
2. Is the port denial a sound substitute for quitting the owner's instance?
3. Does a synthesized chord that delivers `ctrl-shift-g` but not ``ctrl-` ``
   settle the terminal chord row? This record says it does not.
4. Does the release-record overwrite make rc18 unusable for criteria 6 and 9?
   This record says it does.
