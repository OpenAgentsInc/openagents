# Briefing: #10893 Drop the command hint from the agent's PROPOSED line

## The issue

`openagents agent ask NAME ... --wait` and `openagents agent show NAME` print a pending proposal as:

```
PROPOSED: touch notes.txt (Coder needs it for this step) -- openagents agent answer alice confirm|reject
```

The trailing hint repeats what `openagents agent --help` already says, and it doubles the line's length in terminals and logs.

## Acceptance

- Both places print `PROPOSED: COMMAND (WHY)`, with no `-- openagents agent answer ...` hint.
- One function in `crates/openagents-cli/src/agent.rs` formats the line, and both places use it.
- A unit test pins the format.
- `cargo test -p openagents-cli --bin openagents -- agent` passes.

Small and repository-local: only `crates/openagents-cli/src/agent.rs` changes.

## Change plan

1. Goal: Drop the command hint from the agent's PROPOSED line
2. Required: Both places print `PROPOSED: COMMAND (WHY)`, with no `-- openagents agent answer ...` hint.
3. Required: One function in `crates/openagents-cli/src/agent.rs` formats the line, and both places use it.
4. Required: A unit test pins the format.
5. Required: `cargo test -p openagents-cli --bin openagents -- agent` passes.
6. Change the behavior where it lives, most likely in `crates/openagents-cli/src/agent.rs`, `crates/coder/src/cli_route/tree.rs`, `scripts/desktop/package-macos.sh`.
7. Add or update a test that pins the new behavior, beside the code's existing tests.
8. Run `check:openagents-cli`, then `test:openagents-cli`, then `check:coder`, then `test:coder`, then `fmt`; stop when they pass.

## Files to change

### `crates/openagents-cli/src/agent.rs`

Why: the issue names `crates/openagents-cli/src/agent.rs`; has `openagents agent --help`; has `--wait`; has `PROPOSED`

