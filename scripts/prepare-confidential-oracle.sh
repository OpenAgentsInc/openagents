#!/bin/sh
# Export only the already-qualified historical testing oracle, never the old app.
set -eu
umask 077
[ "$#" -eq 1 ] || { echo 'usage: prepare-confidential-oracle.sh NEW_CONTEXT_DIR' >&2; exit 2; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
out=$1
[ ! -e "$out" ] && [ ! -L "$out" ] || { echo 'oracle context must be a new directory' >&2; exit 2; }
source_commit=853417f290e7cae1df18c081b89b0818432e60af
builder='rust:1.97.1-alpine@sha256:3c38f3f82c2f3d73da3b38e18d279393a04cb43ddded0e35088a8c3324d40900'
git -C "$root" cat-file -e "$source_commit^{commit}" || { echo 'the optional oracle requires full historical objects' >&2; exit 1; }
command -v jq >/dev/null 2>&1 || { echo 'install jq for context metadata' >&2; exit 1; }
mkdir -p "$out/oracle"
git -C "$root" archive "$source_commit" crates/oa-att crates/nostr crates/nostr-transport \
  crates/receipts crates/agent-contract crates/psionic/vendor/openagents-workspace-hack \
  nips/openagents/schemas LICENSE > "$out/source.tar"
tar -xf "$out/source.tar" -C "$out/oracle"
cat > "$out/oracle/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = ["crates/oa-att", "crates/nostr", "crates/nostr-transport", "crates/receipts", "crates/agent-contract"]
[workspace.package]
version = "1.0.0"
publish = false
edition = "2024"
rust-version = "1.97.1"
[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
[patch.crates-io]
openagents-workspace-hack = { path = "crates/psionic/vendor/openagents-workspace-hack" }
EOF
cp "$root/docs/verification/data/oct30-confidential/oracle-Cargo.lock" "$out/oracle/Cargo.lock"
cp "$root/scripts/probe-confidential-pylon.sh" "$root/scripts/confidential-pylon-summary.jq" "$out/"
hash_file() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d ' ' -f 1; }
jq -n --arg source_commit "$source_commit" --arg source_archive_sha256 "$(hash_file "$out/source.tar")" \
  --arg cargo_lock_sha256 "$(hash_file "$out/oracle/Cargo.lock")" --arg builder "$builder" \
  '{tooling_oracle_only:true,source_commit:$source_commit,source_archive_sha256:$source_archive_sha256,cargo_lock_sha256:$cargo_lock_sha256,rust_toolchain:"1.97.1",target:"x86_64-unknown-linux-musl",builder:$builder}' > "$out/source.json"
chmod 755 "$out/probe-confidential-pylon.sh"
chmod 644 "$out/confidential-pylon-summary.jq" "$out/source.json" "$out/oracle/LICENSE"
cat > "$out/Dockerfile" <<'EOF'
# Optional verification component. No current product or model runtime is here.
FROM --platform=linux/amd64 rust:1.97.1-alpine@sha256:3c38f3f82c2f3d73da3b38e18d279393a04cb43ddded0e35088a8c3324d40900 AS build
RUN apk add --no-cache build-base
WORKDIR /oracle
COPY oracle/ ./
ENV CARGO_TARGET_DIR=/target
RUN cargo build --locked --release --target x86_64-unknown-linux-musl -p oa-att --features net --jobs 2 \
 && sha256sum /target/x86_64-unknown-linux-musl/release/oa-att | cut -d ' ' -f 1 > /target/oracle.sha256
FROM scratch
LABEL org.opencontainers.image.description="Optional historical NIP-ATT verification oracle; not a product runtime"
LABEL org.opencontainers.image.revision="853417f290e7cae1df18c081b89b0818432e60af"
COPY --from=build /target/x86_64-unknown-linux-musl/release/oa-att /opt/oa-confidential/oa-att
COPY --from=build /target/oracle.sha256 /opt/oa-confidential/oracle.sha256
COPY probe-confidential-pylon.sh confidential-pylon-summary.jq source.json /opt/oa-confidential/
COPY oracle/LICENSE /opt/oa-confidential/LICENSE
ENTRYPOINT ["/opt/oa-confidential/oa-att"]
EOF
cat > "$out/.dockerignore" <<'EOF'
source.tar
**/target
**/.git
EOF
printf 'Optional oracle build context: %s\n' "$out"
