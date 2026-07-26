# OMEGA-BRAND-06: rc18 installed observation record

- Date: 2026-07-26
- Packet: OMEGA-BRAND-06 and OMEGA-OID-09
- Issues: omega#16, omega#9, omega#8
- Lane: `claude/rc18-proof`
- STE issue: 9
- Glossary revision: openagents-ste-glossary-v1
- Status: producer record. Not signed. No issue closed.

## Result

This record re-binds the installed observation from `0.2.0-rc17` to
`0.2.0-rc18`.

The rc17 record binds itself to one package digest and one host binary.
rc18 carries a different host binary and about two thousand new lines in the
crates that generate strings.
So the rc17 journey, removal, network capture and file-descriptor results do
not carry, and this lane performed each of them again.

Every brand class is clean on rc18, with live controls.
The removal ran again under an isolated `HOME`.
The editor journey ran again.
Offline first start ran again, by a method that did not take the host network
down.

rc18 does not close omega#16, omega#9 or omega#8.
Four named gaps remain, and each is stated below.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc18` |
| Omega commit | `96306681f0` |
| Artifact | `Omega-v0.2.0-rc18-macos-arm64.dmg` |
| Package sha256 | `83e79f68ede7cafa8262082f6c746bb5915e2fa086cde64336189db648ee0a37` |
| Bundle identifier | `com.openagents.omega.rc` |
| Bundle version | `20260726.091602` |
| Code directory hash | `bb404eb3d71759e48762f68574a9ba2f0363da8e` |
| Engine | `omega-effectd-v0.1.0-rc.10` |

Every statement below applies to that package digest.
I computed each value from the artifact.
I read no value from the release record and no value from the release notes.

## Package verification

- `shasum -a 256` on the disk image gives `83e79f68...`.
- The three executables at `/Applications/Omega.app` are byte-identical to the
  ones inside the disk image.

| Executable | sha256 |
| --- | --- |
| `Contents/MacOS/omega` | `4b2a5e9a845233edda18664cb72a1c236db76772a123ddd730b9a50b05ce50e9` |
| `Contents/MacOS/cli` | `f240095133d092dc0465b45df4dd63a85811bccacaecce610d5ff7453e9eac53` |
| `Contents/MacOS/omega-identity-proof` | `dee3d046c44eecc9e26a94d7521679b5147858ab7f03f140ddbd3a71b286b098` |

- The installed application and the disk image agree on three independent
  identifiers: bundle version, host binary digest, and code directory hash.
  So the installed product is the candidate, and no substitution happened.
- `codesign` reports `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`,
  team `HQWSG26L43`, and the hardened runtime flag.
- `codesign --verify --deep --strict` gives `valid on disk` and
  `satisfies its Designated Requirement`.
- `spctl -a -t install` on the disk image and `spctl -a -t exec` on a copy taken
  out of the image both give `accepted` and `source=Notarized Developer ID`.
- `xcrun stapler validate` passes on the disk image and on the copy taken out of
  the image. The ticket travels with the copy.
- Bundle metadata reads `com.openagents.omega.rc`, `Omega`, `0.2.0`, and icon
  `openagents-icon.icns`.
- The disk image carries `LICENSE-APACHE`, `LICENSE-GPL` and `licenses.md`.
- The engine bundle inside the candidate has digest
  `6922c83e05c89bc7a6ef44cc0f752a895a70b0cb1964d9c20613c538facd089c`, which is
  the digest the installed application also carries.

### The `cli` binary is not reproducible across cuts

The rc18 review recorded a correction that this lane confirms.
`crates/cli` did not change between the two cuts, and the `cli` binary changed
anyway: rc17 gives `811c788a...` and rc18 gives `f2400951...`.
No result about `cli` may be carried from one cut to another on the argument
that the crate did not change.

## Brand classes, counted from the candidate

Controls are included so a zero is a result and not a broken scan.
Each count comes from the binaries inside the read-only mount of the rc18 disk
image.

| Surface | rc16 | rc17 | rc18 |
| --- | ---: | ---: | ---: |
| `in Zed` | 0 | 0 | **0** |
| `You are the Zed coding agent` | 0 | 0 | **0** |
| `Zed Editor` request header | 0 | 0 | **0** |
| `Zed has been uninstalled` | 0 | 0 | **0** |
| `/Applications/Zed.app` in any binary | 0 | 0 | **0** |
| `Zed` in the signed `Info.plist` | 0 | 0 | **0** |
| `zed --existing` / `--classic` / `zed <path>` | 3 | 0 | **0** |
| Embedded `zed_*` image assets | 0 | 0 | **0** |
| `Omega` control, `omega` binary | 379 | 388 | 389 |
| `Omega Agent` control | 7 | 11 | 11 |
| `Omega` control, `cli` binary | — | 19 | 19 |

The three controls are non-zero, so the eight zeros are results.

The six reviewer-held forbidden strings each give 0 against the candidate
`omega` binary: `Unrecognized Project`, `Review .zed/settings.json`,
`Zed's hosted models`, `14 day free trial`, `Pro trial`,
`Move Omega to Applications`.

