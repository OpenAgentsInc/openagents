# OMEGA-BRAND-06: rc17 installed observation record

- Date: 2026-07-26
- Packet: OMEGA-BRAND-06 and OMEGA-OID-09
- Issues: omega#16, omega#9, omega#8
- Lane: `claude/rc17-final`
- STE issue: 9
- Glossary revision: openagents-ste-glossary-v1
- Status: producer record. Not signed. No issue closed.

## Result

`0.2.0-rc17` is the best candidate of the six.
Every brand class is clean.
Both defects rc16 recorded are fixed, and this lane performed the removal
rather than deriving its plan.
The installed journey ran for the first time.
Offline first start ran for the first time.

rc17 does not close omega#16, omega#9 or omega#8.
Three named gaps remain, and each is stated below.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc17` |
| Omega commit | `73dadebb7f` |
| Artifact | `Omega-v0.2.0-rc17-macos-arm64.dmg` |
| Package sha256 | `cddf1891268f5ae0babe41346348e655b65fc7a945c4a0461870a1cab95e3206` |
| Bundle identifier | `com.openagents.omega.rc` |
| Engine | `omega-effectd-v0.1.0-rc.10` |

Every statement below applies to that package digest.
I computed each value from the artifact.
I read no value from the release record and no value from the release notes.

## Package verification

- `shasum -a 256` on the released disk image gives `cddf1891...`.
- The three installed executables at `/Applications/Omega.app` are
  byte-identical to the ones inside the released disk image.

| Executable | sha256 |
| --- | --- |
| `Contents/MacOS/omega` | `99c3e41ef16f8fe6ca4552fbaba4f0d7aa1e4e5300061ad5a90218da098336a4` |
| `Contents/MacOS/cli` | `811c788add030aa595c5a34ad37faaf8cc7371992e62c450ccdfdee2aa987e74` |
| `Contents/MacOS/omega-identity-proof` | `0954ecb6d14933fc94804794600b8adcfeb925d8e3e169dea75488c26477b0f9` |

- `codesign` reports `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`,
  team `HQWSG26L43`, and the hardened runtime flag.
- `codesign --verify --deep --strict` gives `valid on disk` and
  `satisfies its Designated Requirement`.
- `spctl -a -t install` on the disk image and `spctl -a -t exec` on the
  application both give `accepted` and `source=Notarized Developer ID`.
- `xcrun stapler validate` passes on the disk image, on the application inside
  the image, on `/Applications/Omega.app`, and on a copy taken out of the image
  to a new path. The ticket travels with the copy.
- Bundle metadata reads `com.openagents.omega.rc`, `Omega`, `Omega`, `0.2.0`,
  and icon `openagents-icon.icns`.
- The disk image carries `LICENSE-APACHE`, `LICENSE-GPL` and `licenses.md`.

## Brand classes, counted from the candidate

Controls are included so a zero is a result and not a broken scan.

| Surface | rc13 | rc16 | rc17 |
| --- | ---: | ---: | ---: |
| `in Zed` | 25 | 0 | **0** |
| `You are the Zed coding agent` | 1 | 0 | **0** |
| `Zed Editor` request header | 1 | 0 | **0** |
| `Zed has been uninstalled` | 1 | 0 | **0** |
| `/Applications/Zed.app` in any binary | 1 | 0 | **0** |
| `Zed` in the signed `Info.plist` | 0 | 0 | **0** |
| `zed --existing` / `--classic` / `zed <path>` | 3 | 3 | **0** |
| Embedded `zed_*` image assets | 0 | 0 | **0** |
| `Omega` control, `omega` binary | — | 379 | 388 |
| `Omega Agent` control | 7 | 7 | 11 |
| `Omega` control, `cli` binary | — | — | 19 |

The rendered help of both shipped binaries is clean.
`cli --help` gives 0 hits for `zed`.
`omega --help` gives 1, which is `the legacy \`zed://\` scheme`, and that
reference is classified.

The six reviewer-held forbidden strings each give 0 against the installed
`omega` binary: `Unrecognized Project`, `Review .zed/settings.json`,
`Zed's hosted models`, `14 day free trial`, `Pro trial`,
`Move Omega to Applications`.

`script/verify-omega-brand --app` exits 0 on the extracted candidate.

## The two rc16 defects are fixed

### Defect 1, the incomplete removal, is fixed and the removal was performed