```
--- lines 56-88 of 2355 ---
   56                 Explicitly resume the same exact subset at its current digest.
   57                 Disabled jobs stay off; stale approvals cannot resume.
   58    attest NAME --owner-key FILE [--days N]
   59                 Attest her key again.
   60    renew NAME --owner-key FILE [--days N]
   61                 Renew the owner's attestation of her key before it expires,
   62                 and sign her profile again.
   63    list        Every agent: state, activity, last report, service record.
   64    show NAME [--owner-key FILE]
   65                 One agent in full: key, attestation, transcript, jobs, and
   66                 spend today and in all against her budget, from her NIP-AM
   67                 records, decrypted with the owner key in FILE when given.
   68    ask NAME TEXT... [--mode MODE] [--workspace LABEL] [--from DIR] [--wait]
   69                 Hand her a request; MODE is auto, task, or terminal, DIR is
   70                 where you asked from, and --wait follows it to her report.
   71    answer NAME confirm
   72                 CONFIRM the command she proposed.
   73    answer NAME reject
   74                 REJECT the command she proposed.
   75    stop NAME [--reason TEXT]
   76                 The kill switch: her standing jobs go off, her panes are
   77                 released with Ctrl+C, her work is cancelled, and she starts
   78                 nothing until resumed. Each step is journaled.
   79    pause NAME   She keeps everything and starts nothing new.
   80    resume NAME  She takes work again.
   81    retire NAME [--owner-key FILE]
   82                 Stop her, delete her key from the host's key store, and keep
   83                 her journal and engrams, which the owner key still reads.
   84                 With relay sync on and the owner key, ask her relays to
   85                 archive her key (NIP-IA).
   86    rotate NAME [--owner-key FILE] [--reason TEXT] [--days N]
   87                 Give her a new key: every engram is encrypted again under
   88                 it, the owner signs a lineage record and a new attestation,
--- lines 829-859 of 2355 ---
  829                      if charter.drafting { "on" } else { "off" }
  830                  ));
  831              }
  832          }
  833          text.extend([
  834              format!("works in: {}", record.workspace),
  835              format!("charter: {}", record.charter),
  836          ]);
  837          if let Some(view) = &view {
  838              text.push(line(view));
  839              if let Some(pending) = &view.pending {
  840                  text.push(format!(
  841                      "PROPOSED: {} ({}) -- openagents agent answer {name} confirm|reject",
  842                      pending.command, pending.why
  843                  ));
  844              }
  845              if let Some(change) = &view.change {
  846                  text.push(format!("change: task {} ({})", change.task, change.stage));
  847              }
  848              text.push(String::new());
  849              text.extend(view.lines.iter().cloned());
  850          }
  851          for job in &jobs {
  852              text.push(format!(
  853                  "job {} [{}] {} {}/{} {}",
  854                  job.job,
  855                  if job.enabled { "on" } else { "off" },
  856                  job.trigger,
  857                  job.occurrences,
  858                  job.max_occurrences,
  859                  job.last.as_deref().unwrap_or("")
--- lines 969-999 of 2355 ---
  969          let view = view_of(args, name)?;
  970          if !output.json() {
  971              let total = view.lines.len();
  972              for line in view.lines.iter().skip(shown.min(total)) {
  973                  println!("{line}");
  974              }
  975              shown = total;
  976              if let Some(pending) = &view.pending
  977                  && proposed != Some(pending.step)
  978              {
  979                  proposed = Some(pending.step);
  980                  println!(
  981                      "PROPOSED: {} ({}) -- openagents agent answer {name} confirm|reject",
  982                      pending.command, pending.why
  983                  );
  984              }
  985          }
  986          if !view.busy && view.service.requests > before {
  987              output.emit(&json!({"agent": view}), |_| String::new());
  988              return Ok(());
  989          }
  990          if start.elapsed() > Duration::from_secs(60 * 60) {
  991              return Err(Fail::Failed(format!(
  992                  "{} has not reported in an hour",
  993                  agent::Refer::for_name(name).they()
  994              )));
  995          }
  996          std::thread::sleep(Duration::from_millis(500));
  997      }
  998  }
  999  
--- lines 1949-1979 of 2355 ---
 1949          ["delete", job] => {
 1950              jobs.edit(job, Edit::Delete, now).map_err(Fail::Failed)?;
 1951              format!("Deleted {job}.")
 1952          }
 1953          ["renew", job] => {
 1954              let days: u64 = args.number("days", 30).map_err(Fail::Failed)?;
 1955              jobs.edit(job, Edit::Renew(now + days * 86_400), now)
 1956                  .map_err(Fail::Failed)?;
 1957              format!("Renewed {job} for {days} days.")
 1958          }
 1959          _ => {
 1960              return Err(Fail::Failed(
 1961                  "unknown jobs command; see `openagents agent --help`".into(),
 1962              ));
 1963          }
 1964      };
 1965      output.emit(&json!({"done": said}), |_| said.clone());
 1966      Ok(())
 1967  }
 1968  
 1969  /// This computer's offset from UTC in minutes, from `date +%z`.
 1970  fn utc_offset() -> i32 {
 1971      std::process::Command::new("date")
 1972          .arg("+%z")
 1973          .output()
 1974          .ok()
 1975          .and_then(|o| {
 1976              let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
 1977              let sign = if text.starts_with('-') { -1 } else { 1 };
 1978              let digits = text.trim_start_matches(['+', '-']);
 1979              let hours: i32 = digits.get(..2)?.parse().ok()?;
```

### `crates/coder/src/cli_route/tree.rs`

Why: has `--wait`; has `openagents-cli`