The rendered help of both shipped binaries is clean.
`cli --help` gives 0 hits for `zed`.
`omega --help` gives 1, which is the legacy `zed://` scheme, and that reference
is classified.

`script/verify-omega-brand --app` exits 0 on the extracted candidate.

## Retained identifiers a person can see, and their classification

A reader of the counts above could conclude that the word `Zed` is absent from
every surface. That is not true, and hiding the difference would be dishonest.
Three retained identifiers reach a user. Each resolves to the reviewed
classification registry, and none presents Zed as the product or the publisher.

| Identifier | Where a person meets it | Classification |
| --- | --- | --- |
| `Zed` in the Base Keymap list | First-run screen, beside `VS Code`, `JetBrains`, `Sublime Text`, `Atom`, `TextMate`, `Emacs` and `Cursor` | `zed_product`: names another editor's keymap preset |
| `zed://` | One line of `omega --help` | Legacy scheme kept so old links resolve |
| `.zed/settings.json` | Project settings folder Omega reads | Lookup path, not a claim. Renaming it would stop matching existing project settings |

The third one is a state dependency, so it is the one omega#16's falsifier
names directly. It is classified, and the classification is recorded in
`script/omega-brand-gate.json` rather than asserted here.

## The removal, performed against rc18

The rc17 removal result binds to the rc17 `cli` binary, and that binary
changed. So this lane ran the removal again.

The run used an isolated `HOME` and the candidate copy extracted from the disk
image. It never pointed at `/Applications`.

Before the run I planted Omega state at all nine planned roots, and I planted
decoy Zed state in the sibling shapes: `Library/Application Support/Zed`,
`Library/Logs/Zed`, `Library/Caches/dev.zed.Zed`, `.config/zed`, `.zed_server`,
`Applications/Zed.app` and `Library/Preferences/dev.zed.Zed.plist`.

The plan's first entry is the **application bundle**.
Nine removal paths, every one an Omega path.
One held-back path, `.config/omega-rc`, which the product prompts for.
Zero Zed paths, and zero entries naming `Contents/MacOS`.

After the removal:

- The bundle is gone. 0 files and 0 bytes remain.
- All nine planned roots are gone. The held-back configuration root is kept.
- The decoy Zed digest is
  `852706930985a2fff5be586785b0e299fc6cbde57a47a3e31cddcf4df295541f` before and
  after. All fourteen decoy files survive.
- The host's real `/Applications/Zed.app` is present and untouched.
- `/Applications/Omega.app` is untouched, because the removal never named it.

## Offline first start

The rc17 lane took the host network down for nineteen seconds.
Another lane was driving an unattended Full Auto run on this machine while this
lane worked, so taking the host network down would have damaged that run.

This lane denied the network to the candidate process instead.
The denial is narrower than a host outage and it is also stronger, because it
proves the property for this process rather than for the machine.

The controls make the denial a result:

- Under the deny profile, `curl` to `zed.dev` exits 7, and `curl` to
  `github.com` exits 7. Exit 7 means the connection could not be made.
- In the same second, outside the profile, `curl` to `github.com` returns 200.
  So the host network was up throughout, and the other lane was not disturbed.

The candidate started under that profile on a pristine profile directory,
reached a window, and stayed alive.

The first-start screen rendered with no network is the identity-first
onboarding. Read from the rendered pixels, top to bottom:

