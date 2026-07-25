# Omega 0.2.0-rc10 installed observation proof

- Status: producer record, 2026-07-25
- Owner: OpenAgents
- Audience: release, assurance, and brand reviewers
- Lane: A1 of the
  [Omega master delegation plan](./2026-07-25-omega-master-delegation-plan.md)
- Issue: [omega#16](https://github.com/OpenAgentsInc/omega/issues/16),
  OMEGA-BRAND-06
- Related: [release readiness brand audit](./2026-07-24-release-readiness-brand-audit.md)

This record reports what a producer observed against the installed
`0.2.0-rc10` candidate. A producer cannot accept its own evidence. An
independent reviewer must reproduce the observations before any gate closes.

## 1. Result

**`0.2.0-rc10` fails omega#16.** Two Zed brand surfaces ship in the signed,
notarized candidate. Both are on surfaces that no earlier check could see.

Issue #16 stays open.

## 2. Candidate binding

| Item | Value |
| --- | --- |
| Version | `0.2.0-rc10` |
| Source commit | `b768854c56e3238e9971f8590ddd727e7233de8c` |
| Package | `Omega-v0.2.0-rc10-macos-arm64.dmg` |
| Package SHA-256 | `c8f74460a0c2219baedda9cdd395e61f6e2c7a36f1408c764cf97a1624b17c41` |
| Candidate digest | `ca0b2d51b222931a26cb368a99973cc2ae181f4e2e2ff6dfb9d064dc9014504a` |
| Bundle identifier | `com.openagents.omega.rc` |
| Team | `HQWSG26L43` |

Every observation below binds to that candidate digest.

## 3. Package verification

The producer ran each check and did not accept a prior claim.

| Check | Result |
| --- | --- |
| `spctl -a -vvv -t exec /Applications/Omega.app` | `accepted`, `source=Notarized Developer ID`, `origin=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)` |
| `codesign -dv --verbose=4` | `Identifier=com.openagents.omega.rc`, `TeamIdentifier=HQWSG26L43`, `flags=0x10000(runtime)`, `Timestamp=Jul 25, 2026 at 11:10:00 AM` |
| `codesign --verify --deep --strict` | `valid on disk`, `satisfies its Designated Requirement` |
| `xcrun stapler validate` on the disk image | `The validate action worked!` |
| Package digest against the release record | equal |
| `omega`, `cli`, and `omega-identity-proof` digests against the release record | all equal |
| Version, channel, artifact name, bundle identifier | all equal to the release record |

One qualification. The installed application carries **no stapled ticket**.
Stapling attaches to the disk image, and a copy taken out of that image does
not carry it. `spctl` accepts the installed copy on this host, but that
acceptance can rest on an online notarization lookup. An offline first start
on a host that has never seen this candidate is therefore **not proven** by
this record.

## 4. Brand failures in rc10

### 4.1 Thirteen Zed strings in the signed bundle metadata

`Contents/Info.plist` contains 13 strings that name Zed. The file is inside
the sealed, signed, notarized bundle, so these strings are part of the
candidate, not a local edit.

| Key | Value |
| --- | --- |
| `CFBundleDocumentTypes[1].CFBundleTypeName` | `Zed Text Document` |
| `NSAppleEventsUsageDescription` | `An application in Zed wants to use AppleScript.` |
| `NSBluetoothAlwaysUsageDescription` | `An application in Zed wants to use Bluetooth.` |
| `NSCalendarsUsageDescription` | `An application in Zed wants to use Calendar data.` |
| `NSCameraUsageDescription` | `An application in Zed wants to use the camera.` |
| `NSContactsUsageDescription` | `An application in Zed wants to use your contacts.` |
| `NSLocationAlwaysUsageDescription` | `... in Zed ... even in the background.` |
| `NSLocationUsageDescription` | `An application in Zed wants to use your location information.` |
| `NSLocationWhenInUseUsageDescription` | `... in Zed ... while active.` |
| `NSMicrophoneUsageDescription` | `An application in Zed wants to use your microphone.` |
| `NSRemindersUsageDescription` | `An application in Zed wants to use your reminders.` |
| `NSSpeechRecognitionUsageDescription` | `An application in Zed wants to use speech recognition.` |
| `NSSystemAdministrationUsageDescription` | `The operation being performed by a program in Zed requires elevated permission.` |

These are not internal strings. macOS renders a usage description **in the
system permission dialog**. The first time this product asks for the
microphone, the operating system tells the user that an application in Zed
wants it. `CFBundleTypeName` appears in Finder Get Info and in Open With.

This defeats the acceptance criterion "No public surface presents Zed as the
product or publisher".

Source of the strings:

- `crates/zed/resources/info/DocumentTypes.plist`
- `crates/zed/resources/info/Permissions.plist`

Issue #16 lists "Inspect icons, bundle metadata, file associations, shell
integration, installer text, disk image text, licenses, and notices" in its
scope. Bundle metadata was in scope from the start.

### 4.2 Three Zed logo marks in the status bar

Three status-bar buttons render a stylized Zed `Z` mark. The producer captured
them from the running candidate at 4x magnification. In source these are
`IconName::ZedAssistant` and `IconName::ZedAgent`, from
`assets/icons/zed_assistant.svg` and `assets/icons/zed_agent.svg`, used by
`crates/agent_ui/src/agent_panel.rs` and `crates/sidebar/src/sidebar.rs`.

A logo carries no text. No string scan of the source tree or of the compiled
executable can see it. Only looking at the rendered product finds it.

### 4.3 Why the existing gates missed both

`script/bundle-omega-rc` scans the packaged application for exactly three
literals: `BUZZ_PRIVATE_KEY`, `identity.key`, and `get_nsec`. Those are
identity boundaries. **There is no brand literal gate in the bundler**, so no
packaging step reads `Info.plist`, and an earlier claim on omega#16 that the
bundler "already fails the build on forbidden literals" describes the identity
scan, not a brand scan.

Both misses are exactly what the falsifier on omega#16 predicts: "Unit tests,
source scans, or screenshots are the only evidence."

## 5. Installed observations

`script/collect-omega-installed-observations` now performs six of the eleven
observations, waives one, and blocks four.

| Check | State on rc10 |
| --- | --- |
| `zed-data-before-after-isolation` | performed |
| `keyboard-focus-traversal` | performed |
| `viewport-360-pixels` | performed |
| `larger-ui-font` | performed |
| `light-theme` | performed |
| `dark-theme` | performed |
| `screen-reader-output` | **waived**, see section 6 |
| `identity-first-first-run` | blocked |
| `theme-and-agent-setup-baseline` | blocked |
| `high-contrast` | blocked, owner act |
| `reduced-motion` | blocked, owner act |

The report status is `incomplete`. It is not `passed` and not
`passed_with_waivers`.

### 5.1 What the producer newly performed

Three checks previously recorded the literal text `not performed`.

**`viewport-360-pixels`.** The producer drove the live window to 360 points
through the accessibility interface, read the settled frame back to confirm the
width, captured the rendered window, and restored the original frame. At 360
points the product renders and stays operable.

**`larger-ui-font`.** The producer wrote `ui_font_size: 24` into the
candidate's own settings file at `~/.config/omega-rc/settings.json`, waited for
the hot reload, captured the re-rendered window, and restored the original file
byte for byte. The restored digest was confirmed equal to the original.

**`keyboard-focus-traversal`.** Keyboard input only, and no claim read from an
accessibility tree. Each recorded fact has its own measurement:

- Three surfaces each rendered a window distinct from the baseline.
- A forward move inside a focusable list repainted 136,660 pixels, so the
  focused item is indicated on screen.
- The matching reverse move reproduced the earlier image exactly, at 0
  differing pixels, and the full round trip returned to within 136 pixels of
  the start. Focus travelled backwards, not onwards.
- One chord opened a panel and the same chord closed it, both captured.

`light-theme` and `dark-theme` were previously a pair where only the host's
current appearance could be recorded. The producer now drives the host into
each appearance, captures the candidate in it, and restores the original.

### 5.2 What is still blocked, and why

`identity-first-first-run` and `theme-and-agent-setup-baseline` read labels
from the accessibility tree. The candidate publishes none, so these two cannot
be observed that way. They need a clean-profile first start and a person
looking at it. The producer did not perform them and did not record them.

`high-contrast` and `reduced-motion` read `com.apple.universalaccess`. macOS
privacy policy protects that domain, so an agent cannot turn the settings on.
**Owner act:** turn on Increase Contrast and Reduce Motion in System Settings
> Accessibility > Display, then tell the lane to re-run the collector.

## 6. The screen-reader waiver

### 6.1 What was observed

The candidate's front window publishes `entire contents = 0` and exactly three
direct children: close button, full screen button, and minimize button. macOS
supplies those three itself. The candidate therefore publishes **no application
accessibility tree at all**.

### 6.2 Upstream parity, observed and not assumed

The owner direction of 2026-07-25 permits omitting assistive technology only
where upstream Zed omits it too. That is a condition. The producer observed it
against the installed upstream build with the same interface and the same
script:

| Product | Version | `entire contents` | Direct children |
| --- | --- | --- | --- |
| Omega RC | `0.2.0-rc10` | 0 | close, full screen, minimize |
| Zed | `1.12.0` | 0 | close, full screen, minimize |

The two results are identical. **Upstream Zed does not wire this either**, so
the parity condition holds.

One qualification, because the condition is narrower than the direction reads.
GPUI does depend on AccessKit, and `crates/gpui/src/window/a11y.rs`,
`element.rs`, and `elements/div.rs` carry role, label, and action plumbing. The
capability is partly built. What neither product does is publish a usable tree
to macOS. The waiver covers the observed absence of published output. It does
not say the framework has no accessibility code.

`observe-upstream-accessibility-parity` is a separate command because it starts
Zed, and Zed's own writes must fall outside the window the isolation check
measures. Without a parity record the collector blocks and grants no waiver.

### 6.3 A waiver is not a pass

The record says `waived`. It never says `passed`, and it never rolls up into a
green status.

- A waived entry carries the owner's exact words, the direction date, the
  basis, the issue, and the observed parity. It carries **no** `facts`.
- A passed entry carries facts and **no** waiver. The two shapes are disjoint,
  so a waived entry cannot be relabelled `passed` without also inventing the
  observation it stands in for.
- A report holding a waiver reports `passed_with_waivers`, never `passed`.
- The top-level `waivers` list and the waived entries must agree in both
  directions, so a waiver can be neither hidden from the summary nor invented
  in it.
- Only `screen-reader-output` may be waived. The validator refuses a waiver of
  any other check, so the waiver cannot become a shortcut past a 360-pixel
  viewport or a larger UI font.

`script/test-omega-installed-observation-waivers` asserts every rule above.

The plain statement: **no assistive technology consumed this candidate. No
speech, braille, or announcement was produced or recorded.**

## 7. Scope coverage

| Scope item | State | Evidence |
| --- | --- | --- |
| Verify the immutable candidate digest | covered by the producer | section 3 |
| Install beside Zed in a clean user profile | **open** | rc10 was installed beside Zed on the owner's daily host, not in a clean profile |
| Offline first start and identity-first entry | **open** | the installed copy has no stapled ticket, and the identity surface is not readable without a clean-profile look |
| Open a project, edit and save a file, open Git status, open a terminal | partly covered | the producer opened the project panel, the terminal panel, and the Git panel by keyboard, and captured each. **Edit and save is open.** |
| Restart and layout restoration | covered by the producer | full quit and relaunch restored the same project, the same three tabs, the same file, and the frame `96,33,1536,1084` |
| Inspect every visible product surface for Zed branding | **FAILED** | section 4.2, three Zed logo marks in the status bar |
| Inspect icons, bundle metadata, associations, installer, disk image, licenses | **FAILED** | section 4.1, 13 Zed strings in `Info.plist` |
| Capture network destinations | covered by the producer | the running candidate holds one socket, `127.0.0.1:45338 (LISTEN)`. No non-loopback destination and no Zed service |
| Inspect data, cache, log, and credential roots | covered by the producer | section 8 |
| Disabled service states and update behavior | **open** | not exercised in this lane |
| Remove Omega and confirm Zed data unchanged | **open** | removal is destructive to the owner's installed product. Reserved |
| Owner observation and independent verification | **open** | the producer may not sign either |

## 8. Zed isolation

Digest over every regular file under `~/Library/Application Support/Zed`,
before and after a full candidate exercise. The exercise covered a quit and
relaunch, restart restoration, keyboard traversal, the 360-point viewport, a
settings write and restore, both system appearances, and the workroom binding
test:

```
before: 37c09089e980e2120047e33e6d04910acd7aa1bd305725657eea0290d3c7175c
after:  37c09089e980e2120047e33e6d04910acd7aa1bd305725657eea0290d3c7175c
```

Byte-identical. The upstream parity probe runs before this digest is captured,
so the probe's own writes fall outside the measured window.

Roots are separate:

| Kind | Candidate | Upstream |
| --- | --- | --- |
| State | `~/Library/Application Support/Omega RC` | `~/Library/Application Support/Zed` |
| Settings | `~/.config/omega-rc` | `~/.config/zed` |
| Cache | `~/Library/Caches/omega-rc` | `~/Library/Caches/Zed` |
| Logs | `~/Library/Logs/omega-rc` | `~/Library/Logs/Zed` |

The application menu tree was read in full, 283 entries. It contains no Zed
name. Two `Zed` hits appear under the Apple menu Recent Items, which is the
host's own list of recently used applications, not a surface of this product.

## 9. Unclassified Zed dependencies found

The falsifier on omega#16 also fails the candidate if it "exposes any
unclassified Zed product, service, package, or state dependency".

| Dependency | Kind | Classified? |
| --- | --- | --- |
| 13 Zed strings in `Info.plist` | product name on an operating-system surface | **No** |
| 3 Zed logo marks in the status bar | product mark on a visible surface | **No** |
| `ZED_COMMIT_SHA` in the executable | build marker, verified by the harness | Yes |
| Zed service requests | none observed | not applicable |

Two unclassified product surfaces. The falsifier holds against rc10.

## 10. What would close omega#16

1. Rewrite the 13 `Info.plist` strings and add a brand literal gate to
   `script/bundle-omega-rc` so a future candidate carrying them cannot be
   packaged.
2. Replace the three Zed logo marks with OpenAgents marks. A string gate
   cannot catch a logo, so this needs a rendered check or an asset inventory.
3. Cut a new candidate and re-run this record against it.
4. Owner act: turn on Increase Contrast and Reduce Motion, then re-run the
   collector.
5. Owner act: a clean-profile first start, for `identity-first-first-run` and
   `theme-and-agent-setup-baseline`.
6. Owner observation, which is a separate criterion from independent
   verification.
7. Independent review by the designated reviewer, reproducing sections 3, 4,
   5, and 8 from the primary artifacts.

## 11. Reproduction for the reviewer

The reviewer must reproduce, from the artifacts and not from this record:

1. `spctl`, `codesign`, and `stapler` on the exact rc10 disk image and the
   installed application, and the digests in section 3.
2. `plutil -p /Applications/Omega.app/Contents/Info.plist` and count the Zed
   strings.
3. A capture of the running status bar, and the three Zed marks in it.
4. `script/observe-upstream-accessibility-parity` against installed Zed.
5. `script/collect-omega-installed-observations`, and confirm it reports
   `incomplete` with one waiver.
6. `script/test-omega-installed-observation-waivers`.

The producer is the agent that wrote this record and may not sign the review.