```
--- lines 1-28 of 617 ---
    1  //! The `openagents` command tree, generated from its help text.
    2  //!
    3  //! [`build`] reads the top-level help table and each group's `USAGE`
    4  //! string with [`super::usage`] and joins every command it finds with the
    5  //! [`Declared`] effect and `runs_on` its owning module states next to that
    6  //! `USAGE`. A command in the help with no declaration, or a declaration
    7  //! with no command, is an error, so a new command cannot reach the router
    8  //! without saying what it does.
    9  //!
   10  //! `openagents-cli` builds the tree from its live strings and checks that
   11  //! the copy bundled here ([`bundled`], `tree.json`) is the same; set
   12  //! `OPENAGENTS_WRITE_CLI_TREE=1` on its test to write a new one. Nothing
   13  //! in the file is written by hand.
   14  
   15  use std::sync::OnceLock;
   16  
   17  use serde::{Deserialize, Serialize};
   18  
   19  use super::usage::{self, Form, Token};
   20  
   21  /// The tree's schema, recorded in the file and on every proposal.
   22  pub const SCHEMA: &str = "openagents.cli-tree.v1";
   23  
   24  pub use crate::router::{Effect, RunsOn};
   25  
   26  /// Every effect class, in the order of consequence.
   27  pub const EFFECTS: [Effect; 7] = [
   28      Effect::ReadOnly,
--- lines 153-183 of 617 ---
  153      /// A command whose phone equivalent is `screen`.
  154      #[must_use]
  155      pub const fn screen(path: &'static str, effect: Effect, screen: &'static str) -> Self {
  156          Self {
  157              path,
  158              effect,
  159              runs_on: RunsOn::Screen,
  160              screen: Some(screen),
  161          }
  162      }
  163  }
  164  
  165  /// One group's help, as `openagents-cli` hands it over.
  166  #[derive(Clone, Copy, Debug)]
  167  pub struct GroupHelp<'a> {
  168      /// The group's name in the top-level table.
  169      pub name: &'a str,
  170      /// Its `USAGE` text, or `None` for a group with no syntax of its own
  171      /// (`doctor`, `version`), whose summary line is its whole help.
  172      pub usage: Option<&'a str>,
  173      pub declared: &'a [Declared],
  174      /// Another name for a group already in the tree (`xp` is `verse xp`);
  175      /// the tree lists the command once.
  176      pub alias: bool,
  177  }
  178  
  179  /// A runnable command.
  180  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  181  pub struct Leaf {
  182      /// The full command words: group first (`["computer", "list"]`).
  183      pub path: Vec<String>,
--- lines 431-484 of 617 ---
  431                  "`openagents {name} {}` is declared but not in the help",
  432                  help.declared[index].path
  433              ));
  434          }
  435      }
  436      if errors.is_empty() {
  437          Ok(node)
  438      } else {
  439          Err(errors)
  440      }
  441  }
  442  
  443  /// The tree bundled with this crate, generated by `openagents-cli`.
  444  ///
  445  /// # Panics
  446  ///
  447  /// Never in a build whose `tree.json` came from `openagents-cli`'s
  448  /// generator, which a test in that crate checks.
  449  #[must_use]
  450  pub fn bundled() -> &'static CommandTree {
  451      static TREE: OnceLock<CommandTree> = OnceLock::new();
  452      TREE.get_or_init(|| {
  453          serde_json::from_str(include_str!("tree.json")).expect("the bundled command tree parses")
  454      })
  455  }
  456  
  457  #[cfg(test)]
  458  mod tests {
  459      use super::*;
  460  
  461      const TOP: &str = "usage: openagents COMMAND
  462    computer     Enroll with hosts.
  463    doctor       Show the identities.
  464    xp           This identity's XP.";
  465      const COMPUTER: &str = "usage: openagents computer COMMAND [OPTIONS]
  466    list [--wait SECONDS]     Every host this device knows.
  467    show HOST                 One host.
  468  Options: --store DIR.";
  469  
  470      #[test]
  471      fn builds_from_help_and_declarations() {
  472          let declared = [
  473              Declared::device("list", Effect::ReadOnly),
  474              Declared::device("show", Effect::ReadOnly),
  475          ];
  476          let tree = build(
  477              TOP,
  478              &[
  479                  GroupHelp {
  480                      name: "computer",
  481                      usage: Some(COMPUTER),
  482                      declared: &declared,
  483                      alias: false,
  484                  },
```

### `scripts/desktop/package-macos.sh`

Why: has `--wait`; has `openagents-cli`