| Heading | Vertical position |
| --- | --- |
| `Your identity` | 0.167 |
| `Theme` | 0.385 |
| `Agent Setup` | 0.662 |

Identity is above Theme and above Agent Setup.
`Aiur`, `Ayu` and `Gruvbox` all render.
`Agent Setup` renders with `Codex`, `Cursor`, `Claude` and `GitHub Copilot`.

## The clean-profile journey

The profile is a fresh `--user-data-dir`.
Zed stayed installed and running throughout, so this is an install beside Zed.
The project is a fresh Git repository created for this lane. No real repository
was opened, and no real file was edited.

The owner's own Omega was running with a live session attached for the whole
window. This lane never restarted it and never wrote into it. The instance
identifier and the running time were checked before and after every step.

### How a second instance was started without touching the first

Omega allows one instance per release channel per user, and it enforces that
with a handshake on a loopback port.
This lane denied that one port to its own process, and denied nothing else.
The product then took its documented path for a port it cannot reach, which is
to continue without the handshake.

The controls again make this a result:

- Under the profile, a connection to the handshake port fails.
- Outside the profile, in the same second, that connection succeeds.
- A request to `github.com` under the same profile returns 200, so nothing else
  was blocked.

Because the handshake was unreachable, the second instance could not hand
anything to the first. That is the safety property this lane needed.

### Identity, and the gate that is working

On a clean profile the product refuses to adopt the identity it can see.
It renders the correct public identity and fingerprint, states
`Identity setup needs repair`, and says
`A prior recovery transaction needs the same owner-authorized identity candidate`.

The custody state is `Incomplete` with a pending recovery transaction.
The only action the screen offers is `Recover identity`, and that panel asks for
an encrypted recovery file or the advanced secret-key path.
I cancelled it.
I did not supply the owner's recovery material, and I did not read, move or
overwrite the owner's keychain secret.

This is recovery protection working, and it is the correct behavior.
It is also a gate on this lane, recorded below.

To reach the editor I seeded the clean profile with the owner's three **public**
identity records: `identity.json`, `identity.complete.json` and
`identity.recovery-protection.json`.
Those files carry a public key, a public identifier, a fingerprint, a keychain
locator and digests.
A scan of all three finds no secret shape.
This puts the profile in the existing-identity state rather than the pristine
state, and a reader must treat every journey result below as an
existing-identity result.

### What the journey performed

| Step | Result |
| --- | --- |
| Open one local project | Window title becomes `proj-rc18` |
| Edit and save one file | `main.rs` changes on disk, `67592f59...` to `ad6deb3e...` |
| Git status | Panel shows `Changes (1)`, `main.rs`, `+2 -0`, branch `main`, the diff |
| Terminal | `proj-rc18 - zsh`, a live shell, working directory is the project |
| Restart and layout restoration | Relaunched with **no path argument**, workspace returns |

The file digest was unchanged after the typing and changed after the save.
So the save wrote the file, and the typing alone did not.
`git status` then reports `M src/main.rs` and `1 file changed, 2 insertions`.

The terminal ran `echo OMEGA-RC18-TERMINAL-LIVE` and rendered the output.
The shell prompt shows the project directory.

The restart carried back the project, the open file with the edit, the Git panel
with `Changes`, the diff view and the terminal.
The relaunch passed no path.
Restoration came from the profile alone.

### One confound this lane created, and how it was cleared

The first attempt to open a terminal failed with
`/usr/bin/login: Operation not permitted`.
That is not a product defect. The deny profile this lane used to reach a second
instance also refuses to run a set-user-identity program, and `/usr/bin/login`
is one.
The lane set the terminal program to `/bin/zsh` in its own profile directory,
which is a documented product setting, and the terminal then opened and ran.
The owner's settings file was not read and not changed.

### The terminal keybinding

This is stated here and it is also listed as a gap, because the earlier record
kept it out of its gap list while marking the terminal row covered.

- `terminal panel: toggle` is in the command palette. Invoking it by name opens
  a terminal. The action works.
- `ctrl-` backtick is bound to `terminal_panel::Toggle` in the shipped macOS
  keymap at `assets/keymaps/default-macos.json:711` at the rc18 cut.
- That binding and that action name are both compiled into the shipped host
  binary.
