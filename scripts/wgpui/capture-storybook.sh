#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

cargo run -p wgpui --example storybook_capture --features storybook -- "$@"