```
--- lines 5-35 of 428 ---
    5  #
    6  # Steps (docs/desktop/release.md has the full runbook):
    7  #   1. Build universal (arm64 + x86_64) release binaries of the app
    8  #      (`openagents-desktop`), `coder`, `microcoder`, and `openagents`, and glue each pair
    9  #      with `lipo`.
   10  #   2. Assemble OpenAgents.app: Contents/MacOS/{OpenAgents,coder,microcoder},
   11  #      Contents/Helpers/openagents,
   12  #      Info.plist, icon, and the host's launchd plist in
   13  #      Contents/Library/LaunchAgents/.
   14  #   3. Sign every executable, inner ones first, with the Developer ID
   15  #      Application identity, the hardened runtime, a secure timestamp, and
   16  #      entitlements (the app's and the embedded host's).
   17  #   4. Notarize the app with `xcrun notarytool submit --wait` and staple it.
   18  #   5. Build the .dmg (the app plus an Applications symlink to drag it onto),
   19  #      sign it, notarize it, staple it.
   20  #   6. Check: `codesign --verify --strict`, `spctl --assess` on the app and
   21  #      the .dmg, and `stapler validate` on both.
   22  #
   23  # Options:
   24  #   --app PATH          Package an already assembled .app instead of building
   25  #                       one (skips steps 1-2). Any macOS .app works, e.g. the
   26  #                       deck from scripts/bundle-openagents-deck.sh.
   27  #   --out DIR           Where the .app and .dmg go
   28  #                       (default: $CARGO_TARGET_DIR/desktop-release, or
   29  #                       target/desktop-release).
   30  #   --identity ID       Signing identity: a name or SHA-1 from
   31  #                       `security find-identity -v -p codesigning`, or `-` for
   32  #                       ad hoc (local testing; implies --no-notarize).
   33  #                       Default: $OA_DEVELOPER_ID_APPLICATION, else the first
   34  #                       "Developer ID Application" identity in the keychain,
   35  #                       else ad hoc.
--- lines 226-256 of 428 ---
  226  
  227    if [[ -n "$bin_dir" ]]; then
  228      step "taking $bin, coder, microcoder, openagents from $bin_dir"
  229    else
  230      step "building $package, coder, microcoder, openagents ($([[ $native -eq 1 ]] && echo native || echo universal))"
  231    fi
  232    build_universal "$package" "$bin"; app_bin="$built"
  233    build_universal coder coder; coder_bin="$built"
  234    build_universal microcoder microcoder; micro_bin="$built"
  235    # The command a phone's read-only command card runs on this Mac; the
  236    # host puts Contents/Helpers first on its terminals' PATH. Never in
  237    # Contents/MacOS: on a case-insensitive volume openagents is OpenAgents.
  238    build_universal openagents-cli openagents; cli_bin="$built"
  239  
  240    app="$out/OpenAgents.app"
  241    step "assembling $app"
  242    rm -rf "$app"
  243    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Library/LaunchAgents"
  244  
  245    if [[ -f "$macos_dir/Info.plist" ]]; then
  246      cp "$macos_dir/Info.plist" "$app/Contents/Info.plist"
  247      /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist" 2>/dev/null ||
  248        /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $version" "$app/Contents/Info.plist"
  249      # The build number, as bins/openagents-desktop-macos/bundle.sh sets it.
  250      local build
  251      build="$(git -C "$root" rev-list --count HEAD 2>/dev/null || echo 1)"
  252      /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build" "$app/Contents/Info.plist" 2>/dev/null ||
  253        /usr/libexec/PlistBuddy -c "Add :CFBundleVersion string $build" "$app/Contents/Info.plist"
  254    else
  255      cat >"$app/Contents/Info.plist" <<PLIST
  256  <?xml version="1.0" encoding="UTF-8"?>
--- lines 365-395 of 428 ---
  365    fi
  366  done < <(find "$app/Contents" -type f -perm -u+x -o -type f -name '*.dylib' | awk '{ print length, $0 }' | sort -rn | cut -d' ' -f2-)
  367  while IFS= read -r nested; do sign "$nested"; done < <(
  368    find "$app/Contents" -depth \( -name '*.framework' -o -name '*.app' -o -name '*.appex' -o -name '*.xpc' \) -type d)
  369  sign "$app" "$app_entitlements"
  370  codesign --verify --strict --deep --verbose=2 "$app"
  371  
  372  # ---------------------------------------------------------------- notarize
  373  notarize() { # $1 file to submit
  374    local log id status
  375    log="$work/notary-$(basename "$1").json"
  376    step "notarizing $(basename "$1") (this takes a few minutes)"
  377    xcrun notarytool submit "$1" "${notary_auth[@]}" --wait --output-format json >"$log" || true
  378    id="$(plutil -extract id raw -o - "$log" 2>/dev/null || true)"
  379    status="$(plutil -extract status raw -o - "$log" 2>/dev/null || true)"
  380    echo "    submission ${id:-?}: ${status:-no status}"
  381    if [[ "$status" != "Accepted" ]]; then
  382      if [[ -n "$id" ]]; then
  383        xcrun notarytool log "$id" "${notary_auth[@]}" >&2 || true
  384      else
  385        cat "$log" >&2
  386      fi
  387      die "notarization of $(basename "$1") was not accepted"
  388    fi
  389  }
  390  
  391  if [[ $notarize -eq 1 ]]; then
  392    ditto -c -k --keepParent "$app" "$work/$app_name.zip"
  393    notarize "$work/$app_name.zip"
  394    xcrun stapler staple -q "$app"
  395    xcrun stapler validate -q "$app"
```

### `scripts/grid-soak.sh`

Why: has `--wait`; has `openagents-cli`