- Neither `key code 50 using control down` nor
  `keystroke "\`" using control down` changes the terminal state.
- No enabled macOS symbolic hotkey claims key code 50. Nineteen symbolic
  hotkeys exist and none names that key code.
- Other chords reach the product in the same session. This lane drove the whole
  journey with `cmd-p`, `cmd-shift-p`, `cmd-s`, `ctrl-shift-g`, Return and
  Escape.

So the terminal is not broken, the binding is present, and no other program
claims the chord.
What is unresolved is chord **delivery**: whether a person pressing the physical
key reaches the action.
This lane cannot settle that, and it needs one owner keypress.

## Zed isolation

A whole-tree digest of Zed's data is not a valid oracle on this host, because
Zed runs and writes its own database log.
This lane used three oracles instead.

1. **Path set.** 57 files before, 57 after, and the sets are identical.
   The candidate created and removed nothing under Zed's root.
2. **Per-file digests against a control window.** Zero files changed during the
   Omega window. A 90-second control window with the candidate not running also
   showed zero drift, so the control is honest about the sample it takes.
3. **File descriptors.** `lsof` on the candidate gives **0** open paths matching
   `zed`, and the same command on the same process finds 6 paths matching
   `omega`. The engine child also gives 0. The owner's installed rc18 instance
   gives 0 against a control of 14.

The candidate never opened a Zed path.

The candidate also held 0 descriptors under the owner's configuration
directory, which is the separate-roots claim seen from the process side.

## Network destinations

`script/omega-network-capture-journey capture --journey online_first_codex` ran
for 60 seconds and gives `passed`.
`validate` on the receipt exits 0, which re-runs the live signing check and
re-derives the classification rather than trusting the receipt.

The capture binds to the installed root executable, and the receipt records that
executable's digest as `4b2a5e9a...`, which is the rc18 host binary.

Three destinations were observed:

| Destination | Meaning |
| --- | --- |
| `2606:4700:3032::6815:1c1d` port 443 | `cdn.agentclientprotocol.com`, an approved host |
| `lb-140-82-114-6-iad.github.com` port 443 | `github.com`, approved for the Omega repository |
| `localhost` port 11434 | local model server, loopback |

`forbidden_hosts_detected`, `unreviewed_hosts` and `unresolved_destinations` are
all empty.

rc17 saw two destinations and rc18 sees three. The added destination is
`github.com`, which the allowlist approves for the Omega repository.

The allowlist names ten Zed hosts as forbidden on a normal start, including
`zed.dev`, `api.zed.dev`, `cloud.zed.dev` and `collab.zed.dev`.
**None received a request.**

## Disabled service states

The signed binary carries the disabled-service copy that a user sees:
`Hosted billing is unavailable in Omega`, and `unavailable in Omega by default`
for the built-in edit prediction model.

The observed traffic agrees with the compiled policy.

Update behavior was not exercised. That is unchanged from rc17.

## Secret tripwires

The scanner fires.
Its own self-test passes, and a planted needle in a scanned directory is found
by the same expression that returns zero below.

**What was searched.** Every surface omega#8 names, in both the journey profile
and the owner's real profile, for the custody shapes themselves.

| Root | Files | Custody-shape hits |
| --- | ---: | ---: |
| journey profile logs | 2 | 0 |
| journey profile database | 6 | 0 |
| journey profile, whole tree | 61 | 0 |
| owner profile logs | 2 | 0 |
| owner profile database | 6 | 0 |
| owner profile hang traces | 4 | 0 |
| crash reports | 24 | 0 |

Zero, in both profiles.

**Why this is still the wrong proposition.**
omega#8 asks whether the identity secret escapes into those surfaces.
Delivering a canary through the custody path means performing a recovery, and
recovery on this host requires the owner's encrypted recovery file or secret
key.
I refused to touch the owner's key.
So this evidence answers a weaker question than the one omega#8 asks, and it
says so rather than letting a reader assume otherwise.

## Packaged secret-symbol scan

Each forbidden symbol gives 0 against the candidate `omega` binary:
`get_nsec`, `BUZZ_PRIVATE_KEY`, `identity.key`.
The control `omega-sovereign-identity-v1` gives 1, so the scan can find a
string that is present.