rc16 derived the plan and did not run it.
This lane ran it.

The run used an isolated `HOME` and the candidate copy extracted from the disk
image. It never pointed at `/Applications`.

Before the run I planted Omega state at all nine planned roots, and I planted
decoy Zed state in the sibling shapes: `Library/Application Support/Zed`,
`Library/Logs/Zed`, `Library/Caches/dev.zed.Zed`, `.config/zed`, `.zed_server`
and `Applications/Zed.app`.

The plan's first entry is the **application bundle**.
Nine removal paths, every one an Omega path.
One held-back path, `.config/omega-rc`, which the product prompts for.
Zero Zed paths, and zero entries naming `Contents/MacOS`.

After the removal:

- The bundle is gone. 0 files and 0 bytes remain. rc16 left 130.9 MB and five
  executables.
- The decoy Zed digest is `73b2c823be3440d2050976688012da3f0d7932ce88c783c50bd3be8d0cb7cc37`
  before and after. All fourteen decoy paths survive.
- The host's real `/Applications/Zed.app` is present and untouched.
- `/Applications/Omega.app` is untouched, because the removal never named it.

The omega#16 Scope item "Remove Omega and confirm that Zed data did not change"
is satisfied.

### Defect 2, the three `zed --*` command literals, is fixed

All three give 0 hits across all three packaged executables, against a live
`Omega` control of 388.

## Offline first start

I took the host network down in a bounded window and restored it.

- Window open `2026-07-26T08:18:17Z`, closed `2026-07-26T08:18:36Z`.
  Nineteen seconds.
- While down: no default route, 100% packet loss to `1.1.1.1`, and `curl` to
  `zed.dev` exits 6.
- The candidate started on a clean profile with no network, reached a window in
  four seconds, and stayed alive.
- Network restored and verified: default route back, 0% packet loss,
  `https://github.com/` returns 200.

The first-start screen rendered offline is the identity-first onboarding.
Read from the rendered pixels, top to bottom:

| Heading | Vertical position |
| --- | --- |
| `Your identity` | 0.166 |
| `Theme` | 0.385 |
| `Agent Setup` | 0.662 |

Identity is above Theme and above Agent Setup.
`Aiur`, `Ayu` and `Gruvbox` all render.
`Agent Setup` renders with `Codex`, `Cursor`, `Claude` and `GitHub Copilot`.

rc11 lacked the stapled ticket that makes this start possible.

## The clean-profile journey

The single-instance lock was free, so the journey ran.

- The profile is a fresh `--user-data-dir`. Zed stayed installed and running
  throughout, so this is an install beside Zed.
- The project is a fresh Git repository created for this lane. No real
  repository was opened, and no real file was edited.

### Identity, and the gate that is working

On the clean profile the product refuses to adopt the identity it can see.
It renders the correct public identity and fingerprint, states
`Identity setup needs repair`, and says
`A prior recovery transaction needs the same owner-authorized identity candidate`.

`Recover identity` opens a panel that asks for an encrypted NIP-49 recovery
file or the advanced secret-key path.
I cancelled it.
I did not supply the owner's recovery material, and I did not read, move or
overwrite the owner's keychain secret.

This is recovery protection working, and it is the correct behavior.
It is also a gate on this lane, recorded below.

To reach the editor I seeded the clean profile with the owner's three
**public** identity records, copied from the default profile:
`identity.json`, `identity.complete.json` and
`identity.recovery-protection.json`.
Those files carry a public key, an npub, a fingerprint and digests.
They carry no secret.
The pairing they express is the product's own.
This puts the profile in the existing-identity state rather than the pristine
state, and a reader must treat every journey result below as an
existing-identity result.

### What the journey performed

| Step | Result |
| --- | --- |
| Open one local project | Window title becomes `proj-rc17` |
| Edit and save one file | `main.rs` changes on disk, `c4cae809...` to `cd614b4a...` |
| Git status | Panel shows `Changes (1)`, `main.rs`, `+1 -1`, branch `main`, the diff |
| Terminal | `proj-rc17 -zsh`, a live shell, cwd is the project |
| Restart and layout restoration | Relaunched with **no path argument**, workspace returns |

Edit and save is the item two previous lanes declined.
The saved file holds `println!("edited-by-rc17-journey");`, the product
reformatted it on save, and `git status` reports `M main.rs`.