```
--- lines 56-86 of 154 ---
   56  if [[ "$world" != verse-bare ]]; then
   57    walkers=$players
   58  fi
   59  port="${GRID_SOAK_PORT:-7457}"
   60  pg_port="${GRID_SOAK_PG_PORT:-55457}"
   61  target="${CARGO_TARGET_DIR:-$root/target}"
   62  
   63  for tool in initdb pg_ctl createdb cargo; do
   64    command -v "$tool" >/dev/null || { echo "grid-soak: $tool is not on PATH" >&2; exit 1; }
   65  done
   66  
   67  cargo build -q --release --manifest-path "$root/Cargo.toml" \
   68    -p nostr-relay --bin nostr-relay -p openagents-cli --bin openagents
   69  cargo build -q --release --manifest-path "$root/Cargo.toml" -p verse --example grid_soak
   70  
   71  scratch="$(mktemp -d "${TMPDIR:-/tmp}/grid-soak.XXXXXX")"
   72  mkdir -p "$scratch/home" "$scratch/socket"
   73  pids=()
   74  stop() {
   75    for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
   76    wait 2>/dev/null || true
   77    pg_ctl -D "$scratch/data" -m fast stop >/dev/null 2>&1 || true
   78    rm -rf "$scratch"
   79  }
   80  trap stop EXIT
   81  
   82  initdb -D "$scratch/data" -A trust --no-locale -E UTF8 >/dev/null
   83  pg_ctl -D "$scratch/data" -l "$scratch/postgres.log" \
   84    -o "-c listen_addresses='127.0.0.1' -c port=$pg_port -c unix_socket_directories='$scratch/socket'" \
   85    -w start >/dev/null
   86  createdb -h "$scratch/socket" -p "$pg_port" -U "$(id -un)" grid_soak
--- lines 116-154 of 154 ---
  116    "players": $players,
  117    "walkers": $walkers,
  118    "clients": $(if [[ "$world" == verse-bare ]]; then echo '["phone (simulated)", "desktop (simulated)", "browser (simulated)"]'; else echo '[]'; fi),
  119    "seconds": $seconds,
  120    "leases": "${OPENAGENTS_LEASES:-}",
  121    "host": "$(uname -sm), $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -p)",
  122    "started": "$(date -u +%FT%TZ)"
  123  }
  124  EOF
  125  
  126  oa="$target/release/openagents"
  127  HOME="$scratch/home" "$oa" verse walkers "$walkers" --relay "$relay" --world "$world" \
  128    --wait $((seconds + 120)) --json >"$out/walkers.ndjson" 2>&1 &
  129  pids+=($!)
  130  sleep 10
  131  HOME="$scratch/home" "$oa" verse load --relay "$relay" --world "$world" --players "$players" \
  132    --max-age-ms 1000 --wait "$seconds" --json >"$out/load.json" 2>&1 &
  133  load_pid=$!
  134  pids+=($load_pid)
  135  
  136  # Halfway through, a 21st player tries to join the full world; the relay
  137  # must refuse it while the 20 play on.
  138  (sleep $((seconds / 2)); HOME="$scratch/home" "$oa" verse walkers 1 --relay "$relay" --world "$world" \
  139    --wait 20 --json >"$out/cap.ndjson" 2>&1) &
  140  pids+=($!)
  141  
  142  set +e
  143  clients=0
  144  if [[ "$world" == verse-bare ]]; then
  145    HOME="$scratch/home" "$target/release/examples/grid_soak" --relay "$relay" \
  146      --seconds "$seconds" --out "$out" 2>"$out/clients.log"
  147    clients=$?
  148  fi
  149  wait "$load_pid"
  150  load=$?
  151  set -e
  152  
  153  echo "grid-soak: clients exit $clients, load exit $load; receipts in $out"
  154  [[ $clients -eq 0 && $load -eq 0 ]]
```

### `scripts/release/coder.sh`

Why: has `--wait`; has `openagents-cli`

