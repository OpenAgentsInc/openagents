# Mac-only steps on a linked Mac

Some steps only run on macOS: Xcode and iOS builds, the iOS release gate UI
tests, TestFlight uploads, and desktop captures. A cloud environment, or an
agent in one, sends such a step to a Mac linked to the same account and gets
the result back without anyone touching the Mac (#11223).

## How it works

1. **The Mac serves.** On the Mac, sign in to Coder (`coder login`), then run
   `openagents mac serve` from a checkout of this repository. It reports what
   the Mac can do: its macOS and Xcode versions, its code-signing identities
   by name only, its simulators, its free space, and whether an App Store
   Connect key is present (never the key). It reports every few seconds and
   takes the jobs waiting for it, one at a time.
2. **The environment sends a job.** A job is typed: a repository, a ref, and
   one named recipe with arguments from that recipe's allowlist. There is no
   shell.

   ```sh
   openagents mac run ios-release-gate --ref main --out gate/
   openagents mac run ios-testflight --ref main -- --validate-only
   openagents mac run xcodebuild --ref my-branch -- test \
     -project bins/openagents-ios/host/OpenAgents.xcodeproj -scheme OpenAgents \
     -destination 'platform=iOS Simulator,name=iPhone 17 Pro'
   ```

   The same job goes to `POST /v1/mac-jobs` with
   `{repo, ref, recipe, args, computer?}` under the account's app token.
3. **The Mac runs it.** It fetches the ref into a ref of the job's own, adds
   a worktree at that commit, and runs the recipe there with its own build
   folder (`CARGO_TARGET_DIR`, derived data). A release gate gets a fresh
   simulator. Log lines go up as they come, with anything shaped like a
   credential redacted. When the job ends, the Mac uploads what it made
   (the `.ipa` or zipped `.app`, the test summary, screenshots, the whole
   log) and removes the folder, the worktree, the ref, and the simulator.
   A job starts only when the Mac has at least 40 GB free
   (`--min-free-gb`).
4. **The results come back.** `openagents mac run` follows the log and saves
   the files with `--out DIR`; `GET /v1/mac-jobs/{id}` and
   `GET /v1/mac-jobs/{id}/artifacts/{file}` serve them. The web shows the
   Macs and jobs at `/settings/mac-jobs`, and each job is an item on its
   Mac's board (`GET /v1/agents`), so the phone shows it with the computer's
   other work.

## Recipes

| Recipe | Kind | What runs | Arguments |
| --- | --- | --- | --- |
| `ios-release-gate` | test | `bins/openagents-ios/build.sh sim`, then `ReleaseGateUITests` on a fresh simulator, then the test summary | `--simulator NAME` |
| `ios-testflight` | upload | `scripts/release/testflight.sh run` | `--validate-only`, `--build N` |
| `desktop-capture` | capture | `cargo run -p openagents-desktop -- --capture DIR` | `--kept` |
| `xcodebuild` | build or test | `xcodebuild` with the job's arguments | The actions `build`, `test`, `build-for-testing`, `test-without-building`, `analyze`; `-project`, `-workspace` (inside the checkout), `-scheme`, `-configuration`, `-destination`, `-sdk`, `-testPlan`, `-only-testing:`, `-skip-testing:`, `-quiet`; and the settings `CODE_SIGN_IDENTITY=-`, `CODE_SIGNING_ALLOWED=NO`, `CODE_SIGNING_REQUIRED=NO`, `ONLY_ACTIVE_ARCH` |

`archive`, `-exportArchive`, provisioning updates, and authentication keys
are never `xcodebuild` job arguments; signing and shipping are only the
TestFlight recipe's. The allowlists live in `crates/mac-jobs`.

## Keys and approvals

Signing identities and the App Store Connect key stay on the Mac; the steps
find them there, and only whether a key is present is ever reported.

A recipe that sends something outside the Mac (`ios-testflight`, including
`--validate-only`) waits for the owner. The Mac asks with the exact subject:
the repository at its commit, the recipe, and its arguments. The owner
answers on the phone (the job's item on the Mac's board, **Approve** or
**Deny**) or on the web (`/settings/mac-jobs/{id}`). No API route approves,
so the caller that sent the job can't open its own gate. The Mac records the
answer in its approvals file (`~/.openagents/approvals.jsonl`, ability
`mac.upload`, #11170) and uses an approval once, for exactly that subject.
A checkout's `.openagents/approvals.json` can deny `mac.upload` but never
lets it run without asking.

## Code

- `crates/mac-jobs`: recipes, allowlists, steps, capabilities.
- `crates/openagents-web/src/mac_jobs.rs`: the jobs, the routes, and the
  board items; `mac_jobs_page.rs` is `/settings/mac-jobs`.
- `crates/coder-sync/src/mac_jobs.rs`: the Mac's calls.
- `crates/openagents-cli/src/mac.rs` and `mac_serve.rs`: `openagents mac`.
