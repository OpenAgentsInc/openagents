#!/usr/bin/env bash
# Run the live store/gateway/multiprocess and binary deployment suites against disposable databases.
set -euo pipefail
cd "$(dirname "$0")/.."

for command_name in initdb pg_ctl createdb pg_dump pg_restore psql cargo curl python3 sed tar sha256sum awk; do
  command -v "${command_name}" >/dev/null || {
    echo "test-postgres: missing ${command_name}" >&2
    exit 1
  }
done

cluster_dir="$(mktemp -d /tmp/nostr-relay-postgres.XXXXXX)"
socket_dir="${cluster_dir}/socket"
data_dir="${cluster_dir}/data"
mkdir -p "${socket_dir}"
relay_binary="${CARGO_TARGET_DIR:-target}/debug/nostr-relay"
relay_pid=""
relay_two_pid=""

cleanup() {
  if test -n "${relay_pid}" && kill -0 "${relay_pid}" 2>/dev/null; then
    kill -TERM "${relay_pid}"
    wait "${relay_pid}" || true
  fi
  if test -n "${relay_two_pid}" && kill -0 "${relay_two_pid}" 2>/dev/null; then
    kill -TERM "${relay_two_pid}"
    wait "${relay_two_pid}" || true
  fi
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
createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test store_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_gateway_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_gateway_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test gateway_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_conformance_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_conformance_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test multiprocess_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_block_lane_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_block_lane_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test block_lane_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_import_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_import_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test bulk_import_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_backup_test
createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_restore_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_backup_test" \
  NOSTR_RELAY_TEST_RESTORE_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_restore_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked -p nostr-relay --test backup_postgres -- --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_load_test
NOSTR_RELAY_TEST_DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_load_test" \
  NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE=1 \
  cargo test --locked --release -p nostr-relay --test load_postgres -- --ignored --nocapture

createdb -h "${socket_dir}" -U "${database_user}" nostr_relay_deploy_test
cargo build --locked -p nostr-relay --bin nostr-relay
relay_log="${cluster_dir}/relay.log"
DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_deploy_test" \
  NOSTR_RELAY_PORT=0 \
  NOSTR_RELAY_URL=ws://relay.test \
  NOSTR_RELAY_SUPPORTED_NIPS=11,1,50 \
  "${relay_binary}" >"${relay_log}" 2>&1 &
relay_pid=$!

relay_port=""
for _ in $(seq 1 100); do
  relay_port="$(sed -n 's/.*"address":"127.0.0.1:\([0-9][0-9]*\)".*/\1/p' "${relay_log}" | tail -1)"
  if test -n "${relay_port}"; then
    break
  fi
  if ! kill -0 "${relay_pid}" 2>/dev/null; then
    sed -n '1,120p' "${relay_log}" >&2
    exit 1
  fi
  sleep 0.05
done
test -n "${relay_port}"
curl -fsS "http://127.0.0.1:${relay_port}/health" | grep -q '"status":"ok"'
curl -fsS -H 'Accept: application/nostr+json' \
  "http://127.0.0.1:${relay_port}/" | grep -q '"supported_nips":\[11,1,50\]'
NOSTR_RELAY_ACCEPTANCE_PORT="${relay_port}" python3 scripts/debian-acceptance-client.py

relay_two_log="${cluster_dir}/relay-two.log"
DATABASE_URL="host=${socket_dir} user=${database_user} dbname=nostr_relay_deploy_test" \
  NOSTR_RELAY_PORT=0 \
  "${relay_binary}" >"${relay_two_log}" 2>&1 &
relay_two_pid=$!
relay_two_port=""
for _ in $(seq 1 100); do
  relay_two_port="$(sed -n 's/.*"address":"127.0.0.1:\([0-9][0-9]*\)".*/\1/p' "${relay_two_log}" | tail -1)"
  if test -n "${relay_two_port}"; then
    break
  fi
  if ! kill -0 "${relay_two_pid}" 2>/dev/null; then
    sed -n '1,120p' "${relay_two_log}" >&2
    exit 1
  fi
  sleep 0.05
done
test -n "${relay_two_port}"
shadow_output="${cluster_dir}/relay-shadow.json"
python3 scripts/relay-readonly-shadow.py \
  --incumbent "ws://127.0.0.1:${relay_port}/" \
  --candidate "ws://127.0.0.1:${relay_two_port}/" \
  --workload tests/fixtures/migration/relay-shadow-v1.json \
  --output "${shadow_output}" >/dev/null
grep -q '"matched": true' "${shadow_output}"
grep -q '"event_count": 1' "${shadow_output}"
kill -TERM "${relay_two_pid}"
wait "${relay_two_pid}"
relay_two_pid=""
kill -TERM "${relay_pid}"
wait "${relay_pid}"
relay_pid=""