```
--- lines 85-115 of 786 ---
   85    *) native_platform='' ;;
   86  esac
   87  
   88  # The bucket prefix every name below lives under.
   89  prefix=coder
   90  product=coder
   91  companion=openagents
   92  engine=microcoder
   93  helper=coder-boundary
   94  # The cargo package and binary behind each published name.
   95  product_package=coder-new
   96  product_bin=coder-new
   97  companion_package=openagents-cli
   98  companion_bin=openagents
   99  engine_package=microcoder
  100  engine_bin=microcoder
  101  
  102  version=''
  103  commit_ref=HEAD
  104  targets=''
  105  publish=0
  106  channel=rc
  107  allow_partial=0
  108  skip_notarization=0
  109  point_channel_name=''
  110  publish_installers=0
  111  
  112  assess_attempts=${CODER_RELEASE_ASSESS_ATTEMPTS:-45}
  113  assess_delay=${CODER_RELEASE_ASSESS_DELAY:-60}
  114  notarized=''
  115  gatekeeper_verdicts=''
--- lines 427-457 of 786 ---
  427  existing=$(bucket_objects)
  428  for platform in $all_platforms; do
  429    for name in $(products_for "$platform"); do
  430      if printf '%s\n' "$existing" | grep -qxF "$(artifact_name "$name" "$platform")"; then
  431        die "refusing to build: $root/$(artifact_name "$name" "$platform") already exists; take the next version"
  432      fi
  433    done
  434  done
  435  
  436  # The version the binaries print is their crates' own, so a --version that
  437  # differs would publish artifacts named for one release that call themselves
  438  # another.
  439  for manifest in crates/coder-new/Cargo.toml crates/openagents-cli/Cargo.toml crates/microcoder/Cargo.toml; do
  440    crate_version=$(git -C "$repo_root" show "$sha:$manifest" 2>/dev/null |
  441      sed -n 's/^version = "\(.*\)"$/\1/p' | head -1)
  442    [ "$crate_version" = "$version" ] ||
  443      die "$manifest says version ${crate_version:-(workspace)} at $short, and this is $version; commit the bundled version bump first"
  444  done
  445  
  446  [ -n "$targets" ] || targets=$all_platforms
  447  
  448  for command_name in cargo file shasum git; do
  449    command -v "$command_name" >/dev/null 2>&1 || die "$command_name is required to build a release"
  450  done
  451  
  452  dist="$repo_root/dist/releases/$prefix/$version"
  453  rm -rf "$dist"
  454  mkdir -p "$dist"
  455  
  456  source=$(mktemp -d "${TMPDIR:-/tmp}/coder-terminal-source.XXXXXX")
  457  trap 'rm -rf "$source"' EXIT
--- lines 475-508 of 786 ---
  475    fi
  476    [ -n "${OA_DEVELOPER_ID_APPLICATION:-}" ] ||
  477      die "no Developer ID Application identity: set OA_DEVELOPER_ID_APPLICATION or install the certificate"
  478    if [ "$skip_notarization" = 0 ] && [ -z "${NOTARY_KEYCHAIN_PROFILE:-}" ]; then
  479      [ -n "${ASC_API_KEY_ID:-}" ] && [ -n "${ASC_API_ISSUER_ID:-}" ] && [ -n "${ASC_API_PRIVATE_KEY_PATH:-}" ] ||
  480        die "notarization needs NOTARY_KEYCHAIN_PROFILE or ASC_API_KEY_ID, ASC_API_ISSUER_ID and ASC_API_PRIVATE_KEY_PATH (OPENAGENTS_NOTARY_ENV names a file)"
  481    fi
  482    signing_loaded=1
  483  }
  484  
  485  notarize_zip() {
  486    if [ -n "${NOTARY_KEYCHAIN_PROFILE:-}" ]; then
  487      xcrun notarytool submit "$1" --keychain-profile "$NOTARY_KEYCHAIN_PROFILE" --wait --timeout 30m
  488    else
  489      xcrun notarytool submit "$1" --key "$ASC_API_PRIVATE_KEY_PATH" \
  490        --key-id "$ASC_API_KEY_ID" --issuer "$ASC_API_ISSUER_ID" --wait --timeout 30m
  491    fi
  492  }
  493  
  494  # Sign each macOS artifact of a platform and notarize them in one
  495  # submission. Sets `notary_status` and `notary_submission`.
  496  sign_and_notarize() {
  497    platform=$1
  498    shift
  499    load_signing
  500    for artifact in "$@"; do
  501      case "$(basename "$artifact")" in
  502        "$engine"-*) identifier=$(signing_identifier "$engine") ;;
  503        "$companion"-*) identifier=$(signing_identifier "$companion") ;;
  504        *) identifier=$(signing_identifier "$product") ;;
  505      esac
  506      echo "  signing $(basename "$artifact") as $identifier"
  507      codesign --force --timestamp --options runtime --entitlements "$entitlements" \
  508        --identifier "$identifier" --sign "$OA_DEVELOPER_ID_APPLICATION" "$artifact" \
```

