#!/bin/sh
set -eu
cargo test -p verse --lib render::
cargo test -p verse --lib gles_tests::
cargo test -p coder-mobile --lib bare_presence_tests::bare_world_players_see_each_other_move_and_pausing_stops_publishing
cargo test -p verse --example meteor_showcase_capture --features capture
cargo build -p verse --example meteor_showcase_capture --features capture
cargo build --release -p verse --example meteor_showcase_capture --features capture
