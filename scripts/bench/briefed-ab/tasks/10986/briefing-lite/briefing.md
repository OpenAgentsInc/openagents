# Briefing: #10986 Coder landing: cargo fmt drift in files the change did not touch strands the change

## The issue

When the issue flow's gate (`crates/coder/src/task/issue_run.rs`, `Gate::check`) runs `cargo fmt -p PACKAGE -- --check`, it reports every unformatted file in the package, including files the change never touched. On #10940 the change was green on its first rebase, then `main` moved and brought `d138fad182`, whose `kit_bake.rs` was not formatted; the second recheck went red on that file alone, the landing stranded the change on `coder/stranded-1eac219c`, and the issue was released. The change itself was formatted.

Fix: keep only the `Diff in PATH:LINE:` hunks of files in the staged change (`git diff --cached --name-only`); when none remain, count the package as formatted and say in the evidence that drift in untouched files was left alone. Drift that `main` carries belongs to `main`, not to the change being landed.

Found while running #10940 through `openagents chat work --on boat`.

## Change plan

1. Goal: Coder landing: cargo fmt drift in files the change did not touch strands the change
2. Change the behavior where it lives, most likely in `crates/coder/src/task/issue_run.rs`, `crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs`, `NEEDS_OWNER.md`.
3. Add or update a test that pins the new behavior, beside the code's existing tests.
4. Run `check:coder`, then `test:coder`, then `check:verse-zone-everglade`, then `test:verse-zone-everglade`, then `fmt`; stop when they pass.

## Files to change

### `crates/coder/src/task/issue_run.rs`

Why: the issue names `crates/coder/src/task/issue_run.rs`; has `--cached`; has `--name-only`; has `--on`; has `issue_run`

```
--- lines 1798-1828 of 2589 ---
 1798                              "Coder asked a question instead of finishing: {}",
 1799                              clip(text.trim(), 600)
 1800                          ),
 1801                          None,
 1802                      );
 1803                  }
 1804                  Turn::Failed(why) => return self.failed(&why, None),
 1805              }
 1806              if self.stopping() {
 1807                  return self.stopped("Stopped by the person who started it.");
 1808              }
 1809              let _ = local::git_out(self.worktree, &["add", "-A"]);
 1810              let staged = local::git_out(self.worktree, &["diff", "--cached", "--name-only"])
 1811                  .unwrap_or_default();
 1812              if staged.trim().is_empty() {
 1813                  return self.unchanged();
 1814              }
 1815              self.note("Running the repository's checks on the change.");
 1816              self.checked = self.work.checks.check(self.worktree, &self.work.policy);
 1817              if self.stopping() {
 1818                  return self.stopped("Stopped by the person who started it.");
 1819              }
 1820              if self.checked.problems.is_empty() {
 1821                  self.note("The checks pass.");
 1822                  break;
 1823              }
 1824              let count = self.checked.problems.len();
 1825              let listed = clip(&self.checked.problems.join("; "), 600);
 1826              if self.rounds >= self.work.policy.fix_rounds {
 1827                  self.note(format!(
 1828                      "The checks still find {count} problem(s) after {} fix turn(s): {listed}",
--- lines 2232-2262 of 2589 ---
 2232              } else {
 2233                  providers.join(", ")
 2234              },
 2235              placement().run
 2236          )
 2237      }
 2238  
 2239      /// What the worktree changed from the branch it started on, as
 2240      /// `git diff --stat` shows it; empty when nothing changed.
 2241      fn diff_stat(&self) -> String {
 2242          let _ = local::git_out(self.worktree, &["add", "-A"]);
 2243          let base = self.started_on();
 2244          local::git_out(self.worktree, &["diff", "--cached", "--stat", &base])
 2245              .unwrap_or_default()
 2246              .trim_end()
 2247              .to_owned()
 2248      }
 2249  
 2250      /// The commit the run's change starts from: where `HEAD` meets the
 2251      /// branch, or the run's first base.
 2252      fn started_on(&self) -> String {
 2253          let upstream = format!("origin/{}", self.work.branch);
 2254          local::git_out(self.worktree, &["merge-base", "HEAD", &upstream])
 2255              .map(|base| base.trim().to_owned())
 2256              .ok()
 2257              .filter(|base| !base.is_empty())
 2258              .unwrap_or_else(|| self.record.base.clone())
 2259      }
 2260  
 2261      /// The comment's section linking this run's uploaded artifacts, with
 2262      /// the change diffed from `base` (where the run started when `None`);
--- lines 2542-2575 of 2589 ---
 2542  fn clip(text: &str, max: usize) -> String {
 2543      if text.len() <= max {
 2544          return text.to_owned();
 2545      }
 2546      let mut end = max;
 2547      while !text.is_char_boundary(end) {
 2548          end -= 1;
 2549      }
 2550      format!("{}…", &text[..end])
 2551  }
 2552  
 2553  #[cfg(test)]
 2554  #[path = "issue_run_tests.rs"]
 2555  mod tests;
 2556  
 2557  /// The variable `chat work --on boat|gce` sets on the machine it runs an
 2558  /// issue's flow on, naming that placement for the issue's comments.
 2559  pub const PLACEMENT_ENV: &str = "OPENAGENTS_CODER_PLACEMENT";
 2560  
 2561  /// Where the issue's comments say the run is.
 2562  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 2563  pub(crate) struct Placement {
 2564      /// For the claim, said by the machine itself.
 2565      pub claim: &'static str,
 2566      /// For the evidence's run line.
 2567      pub run: &'static str,
 2568  }
 2569  
 2570  fn placement() -> Placement {
 2571      placement_named(std::env::var(PLACEMENT_ENV).ok().as_deref())
 2572  }
 2573  
 2574  pub(crate) fn placement_named(name: Option<&str>) -> Placement {
 2575      match name {
```