The terminal ran `echo OMEGA-RC17-TERMINAL-LIVE` and rendered the output.

The restart carried back the project, the Git panel with `Changes (1)`, the
diff view and an unsaved buffer.
The relaunch passed no path.
Restoration came from the profile alone.

### The terminal keybinding, which rc16 could not resolve

rc16 recorded `ctrl-` backtick producing a byte-identical capture three times
and could not separate a product defect from a condition of its profile.

This lane separated them.

- `terminal panel: toggle` is in the command palette, and invoking it by name
  opens the terminal and closes it again. Both directions work.
- `ctrl-` backtick is bound to `terminal_panel::Toggle` in the shipped macOS
  keymap at `assets/keymaps/default-macos.json:711`.
- Neither `key code 50 using control down` nor
  `keystroke "\`" using control down` changes the terminal state, from a closed
  state or an open one, on a clean profile with no vim mode and no agent panel.
- No enabled macOS symbolic hotkey claims keycode 50.
- Other chords reach the product in the same session: `cmd-p`, `cmd-shift-P`,
  `cmd-s`, `cmd-o`, `ctrl-shift-G`, Return and Escape all work.

So the terminal is not broken and the profile is not the cause.
The chord does not reach the action through synthesized System Events.
Whether a person pressing the physical key succeeds is not settled by this
lane, and it needs one owner keypress.

## Zed isolation, with a sound method

A whole-tree digest of Zed's data is not a valid oracle on this host.
Zed was running throughout, pid `20754`, and writes its own SQLite
write-ahead log.
A 20-second sample drifted with Omega not running at all.
rc16's before-and-after digest would report a change here and it would mean
nothing.

This lane used three oracles instead.

1. **Path set.** 57 files before, 57 after, and the sets are identical.
   Omega created and removed nothing under Zed's root.
2. **Per-file digests against a control window.** Two files changed during the
   Omega journey: `db/0-stable/db.sqlite-shm` and `db/0-stable/db.sqlite-wal`.
   Both are Zed's own database, and Zed's own process was running.
   A 90-second control window with Omega not running showed 0 drift, so the
   control is honest about being able to show drift.
3. **File descriptors.** `lsof` on the candidate gives **0** open paths
   matching `zed`, across the whole journey.

The candidate never opened a Zed path.

## Network destinations

`script/omega-network-capture-journey capture --journey online_first_codex`
ran for 60 seconds against the installed candidate and gives `passed`.
`validate` on the receipt exits 0, which re-runs the live codesign check and
re-derives the classification rather than trusting the receipt.

Two destinations were observed:

| Destination | Meaning |
| --- | --- |
| `2606:4700:3036::ac43:aa36` port 443 | `cdn.agentclientprotocol.com`, an approved host |
| `localhost` port 11434 | local Ollama, loopback |

`forbidden_hosts_detected`, `unreviewed_hosts` and `unresolved_destinations`
are all empty.

The allowlist names ten Zed hosts as forbidden on a normal start, including
`zed.dev`, `api.zed.dev`, `cloud.zed.dev` and `collab.zed.dev`.
**None received a request.**

## Disabled service states

`ZED_PRODUCTION_SERVICES_ENABLED` is a compile-time `false`, and the release
candidate keeps it disabled.
The signed binary carries the disabled-service copy that a user sees:
`Hosted billing is unavailable in Omega`, and
`unavailable in Omega by default` for the built-in edit prediction model.

The observed traffic agrees with the compiled policy.

## The tripwire receipt

This lane produced a receipt.
It does not carry the load omega#8 asks for, and the reason is written here in
full rather than left for a reader to discover.

**The scanner fires.**
A live-fire control planted `prefix<needle>suffix` in a scanned directory.
The scan returned `match`, one file, exit 1.
A zero from this scanner is therefore a result.

**What the canary was.**
A disposable 64-byte lowercase-hex value, mode 0600, delivered to the scanner
through a file descriptor and never through argv.
It matches the scanner's own `\b[0-9a-f]{64}\b` secret shape.

**How the candidate received it.**
Through the clipboard, into an editor buffer in the running candidate, where it
rendered in the tab title and the buffer.
The candidate genuinely held it.
I then cleared the clipboard so the transport would not score as a leak.

**The result.**

