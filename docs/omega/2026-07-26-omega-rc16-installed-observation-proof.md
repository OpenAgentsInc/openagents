# OMEGA-BRAND-06: rc16 installed observation record

- Date: 2026-07-26
- Packet: OMEGA-BRAND-06 and OMEGA-OID-09
- Issues: omega#16, omega#9, omega#8
- Lane: `claude/rc16-proof`
- STE issue: 9
- Glossary revision: openagents-ste-glossary-v1
- Status: producer record. Not signed. No issue closed.

## Result

`0.2.0-rc16` corrects every brand class that failed rc10, rc11, rc13 and rc15.
This lane reproduced each correction from the artifact. The brand result is the
best of the five candidates.

rc16 does not close omega#16, omega#9 or omega#8. The blocker is not brand.
The blocker is that the installed journey, the tripwire receipt and the
independent review are all unperformed. Two new defects are also recorded
below.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc16` |
| Omega commit | `79a63f1f157f96b7669b0bccd6511c2b378e342b` |
| Artifact | `Omega-v0.2.0-rc16-macos-arm64.dmg` |
| Package sha256 | `bc39acd6937d86f6f48531516abc52d13eab6944412d45ba2abf68dd199788be` |
| Candidate digest | `1ae7bc5b86a3196c0e0b19610c2af6e16e257b7f4d698991986e98c299efaf3e` |
| Bundle identifier | `com.openagents.omega.rc` |
| Engine | `omega-effectd-v0.1.0-rc.10` |

Every statement in this record applies to that candidate digest. I made each
value from the artifact. I did not read any value from the release record or
from the release notes.

## Package verification

- `shasum -a 256` on the released disk image gives `bc39acd6...`.
- The installed `omega` and `cli` binaries at `/Applications/Omega.app` are
  byte-identical to the binaries inside the released disk image. The installed
  candidate is rc16.
- `codesign` reports `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`,
  team `HQWSG26L43`, and the hardened runtime flag.
- `codesign --verify --deep --strict` gives `valid on disk` and
  `satisfies its Designated Requirement`.
- `spctl -a -t exec` on the application and `spctl -a -t install` on the disk
  image both give `accepted` and `source=Notarized Developer ID`.
- `xcrun stapler validate` passes on the disk image, on
  `/Applications/Omega.app`, and on a copy taken out of the image to a new
  path. The ticket travels with the copy. rc11 lacked that condition.
- The notice digests, both packaged icon digests, the brand gate script digest,
  the brand policy digest and the identity verifier digest all agree with the
  release record.
- The three forbidden identity literals give 0 hits across all three packaged
  executables.

## The five rc15 brand classes are corrected

Each count comes from the shipped binaries. Controls are included so a zero is
a result and not a broken scan.

| Surface | rc13 | rc14 | rc16 |
| --- | ---: | ---: | ---: |
| `in Zed` | 25 | 0 | **0** |
| `You are the Zed coding agent` | 1 | 0 | **0** |
| `Zed` in the signed `Info.plist` | 0 | 0 | **0** |
| `Zed Editor` request header | 1 | 1 | **0** |
| `Zed has been uninstalled` | 1 | 1 | **0** |
| `/Applications/Zed.app` in any binary | 1 | 1 | **0** |
| `zed` in rendered `omega --help` | present | present | **0** |
| `zed` in rendered `cli --help` | present | present | **0** |
| Embedded `zed_*` image assets | 0 | 0 | **0** |
| `Omega` control | — | 596 | 379 |
| `Omega Agent` control | 7 | 7 | **7** |

The rendered help of both shipped binaries is clean. `cli --help` now gives
`~/Library/Application Support/<Omega channel>` as the data root. rc15 gave
Zed's directory, which was also false.

The single Zed reference in the rendered help is
`the legacy `zed://` scheme`. That reference is classified.

The packaged first-party agent family gives 0 hits for all six phrases and 0
for the `ZED_AGENT_ID` symbol, against a control of 7 for `Omega Agent`.

`script/verify-omega-brand --app` exits 0 on the extracted candidate.

## The destructive uninstall is gone

I derived the plan from the product. I did not run a removal. The command ran
under an isolated `HOME` and with `OMEGA_UNINSTALL_DRY_RUN=1`, so two
independent conditions stopped any removal.

```sh
env HOME="$ISOLATED" OMEGA_UNINSTALL_DRY_RUN=1 \
  /Applications/Omega.app/Contents/MacOS/cli --uninstall