### `crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs`

Why: the issue names `kit_bake.rs`

```
--- lines 1-118 of 118 ---
    1  //! The kit town's offline-baked light: the sky, sun, and lamp layers
    2  //! `verse-bake --layers` bakes from the town with the licensed kit
    3  //! installed ([`verse_pbr::pbr::baked_layers`]).
    4  //!
    5  //! The layers are derived from licensed geometry, so they ship the way the
    6  //! kit pack does: built outside the repository, kept in the private bucket
    7  //! and the zone cache, and pinned here by digest through the artifact queue
    8  //! (`openagents artifact submit everglade-kit-bake`). They sit beside the kit
    9  //! pack, not inside it, so the kit pack stays small for the web and
   10  //! phones, and a layout change that rebakes the light doesn't rebuild the
   11  //! kit.
   12  //!
   13  //! A loader offers the layers it finds ([`offer`]); the zone uses them only
   14  //! when their scene digest matches the town it builds. Without layers, or
   15  //! with layers for another scene, the town bakes its light at load as
   16  //! before (`textured_bake::BakeJob`).
   17  
   18  use std::path::Path;
   19  use std::sync::atomic::AtomicBool;
   20  use std::sync::{Arc, Mutex};
   21  
   22  use verse_pbr::pbr::baked_layers::Layers;
   23  
   24  use super::kit::KIT_ORIGIN;
   25  use super::pinned::PinnedFile;
   26  
   27  /// Exact content identity of the reviewed light layers, or empty while
   28  /// none is published.
   29  #[rustfmt::skip]
   30  pub const KIT_BAKE_SHA256: &str = "14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23";
   31  /// Transfer size of the reviewed light layers; zero while none is
   32  /// published.
   33  pub const KIT_BAKE_BYTES: u64 = 51682623;
   34  // Retain previous reviewed digests here when changing KIT_BAKE_SHA256.
   35  const KIT_BAKE_HISTORY: &[&str] = &[KIT_BAKE_SHA256];
   36  /// Environment variable naming a local layer file for offline tools, such
   37  /// as `everglade_capture`.
   38  pub const LOCAL_ENV: &str = "VERSE_KIT_BAKE";
   39  
   40  /// The reviewed light layers and their source,
   41  /// `<KIT_ORIGIN>/bake/<KIT_BAKE_SHA256>.vlay`.
   42  #[must_use]
   43  pub fn pinned() -> PinnedFile {
   44      PinnedFile {
   45          label: "Everglade kit light layers",
   46          sha256: KIT_BAKE_SHA256,
   47          bytes: KIT_BAKE_BYTES,
   48          url: format!("{KIT_ORIGIN}/bake/{KIT_BAKE_SHA256}.vlay"),
   49          extension: "vlay",
   50          temp_prefix: ".everglade-kit-bake-",
   51          history: KIT_BAKE_HISTORY,
   52      }
   53  }
   54  
   55  static OFFERED: Mutex<Option<Arc<Layers>>> = Mutex::new(None);
   56  
   57  /// Offers `layers` to the next town that bakes its light.
   58  pub fn offer(layers: Layers) {
   59      *OFFERED
   60          .lock()
   61          .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(layers));
   62  }
   63  
   64  /// The layers last offered, if any.
   65  #[must_use]
   66  pub fn offered() -> Option<Arc<Layers>> {
   67      OFFERED
   68          .lock()
   69          .unwrap_or_else(std::sync::PoisonError::into_inner)
   70          .clone()
   71  }
   72  
   73  /// Verifies the pinned digest, then decodes the layers.
   74  ///
   75  /// # Errors
   76  ///
   77  /// Returns a message when the bytes are not the pinned layers or fail to
   78  /// decode.
   79  pub fn decode_pinned(bytes: &[u8]) -> Result<Layers, String> {
   80      pinned().verify(bytes)?;
   81      Layers::decode(bytes)
   82  }
   83  
   84  /// The pinned layers in `cache`, downloaded from [`KIT_ORIGIN`] when
   85  /// `download` allows it.
   86  ///
   87  /// # Errors
   88  ///
   89  /// Returns a message when none are published, the cache holds none and
   90  /// downloading is off, or the transfer or decoding fails.
   91  pub fn fetch(cache: &Path, download: bool, cancel: &AtomicBool) -> Result<Layers, String> {
   92      let file = pinned();
   93      if file.bytes == 0 {
   94          return Err("No kit light layers are published".into());
   95      }
   96      if download {
   97          return file.fetch(cache, cancel, &mut |_, _| (), decode_pinned);
   98      }
   99      let bytes = file.read_bounded(&cache.join(file.cache_name()))?;
  100      decode_pinned(&bytes)
  101  }
  102  
  103  /// Layers from a local file, for offline tools: the pinned layers, or,
  104  /// while none are pinned or with the kit's unpinned switch set
  105  /// ([`super::kit::UNPINNED_ENV`]), any layer file that decodes. The zone
  106  /// still uses them only for the scene they were baked for.
  107  ///
  108  /// # Errors
  109  ///
  110  /// Returns a message when the file is unreadable, is not the pinned
  111  /// layers, or fails to decode.
  112  pub fn load_local(path: &Path) -> Result<Layers, String> {
  113      let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
  114      if KIT_BAKE_BYTES == 0 || std::env::var_os(super::kit::UNPINNED_ENV).is_some() {
  115          return Layers::decode(&bytes);
  116      }
  117      decode_pinned(&bytes)
  118  }
```