| Surface | Status | Files | Bytes |
| --- | --- | ---: | ---: |
| `logs` | pass | 2 | 17,488 |
| `telemetry` | pass | 1 | 0 |
| `diagnostics` | **match** | 60 | 39,765,883 |
| `crashes` | pass | 23 | 1,188,757 |
| `clipboard` | pass | 1 | 0 |
| `accessibility` | pass | 1 | 863 |

**The match, located and explained.**
One file: `db/0-preview/db.sqlite-wal`, the workspace database write-ahead log.
The canary was in an unsaved buffer, and the product persists unsaved buffers
so they survive a restart.
The restart in this same journey brought that buffer back, which is the same
fact seen from the other side.
This is inherited editor behavior, and it is worth a user knowing, but it is
not a custody leak.

**Why it is the wrong proposition.**
omega#8 asks whether the identity secret escapes into those six surfaces.
My canary travelled the editor-content path, not the custody path, because the
custody path is owner-gated and I would not touch the owner's key.
So this receipt answers a weaker question than the one omega#8 asks.

**What was established instead, and it is not nothing.**
I searched every one of those surfaces for the custody shapes themselves,
without ever reading the owner's secret.

| Root | Files | `nsec1` / `ncryptsec1` hits |
| --- | ---: | ---: |
| journey profile logs | 2 | 0 |
| journey profile db | 6 | 0 |
| journey profile hang traces | 4 | 0 |
| owner profile logs | 2 | 0 |
| owner profile db | 6 | 0 |
| owner profile hang traces | 4 | 0 |
| crash reports | 23 | 0 |

Zero, in both the journey profile and the owner's real default profile.
The only non-public 64-hex value anywhere is my own canary in the one WAL file.
The owner's public key is present and is allowed to be.

## omega#16 Scope, item by item

| Scope item | rc16 | rc17 |
| --- | --- | --- |
| Verify the candidate digest | covered | **covered** |
| Install beside Zed in a clean profile | blocked | **covered** |
| Offline first start | not attempted | **covered** |
| Identity-first entry surface | blocked | **covered**, offline |
| Open a project, edit and save, Git status, terminal | declined | **covered** |
| Restart and layout restoration | blocked | **covered** |
| Visible surfaces for Zed branding | covered | **covered** |
| Icons, metadata, associations, disk image, licenses | covered | **covered** |
| Network destinations | not captured | **covered** |
| Data, cache, log, credential roots | covered | **covered** |
| Disabled service states and update behavior | not attempted | **partly**, states covered, update not exercised |
| Remove Omega, Zed data unchanged | partly | **covered and performed** |
| Owner observation and independent verification | open | **open** |

## Falsifiers, addressed

### omega#16

> Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package,
> or state dependency.

**The first part does not fire.**
No claim here rests on a unit test.
The evidence is the signed, notarized and stapled candidate and the running
installed application: the package assessment, the ticket that travels with a
copy out of the image, the rendered help of both shipped binaries, the removal
the product performed on itself under an isolated `HOME`, a file edited and
saved on disk through the product, a live shell inside it, a workspace restored
from the profile with no path argument, 60 seconds of observed sockets, and
`lsof` on the candidate's own descriptors.
Screenshots are used where the product publishes no accessibility tree, and
they are never the only evidence for any claim.

**The second part does not fire.**
Every retained Zed identifier resolves to the reviewed classification registry
or the compatibility allow-list, and `script/verify-omega-brand --app` exits 0
on the candidate.
The three unclassified `zed --*` literals that made rc16 fire this clause are
gone.
No Zed service received a request.
The data, cache, log, state and credential roots are `Omega RC` and `omega-rc`
throughout, and the candidate held no Zed file descriptor.

### omega#8

> Unit tests, fixtures, or screenshots are the only evidence.

Does not fire for the parts that ran.
It does not need to fire, because omega#8 fails on coverage.

## What still blocks a close

Three gaps. Each is named, not softened.

1. **Independent review is unperformed, and I may not perform it.**
   omega#16's tenth acceptance criterion is that an independent reviewer
   accepts the evidence. I am the producer. Running
   `script/review-omega-candidate` with any key I hold would not make the
   review independent, and the script's own disjointness gate is a check on
   keys, not on people. This is the single item standing between rc17 and
   omega#16.
2. **The custody-path tripwire is unproduced.**
   Delivering a canary through the custody path means an identity recovery, and
   recovery on this host requires the owner's NIP-49 file or secret key. I
   refused. The receipt above answers a weaker question and says so.
