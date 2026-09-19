#!/usr/bin/env bash
# Run the manual long-run long-run soak against a disposable local Postgres cluster.
set -euo pipefail
cd "$(dirname "$0")/.."

for command_name in initdb pg_ctl createdb cargo; do
  command -v "${command_name}" >/dev/null || {
    echo "test-soak: missing ${command_name}" >&2
    exit 1
  }
done

cluster_dir="$(mktemp -d /tmp/nostr-relay-soak.XXXXXX)"
socket_dir="${cluster_dir}/socket"
data_dir="${cluster_dir}/data"
mkdir -p "${socket_dir}"

cleanup() {
  if test -f "${data_dir}/postmaster.pid"; then
    pg_ctl -D "${data_dir}" -m immediate -w stop >/dev/null
  fi
  rm -rf "${cluster_dir}"
}
trap cleanup EXIT INT TERM

initdb -D "${data_dir}" -A trust --no-locale -E UTF8 >/dev/null
pg_ctl -D "${data_dir}" \
  -o "-c listen_addresses='' -c unix_socket_directories='${socket_dir}'" \
  -w start >/dev/null

database_user="$(id -un)"
createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_soak
soak_command=(cargo test --locked --release -p nostr-relay --test soak_postgres -- --ignored --nocapture)
if command -v caffeinate >/dev/null; then
  soak_command=(caffeinate -i "${soak_command[@]}")
fi
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_soak" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  "${soak_command[@]}"