### `NEEDS_OWNER.md`

Why: has `kit_bake.rs`; has `kit_bake`

```
--- lines 54-84 of 2118 ---
   54  
   55  After the openagents.com deploy that serves the kit pack, open
   56  `https://openagents.com/everglade?frames` on the reference laptop and
   57  confirm about 60 frames per second on Stoop Lane and Main Street. On a
   58  phone, open Everglade and confirm the town draws the kit, not its grey
   59  proxies, and holds its frame rate. P3 now serves a 21,467,658-byte kit with
   60  60.5 MiB of decoded textures; B4 (#10908) still needs web and phone tiers.
   61  Repeat the physical-device measurements after that tier work.
   62  
   63  ## Upload the kit town's baked light layers (#10906)
   64  
   65  The kit town's light layers are pinned by `KIT_BAKE_SHA256` in
   66  `crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs` and live
   67  only in `~/.openagents/verse/private/medieval-town/bakes/` and the zone
   68  cache. Desktops that download the kit fetch them from
   69  `https://openagents.com/everglade/kit/bake/<KIT_BAKE_SHA256>.vlay`. Copy
   70  the pinned file to `gs://openagentsgemini-verse-private-assets/bakes/`
   71  and serve it at that path as the kit pack is served (the website image
   72  does not copy it yet).
   73  Until then, a desktop without the file in its cache bakes the town's light
   74  at load as before.
   75  
   76  ## Admit the villagers' fuller days (town clock)
   77  
   78  The town clock now runs by default. On it, a daylight town hour is 4.25
   79  real minutes, and the demo villagers stood still from 13:00 to 17:00: 17
   80  real minutes with nobody moving. Branch `town-clock-routines` gives Mira,
   81  Tobin, and Wren one or two short errands a half day, so some villager
   82  walks in every five real minutes from 05:00 to 21:00, and stages their
   83  proposals. Only you admit villagers. Check out the branch, run
   84  `openagents verse town admit mira-baker --owner`, then `tobin-smith` and
```

### `crates/openagents-web/cloudbuild.yaml`

Why: has `kit_bake.rs`; has `kit_bake`

```
--- lines 1-57 of 57 ---
    1  # Builds and pushes the openagents.com website image. From the repository root:
    2  #
    3  #   gcloud builds submit --project openagentsgemini \
    4  #     --config crates/openagents-web/cloudbuild.yaml \
    5  #     --ignore-file crates/openagents-web/web.gcloudignore \
    6  #     --substitutions _TAG=$(git rev-parse --short=10 HEAD) .
    7  #
    8  # See docs/deployment/openagents-web.md for the deploy and the rollback.
    9  steps:
   10    # The pinned medieval kit pack (KIT_SHA256 in
   11    # crates/verse-zone-everglade/src/zones/everglade_pack/kit.rs), from the
   12    # private bucket into the build context, so /everglade/kit/ serves it.
   13    # It never enters the repository. Without it the web build draws the
   14    # kit's committed proxies.
   15    - name: gcr.io/cloud-builders/gcloud
   16      entrypoint: bash
   17      args:
   18        - -c
   19        - |
   20          set -e
   21          sha="$$(sed -n 's/^pub const KIT_SHA256: &str = "\([0-9a-f]*\)";$$/\1/p' crates/verse-zone-everglade/src/zones/everglade_pack/kit.rs)"
   22          mkdir -p crates/openagents-web/kit
   23          if [ -n "$$sha" ]; then
   24            gcloud storage cp "gs://openagentsgemini-verse-private-assets/packs/$$sha.vtp" crates/openagents-web/kit/ \
   25              || echo "the kit pack $$sha is not in the bucket; the web build draws proxies"
   26          fi
   27          # Reviewed layers are optional until pinned. A published pin must
   28          # resolve to its exact private object before the image is built.
   29          bake_pin=crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs
   30          bake_sha="$$(sed -n 's/^pub const KIT_BAKE_SHA256: &str = "\([0-9a-f]*\)";$$/\1/p' "$$bake_pin")"
   31          bake_bytes="$$(sed -n 's/^pub const KIT_BAKE_BYTES: u64 = \([0-9]*\);$$/\1/p' "$$bake_pin")"
   32          mkdir -p crates/openagents-web/kit/bake
   33          if [ -z "$$bake_sha" ]; then
   34            test "$$bake_bytes" -eq 0
   35          else
   36            test "$$bake_bytes" -gt 0
   37            bake_file="crates/openagents-web/kit/bake/$$bake_sha.vlay"
   38            gcloud storage cp "gs://openagentsgemini-verse-private-assets/bakes/$$bake_sha.vlay" "$$bake_file"
   39            printf '%s  %s\n' "$$bake_sha" "$$bake_file" | sha256sum -c -
   40            test "$$(wc -c < "$$bake_file")" -eq "$$bake_bytes"
   41          fi
   42    - name: gcr.io/cloud-builders/docker
   43      args:
   44        - build
   45        - -f
   46        - crates/openagents-web/Dockerfile
   47        - -t
   48        - us-central1-docker.pkg.dev/openagentsgemini/openagents/openagents-web:${_TAG}
   49        - .
   50  images:
   51    - us-central1-docker.pkg.dev/openagentsgemini/openagents/openagents-web:${_TAG}
   52  substitutions:
   53    _TAG: latest
   54  options:
   55    machineType: E2_HIGHCPU_32
   56    logging: CLOUD_LOGGING_ONLY
   57  timeout: 3600s
```

### `scripts/release/coder.sh`

Why: has `git diff --cached --name-only`; has `--cached`; has `--name-only`

```
--- lines 336-366 of 786 ---
  336    git -C "$1" ls-files -ci --exclude-standard 2>/dev/null |
  337      while IFS= read -r _path; do
  338        if [ -e "$3/$_path" ] || [ -L "$3/$_path" ]; then
  339          (cd "$3" && GIT_DIR=$_gitdir GIT_WORK_TREE=$3 GIT_INDEX_FILE=$_index \
  340            git -c core.excludesFile=/dev/null add -f -- "$_path" 2>/dev/null || :)
  341        fi
  342      done
  343    _got=$(GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git write-tree)
  344    if [ "$_got" = "$_want" ]; then
  345      rm -f "$_index"
  346      return 0
  347    fi
  348    GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git diff --cached --name-only "$2" -- | head -20
  349    rm -f "$_index"
  350    return 1
  351  }
  352  
  353  # `scripts/test-release-coder.sh` sources the functions above and stops
  354  # here.
  355  if [ "${CODER_RELEASE_LIBRARY:-}" = 1 ]; then
  356    return 0
  357  fi
  358  
  359  while [ $# -gt 0 ]; do
  360    case "$1" in
  361      --version) version=${2:-}; shift 2 ;;
  362      --commit) commit_ref=${2:-}; shift 2 ;;
  363      --targets) targets=${2:-}; shift 2 ;;
  364      --channel) channel=${2:-}; shift 2 ;;
  365      --publish) publish=1; shift ;;
  366      --allow-partial) allow_partial=1; shift ;;