The prefixes `nsec1` and `ncryptsec1` each appear once, in one string.
That string is the redaction table, which lists the markers the redactor
searches for beside `bearer`, `authorization` and `github_pat_`.
No bech32 payload follows either prefix anywhere in the binary, so no key is
present.

The reviewed dependency versions are locked at the rc18 cut: `nostr 0.44.4`,
`keyring 3.6.3`, and `atomic-write-file 0.3.0`.

## omega#16 Scope, item by item

The terminal row is split, because the earlier record marked it covered while a
part of it was unresolved.

| Scope item | rc17 | rc18 |
| --- | --- | --- |
| Verify the candidate digest | covered | **covered** |
| Install beside Zed in a clean profile | covered | **covered** |
| Offline first start | covered | **covered** |
| Identity-first entry surface | covered | **covered**, offline |
| Open a project, edit and save, Git status | covered | **covered** |
| Open a terminal, by the palette | covered | **covered** |
| Open a terminal, by the shipped chord | covered | **open**, delivery unresolved |
| Restart and layout restoration | covered | **covered** |
| Visible surfaces for Zed branding | covered | **covered** |
| Icons, metadata, associations, disk image, licenses | covered | **covered** |
| Network destinations | covered | **covered** |
| Data, cache, log, credential roots | covered | **covered** |
| Disabled service states and update behavior | partly | **partly**, states covered, update not exercised |
| Remove Omega, Zed data unchanged | covered | **covered and performed** |
| Owner observation and independent verification | open | **open** |

## Falsifiers, addressed

### omega#16

> Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package,
> or state dependency.

**The first part does not fire.**
No claim here rests on a unit test.
The evidence is the signed, notarized and stapled candidate and the running
application: the package assessment, the ticket that travels with a copy out of
the image, the rendered help of both shipped binaries, the removal the product
performed on itself under an isolated `HOME`, a file edited and saved on disk
through the product, a live shell inside it, a workspace restored from the
profile with no path argument, 60 seconds of observed sockets, and `lsof` on the
candidate's own descriptors.
Rendered pixels are read where the product publishes no accessibility
information, and they are never the only evidence for any claim.

**The second part does not fire.**
The three retained identifiers a user can meet are named above, and each
resolves to the reviewed classification registry.
`script/verify-omega-brand --app` exits 0 on the candidate.
No Zed service received a request.
The data, cache, log, state and credential roots are `Omega RC` and `omega-rc`
throughout, and the candidate held no Zed file descriptor.

### omega#8

> Unit tests, fixtures, or screenshots are the only evidence.

Does not fire for the parts that ran.
It does not need to fire, because omega#8 fails on coverage.

## What still blocks a close

Four gaps. Each is named, not softened.

1. **Independent review is unperformed, and I may not perform it.**
   omega#16's tenth acceptance criterion is that an independent reviewer accepts
   the evidence. I am the producer.
   The rc18 review that already exists accepts a different obligation and states
   in its own text that it does not satisfy this line.
   This is the single item standing between rc18 and omega#16.
2. **The custody-path tripwire is unproduced.**
   Delivering a canary through the custody path means an identity recovery, and
   recovery on this host requires the owner's material. I refused.
   The evidence above answers a weaker question and says so.
3. **The omega#8 gate matrix is unmet.**
   `script/prove-omega-rc-install` expects eleven identity gates.
   Three of them cannot be reached from inside this lane: `owner_observation`
   needs the owner, `independent_verification` needs a reviewer, and
   `recovery_scenarios` needs the owner's recovery material.
   The installed observation also reads an identity state of `Identity ready`,
   and the custody state on this host is `Incomplete`.
   Only a recovery moves it, and only the owner can perform one.
4. **Terminal chord delivery is unresolved.**
   The action works, the binding ships, and no other program claims the chord.
   Whether a person pressing the physical key reaches the action is not settled,
   and it needs one owner keypress.

`screen-reader-output` remains **waived, not passed**.
The candidate publishes no application accessibility information, and the
upstream editor publishes none either.
A report carrying it reads `passed_with_waivers` and never `passed`.
I did not widen the waiver.

`high-contrast` and `reduced-motion` are recorded **blocked**.
Both owner flags are off on this host. Per owner direction, I did not ask again.

## What a reviewer must reproduce

I am the producer and I must not sign this.