### `scripts/release/native-terminal.py`

Why: has `--wait`; has `openagents-cli`

```
--- lines 51-99 of 249 ---
   51      destination = Path(args.out).resolve()
   52      destination.mkdir(parents=True, exist_ok=True)
   53      with tempfile.TemporaryDirectory(prefix="native-terminal-source-") as directory:
   54          source = Path(directory)
   55          archive = source / "source.tar"
   56          with archive.open("wb") as file:
   57              run("git", "archive", commit, cwd=REPO, stdout=file)
   58          with tarfile.open(archive) as file:
   59              file.extractall(source, filter="data")
   60          archive.unlink()
   61          snapshot = {str(path.relative_to(source)): digest(path) for path in source.rglob("*") if path.is_file()}
   62          versions = [tomllib.loads((source / f"crates/{crate}/Cargo.toml").read_text())["package"]["version"]
   63                      for crate in ("terminal-app", "openagents-cli", "microcoder")]
   64          if len(set(versions)) != 1:
   65              raise ValueError("App and helper versions must match.")
   66          version = versions[0]
   67          stage = destination / version
   68          if stage.exists():
   69              raise ValueError(f"Refusing to replace {stage}.")
   70          stage.mkdir()
   71          bundle = stage / APP
   72          contents = bundle / "Contents"
   73          binaries = contents / "MacOS"
   74          binaries.mkdir(parents=True)
   75          shutil.copy2(source / "scripts/release/openagents-terminal.entitlements", stage / "release-entitlements.plist")
   76          shutil.copy2(source / "scripts/release/install-native-terminal.py", stage / "install-native-terminal.py")
   77          env = dict(os.environ, OPENAGENTS_BUILD_COMMIT=commit)
   78          # Separate graphs keep the CLI's Verse features out of the GUI executable.
   79          run("cargo", "build", "--locked", "--release", "-p", "terminal-app", cwd=source, env=env)
   80          shutil.copy2(Path(target) / "release/openagents-terminal", binaries)
   81          run("cargo", "build", "--locked", "--release", "-p", "openagents-cli", "-p", "microcoder", cwd=source, env=env)
   82          for name in ("openagents", "microcoder"):
   83              shutil.copy2(Path(target) / f"release/{name}", binaries)
   84          for relative, expected in snapshot.items():
   85              if digest(source / relative) != expected:
   86                  raise ValueError(f"A build changed archived source: {relative}")
   87          for binary in binaries.iterdir():
   88              identity = subprocess.check_output(["file", str(binary)], text=True)
   89              if "Mach-O" not in identity or "arm64" not in identity:
   90                  raise ValueError(f"Wrong executable architecture: {binary.name}")
   91          with (contents / "Info.plist").open("wb") as file:
   92              plistlib.dump({"CFBundleIdentifier": "com.openagents.terminal", "CFBundleName": "OpenAgents Terminal",
   93                            "CFBundleExecutable": "openagents-terminal", "CFBundlePackageType": "APPL",
   94                            "CFBundleShortVersionString": version.split("-")[0], "CFBundleVersion": version.replace("-rc.", "."),
   95                            "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "13.0"}, file)
   96          manifest = {"schema": "openagents.native-terminal.release.v1", "version": version,
   97                      "commit": commit, "tree": tree, "prefix": PREFIX, "platform": "darwin-arm64",
   98                      "toolchain": output("rustc", "--version"), "bundle": APP,
   99                      "executables": {name: digest(binaries / name) for name in ("openagents-terminal", "openagents", "microcoder")},
--- lines 140-170 of 249 ---
  140      entitlements = stage / "release-entitlements.plist"
  141      for name in manifest["executables"]:
  142          run("codesign", "--force", "--options", "runtime", "--timestamp", "--entitlements", str(entitlements),
  143              "--sign", identity, str(bundle / "Contents/MacOS" / name))
  144      run("codesign", "--force", "--options", "runtime", "--timestamp", "--sign", identity, str(bundle))
  145      run("codesign", "--verify", "--deep", "--strict", str(bundle))
  146      manifest["executables"] = {name: digest(bundle / "Contents/MacOS" / name) for name in manifest["executables"]}
  147      manifest.update(signing="passed", notarization="not-run", gatekeeper="not-run", public_readback="not-run")
  148      save_manifest(stage, manifest)
  149      archive = stage / "OpenAgents-Terminal.zip"
  150      run("ditto", "-c", "-k", "--keepParent", str(bundle), str(archive))
  151      response = subprocess.check_output(["xcrun", "notarytool", "submit", str(archive), *auth,
  152                                          "--wait", "--timeout", "30m", "--output-format", "json"], text=True)
  153      verdict = json.loads(response)
  154      if verdict.get("status") != "Accepted":
  155          raise ValueError("Notarization was not accepted; nothing will be published.")
  156      run("xcrun", "stapler", "staple", str(bundle))
  157      run("xcrun", "stapler", "validate", str(bundle))
  158      run("codesign", "--verify", "--deep", "--strict", str(bundle))
  159      run("spctl", "--assess", "--type", "execute", "--verbose=2", str(bundle))
  160      # The public archive contains the stapled app, not the pre-submission archive.
  161      archive.unlink()
  162      run("ditto", "-c", "-k", "--keepParent", str(bundle), str(archive))
  163      manifest.update(signing="passed", notarization="passed", gatekeeper="passed", notarization_id=verdict.get("id"),
  164                      archive_sha256=digest(archive), public_readback="not-run")
  165      manifest["executables"] = {name: digest(bundle / "Contents/MacOS" / name) for name in manifest["executables"]}
  166      save_manifest(stage, manifest)
  167      (stage / "SHA256SUMS").write_text(f"{manifest['archive_sha256']}  {archive.name}\n{digest(stage / 'release-manifest.json')}  release-manifest.json\n{manifest['installer_sha256']}  install-native-terminal.py\n")
  168      print("Signed, notarized, stapled, and Gatekeeper accepted; publication has not run.")
  169  
  170  
```