```

### `scripts/release/terminal.sh`

Why: has `git diff --cached --name-only`; has `--cached`; has `--name-only`

```
--- lines 373-403 of 793 ---
  373    git -C "$1" ls-files -ci --exclude-standard 2>/dev/null |
  374      while IFS= read -r _path; do
  375        if [ -e "$3/$_path" ] || [ -L "$3/$_path" ]; then
  376          (cd "$3" && GIT_DIR=$_gitdir GIT_WORK_TREE=$3 GIT_INDEX_FILE=$_index \
  377            git -c core.excludesFile=/dev/null add -f -- "$_path" 2>/dev/null || :)
  378        fi
  379      done
  380    _got=$(GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git write-tree)
  381    if [ "$_got" = "$_want" ]; then
  382      rm -f "$_index"
  383      return 0
  384    fi
  385    GIT_DIR=$_gitdir GIT_INDEX_FILE=$_index git diff --cached --name-only "$2" -- | head -20
  386    rm -f "$_index"
  387    return 1
  388  }
  389  
  390  # `scripts/test-release-terminal.sh` sources the functions above and stops
  391  # here.
  392  if [ "${OPENAGENTS_RELEASE_LIBRARY:-}" = 1 ]; then
  393    return 0
  394  fi
  395  
  396  while [ $# -gt 0 ]; do
  397    case "$1" in
  398      --version) version=${2:-}; shift 2 ;;
  399      --commit) commit_ref=${2:-}; shift 2 ;;
  400      --targets) targets=${2:-}; shift 2 ;;
  401      --channel) channel=${2:-}; shift 2 ;;
  402      --publish) publish=1; shift ;;
  403      --allow-partial) allow_partial=1; shift ;;