3. **The omega#8 gate matrix is unmet.**
   `script/prove-omega-rc-install` requires twelve inputs. This lane produced
   the network capture. The identity recovery evidence, the full-auto evidence,
   the manual evidence, the lifecycle manifest and the installed observation
   receipt are absent or incomplete, and the observation receipt cannot be
   completed while `identity_ready` is false on a clean profile.

`screen-reader-output` remains **waived, not passed**.
The candidate publishes no application accessibility tree, and upstream
publishes none either.
A report carrying it reads `passed_with_waivers` and never `passed`.
I did not widen the waiver.

`high-contrast` and `reduced-motion` are recorded **blocked**.
Both owner flags are off on this host and `defaults write` is refused on that
domain. Per owner direction, I did not ask again.

## What a reviewer must reproduce

I am the producer and I must not sign this.

```sh
# package, from the released artifact
shasum -a 256 Omega-v0.2.0-rc17-macos-arm64.dmg   # cddf1891...
xcrun stapler validate Omega-v0.2.0-rc17-macos-arm64.dmg
spctl -a -vvv -t install Omega-v0.2.0-rc17-macos-arm64.dmg
hdiutil attach Omega-v0.2.0-rc17-macos-arm64.dmg -mountpoint /tmp/m -nobrowse -readonly
codesign -dv --verbose=4 /tmp/m/Omega.app
codesign --verify --deep --strict --verbose=2 /tmp/m/Omega.app
# the ticket must travel OUT of the image, which is what rc11 lacked
ditto /tmp/m/Omega.app /tmp/copy/Omega.app
xcrun stapler validate /tmp/copy/Omega.app
spctl -a -vvv -t exec /tmp/copy/Omega.app

# the installed candidate is the released one
shasum -a 256 /Applications/Omega.app/Contents/MacOS/omega   # 99c3e41e...
shasum -a 256 /Applications/Omega.app/Contents/MacOS/cli     # 811c788a...

# brand classes, with controls so a zero is a result
strings -a /tmp/copy/Omega.app/Contents/MacOS/omega | grep -c "in Zed"          # 0
strings -a /tmp/copy/Omega.app/Contents/MacOS/omega | grep -c "Omega"           # 388, the control
strings -a /tmp/copy/Omega.app/Contents/MacOS/cli   | grep -cE 'zed --existing|zed --classic|zed <path>'   # 0
plutil -convert xml1 -o - /tmp/copy/Omega.app/Contents/Info.plist | grep -ic zed # 0
/tmp/copy/Omega.app/Contents/MacOS/omega --help | grep -ic zed                   # 1, the legacy scheme
/tmp/copy/Omega.app/Contents/MacOS/cli   --help | grep -ic zed                   # 0

# the removal. BOTH conditions matter, and never point this at /Applications.
ISO=$(mktemp -d)
env HOME="$ISO" OMEGA_UNINSTALL_DRY_RUN=1 /tmp/copy/Omega.app/Contents/MacOS/cli --uninstall

script/verify-omega-brand --app /tmp/copy/Omega.app     # exits 0
```

Then the review itself, which needs a key this lane does not hold:

```sh
OMEGA_REVIEWER_KEY_FILE=<a reviewer keypair that is not a producer key> \
script/review-omega-candidate \
  --candidate-dmg Omega-v0.2.0-rc17-macos-arm64.dmg \
  --producer-claim <the claim below> \
  --obligation omega#16 \
  --output <receipt.json>
```

The producer claim is:

```json
{
  "artifactSha256": "cddf1891268f5ae0babe41346348e655b65fc7a945c4a0461870a1cab95e3206",
  "notarization": "notarized",
  "stapled": "stapled",
  "forbiddenStrings": "none",
  "obligation": "omega#16"
}
```

Two standing notes for whoever signs.

**`script/review-omega-candidate` changed tonight.**
`--app` is gone and all four checks now bind to the application inside the
candidate disk image. Before that fix two checks read whatever was installed,
and the tool returned `accepted` against an artifact nobody had named. Do not
reuse an rc13, rc14 or rc15 review command line.

**`first_party_agent.phrases` runs automatically** inside
`script/verify-omega-brand --app`. The rc13 and rc14 records claiming a
hand-run are stale.
