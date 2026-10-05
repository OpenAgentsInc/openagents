#!/bin/sh
# Create a scratch Rust repository for the physical shell/request/approval demo.
set -eu
fixture=$(mktemp -d "${TMPDIR:-/tmp}/openagents-terminal-demo.XXXXXX")
mkdir "$fixture/src"
cat > "$fixture/Cargo.toml" <<'EOF'
[package]
name = "terminal-demo-fixture"
version = "0.1.0"
edition = "2024"
[workspace]
EOF
cat > "$fixture/src/lib.rs" <<'EOF'
#[test]
fn addition() {
    assert_eq!(2 + 2, 5);
}
EOF
git -C "$fixture" init -q
git -C "$fixture" -c user.name='Terminal fixture' -c user.email='fixture@example.invalid' add Cargo.toml src/lib.rs
git -C "$fixture" -c user.name='Terminal fixture' -c user.email='fixture@example.invalid' commit -qm 'Add the failing terminal demo'
printf '%s\n' "$fixture"
