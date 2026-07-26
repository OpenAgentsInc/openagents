# Omega `0.2.0-rc14` installed observation proof

- Status: producer record. rc14 does not close omega#16, omega#9, or omega#8.
- Owner: OpenAgents
- Date: 2026-07-25
- Audience: release, assurance, and review
- Issues: [omega#16](https://github.com/OpenAgentsInc/omega/issues/16),
  [omega#9](https://github.com/OpenAgentsInc/omega/issues/9),
  [omega#8](https://github.com/OpenAgentsInc/omega/issues/8)
- Predecessor: `2026-07-25-omega-rc13-installed-observation-proof.md`

I am the producer. I did not review this record. I must not sign it.

## Summary

rc14 corrects the prose failure that rc13 had. I made that result again from
the rc14 binary. The correction is real.

rc14 does not close omega#16. I found five classes of user-facing text that
name Zed as the product. The brand gate does not read any of them. One of the
five is a command that destroys the user's Zed installation and Zed data.

rc14 does not close omega#8 or omega#9. Two checks in the proof harness give a
result that the harness cannot fail. A third check writes a fact that it never
measured.

## Candidate

| Field | Value |
| --- | --- |
| Version | `0.2.0-rc14` |
| Omega commit | `8505325c54bd601e874773fe2784376bb5ac60b7` |
| Artifact | `Omega-v0.2.0-rc14-macos-arm64.dmg` |
| Package sha256 | `17c400578646cf51e8b4b858c53d944b80be76612a5f36cf88a2ee6561b56f3e` |
| Candidate digest | `a0bd0c9d33d5a2ef67a0a7c908e597e6a21c172eb68b607dd7672ef3b8f66d45` |
| Bundle identifier | `com.openagents.omega.rc` |
| Engine | `omega-effectd-v0.1.0-rc.9` |

Each statement below applies to that candidate digest.

## The installed application was not the candidate

The task brief said that rc14 was installed at `/Applications/Omega.app`. That
statement was not correct when this lane started.

- The rc14 disk image holds an `omega` binary with digest `d55d0961…`.
- The binary at `/Applications/Omega.app/Contents/MacOS/omega` had digest
  `cca81f5d…`.
- The signature on the installed bundle has timestamp `2026-07-26T01:37:41Z`.
  The rc14 disk image asset has upload time `2026-07-26T00:45:12Z`.

The installed bundle is a later build than rc14. A different lane wrote it.
I did not change it, and I did not use it as evidence.

I took the candidate from the released disk image instead. I put a copy of
`Omega.app` at a path that this lane controls. Each result below comes from
that copy or from the disk image.

## Package verification: clean

I made these results from the artifacts. I did not read them from the release
record.

- `shasum -a 256` on the released disk image gives
  `17c400578646cf51e8b4b858c53d944b80be76612a5f36cf88a2ee6561b56f3e`.
- The disk image that this host built has the same digest as the released disk
  image. The two files agree byte for byte.
- The `omega` binary digest is `d55d0961…`. The `cli` binary digest is
  `d8dbb9d6…`. Both agree with the release record.
- `codesign --verify --deep --strict` gives `valid on disk` and
  `satisfies its Designated Requirement`.
- `codesign -dv` gives authority
  `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`, team
  `HQWSG26L43`, and flag `runtime`.
- `spctl -a -t exec` on the application gives `accepted` and
  `source=Notarized Developer ID`.
- `spctl -a -t install` on the disk image gives `accepted`.
- `xcrun stapler validate` gives `The validate action worked!` for the
  application and for the disk image.
- The volume name is `Omega RC`. The disk image holds no license agreement and
  no installer text.
- The three notice digests agree with the release record.
- The two `.icns` digests agree with the release record and with the pinned
  icon digest.
- Version, channel, artifact name, bundle identifier, team, and source commit
  agree across the `Info.plist`, the release record, and the GitHub release.

### The stapled ticket moves with a copy

rc11 stapled only the disk image. A copy that a person moved out of the disk
image had no ticket.

rc14 staples the application and the disk image. I copied `Omega.app` out of
the disk image to a new path. `xcrun stapler validate` on that copy gives
`The validate action worked!`, and `spctl` accepts it.

This is the condition that offline first start needs. It is not the same as an
offline start. See "Not performed".

## Brand result: the rc13 failure is corrected

Each result comes from the rc14 binary.

| Surface | rc13 | rc14 |
| --- | ---: | ---: |
| `in Zed` | 25 | **0** |
| `You are the Zed coding agent running inside the Zed editor` | 1 | **0** |
| `Zed` in the signed `Info.plist` | 0 | **0** |
| Embedded `zed_*` image assets | 0 | **0** |
| `Zed Agent` | 0 | **0** |
| `Welcome to Zed` | 0 | **0** |
| `About Zed` | 0 | **0** |
| `Use GitHub Copilot in Zed` | 0 | **0** |
| `Checking for Zed Updates` | 0 | **0** |
| `Settings related to calls in Zed` | 0 | **0** |

Control values from the same binary: `Omega` gives 596, `Omega Agent` gives 7,
and `in Omega` gives 37. The scan reads the binary. A zero is a result, not a
broken scan.

`script/verify-omega-brand --app <candidate>` exits `0` on the candidate. The
gate script digest and the policy digest agree with the release record.

The packaged `first_party_agent.phrases` scan is clean. Each of the six
phrases gives 0, and the control `Omega Agent` gives 7. **No gate makes this
result.** The string `first_party_agent` does not occur in
`script/verify-omega-brand`. Only a Rust test reads that list, and that test
walks the source tree. A reviewer must run this scan by hand.

## Brand result: five classes of user-facing text still name Zed

The gate reads Rust string literals, settings-schema descriptions, action
descriptions, `--help` doc lines, and shipped asset lines. Each of the five
classes below is outside what the gate reads. None of them is in
`prose.classified`. None of them is in
`crates/app_identity/fixtures/compatibility_allowlist.json`.

### 1. The rendered `--help` names Zed as the product

I ran the shipped binaries. This is the text a user sees.

`Contents/MacOS/omega --help`:

```
--dev-server-token <DEV_SERVER_TOKEN>
    Instructs zed to run as a dev server on this machine. (not implemented)
```

`Contents/MacOS/cli --help`:

```
--user-data-dir <DIR>
    Sets a custom directory for all user data (e.g., database, extensions,
    logs). This overrides the default platform-specific data directory
    location: `~/Library/Application Support/Zed`

--foreground
    Run zed in the foreground (useful for debugging)

--zed <ZED>
    Custom path to Omega.app or the omega binary

--dev-server-token <DEV_SERVER_TOKEN>
    Run zed in dev-server mode
```

Two independent mechanisms let this text through.

- `brand.words` holds only `Zed`. The policy comment says that lowercase `zed`
  is not a word on purpose, because it is a part of `authorized` and
  `normalized`. The match rule already stops those two words. `brand_hits`
  matches at ASCII alphanumeric boundaries, and the `zed` in `authorized` has
  an alphanumeric character before it. The stated reason does not need the
  exclusion, and the exclusion hides `Run zed in the foreground`.
- `DOC_LINE` matches only `///` and `//!`. The text
  `~/Library/Application Support/Zed` is in a
  `#[cfg_attr(target_os = "macos", doc = "…")]` attribute at
  `crates/cli/src/main.rs:90`. No scanner reads that attribute.

There is a third cause under both. The gate reads source lines. `clap` joins
several source lines into one sentence at run time. No gate reads the rendered
`--help` output.

The `--user-data-dir` line is also not correct. The data directory of the
candidate is `~/Library/Application Support/Omega RC`. The line tells the user
that it is Zed's directory.

### 2. `--uninstall` removes Zed and keeps Omega

`Contents/MacOS/cli --help` advertises this command:

```
--uninstall
    Uninstall Omega from user system
```

`crates/cli/src/main.rs:566` puts `script/uninstall.sh` into the binary with
`include_bytes!`. That file has no occurrence of `Omega` or `omega`. I read the
script back out of the shipped signed binary at offset `1819657`. It is
unchanged upstream text.

On macOS the script does this:

```sh
app="Zed.app";  app_id="dev.zed.Zed"
rm -rf "/Applications/$app"
rm -rf "$HOME/Library/Application Support/Zed/db/0-$db_suffix"
rm -rf "$HOME/Library/Caches/$app_id"
rm -rf "$HOME/Library/HTTPStorages/$app_id"
rm -rf "$HOME/Library/Preferences/$app_id.plist"
# then, if no /Applications/Zed*.app remains:
rm -rf "$HOME/Library/Application Support/Zed"
rm -rf "$HOME/Library/Logs/Zed"
printf "Do you want to keep your Zed preferences? [Y/n] "   # removes ~/.config/zed
rm -rf $HOME/.zed_server
echo "Zed has been uninstalled"
```

The command removes no Omega path. It does not remove
`/Applications/Omega.app`. It does not remove
`~/Library/Application Support/Omega RC`. It does not remove
`~/Library/Logs/omega-rc`.

The command removes the user's Zed application and the user's Zed data. It
then tells the user `Zed has been uninstalled`.

I did not run this command.

This defeats the omega#16 scope item *"Remove Omega and confirm that Zed data
did not change"*. The shipped removal path changes Zed data by design and
leaves Omega in place.

### 3. Omega identifies itself to a third party as Zed

`crates/open_router/src/open_router.rs:498` and `:591` set these headers on
each OpenRouter request:

```rust
.header("HTTP-Referer", "https://zed.dev")
.header("X-Title", "Zed Editor")
```

The string `Zed Editor` occurs one time in the shipped binary. OpenRouter
shows the `X-Title` value to the user in the user's own dashboard. The product
name that the user reads there is `Zed Editor`.

### 4. The shipped keymap template tells the user to run a `zed:` command

The default keymap template that Omega writes for the user holds this text:

```
// For information on binding keys, see the Zed
// documentation: https://zed.dev/docs/key-bindings
//
// To see the default key bindings run `zed: open default keymap`
// from the command palette.
```

`zed: open default keymap` occurs three times in the binary. The text is also
not correct. `zed::OpenDefaultKeymap` is a deprecated alias, and the policy
says that a deprecated alias never appears in the command palette.

### 5. The `cli` binary is outside every packaged check

`script/verify-omega-brand` opens `Contents/MacOS/omega` at three places. It
never opens `Contents/MacOS/cli` or `Contents/MacOS/omega-identity-proof`.
`script/bundle-omega-rc` copies and signs `cli` into the bundle.

Classes 1 and 2 are both in `cli`. A scan of the shipped `omega` binary cannot
find either one.

### Retained identifiers that the allow-list does not cover

`compatibility_allowlist.json` has 17 entries. These classes ship in the
candidate and no entry covers them:

- the `x-zed-*` and `X-Zed-Predict-*` HTTP headers
- `HTTP-Referer: https://zed.dev` and `X-Title: Zed Editor`
- the `.ZedMono` and `.ZedSans` font aliases
- the `base_keymap` values `Zed` and `Zed (Default)`
- `Zed.dmg` in the update path
- the `ZedPredictModal` keymap context that a user types
- the `zed.dev` settings-provider key and the `ZedDotDev*` settings types
- `dev.zed.Zed`, `~/.zed_server`, `~/.config/zed`, and `/Applications/Zed*.app`
  inside the embedded uninstall script
- entry 9 permits `https://zed.dev/docs` in one named file. The binary holds
  such links from about 20 further places.

The omega#16 criterion *"Every retained Zed identifier is in the reviewed
compatibility allow-list"* fails on rc14.

## Installed observations: blocked

The observation set did not run against rc14. This is the honest result, and I
did not weaken it.

Omega refuses a second instance. `crates/zed/src/zed/mac_only_instance.rs`
holds the lock on a TCP port on `127.0.0.1`. The port comes from the release
channel and the user identifier. It does not come from the data directory, so
`--user-data-dir` does not give a second instance a separate lock.

A different lane started `/Applications/Omega.app` at `2026-07-25T19:44:35`
and still held it. My launch printed `zed is already running` and stopped.

There is one bypass. `ZED_STATELESS` makes the single-instance check pass. It
also makes the database an in-memory database at `crates/db/src/db.rs:178`. A
run under that variable observes a different mode, not the shipped product. I
did not use it.

I did not stop the other lane's application, and I did not stop the host
network. Both acts need agreement first.

The two owner-set flags are still on. `increaseContrast` gives `1` and
`reduceMotion` gives `1`. The `high-contrast` and `reduced-motion` checks can
run as soon as a candidate instance can start.

## Harness defects that a journey cannot correct

These three defects are in the proof harness. They apply to omega#8 whether or
not the journey runs.

### A. The secret tripwire cannot fire

`script/collect-omega-installed-tripwires`, function
`deliver_needle_through_protected_fd`, lines 144 to 159.

The function makes a pipe, writes the needle into the pipe, and closes both
ends in the same function. It starts no process. The `--app` argument does not
occur anywhere in the code, only in the docstring.

The needle is a new random value from `secrets.token_hex(32)`. No other
process ever receives it. The scan then looks for that value on disk.

The scan cannot match. The status `pass` is certain by construction. The
docstring says that the needle goes to the candidate through a protected file
descriptor. The code does not do this.

### B. The tripwire surfaces point at directories that Omega does not write

The constant `SURFACE_ROOTS` resolves against
`~/Library/Application Support/Omega RC`. The real destinations are elsewhere.

| Surface | Scanned path | Real path |
| --- | --- | --- |
| logs | `…/Omega RC/logs` | `~/Library/Logs/omega-rc/omega-rc.log` |
| telemetry | `…/Omega RC/telemetry` | `~/Library/Logs/omega-rc/telemetry.log` |
| crashes | `…/Omega RC/crashes` | `~/Library/Logs/DiagnosticReports` |
| clipboard | `…/Omega RC/clipboard` | the pasteboard, which is not a file |
| accessibility | `…/Omega RC/accessibility` | the tree, which is not a file |

The live Omega log file has 191650 bytes. The tripwire never reads it. Four of
the six surfaces record `absent`, and `absent` does not fail the receipt.

Defect A and defect B each make the receipt empty. Together they make the
omega#8 criterion *"No secret tripwire fires"* impossible to support.

### C. The theme checks write a fact that they do not measure

`script/collect-omega-installed-observations`, the appearance block, lines
1773 to 1813.

The block writes `content_legible: True` as a constant. The block calls
`ocr_lines` zero times. It calls `differing_pixels` zero times. It never
compares the light capture against the dark capture. Its only conditions are
that the host appearance changed and that a screenshot file exists.

A frozen window passes `light-theme` and `dark-theme`. A blank window passes
both. This undercuts the omega#8 criterion *"Theme and registry-agent behavior
match the accepted baseline"*.

The `high-contrast` and `reduced-motion` checks had the same shape earlier.
Commit `5ce7f9855f` corrected them. They now use the system flag only as a
condition, and they take their facts from optical character recognition and
from pixel differences. The appearance block did not get the same correction.

For contrast, the Zed isolation check is sound. It makes a digest over each
regular file under `~/Library/Application Support/Zed` before and after, and
it blocks when the directory is absent.

## omega#8 items that hold

These results come from the artifact and from the rc14 source commit.

- The build lockfile digest is `ce59e721…`, which agrees with the release
  record. The one difference from the committed lockfile is the `zed` package
  version, which `script/bundle-omega-rc` sets to `0.2.0` before the snapshot.
- `nostr 0.44.4`, `keyring 3.6.3`, and `atomic-write-file 0.3.0` are pinned,
  and each checksum agrees with the pin in `script/verify-omega-identity`.
- `BUZZ_SOURCE_COMMIT` is `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`.
- The public vector digest is `1e25670b…`, which agrees with the pin.
- `script/verify-omega-identity` exits `0` against the rc14 source commit. Its
  own digest agrees with the release record.
- The packaged scan gives 0 for `BUZZ_PRIVATE_KEY`, 0 for `identity.key`, and
  0 for `get_nsec`. Control values from the same binary are not zero.
- The identity contract version is `1`. The contract source digest is
  `11e1de73…`.
- `cargo test --package omega_identity --lib` gives 65 passed and 0 failed.

Two limits apply to that list.

- The packaged scan covers three of the six forbidden things in omega#8. The
  other three are structural properties, and only the source scan covers them.
- The release record holds no contract version and no contract source digest.
  Those two values live in the candidate evidence, which is a journey product.

## omega#16 Scope, item by item

| Scope item | rc14 |
| --- | --- |
| Verify the candidate digest | covered |
| Install beside Zed in a clean user profile | **blocked**. The single-instance lock. |
| Offline first start | **blocked**. Needs a candidate instance first. |
| Identity-first entry surface | **blocked** |
| Open a project, edit and save a file, Git status, terminal | **blocked** |
| Restart and layout restoration | **blocked** |
| Visible product surfaces for Zed branding | **FAILED**. Five classes. |
| Icons, bundle metadata, associations, installer, disk image, licenses | **covered**. Each digest agrees with the record. No Zed name in the metadata. |
| Network destinations | **partly**. Not captured. The binary sends `HTTP-Referer: https://zed.dev` and `X-Title: Zed Editor` to OpenRouter. |
| Data, cache, log, credential roots | **FAILED** as documented. The shipped `--help` gives Zed's directory as the Omega data root. |
| Disabled service states and update behavior | **blocked** |
| Remove Omega, confirm Zed data unchanged | **FAILED**. The shipped `--uninstall` removes Zed and keeps Omega. |
| Owner observation and independent verification | **open**. I am the producer. I must not sign either. |

## Falsifiers, addressed

### omega#16

> *Unit tests, source scans, or screenshots are the only evidence, or the
> installed candidate exposes any unclassified Zed product, service, package,
> or state dependency.*

The second part fires. Each of these is unclassified by the prose registry and
by the compatibility allow-list:

- a Zed **product** name in the rendered `--help` of both shipped binaries
- a Zed **service** identity in the `X-Title` header that a third party shows
  to the user
- a Zed **state** dependency in `--uninstall`, which removes five Zed paths
- a wrong Zed **state** claim in `--user-data-dir`, which names Zed's directory
  as the Omega data root

The first part does not fire. The evidence is the signed and notarized
candidate: its package assessment, its stapled ticket, its recovered embedded
script, and its own rendered `--help` output. Class 2 came from text that I
read back out of the shipped signed binary, not from the source tree.

### omega#8

> *Unit tests, fixtures, or screenshots are the only evidence.*

This does not fire for the supply-chain items. Those come from the artifact,
from its lockfile snapshot, and from digests that I made again.

omega#8 still fails. Its criterion *"No secret tripwire fires"* rests on a
receipt that cannot fail. Its criterion *"Theme and registry-agent behavior
match the accepted baseline"* rests on a constant. The journey did not run.

### omega#9

omega#9 closes only when omega#8 passes. omega#8 does not pass.

## What must happen next

1. Correct the five brand classes. `--uninstall` is first, because it destroys
   the user's data now.
2. Add the `cli` binary and the `omega-identity-proof` binary to the packaged
   brand scan.
3. Read rendered `--help` output in the gate, not source doc lines only.
4. Add lowercase `zed` to the word rule. The boundary rule already excludes
   `authorized` and `normalized`.
5. Read `#[cfg_attr(…, doc = "…")]` attributes in the doc-line scanner.
6. Apply `first_party_agent.phrases` to a package in the gate.
7. Correct harness defects A, B, and C.
8. Cut the next candidate and repeat this record with a free host.
9. Independent review. The reviewer must not be the producer.

## Reproduction for the reviewer

The reviewer must make each result from the artifacts and from the running
application. The reviewer must not accept a value from this document.

```sh
# 1. the package, from the released artifacts
gh release download v0.2.0-rc14 -p '*' -D rc14
shasum -a 256 rc14/Omega-v0.2.0-rc14-macos-arm64.dmg
hdiutil attach -nobrowse -readonly -mountpoint /tmp/rc14mnt \
  rc14/Omega-v0.2.0-rc14-macos-arm64.dmg
ditto /tmp/rc14mnt/Omega.app /tmp/rc14app/Omega.app
shasum -a 256 /tmp/rc14app/Omega.app/Contents/MacOS/omega   # d55d0961…
xcrun stapler validate /tmp/rc14app/Omega.app               # the copy keeps the ticket
spctl -a -vvv -t exec /tmp/rc14app/Omega.app
codesign --verify --deep --strict --verbose=2 /tmp/rc14app/Omega.app
```

```sh
# 2. the prose correction, and the control that proves the scan reads
strings -a /tmp/rc14app/Omega.app/Contents/MacOS/omega | grep -c "in Zed"    # 0
strings -a /tmp/rc14app/Omega.app/Contents/MacOS/omega \
  | grep -c "You are the Zed coding agent running inside the Zed editor"     # 0
strings -a /tmp/rc14app/Omega.app/Contents/MacOS/omega | grep -c "Omega"     # 596
plutil -convert xml1 -o - /tmp/rc14app/Omega.app/Contents/Info.plist \
  | grep -ic zed                                                             # 0
```

```sh
# 3. the five classes. Run the shipped binaries; do not read the source.
/tmp/rc14app/Omega.app/Contents/MacOS/omega --help | grep -i zed
/tmp/rc14app/Omega.app/Contents/MacOS/cli  --help | grep -i zed
python3 - <<'PY'
from pathlib import Path
b = Path("/tmp/rc14app/Omega.app/Contents/MacOS/cli").read_bytes()
i = b.find(b"# Uninstalls")
print(b[i:i+4200].split(b"\x00")[0].decode())     # the embedded uninstall script
PY
strings -a /tmp/rc14app/Omega.app/Contents/MacOS/omega | grep -c "Zed Editor"
strings -a /tmp/rc14app/Omega.app/Contents/MacOS/omega | grep -c "zed: open default keymap"
```

Do not run `cli --uninstall`. It removes the reviewer's own Zed installation
and Zed data.

```sh
# 4. the gate, and the gap the gate leaves
./script/verify-omega-brand --app /tmp/rc14app/Omega.app   # exits 0
grep -c first_party_agent script/verify-omega-brand        # 0
grep -n "Contents/MacOS" script/verify-omega-brand         # only Contents/MacOS/omega
# hand-run the phrase list, because no gate applies it to a package
python3 - <<'PY'
import json, subprocess
phrases = json.load(open("script/omega-brand-gate.json"))["first_party_agent"]["phrases"]
out = subprocess.run(["strings","-a","/tmp/rc14app/Omega.app/Contents/MacOS/omega"],
                     capture_output=True, text=True).stdout.lower()
for p in phrases:
    print(f"{p}: {out.count(p.lower())}")
print("Omega Agent (control):", out.count("omega agent"))
PY
```

```sh
# 5. the harness defects
sed -n '144,159p' script/collect-omega-installed-tripwires   # the needle goes nowhere
grep -c -- --app script/collect-omega-installed-tripwires    # docstring only
sed -n '1773,1813p' script/collect-omega-installed-observations  # content_legible: True
```

The reviewer identity is `0326d8f9…`. The command is
`script/review-omega-candidate`, and it needs `OMEGA_REVIEWER_KEY_FILE`. The
producer must not set that variable and must not sign the receipt.
