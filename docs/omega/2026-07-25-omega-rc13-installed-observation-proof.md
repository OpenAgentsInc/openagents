# Omega `0.2.0-rc13` installed observation proof

- Status: producer record. rc13 does not close omega#16.
- Owner: OpenAgents
- Date: 2026-07-25
- Audience: release, assurance, and review
- Issues: [omega#16](https://github.com/OpenAgentsInc/omega/issues/16),
  [omega#9](https://github.com/OpenAgentsInc/omega/issues/9),
  [omega#8](https://github.com/OpenAgentsInc/omega/issues/8)
- Predecessor: `2026-07-25-omega-rc11-installed-observation-proof.md`

I am the producer. I did not review this record. I must not sign it.

## Authority for this record

The owner removed the observation reservation on 2026-07-25. Under that grant
an agent can make an emulated user profile and can do the installed journeys.
An agent journey is not an owner observation. Each observation in this record
shows `performed_by: agent` and shows the name of the emulated profile.

`script/collect-omega-installed-observations` refuses an observation that is
only asserted. This record did not make that refusal weaker.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc13` |
| Omega commit | `abfc70db89e59739b7fd000e228f7bae7d5ab9a5` |
| Artifact | `Omega-v0.2.0-rc13-macos-arm64.dmg` |
| Package sha256 | `bd5e0ec75b1e8d910d77e4c226e60d431cb061f46fa41bf7c3e3f4019027ffec` |
| Candidate digest | `2847fc3c5ae6dd7c1a6db7ccce643e4cdd2e9b57720aab501de378b936de64e0` |
| Bundle identifier | `com.openagents.omega.rc` |

Each statement below applies to that candidate digest.

## Package verification: clean

I made these results from the artifacts. I did not read them from the release
record.

- `shasum -a 256` on the released disk image gives `bd5e0ec7…`.
- The disk image that this host built has the same digest as the released disk
  image.
- `spctl -a -vvv -t install /Applications/Omega.app` gives `accepted`,
  `source=Notarized Developer ID`, and
  `origin=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
- `codesign --verify --deep --strict` gives `valid on disk` and
  `satisfies its Designated Requirement`.
- The installed `omega` binary has digest `8ccca2e4…` and the installed `cli`
  binary has digest `da0dfc6d…`. Both agree with the release record.

### The rc11 qualification is gone

`xcrun stapler validate /Applications/Omega.app` gives
**`The validate action worked!`**

rc11 stapled only the disk image, so a copy that a person dragged out of the
disk image had no ticket. Gatekeeper acceptance could then use an online
lookup, and offline first start was not provable. rc13 staples the application
and the disk image. The ticket now moves with the copy.

This removes the cause of the rc11 blocker. It is not the same as an offline
start. See "Not performed" below.

## Brand result: clean on each gated surface, and not clean in prose

### Each gated surface is clean

| Surface | rc13 |
| --- | ---: |
| `zed` in the signed `Info.plist` | 0 |
| Embedded `zed_*` icon or image assets | 0 |
| `Zed Agent` | 0 |
| `Use GitHub Copilot in Zed` | 0 |
| `Welcome to Zed` | 0 |
| `About Zed` | 0 |

The one remaining asset is `icons/ai_zed.svg`. It identifies Zed's keymap
preset and Zed's hosted model service, which is a permitted third-party
reference.

The `zed:` command namespace that rc11 showed in the command palette is gone.
80 `zed::` names stay in the binary, and each one is a deprecated alias that
keeps an inherited keymap correct. Each palette entry names an `omega::`
target.

`script/verify-omega-brand --app /Applications/Omega.app` exits `0`.

### The packaged first-party-agent scan, for omega#75

The omega#75 lane asked for a scan of the packaged binary against
`first_party_agent.phrases`, not only against `brand.words`. I did that scan.

| Item | Hits in the packaged binary |
| --- | ---: |
| `zed agent` | 0 |
| `zed's agent` | 0 |
| `zed's native agent` | 0 |
| `zed's first-party agent` | 0 |
| `zed's own agent` | 0 |
| `zed-agent-terminal-` | 0 |
| `ZED_AGENT_ID` | 0 |
| `Omega Agent` (anti-vacuity) | 7 |

The result is clean. **The gate does not make this result.**
`script/verify-omega-brand` never reads `first_party_agent` at all, for the
source tree or for a package. Only the Rust test suite reads it, and that test
walks the source tree. The gap is still present after the rc14 prose change.
A reviewer must repeat this scan by hand until a gate reads that list against a
package.

### Prose fails omega#16

The signed and notarized rc13 binary holds **207 distinct sentences that name
Zed as the product**. Examples:

| Surface | Text |
| --- | --- |
| Provider onboarding | `Click 'Connect' below to start using Ollama in Zed` |
| Title bar | `Checking for Zed Updates…` |
| The user's browser | `Authorization Successful — Zed` |
| The model, on each turn | `You are the Zed coding agent running inside the Zed editor.` |
| The user's repository | `# ====== Auto-added by Zed: =======` |
| Settings editor | about 65 schema descriptions such as `Settings related to calls in Zed` |

`crates/app_identity/fixtures/compatibility_allowlist.json` has no entry for
any of the 207. Each of its 17 entries is an identifier class or one blocked
phrase. None of them is a prose disposition.

The omega#16 criterion *"No public surface presents Zed as the product or
publisher"* fails on rc13. The criterion *"Every retained Zed identifier is in
the reviewed compatibility allow-list"* also fails on rc13.

`OMEGA-DELTA-0031` (omega commit `9c00b53658`) corrects this on `main`, and
`0.2.0-rc14` (omega commit `8b2cee71bf`) is the first candidate that carries
the correction. **omega#16 stays open until an installed rc14 shows the
correction.** The delta from this record is small: the prose change touches no
packaging, signing, or notarization path.

## Installed observations against `2847fc3c…`

`script/collect-omega-installed-observations` ran against the installed rc13 in
the emulated clean profile `omega-qa-clean-01`. The rc11 observations do not
move to rc13. Each result below comes from this binary.

| Check | rc13 |
| --- | --- |
| `zed-data-before-after-isolation` | **performed** |
| `identity-first-first-run` | **performed** |
| `theme-and-agent-setup-baseline` | **performed** |
| `viewport-360-pixels` | **performed** |
| `larger-ui-font` | **performed** |
| `light-theme` | **performed** |
| `dark-theme` | **performed** |
| `screen-reader-output` | **waived**. Not observed. Not a pass. |
| `keyboard-focus-traversal` | blocked. One part of four did not repeat. |
| `high-contrast` | blocked. Owner act. |
| `reduced-motion` | blocked. Owner act. |

Collector status: **`incomplete`**. It is not `passed`. It is not
`passed_with_waivers`. `validate_installed_observations` refuses this record,
which is correct.

Seven checks are performed. rc11 also performed seven, but two of these seven
are checks that no earlier candidate could perform at all.

### The two journey checks that were never performable

rc10, rc11 and rc13 all record `blocked` for `identity-first-first-run` and
`theme-and-agent-setup-baseline`, because both checks read the macOS
accessibility tree and Omega publishes none. The owner has directed that this
release does not add assistive technology while Zed omits it. A check that
waits for a tree therefore waits for something that nobody will build.

What those two checks state is not a fact about assistive technology. It is a
fact about the first-run screen and the order of its sections. That is
rendered pixels. Omega commit `d313332042` makes both checks read the window
with optical character recognition, which also gives the position of each
line.

Result on rc13, from the rendered window:

| Heading | Position down the window |
| --- | ---: |
| `Your identity` | 0.167 |
| `Theme` | 0.385 |
| `Agent Setup` | 0.662 |

Identity is above Theme and above Agent Setup in one capture. The three theme
families `Aiur`, `Ayu` and `Gruvbox` are rendered, and `Agent Setup` is
rendered.

A pixel read is stronger than a tree read. A label that a tree carries but the
screen cuts off passes a tree read and fails a pixel read.

### Three refusals that this harness did not have

The same commit adds three refusals. Each one closes a way for the harness to
record one application while it observes a different application.

1. **A name is not an identity.** This host runs the installed candidate, a
   copy under an emulated profile, and a development build. macOS reports all
   three as `omega`. The harness now addresses one process identifier, and the
   frontmost guard compares the identifier.
2. **Frontmost is not visible.** An application can hold the front while its
   window is on a space that the display does not show. A region capture then
   reads the windows that the display does show. This was not a theory: before
   the guard, a first-run capture recorded a web browser. Each capture now asks
   the window server whether the window is on screen, and writes nothing if it
   is not.
3. **An empty accessibility tree and no tree are different.** A tree that
   carries no window at all is a weaker publication than an empty tree, and it
   is not the same as an application that does not run. The parity probe
   reported the first as a harness failure.

### Zed isolation

Digest over each regular file under `~/Library/Application Support/Zed`,
across the full exercise. The exercise includes first start, onboarding
render, both appearances, a settings write and restore, a 360-point viewport,
keyboard traversal, and removal of the emulated profile.

```
before: 129000b4f795b72b6098da79d25353733bb6ff1bc72da0bfb039b0daa024602d
after:  129000b4f795b72b6098da79d25353733bb6ff1bc72da0bfb039b0daa024602d
```

The two digests agree. The upstream parity probe ran before the `before`
digest, so Zed's own start is outside the measured period.

### The screen-reader waiver

Observed on rc13. Not assumed.

| Product | Version | `entire contents` | Direct children |
| --- | --- | --- | --- |
| Omega RC | `0.2.0-rc13` | 0 | close, full screen, minimize |
| Zed | `1.12.0` | 0 | close, full screen, minimize |

macOS supplies those three buttons. Neither product publishes an application
accessibility tree, so the owner parity condition of 2026-07-25 holds.

**No assistive technology read this candidate. No speech, no braille, and no
announcement was made or recorded.** A waived entry holds the owner's exact
words and no facts. A passed entry holds facts and no waiver. The two shapes
are different. `screen-reader-output` is the only waivable check.
`viewport-360-pixels` and `larger-ui-font` are ordinary rendering conditions,
and this run performed both.

### The two owner acts

`high-contrast` and `reduced-motion` read
`defaults read com.apple.universalaccess`. I tried to set both flags. macOS
refused:

```
Could not write domain com.apple.universalaccess; exiting
```

The domain has privacy protection. Only the owner can set those two flags, in
**System Settings > Accessibility > Display**. This is a real owner act, and
the owner grant does not remove it.

## omega#16 Scope, item by item

| Scope item | rc13 |
| --- | --- |
| Verify the candidate digest | covered |
| Install beside Zed in a clean user profile | **covered**. Emulated profile `omega-qa-clean-01` under `--user-data-dir`, beside the installed Zed 1.12.0. |
| Offline first start | **open**. The application now carries a stapled ticket, which is the condition rc11 lacked. The start itself needs a host with no network. |
| Identity-first entry surface | **covered** |
| Open a project, edit and save a file, Git status, terminal | **partly**. Project, Git and terminal panels open from the keyboard. Edit and save is open. |
| Restart and layout restoration | not repeated on rc13 |
| Visible product surfaces for Zed branding | **FAILED**. 207 prose sentences. |
| Icons, bundle metadata, associations, installer, disk image, licenses | **covered** for icons and metadata. `Info.plist` is clean and no `zed_*` asset ships. |
| Network destinations | not repeated on rc13 |
| Data, cache, log, credential roots | **covered**. `Omega RC` and `omega-rc` throughout, and the emulated profile holds each root. |
| Disabled service states and update behavior | **open** |
| Remove Omega, confirm Zed data unchanged | **partly**. Removal of the emulated profile leaves Zed data byte-identical. Removal of the shared `/Applications` bundle is not performed, because another lane runs against that bundle. |
| Owner observation and independent verification | **open**. I am the producer. I must not sign either. |

## Not performed, and why

- **Offline first start.** This needs the host network to stop. Other lanes on
  this host need their relay and their GitHub access. I did not stop the
  network without agreement.
- **Edit and save.** This needs synthesized keystrokes into an editor buffer.
  macOS sends a synthesized key to the frontmost application, not to the named
  process. The harness now checks the process identifier and the window
  visibility before each key, and both checks failed intermittently while other
  lanes took the front. An unforced observation is worth more than a forced
  one.
- **The fourth part of `keyboard-focus-traversal`.** Three parts repeat: each
  of three surfaces renders a window that the baseline does not, a forward move
  repaints the window well past the stated minimum, and a reverse move returns
  the window close to the earlier image. The fourth part opens a panel with a
  chord and closes it with the same chord. The two captures were identical, so
  the harness recorded no observation.
- **`high-contrast` and `reduced-motion`.** Owner acts. See above.

## Falsifiers, addressed

### omega#16

> *Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package,
> or state dependency.*

The second part fires. 207 user-facing sentences name Zed as the product, and
the compatibility allow-list classifies none of them. `OMEGA-DELTA-0031` and
rc14 correct this, and rc13 does not carry the correction.

The first part does not fire for this record. The evidence is the installed,
signed, and notarized candidate: its package assessment, its stapled ticket,
its binary strings, its rendered window, and the Zed data digest on this host.

No Zed service received a request during this exercise.

### omega#8

> *Unit tests, fixtures, or screenshots are the only evidence.*

This does not fire. Each observation here is a read of the installed candidate
while it ran. The record is bound to the candidate digest, and the collector
refused the checks that this host could not perform.

## What must happen next

1. Cut `0.2.0-rc14` and install it. The prose correction is already on `main`.
2. Repeat this record against rc14. The delta is small, because the prose
   change touches no packaging or signing path.
3. Give the harness one host period with the front and with no network, for
   offline first start, for edit and save, and for the panel chord.
4. Ask the owner to set **Increase contrast** and **Reduce motion** in System
   Settings > Accessibility > Display, then repeat the two checks.
5. Independent review. The reviewer must not be the producer.

## Reproduction for the reviewer

```sh
# package, from the released artifacts
shasum -a 256 Omega-v0.2.0-rc13-macos-arm64.dmg
spctl -a -vvv -t install /Applications/Omega.app
codesign --verify --deep --strict --verbose=2 /Applications/Omega.app
xcrun stapler validate /Applications/Omega.app       # expect: the validate action worked
xcrun stapler validate Omega-v0.2.0-rc13-macos-arm64.dmg

# gated surfaces
plutil -convert xml1 -o - /Applications/Omega.app/Contents/Info.plist | grep -ic zed   # 0
strings -a /Applications/Omega.app/Contents/MacOS/omega \
  | grep -oE '[a-z_/]*\.svg' | sort -u | grep -i zed                                   # icons/ai_zed.svg

# the packaged first-party-agent family, which no gate reads
for phrase in "zed agent" "zed's agent" "zed's native agent" \
              "zed's first-party agent" "zed's own agent" "zed-agent-terminal-"; do
  printf '%s: ' "$phrase"
  strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -ic "$phrase"
done
strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -c 'Omega Agent'        # anti-vacuity

# the prose failure
strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -c "in Zed"
strings -a /Applications/Omega.app/Contents/MacOS/omega \
  | grep -c "You are the Zed coding agent running inside the Zed editor"

# the observation set, in an emulated profile beside Zed
swiftc -O -o /tmp/omega-window-list script/omega-window-list.swift
swiftc -O -o /tmp/omega-window-ocr  script/omega-window-ocr.swift
python3 script/observe-upstream-accessibility-parity \
  --window-list /tmp/omega-window-list --output /tmp/upstream-parity.json
python3 script/collect-omega-installed-observations \
  --candidate-evidence <candidate-evidence.json> --evidence-root <dir> \
  --zed-before <digest> --upstream-parity /tmp/upstream-parity.json \
  --pid <pid> --ocr /tmp/omega-window-ocr --window-list /tmp/omega-window-list \
  --settings-path <profile>/config/settings.json \
  --performed-by agent --profile omega-qa-clean-01 --output <out.json>
```

The reviewer must make each result from the artifacts and from the running
application. The reviewer must not accept a value from this document.