```sh
# package, from the released artifact
shasum -a 256 Omega-v0.2.0-rc18-macos-arm64.dmg   # 83e79f68...
xcrun stapler validate Omega-v0.2.0-rc18-macos-arm64.dmg
spctl -a -vvv -t install Omega-v0.2.0-rc18-macos-arm64.dmg
hdiutil attach Omega-v0.2.0-rc18-macos-arm64.dmg -mountpoint /tmp/m -nobrowse -readonly
ditto /tmp/m/Omega.app /tmp/copy/Omega.app
codesign -dv --verbose=4 /tmp/copy/Omega.app          # CDHash bb404eb3...
codesign --verify --deep --strict --verbose=2 /tmp/copy/Omega.app
xcrun stapler validate /tmp/copy/Omega.app
spctl -a -vvv -t exec /tmp/copy/Omega.app

# the installed application is the candidate, on three identifiers
shasum -a 256 /Applications/Omega.app/Contents/MacOS/omega   # 4b2a5e9a...
shasum -a 256 /Applications/Omega.app/Contents/MacOS/cli     # f2400951...
codesign -dv --verbose=4 /Applications/Omega.app 2>&1 | grep CDHash=   # bb404eb3...
/usr/libexec/PlistBuddy -c "Print :CFBundleVersion" \
  /Applications/Omega.app/Contents/Info.plist          # 20260726.091602

# brand classes, with controls so a zero is a result
strings -a /tmp/copy/Omega.app/Contents/MacOS/omega | grep -c "in Zed"    # 0
strings -a /tmp/copy/Omega.app/Contents/MacOS/omega | grep -c "Omega"     # 389, control
strings -a /tmp/copy/Omega.app/Contents/MacOS/omega | grep -c "Omega Agent"  # 11, control
strings -a /tmp/copy/Omega.app/Contents/MacOS/cli   | grep -c "Omega"     # 19, control
plutil -convert xml1 -o - /tmp/copy/Omega.app/Contents/Info.plist | grep -ic zed  # 0
/tmp/copy/Omega.app/Contents/MacOS/omega --help | grep -ic zed  # 1, the legacy scheme
/tmp/copy/Omega.app/Contents/MacOS/cli   --help | grep -ic zed  # 0

# the removal. BOTH conditions matter, and never point this at /Applications.
ISO=$(mktemp -d)
ditto /tmp/copy/Omega.app "$ISO/Applications/Omega.app"
env HOME="$ISO" OMEGA_UNINSTALL_DRY_RUN=1 \
  "$ISO/Applications/Omega.app/Contents/MacOS/cli" --uninstall

script/verify-omega-brand --app /tmp/copy/Omega.app     # exits 0

# the network receipt re-derives its own classification
python3 script/omega-network-capture-journey validate \
  --candidate-digest 83e79f68ede7cafa8262082f6c746bb5915e2fa086cde64336189db648ee0a37 \
  --input <the capture receipt>
hdiutil detach /tmp/m
```

Then the review itself, which needs a key this lane does not hold:

```sh
OMEGA_REVIEWER_KEY_FILE=<a reviewer keypair that is not a producer key> \
script/review-omega-candidate \
  --candidate-dmg Omega-v0.2.0-rc18-macos-arm64.dmg \
  --producer-claim <the claim below> \
  --obligation omega#16 \
  --output <receipt.json>
```

The producer claim is:

```json
{
  "artifactSha256": "83e79f68ede7cafa8262082f6c746bb5915e2fa086cde64336189db648ee0a37",
  "notarization": "notarized",
  "stapled": "stapled",
  "forbiddenStrings": "none",
  "obligation": "omega#16"
}
```

Three standing notes for whoever signs.

**The harness forbidden-strings check is six needles.**
It is not the eight-class brand scan omega#16 requires. A clean harness result
must not be read as satisfying omega#16's brand criteria. Run the eight classes
separately, with the controls above.

**`script/review-omega-candidate` binds to the candidate.**
The earlier warning that only two of its four checks read the disk image is
stale. It was fixed before the rc18 cut, `--app` no longer exists, and all four
checks read the application inside the candidate disk image.

**`first_party_agent.phrases` runs automatically** inside
`script/verify-omega-brand --app`. Any record claiming a hand-run is stale.