```

The product printed its own plan:

- Product name `Omega RC`.
- 9 removal paths. Every one is an Omega path.
- 1 held-back path, `.config/omega-rc`, which the script prompts for.
- **0 Zed paths.** No `/Applications/Zed.app`. No Zed application support. No
  `~/.config/zed`. No `~/.zed_server`.

After the run, `/Applications/Zed.app` and the real Zed application support
directory were both still present. The shipped `script/uninstall.sh` holds no
path table at all. It refuses a relative path, an empty plan, and each of `/`,
`/Applications`, `$HOME` and the other top-level roots.

## New defect 1: the uninstall removes the executable, not the bundle

The plan's application entry is
`/Applications/Omega.app/Contents/MacOS/omega`. That is the main executable,
not the application bundle.

After the plan runs, `/Applications/Omega.app` remains. 130.9 MB and 5
executables survive:

- `Contents/MacOS/cli`, which still carries `--uninstall`
- `Contents/MacOS/omega-identity-proof`
- `Contents/Resources/omega-effectd/bin/omega-effectd`
- `Contents/Resources/omega-effectd/dist/omega-effectd.mjs`
- `Contents/Resources/omega-effectd/runtime/bin/node`, a bundled Node runtime

The removal is incomplete. A signed but non-functional application bundle stays
in `/Applications`. The omega#16 Scope item "Remove Omega" is therefore not
satisfied even when the plan runs to completion.

The cause is in `crates/cli/src/main.rs`. `from_installed_paths(Some(app.path()))`
receives the executable path where the bundle root belongs.

## New defect 2: three unclassified Zed command literals in the signed `cli`

`crates/cli/src/main.rs` renders an interactive terminal prompt. The literals
are these:

- `Add to existing Omega window (zed --existing)`
- `Open a new window (zed --classic)`
- `Configure default behavior for zed <path>`

All three literals are in the signed `cli` binary. `zed --existing` and
`zed --classic` are not Omega commands. The brand rule written into
`script/omega-brand-gate.json` says that a sentence which stays true with our
own name in it is a product claim and must be rewritten. Each of these stays
true with `omega` in place of `zed`.

Neither `prose.classified` nor `crates/app_identity/fixtures/compatibility_allowlist.json`
carries any of the three. The gate passes because its prose filter drops one-
and two-word tokens, which the release notes record as a known gap.

I could not make the product render the prompt on this host. The send condition
in `crates/zed/src/zed/open_listener.rs` held, and the CLI opened the directory
without a prompt. So the literals ship and the render was not observed. A
reviewer must treat this as a shipped-string result, not an installed-render
result.

## Installed observations

Performed against the installed candidate, process 25306, window 13163, bound
to candidate digest `1ae7bc5b...`. Recorded as `performed_by: agent`, profile
`installed-default`. Record digest
`ea276b4be29f6b1378d433312983eccf2c67111cae67cc76d4ee57847900b96e`.

| Check | Result | Facts |
| --- | --- | --- |
| `zed-data-before-after-isolation` | passed | before and after both `bdc2fb82...` |
| `viewport-360-pixels` | passed | width 360, no horizontal overflow, completion action visible |
| `larger-ui-font` | passed | `ui_font_size` 24, no clipped content, settings restored |
| `light-theme` | passed | appearance light, content legible |
| `dark-theme` | passed | appearance dark, content legible |
| `screen-reader-output` | **waived** | see below |
| `high-contrast` | blocked | `increaseContrast` is 0 on this host |
| `reduced-motion` | blocked | `reduceMotion` is 0 on this host |
| `identity-first-first-run` | blocked | the default profile is past first run |
| `theme-and-agent-setup-baseline` | blocked | the default profile is past first run |
| `keyboard-focus-traversal` | blocked | see below |

Report status is `incomplete`. 5 performed, 1 waived, 5 blocked.

`screen-reader-output` is **waived and not passed**. The candidate publishes no
application accessibility tree. Upstream Zed 1.12.0, read through the same
interface, publishes none either. The waiver carries the owner direction of
2026-07-25 and the parity record. The report status is never `passed` for a
waived check. I did not widen the waiver.

The two accessibility flags are off. The owner set them for the rc13 window and
was then told they could turn them off. `defaults write` is refused on that
domain. These two record `blocked`, and this lane did not ask the owner to set
them again.

## keyboard-focus-traversal, in detail

Two of three surfaces opened from the keyboard and each rendered a window the
baseline did not: the command palette and the project panel. Focus movement was
visible and reverse traversal worked.

The terminal panel did not. `ctrl-` backtick produced a window capture
byte-identical to the baseline, three times: once for reachability and twice
for the activation open and close pair. Screen capture itself was working,
because the other two surfaces produced different captures in the same run.

`terminal_panel::Toggle` is present in the packaged binary and the shipped
macOS keymap binds `ctrl-` backtick to it. So the action exists and the binding
exists.

I cannot separate a product defect from a condition of this profile. The window
belongs to another lane. It has `vim_mode` on, an agent panel in the bottom
dock, and a live agent session. A clean profile would separate the two, and a
clean profile needs a free instance.

## Why the journey is blocked

Omega refuses a second instance. `crates/zed/src/zed/mac_only_instance.rs` binds
a TCP port on `127.0.0.1` keyed on release channel and user identifier, not on
the data directory. `--user-data-dir` does not free the lock.

`ZED_RELEASE_CHANNEL` cannot move the port either. `crates/release_channel/src/lib.rs`
reads that variable only under `debug_assertions`, so a release build ignores
it.

Another lane held `/Applications/Omega.app` throughout this run, with a real
`openagents` checkout open and a live agent session inside it.

Two consequences follow.

**Edit and save is declined.** The only running Omega has a real repository
open. Typing into it would edit actual repository files. A previous lane
declined for the same reason. The condition has not changed.

**The tripwire receipt cannot be produced.** `script/collect-omega-installed-tripwires`
requires the disposable canary the candidate was actually given during the
journey, and refuses to invent one. Giving the installed candidate a canary
through a secret channel means an identity recovery on a fresh profile. That
replaces the running lane's identity. I did not do it.

## omega#8 gate matrix

`script/prove-omega-rc-install` refuses in prove mode. It requires six evidence
inputs this lane does not have: installed tripwires, identity recovery
evidence, full-auto evidence, manual evidence, network evidence and lifecycle
evidence. `--harness-check` passes.

The candidate evidence record reports `pending_required_gates` and
`candidate_admitted: false`.

| Gate | Status |
| --- | --- |
| `custody_scenarios` | pending |
| `recovery_scenarios` | pending |
| `forged_request_rejection` | pending |
| `stale_task_fencing` | pending |
| `installed_secret_tripwires` | pending |
| `accessibility` | pending |
| `manual_journey` | pending |
| `install_lifecycle` | pending |
| `assurance_spec_admission` | pending |
| `owner_observation` | pending |
| `independent_verification` | pending |

The identity proof matrix is the one part that ran end to end.
`script/run-omega-identity-proof-matrix` gives `passed` over 28 cases against
the packaged `omega-identity-proof` driver, bound to candidate digest
`1ae7bc5b...`. Record digest
`8eb37a9090fcddeb674c6f13c4248e0dcfdf278549be1699f273c460ca6cdacc`. The
disposable keychain namespace cleaned up and production locator access was
rejected by construction.

## Build inputs are reproducible, but not from the release alone

The release record names a `Cargo.lock` snapshot with digest `e1124fdb...`.
That digest matches no commit on `origin/main` and matches no published file.
The release record also says `dirty: true`.

`script/generate-omega-identity-candidate-evidence` refuses the committed
`Cargo.lock` for this reason.

The gap is one line. The build snapshot differs from the committed `Cargo.lock`
at commit `79a63f1f15` only in the version of the workspace crate `zed`, which
the bundler rewrites to the bundle version:

```
< version = "0.2.0"      # the build
> version = "1.14.0"     # the commit
```

A reviewer can rebuild the snapshot exactly. Take `Cargo.lock` at
`79a63f1f15`, change the `zed` package version from `1.14.0` to `0.2.0`, and
the digest becomes `e1124fdb...`. I did that, and the candidate evidence record
above was generated from the reconstruction rather than from the local build
artifact.

The release publishes the disk image and the release record. It does not
publish the snapshot. That is the gap to close.

## omega#16 Scope, item by item

| Scope item | rc16 |
| --- | --- |
| Verify the candidate digest | covered |
| Install beside Zed in a clean profile | **blocked**, single-instance lock |
| Offline first start | **not attempted**, needs the host network down |
| Identity-first entry surface | **blocked**, default profile is past first run |
| Open a project, edit and save, Git status, terminal | **declined**, a real repository is open |
| Restart and layout restoration | **blocked** |
| Visible surfaces for Zed branding | **covered**, and clean on all five rc15 classes |
| Icons, metadata, associations, disk image, licenses | **covered**, every digest agrees |
| Network destinations | **not captured** |
| Data, cache, log, credential roots | **covered**, `Omega RC` and `omega-rc` throughout |
| Disabled service states and update behavior | **not attempted** |
| Remove Omega, Zed data unchanged | **partly** — 0 Zed paths in the plan, and the removal is incomplete |
| Owner observation and independent verification | **open**, I am the producer |

## Falsifiers, addressed

### omega#16

> Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package,
> or state dependency.

**The first part does not fire.** The evidence is the signed and notarized
candidate. It includes the package assessment, the stapled ticket that travels
with a copy, the rendered help of both shipped binaries, the uninstall plan the
product printed about itself, the window captures of the running application,
and the Zed data digest on this host.

**The second part fires once, weakly.** The three `zed --*` literals in the
signed `cli` are in no classification and in no allow-list. The render was not
observed. Every other class the four earlier candidates failed on is now
classified or gone.

### omega#8

> Unit tests, fixtures, or screenshots are the only evidence.

Does not fire for the parts that ran. It does not need to fire, because omega#8
fails on coverage rather than on evidence quality. Six of its required inputs
do not exist and eleven gates are pending.

## What must happen next

1. Free the installed candidate, or accept a window in which one lane holds it.
   Every blocked item above needs one instance nobody else is using.
2. Agree a window for the offline first start. The host network must be down,
   and other lanes are live on this machine.
3. Produce the tripwire receipt from a fresh profile with a disposable canary.
4. Decide the two new defects. The incomplete removal is the more serious one.
5. Hand the reviewer packet below to `script/review-omega-candidate`.

## Reproduction for the reviewer

Reviewer identity `0326d8f9...`. I am the producer and I must not sign this.

```sh
# package, from the released artifacts
shasum -a 256 Omega-v0.2.0-rc16-macos-arm64.dmg   # bc39acd6...
codesign -dv --verbose=4 /Applications/Omega.app
codesign --verify --deep --strict --verbose=2 /Applications/Omega.app
spctl -a -vvv -t exec /Applications/Omega.app
spctl -a -vvv -t install Omega-v0.2.0-rc16-macos-arm64.dmg
xcrun stapler validate /Applications/Omega.app
xcrun stapler validate Omega-v0.2.0-rc16-macos-arm64.dmg
# and on a copy taken OUT of the image, which is what rc11 lacked
ditto /Volumes/"Omega RC"/Omega.app /tmp/copy/Omega.app
xcrun stapler validate /tmp/copy/Omega.app

