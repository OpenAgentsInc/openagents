# Omega `0.2.0-rc11` installed observation proof

- Status: producer record, **rc11 fails omega#16**
- Owner: OpenAgents
- Date: 2026-07-25
- Audience: release, assurance, and review
- Issues: [omega#16](https://github.com/OpenAgentsInc/omega/issues/16),
  [omega#9](https://github.com/OpenAgentsInc/omega/issues/9),
  [omega#8](https://github.com/OpenAgentsInc/omega/issues/8)
- Predecessor: `2026-07-25-omega-rc10-installed-observation-proof.md`

I am the producer. I did not review this record and may not sign it.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc11` |
| Omega commit | `e210ddbc3f11880e64f1b7959bacc35149adaab8` |
| Artifact | `Omega-v0.2.0-rc11-macos-arm64.dmg` |
| Package sha256 | `90b8d6fd4153c3167cffd1933f3853a7cd68531d9832d0b0e475691720a577c8` |
| Candidate digest | `615b60b4b20885f7207bafa200c6c2195988ae99901e67d91e56bdf011d6c689` |
| Bundle identifier | `com.openagents.omega.rc` |

Every claim below is bound to that candidate digest.

## Package verification: clean

Independently reproduced against the primary artifacts, not read from the
release record:

- `shasum -a 256` on the DMG equals `90b8d6fd…`.
- `spctl -a -vvv -t install /Applications/Omega.app` → `accepted`,
  `source=Notarized Developer ID`,
  `origin=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
- `codesign --verify --deep --strict` → `valid on disk`,
  `satisfies its Designated Requirement`.
- `xcrun stapler validate` on the DMG → `The validate action worked!`.
- All three installed executables equal the release record byte for byte:
  `omega` `2824a4c8…`, `cli` `787001b9…`, `omega-identity-proof` `37febd92…`.
- `generate-omega-identity-candidate-evidence` mounted the DMG, verified three
  signing authorities, matched the `ZED_COMMIT_SHA` source marker to
  `e210ddbc3f…`, and passed a fresh identity-boundary scan over every regular
  file in the mounted application.

### One qualification, unchanged from rc10

`xcrun stapler validate /Applications/Omega.app` →
**`Omega.app does not have a ticket stapled to it.`**

`script/bundle-omega-rc` staples the DMG only (line 1314,
`stapler staple "${DMG_PATH}"`). A copy dragged out of the disk image carries no
ticket, so `spctl` acceptance here can rest on an online notarization lookup.
**Offline first start is still not proven.** The fix is to staple the `.app`
before the disk image is built. The ticket then travels with the copy.

## rc10's two failures are fixed

| rc10 failure | rc11 |
| --- | --- |
| 13 Zed strings in the signed `Info.plist` | **0** — `plutil -convert xml1` piped to `grep -ic zed` returns `0` |
| `CFBundleTypeName` = `Zed Text Document` | gone |
| `NSMicrophoneUsageDescription` = "An application in Zed…" | "An application in **Omega** wants to use your microphone." |
| 3 Zed logo marks from `assets/icons/zed_*.svg` | **0** `icons/zed_*` embedded in the packaged binary |

`script/verify-omega-brand --app /Applications/Omega.app`, run with
`OMEGA_BRAND_VERIFY_ROOT` pinned to the rc11 source tree at `e210ddbc3f`, exits
`0`: *Omega brand gate ok*.

`OMEGA-DELTA-0017` and `OMEGA-DELTA-0018` did what they were built to do.

## rc11 fails omega#16: three Zed presentations the new gate cannot see

`OMEGA-DELTA-0018` inventories `assets/icons/*.svg` against the `IconName`
enum, and its packaged scan matches `icons/[A-Za-z0-9_]+\.svg` in the binary.

**`assets/images/*.svg` and the `VectorName` enum are outside the gate
entirely.** That directory is where the actual Zed logo lives.

```
$ strings -a /Applications/Omega.app/Contents/MacOS/omega \
    | grep -oE '[a-z_/]*\.svg' | sort -u | grep -i zed
icons/ai_zed.svg          # allowed: identifies Zed's keymap preset, like ai_anthropic.svg
images/zed_logo.svg       # NOT covered by any gate
images/zed_x_copilot.svg  # NOT covered by any gate
```

Both files' bytes are embedded in the shipped, signed, notarized binary. This is
the rc10 hole one directory over.

### Finding 1 — the Zed logo renders in the installed candidate

`workspace: open component preview` is reachable from the ordinary command
palette in the **release** build. It is not dev-gated: the product does gate
dev surfaces (`dev::ToggleInspector is only available in debug builds`,
`dev::ResetOnboarding is only available in debug builds`), and this action
carries no such gate.

Opening it and selecting the `Vector` component renders the Zed `Z` mark. I
captured it from the running candidate. The captures are producer-local and are
deliberately not the evidence of record: the reviewer should reproduce the two
palette steps at the end of this document against their own installed copy
rather than accept a screenshot of mine.

Source: `crates/ui/src/components/image.rs` renders `VectorName::ZedLogo` four
times in `Vector::preview` (lines 126, 134, 147, 153) and
`VectorName::ZedXCopilot` once (line 163).

**A logo has no text.** No string scan of the source tree, the binary, or
`Info.plist` can see this. Only looking at the running product finds it — the
same lesson rc10 taught, in the same product, one asset directory away.

### Finding 2 — "Use GitHub Copilot in Zed", unclassified

`crates/copilot_ui/src/sign_in.rs` presents Zed as the product three times on
one user-facing surface:

- line 73 — the floating window's **title bar**: `"Use GitHub Copilot in Zed"`
- line 255 — a **`Headline`** in the modal body: `"Use GitHub Copilot in Zed"`
- line 462 — `Vector::new(VectorName::ZedXCopilot, …)`, the Zed × Copilot
  **logo lockup**

The string ships in the installed binary. `strings -a` on
`Contents/MacOS/omega` returns `Use GitHub Copilot in Zed`.

`crates/app_identity/fixtures/compatibility_allowlist.json` contains **no entry
for it**. It is the same class as `Welcome to Zed` and `About Zed`, both of
which that file lists with disposition `blocked` and the reason *"Forbidden
public product claim. It must not appear in reachable Omega UI."*

omega#16 acceptance criterion *"Every retained Zed identifier is in the reviewed
compatibility allow-list"* is **not met**.

### Finding 3 — the `zed:` command namespace is user-facing

Typing `zed ` into the installed candidate's command palette lists, among
others:

```
zed: zoom          zed: quit        zed: hide        zed: about
zed: show all      zed: open log    zed: minimize    zed: open docs
zed: get merch     zed: open tasks  zed: extensions  zed: open keymap
```

Reproduce it in one keystroke. See the end of this document.

The allow-list classifies this as `approved_compatibility` with the reason
*"Internal crate and action namespaces stay as fork compatibility seams. **They
are not user-facing product copy**."*

**The installed candidate falsifies that reason.** They are user-facing product
copy, in the product's most public affordance. `zed: about` is the palette's own
casing of `About Zed`, which the same file lists as `blocked`.

To be precise about what is *not* wrong: the action targets are correctly
rebranded. `MERCH_URL` is `https://www.openagents.com/`, `DOCS_URL` is
`app_identity::PRODUCT_DOCS_URL`, and the About window title is `About Omega`.
The defect is the rendered label, and the allow-list entry that says the label
is not rendered.

### Disposition drift, stated plainly

| Allow-list entry | Recorded disposition | Observed on the installed candidate |
| --- | --- | --- |
| `IconName::ZedAgent / VectorName::Zed*` (`crates/ui`) | `source_only` | **rendered** in Component Preview, and rendered by `crates/copilot_ui` — outside the recorded path |
| `crates named zed / zed_actions / zed_*` | `approved_compatibility`, "not user-facing product copy" | **user-facing** in the command palette |
| `Use GitHub Copilot in Zed` | *(absent)* | rendered as window title and headline |

## Installed observations, bound to `615b60b4…`

`script/collect-omega-installed-observations`, run against the installed rc11
with an upstream parity record. rc10's observations do not transfer. Every one
of these was re-performed against this binary.

| Check | rc11 |
| --- | --- |
| `zed-data-before-after-isolation` | **performed** |
| `keyboard-focus-traversal` | **performed** |
| `viewport-360-pixels` | **performed** |
| `larger-ui-font` | **performed** |
| `light-theme` | **performed** |
| `dark-theme` | **performed** |
| `screen-reader-output` | **waived** — not observed, not a pass |
| `identity-first-first-run` | blocked — reads AX labels, and the candidate publishes none |
| `theme-and-agent-setup-baseline` | blocked — same cause |
| `high-contrast` | blocked — owner act |
| `reduced-motion` | blocked — owner act |

Collector status: **`incomplete`**. Not `passed`. Not `passed_with_waivers`.

### Zed isolation

Digest over every regular file under `~/Library/Application Support/Zed`, across
the full exercise — keyboard traversal, the 360 pt viewport, a settings write and
restore, both appearances, palette and component-preview navigation:

```
before: 258827c5c6895e495a0d2d5772b4c3fd6a6c67e229d4c571579d4882618f7a0d
after:  258827c5c6895e495a0d2d5772b4c3fd6a6c67e229d4c571579d4882618f7a0d
```

Byte-identical. The upstream parity probe ran **before** the `before` digest was
taken, so Zed's own launch falls outside the measured window.

### The screen-reader waiver

Observed, not assumed:

| Product | Version | `entire contents` | Direct children |
| --- | --- | --- | --- |
| Omega RC | `0.2.0-rc11` | 0 | close, full screen, minimize |
| Zed | `1.12.0` | 0 | close, full screen, minimize |

macOS supplies those three buttons. Neither product publishes an application
accessibility tree, so the owner's parity condition of 2026-07-25 holds.

**No assistive technology consumed this candidate. No speech, braille, or
announcement was produced or recorded.** A waived entry carries the owner's
exact words and **no facts**. A passed entry carries facts and no waiver. The
shapes are disjoint. Only `screen-reader-output` is waivable — `viewport-360-pixels`
and `larger-ui-font` are ordinary rendering conditions and were performed.

## omega#16 Scope, item by item

| Scope item | rc11 |
| --- | --- |
| Verify the immutable candidate digest | covered |
| Install beside Zed in a clean user profile | **open** — installed beside Zed on the daily host, not a clean profile |
| Offline first start, identity-first entry | **open** — the installed copy has no stapled ticket |
| Open a project, edit and save a file, Git status, terminal | **partly** — project, terminal and Git panels opened by keyboard, and **edit and save is still open** |
| Restart and layout restoration | covered at rc10, and not re-performed on rc11 |
| Visible product surfaces for Zed branding | **FAILED** — Findings 1 and 3 |
| Icons, bundle metadata, associations, installer, DMG, licenses | `Info.plist` **clean**, and **FAILED** on embedded `images/zed_*.svg` |
| Network destinations | not re-performed on rc11 |
| Data, cache, log, credential roots | covered — `Omega RC` / `omega-rc` throughout |
| Disabled service states, update behavior | **open** — not exercised |
| Remove Omega, confirm Zed data unchanged | **open** — destructive to the owner's installed product, so it is reserved |
| Owner observation and independent verification | **open** — I am the producer and may sign neither |

On edit-and-save: I stopped rather than forced it. The host's desktop had other
lanes' windows overlapping the candidate's frame, and driving synthesized
keystrokes into that arrangement is the exact hazard this harness exists to
refuse — an earlier pass of it opened a Page Setup sheet in an operator terminal
and nearly recorded it as the candidate responding. An unforced observation is
worth more than a forced one.

## Falsifier, addressed

> *Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package, or
> state dependency.*

The second half fires. Three unclassified or misclassified Zed presentations,
all found by looking at the running installed candidate:

1. the Zed logo rendered in Component Preview, classified `source_only`,
2. `Use GitHub Copilot in Zed` as window title and headline, classified nowhere,
3. the `zed:` command namespace across the palette, classified as "not
   user-facing".

No Zed *service* received a request. `ZED_COMMIT_SHA` remains classified and
verified by the harness.

## What would close omega#16

1. Extend `OMEGA-DELTA-0018` to cover `assets/images/*.svg` and the
   `VectorName` enum, with the same complete-inventory and digest-pin discipline
   `assets/icons` already gets. The gate hole is the root cause of Finding 1.
2. Replace the `zed_logo` / `zed_x_copilot` artwork and rename the vectors.
3. Rewrite `Use GitHub Copilot in Zed` in both the window title and the
   headline, and add it to the brand string gate.
4. Decide the `zed:` action namespace: either rename it, or amend the allow-list
   entry so its recorded reason matches what the product actually renders. The
   current entry states something the installed candidate disproves.
5. Staple the `.app`, not only the DMG, so offline first start becomes provable.
6. Cut a new candidate and re-run this record against it.
7. Owner observation, and independent review reproducing package verification,
   all three findings, the observation set, and the isolation digest **from the
   primary artifacts** rather than from this document.

## Reproduction for the reviewer

```sh
# package
shasum -a 256 Omega-v0.2.0-rc11-macos-arm64.dmg
spctl -a -vvv -t install /Applications/Omega.app
codesign --verify --deep --strict --verbose=2 /Applications/Omega.app
xcrun stapler validate /Applications/Omega.app          # expect: no ticket

# rc10 regressions are fixed
plutil -convert xml1 -o - /Applications/Omega.app/Contents/Info.plist | grep -ic zed   # 0
plutil -extract NSMicrophoneUsageDescription raw /Applications/Omega.app/Contents/Info.plist

# the gate hole
strings -a /Applications/Omega.app/Contents/MacOS/omega \
  | grep -oE '[a-z_/]*\.svg' | sort -u | grep -i zed
strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -c 'Use GitHub Copilot in Zed'

# the rendered findings — open the installed candidate and look
#   cmd-shift-p → "component preview" → enter → search "Vector"
#   cmd-shift-p → type "zed "
```
