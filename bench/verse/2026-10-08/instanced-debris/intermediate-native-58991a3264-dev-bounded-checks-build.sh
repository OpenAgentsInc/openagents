#!/bin/sh
set -e
cargo test -p verse-pbr --lib
cargo test -p verse-zone-everglade --lib
cargo test -p verse --lib render::
cargo test -p verse --lib gles_tests::
cargo test -p verse --lib meteor_showcase
cargo test -p verse --example meteor_showcase_capture --features capture
cargo build -p verse --example meteor_showcase_capture --features capture
