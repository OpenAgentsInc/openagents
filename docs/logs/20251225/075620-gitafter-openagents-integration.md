# GitAfter integration

- added `gitafter::run` / `run_with_route` to launch the desktop UI from the unified binary
- wired `openagents gitafter repos` and `openagents gitafter repo <id>` to open the GitAfter UI on the relevant route
- moved GitAfter runtime to `crates/gitafter/src/app.rs`
- added wry/tao dependencies to the gitafter crate