## Similar past changes

### 063044d929 Signed Mac app can run plugins: allow wasmtime's executable memory

```diff
diff --git a/scripts/desktop/package-macos.sh b/scripts/desktop/package-macos.sh
index 7d194eac2f..5fb53f48df 100755
--- a/scripts/desktop/package-macos.sh
+++ b/scripts/desktop/package-macos.sh
@@ -136,4 +136,7 @@ fi
 
 # ------------------------------------------------------------ entitlements
+# Plugins run as Wasm under wasmtime, which makes compiled code executable at
+# run time; the hardened runtime kills that without
+# allow-unsigned-executable-memory, so every binary holds it.
 # Outside the App Sandbox the network and the login keychain need no
 # entitlement; the network keys below are declared so the app keeps working
@@ -148,4 +151,6 @@ default_entitlements() {
 <plist version="1.0">
 <dict>
+  <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
+  <true/>
   <key>com.apple.security.network.client</key>
   <true/>
```

## Checks (run them with the `run_check` tool)

- `check:openagents-cli`: `cargo check -p openagents-cli --tests --message-format short` (compile openagents-cli and its tests)
- `test:openagents-cli`: `cargo test -p openagents-cli [FILTER]` (run openagents-cli's tests (pass a test-name filter to run fewer))
- `fmt:openagents-cli`: `cargo fmt -p openagents-cli` (format openagents-cli)
- `check:coder`: `cargo check -p coder --tests --message-format short` (compile coder and its tests)
- `test:coder`: `cargo test -p coder [FILTER]` (run coder's tests (pass a test-name filter to run fewer))
- `fmt:coder`: `cargo fmt -p coder` (format coder)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
- - `crates/openagents-cli` — the `openagents` command: one `--json`-first program over the existing crates for pairing and computers (`coder-host`, `coder-computers`), durable tasks (`coder`), headless Verse presence and the Lagrange zone (`verse`, `verse-lagrange`), relays and keys, and fail-closed NIP-SOV profile handling. It adds no protocol logic of its own; put behavior in the owning crate and expose it here. Read `docs/cli/README.md` before adding a command group.
- - `crates/coder` — the agent: `classify` routes each turn through Jev, `generate` answers through an Open Responses door, and the `coder` binary draws the conversation in the terminal or, with `-p`, runs one turn from a script. Both modes run the same turn, `coder::turn::run`; keep it that way. `permit` is the host's answer to whether a turn runs commands at all, built from the route and the operator's setting before anything generates and narrowing from there; a reply becomes an executable plan only under a permit that runs one, so keep execution policy there rather than in what the model is told. `delegate_door` answers a turn through Microcoder's loop in process, on the first connected provider with capacity in the capacity book (the Codex login, then Claude Code's login, then, always last, Vertex through the OpenAgents cloud, which needs no token on the host; read `docs/coder/runtime