```

## Similar past changes

### 919c441032 A coding request in a checkout whose origin isn't on GitHub runs as a task

```diff
diff --git a/crates/coder/src/task/issue_run.rs b/crates/coder/src/task/issue_run.rs
index 4b8bac39d4..d485d702f1 100644
--- a/crates/coder/src/task/issue_run.rs
+++ b/crates/coder/src/task/issue_run.rs
@@ -809,4 +809,15 @@ fn on_thread(request: &str, earlier: &str, workdir: &Path, pick: bool) -> Option
 /// # Errors
 /// GitHub cannot be read, or no open issue is free.
+/// Whether `dir`'s checkout has an `origin` on GitHub, the only place
+/// its issues can live. A missing remote, a local path, or another forge
+/// is not (#10398).
+#[must_use]
+pub fn on_github(dir: &Path) -> bool {
+    local::git_out(dir, &["remote", "get-url", "origin"])
+        .ok()
+        .and_then(|url| super::publish::github_repository(url.trim()))
+        .is_some()
+}
+
 pub fn pick_here(
     tracker: &dyn Tracker,
```

### 3508a3f77a Keep the artifact pin on one parseable line

```diff
diff --git a/crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs b/crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs
index 143151c305..4757b7a648 100644
--- a/crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs
+++ b/crates/verse-zone-everglade/src/zones/everglade_pack/kit_bake.rs
@@ -27,6 +27,6 @@ use super::pinned::PinnedFile;
 /// Exact content identity of the reviewed light layers, or empty while
 /// none is published.
-pub const KIT_BAKE_SHA256: &str =
-    "14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23";
+#[rustfmt::skip]
+pub const KIT_BAKE_SHA256: &str = "14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23";
 /// Transfer size of the reviewed light layers; zero while none is
 /// published.
```

## Checks (run them with the `run_check` tool)

- `check:coder`: `cargo check -p coder --tests --message-format short` (compile coder and its tests)
- `test:coder`: `cargo test -p coder [FILTER]` (run coder's tests (pass a test-name filter to run fewer))
- `fmt:coder`: `cargo fmt -p coder` (format coder)
- `check:verse-zone-everglade`: `cargo check -p verse-zone-everglade --tests --message-format short` (compile verse-zone-everglade and its tests)
- `test:verse-zone-everglade`: `cargo test -p verse-zone-everglade [FILTER]` (run verse-zone-everglade's tests (pass a test-name filter to run fewer))
- `fmt:verse-zone-everglade`: `cargo fmt -p verse-zone-everglade` (format verse-zone-everglade)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
- - `crates/coder` — the agent: `classify` routes each turn through Jev, `generate` answers through an Open Responses door, and the `coder` binary draws the conversation in the terminal or, with `-p`, runs one turn from a script. Both modes run the same turn, `coder::turn::run`; keep it that way. `permit` is the host's answer to whether a turn runs commands at all, built from the route and the operator's setting before anything generates and narrowing from there; a reply becomes an executable plan only under a permit that runs one, so keep execution policy there rather than in what the model is told. `delegate_door` answers a turn through Microcoder's loop in process, on the first connected provider with capacity in the capacity book (the Codex login, then Claude Code's login, then, always last, Vertex through the OpenAgents cloud, which needs no token on the host; read `docs/coder/runtime