# the installed candidate is the released one
shasum -a 256 /Applications/Omega.app/Contents/MacOS/omega   # 13a24c9f...
shasum -a 256 /Applications/Omega.app/Contents/MacOS/cli     # bd86a33e...

# the five rc15 classes
strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -c "in Zed"
strings -a /Applications/Omega.app/Contents/MacOS/omega | grep -c "Zed Editor"
strings -a /Applications/Omega.app/Contents/MacOS/cli | grep -c "Zed has been uninstalled"
plutil -convert xml1 -o - /Applications/Omega.app/Contents/Info.plist | grep -ic zed
/Applications/Omega.app/Contents/MacOS/omega --help
/Applications/Omega.app/Contents/MacOS/cli   --help

# the uninstall plan, without removing anything.
# BOTH conditions matter. Do not drop either one.
env HOME="$(mktemp -d)" OMEGA_UNINSTALL_DRY_RUN=1 \
  /Applications/Omega.app/Contents/MacOS/cli --uninstall

# new defect 2
strings -a /Applications/Omega.app/Contents/MacOS/cli | grep -E 'zed --existing|zed --classic|zed <path>'

# the gate, which now reads rendered help and walks the bundle for Mach-O
script/verify-omega-brand --app /Applications/Omega.app
```

Two standing notes for whoever signs.

**The omega#89 standing item is resolved.** `first_party_agent.phrases` is now
applied to the package automatically inside `script/verify-omega-brand --app`,
by `check_packaged_first_party_agent`, with an anti-vacuity guard that fails
when no packaged executable carries `Omega Agent`. The reviewer no longer needs
to run that scan by hand. The rc13 and rc14 records said the opposite, and that
is no longer true.

**Every value must come from the artifact.** Do not accept a number from this
record.

## Disclosure

This lane opened an empty temporary directory named `probe-dir` as a second
workspace in the running Omega, while it tried to make the interactive CLI
prompt render. That workspace is still open. It is an empty directory in the
scratchpad. No repository file was touched, and the `openagents` workspace in
that window is unchanged.
