#!/bin/sh
set -eu
cargo test -p verse-pbr -p verse-zone-everglade -p physics
cargo test -p verse --lib render::
cargo test -p verse --lib gles_tests::
cargo test -p verse --example meteor_showcase_capture --features capture
cargo build -p verse --example meteor_showcase_capture --features capture
cargo build --release -p verse --example meteor_showcase_capture --features capture
