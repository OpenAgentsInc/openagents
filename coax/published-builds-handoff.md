# Handoff: publish the Coax compiler from the Mac (darwin-arm64)

For an agent with access to the owner's Mac. Written 2026-11-02.

## Why

Every machine without a cached compiler used to bootstrap it: about 90 minutes
on CoderOS's 28 cores (84 of them mutating `lib/compiler/checker.coax`), several
hours on a small container. The first published Linux build installed in 17 s. Published builds
(`coax/RELEASE.md#published-builds`, INVARIANTS.md "Compiler builds reuse only
exactly matching checked results") fix that. Each platform's release host
builds the compiler once per compiler change and uploads it; every other
machine's `coax/build.sh` downloads it in seconds.

- **Linux (`linux-x86_64`):** CoderOS publishes it. The first build was started
  on 2026-11-02 (`~/work/coax-publish.log` on CoderOS).
- **macOS (`darwin-arm64`):** nothing publishes it yet. **This handoff is that
  job.** Without it, every Mac worktree, agent and fresh checkout still
  bootstraps.

## The pieces (already on `main`, commit `2b96e6dd8a8`)

| File | What |
| --- | --- |
| `coax/provenance.sh source-fingerprint ROOT` | The name of a build: the compiler's inputs and the platform, without the local toolchain |
| `coax/published.sh` | `platform`, `name`, `pack`, `check`, `fetch`, `upload`, `publish` |
| `coax/publish-host.sh [--every SECONDS]` | A release host's loop: in its own clone (`~/.cache/coax-publish/openagents`), it builds `origin/main`'s compiler when no build is published for this platform, then uploads it. On macOS it builds with the system clang and `MACOSX_DEPLOYMENT_TARGET=13.0` |
| `coax/build.sh` | Installs the published build before it bootstraps (`COAX_NO_PUBLISHED=1` skips it) |
| `coax/tests/published-build.sh` | Checks that a bad build is refused |

Builds land at
`gs://openagentsgemini-cli-releases/coax/builds/<source>/<platform>.tar.gz`
(public read at `https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds/...`),
with `.sha256` beside it. A published name is never overwritten.

## The Mac, as found on 2026-11-02

- arm64, macOS 26.4, SDK 26.5, 18 cores.
- The checkout is `~/work/openagents` (it was 22 commits behind `origin/main`; don't
  use it for this, because `publish-host.sh` keeps its own clone).
- gcloud is at `~/google-cloud-sdk/bin/gcloud`, not on PATH in ssh sessions.
  Homebrew is at `/opt/homebrew/bin`.
- The automation service account's config is
  `~/work/.secrets/gcloud-sa-config` (CLAUDE.md, "Non-Interactive gcloud"). It
  has `storage.admin`, enough to upload. Never print the key.

## Steps

1. **Environment for every command below:**

   ```sh
   export PATH="$HOME/google-cloud-sdk/bin:/opt/homebrew/bin:$PATH"
   export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
   ```

   Check that `gcloud storage ls gs://openagentsgemini-cli-releases/coax/` works
   (it may say the prefix is empty or list `builds/`).

2. **Get the scripts.** In a fresh clone, so nobody's working tree is touched:

   ```sh
   git clone -q https://github.com/OpenAgentsInc/openagents ~/.cache/coax-publish/openagents
   ```

   (`publish-host.sh` reuses this clone and keeps it at `origin/main`.)

3. **Run the first publish in the foreground,** so you see it work. It
   bootstraps with every mutation gate (`COAX_MUTATION_FULL=1`). On CoderOS's 28
   cores that took about 90 minutes, 84 of them in `stage0 mutate
   lib/compiler/checker.coax`; on the Mac's 18 cores expect two hours or more.

   ```sh
   ~/.cache/coax-publish/openagents/coax/publish-host.sh 2>&1 | tee ~/work/coax-publish.log
   ```

   It ends with `coax published: uploaded <source>/darwin-arm64`, or
   `... is published` if a build for main's sources already exists.

   - **If `published.sh pack` refuses with "surviving mutants",** main has a
     survivor somewhere in the compiler. Don't publish around it. Report the
     survivor (in `coax/bin/mutation-survivors` in the publish clone) to the
     owner; the next compiler commit that fixes it will publish.
   - **If the bootstrap fails,** check whether `coax/build.sh` fails the same way
     in the owner's normal checkout. If it does, main is broken (not this job).

4. **Check that it installs elsewhere.** The machine's cache
   (`~/.cache/coax-release`) is shared by every worktree and now holds this
   build, so test a new worktree with an empty cache:

   ```sh
   git -C ~/work/openagents worktree add /tmp/coax-fetch-check origin/main
   COAX_RELEASE_CACHE=$(mktemp -d) /tmp/coax-fetch-check/coax/build.sh
   ```

   It must print `installed the published build <12 hex> (darwin-arm64, from <commit>)`
   within a minute, and `/tmp/coax-fetch-check/coax/bin/coax version` must run.
   Then `git -C ~/work/openagents worktree remove --force /tmp/coax-fetch-check`.

   Also run `~/.cache/coax-publish/openagents/coax/tests/published-build.sh`
   (it should print `published-build: ok ...`).

5. **Keep it running: a launchd agent.** The agent publishes after each compiler
   change on main, checking every 10 minutes. Write
   `~/Library/LaunchAgents/com.openagents.coax-publish.plist`:

   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0"><dict>
     <key>Label</key><string>com.openagents.coax-publish</string>
     <key>ProgramArguments</key><array>
       <string>/bin/bash</string>
       <string>-lc</string>
       <string>export PATH="$HOME/google-cloud-sdk/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"; export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"; exec "$HOME/.cache/coax-publish/openagents/coax/publish-host.sh" --every 600</string>
     </array>
     <key>RunAtLoad</key><true/>
     <key>KeepAlive</key><true/>
     <key>ProcessType</key><string>Background</string>
     <key>LowPriorityIO</key><true/>
     <key>Nice</key><integer>10</integer>
     <key>StandardOutPath</key><string>/Users/christopherdavid/work/coax-publish.log</string>
     <key>StandardErrorPath</key><string>/Users/christopherdavid/work/coax-publish.log</string>
   </dict></plist>
   ```

   Load it with `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.openagents.coax-publish.plist`
   and check `launchctl print gui/$(id -u)/com.openagents.coax-publish | head`.

   - `publish-host.sh` updates its own clone to `origin/main` each round,
     including its own scripts. A change to the publishing scripts is picked up
     on the next round, with no reload.
   - It is low priority (`Nice 10`). A build takes the Mac's cores for about two
     hours, but only when the compiler's sources change on main.
   - **Owner choice:** if a build during the working day is unwelcome, use
     `StartCalendarInterval` (for example hourly) instead of `--every 600` and
     `KeepAlive`. Note what you chose in the closing report.

6. **CoderOS, the same way (Linux).** If the owner wants it, install
   `publish-host.sh --every 600` on CoderOS as a systemd user service (the
   first run on 2026-11-02 was a one-off). Nix provides the musl toolchain
   inside the script. The unit:

   ```ini
   # ~/.config/systemd/user/coax-publish.service
   [Unit]
   Description=Publish the Coax compiler for linux-x86_64
   [Service]
   Environment=CLOUDSDK_CONFIG=%h/work/.secrets/gcloud-sa-config
   Environment=PATH=/run/current-system/sw/bin:%h/.nix-profile/bin
   ExecStart=%h/.cache/coax-publish/openagents/coax/publish-host.sh --every 600
   Restart=always
   Nice=10
   [Install]
   WantedBy=default.target
   ```

   Then `systemctl --user enable --now coax-publish` and
   `loginctl enable-linger christopherdavid`, so it runs without a login.

## Done when

- `gs://openagentsgemini-cli-releases/coax/builds/<main's source>/darwin-arm64.tar.gz`
  exists. Get main's source name with `coax/published.sh name <clone>` on the
  Mac; the name includes the platform.
- A fresh worktree with an empty cache installs it in under a minute (step 4).
- The launchd agent is loaded, and its log shows a second round saying
  `... is published`.
- `NEEDS_OWNER.md` in the workspace: nothing, unless the owner must choose the
  schedule (step 5). Then add one line there and push it (CLAUDE.md, Owner
  Actions).

## Don't

- Don't publish from a dirty tree, or with `COAX_NO_PUBLISHED`/`COAX_BUILD_FRESH`
  removed from `publish-host.sh` (a publisher must bootstrap, never re-publish a
  download).
- Don't delete or overwrite anything under `coax/builds/` in the bucket.
- Don't use GitHub Actions (INVARIANTS.md).
- Don't print `CLOUDSDK_CONFIG`'s key or tokens.
