#!/usr/bin/env bash
# Firm reason: 5 (thin local Postgres 16 backup/restore drill verifying auth.workos_identities and workos.* mirror parity, #14102).
# Mirrors coax/web/api/backup/backup.coax's exact pg_dump -Fc -> pg_restore + exact row-count verdict
# on BOTH full-database dumps (oa-db-dump) and 6-hourly auth:workos dumps (oa-db-dump-auth).
# If `age` + `age-keygen` are installed, encrypts & decrypts the custom-format dump with age too.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
api_dir="$(cd "$here/.." && pwd)"
pgbin=/opt/homebrew/opt/postgresql@16/bin
[[ -x "$pgbin/initdb" ]] || pgbin=/usr/lib/postgresql/16/bin
[[ -x "$pgbin/initdb" ]] || pgbin="$(dirname "$(command -v initdb)")"

work="$(mktemp -d)"
cleanup() {
  "$pgbin/pg_ctl" -D "$work/src_db" -m immediate stop >/dev/null 2>&1 || true
  "$pgbin/pg_ctl" -D "$work/dst_full" -m immediate stop >/dev/null 2>&1 || true
  "$pgbin/pg_ctl" -D "$work/dst_auth" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

start_pg() {
  local dir="$1" port="$2"
  "$pgbin/initdb" -D "$dir" -U postgres -A trust --no-sync >/dev/null 2>&1
  "$pgbin/pg_ctl" -D "$dir" -l "$dir/pg.log" -o "-k $work -p $port -c listen_addresses=''" -w start >/dev/null
}

p_src="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
p_full="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
p_auth="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

start_pg "$work/src_db" "$p_src"
"$pgbin/psql" -h "$work" -p "$p_src" -U postgres -q \
  -c "CREATE ROLE oa_analytics LOGIN; CREATE ROLE oa_dump LOGIN;" \
  -c "CREATE DATABASE oa_analytics OWNER oa_analytics;"

src_dsn="host=$work port=$p_src user=oa_analytics dbname=oa_analytics"
dump_dsn="host=$work port=$p_src user=oa_dump dbname=oa_analytics"

echo "==> 1. Applying all migrations (through 0218_workos_auth.sql) and oa_dump grants"
"$pgbin/psql" "$src_dsn" -X -q -v ON_ERROR_STOP=1 -c "CREATE TABLE IF NOT EXISTS schema_migrations (name text PRIMARY KEY, applied timestamptz NOT NULL DEFAULT now())"
for m in "$api_dir"/migrations/*.sql; do
  "$pgbin/psql" "$src_dsn" -X -q -v ON_ERROR_STOP=1 -f "$m" >/dev/null
done
"$pgbin/psql" "$src_dsn" -X -q -v ON_ERROR_STOP=1 -c "
  GRANT USAGE ON SCHEMA auth, workos, public TO oa_dump;
  GRANT SELECT ON ALL TABLES IN SCHEMA auth, workos, public TO oa_dump;
"
sed '/oa_guard.attach/,/\\gexec/d' "$api_dir/db/grants.sql" \
  | "$pgbin/psql" "$src_dsn" -X -q -v ON_ERROR_STOP=1 -v runtime=oa_analytics -v readers=oa_dump -f - >/dev/null

echo "==> 2. Seeding auth.workos_identities + all workos.* mirror tables"
"$pgbin/psql" "$src_dsn" -X -q -v ON_ERROR_STOP=1 <<'SQL'
WITH a AS (
  INSERT INTO auth.accounts (id, kind, verse_account)
  VALUES (auth.new_account_id(), 'person', 'v_drill_1')
  RETURNING id
)
INSERT INTO auth.workos_identities (workos_user_id, account, provider, last_organization_id)
SELECT 'user_01DRILL_ALICE', id, 'GoogleOAuth', 'org_01DRILL_ACME' FROM a;

INSERT INTO workos.users (id, _raw)
VALUES ('user_01DRILL_ALICE', '{"id":"user_01DRILL_ALICE","email":"alice@drill.test","email_verified":true}'::jsonb);

INSERT INTO workos.organizations (id, _raw)
VALUES ('org_01DRILL_ACME', '{"id":"org_01DRILL_ACME","name":"Acme Drill Corp"}'::jsonb);

INSERT INTO workos.organization_memberships (id, _raw)
VALUES ('om_01DRILL_1', '{"id":"om_01DRILL_1","user_id":"user_01DRILL_ALICE","organization_id":"org_01DRILL_ACME","status":"active"}'::jsonb);

INSERT INTO workos.sessions (id, _raw)
VALUES ('sess_01DRILL_1', '{"id":"sess_01DRILL_1","user_id":"user_01DRILL_ALICE","organization_id":"org_01DRILL_ACME","status":"active"}'::jsonb);

SELECT workos.upsert_event('evt_01DRILL_1', 'user.updated', '{"id":"evt_01DRILL_1","event":"user.updated","data":{"id":"user_01DRILL_ALICE","email":"alice@drill.test","email_verified":true}}'::jsonb);

INSERT INTO workos.sync_cursor (stream, after_event_id, updated_at)
VALUES ('events', 'evt_01DRILL_1', now())
ON CONFLICT (stream) DO UPDATE SET after_event_id = EXCLUDED.after_event_id, updated_at = now();

INSERT INTO workos.pending_deletes (workos_user_id, account)
VALUES ('user_01DRILL_PURGE', 'acct_drill_purge_000000001');
SQL

counts_sql="SELECT format('%s.%s', n.nspname, c.relname) || E'\t' ||
  (xpath('/row/c/text()', query_to_xml(format('SELECT count(*) AS c FROM %I.%I', n.nspname, c.relname), false, true, '')))[1]::text
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_toast%'
ORDER BY 1;"

auth_counts_sql="SELECT format('%s.%s', n.nspname, c.relname) || E'\t' ||
  (xpath('/row/c/text()', query_to_xml(format('SELECT count(*) AS c FROM %I.%I', n.nspname, c.relname), false, true, '')))[1]::text
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND n.nspname IN ('auth', 'workos')
ORDER BY 1;"

echo "==> 3. Running full-DB dump + restore drill (as oa_dump)"
"$pgbin/psql" "$dump_dsn" -XAtq -c "$counts_sql" > "$work/full_src.tsv"
"$pgbin/pg_dump" "$dump_dsn" -Fc -Z 6 -f "$work/full.dump"
if command -v age >/dev/null 2>&1 && command -v age-keygen >/dev/null 2>&1; then
  age-keygen -o "$work/drill.key" >/dev/null 2>&1
  pub="$(grep '^# public key:' "$work/drill.key" | awk '{print $4}')"
  age -r "$pub" -o "$work/full.dump.age" "$work/full.dump"
  rm -f "$work/full.dump"
  age -d -i "$work/drill.key" -o "$work/full.dump" "$work/full.dump.age"
  echo "  ok: age-encrypted and decrypted full dump"
fi

start_pg "$work/dst_full" "$p_full"
"$pgbin/createdb" -h "$work" -p "$p_full" -U postgres restore
full_dst_dsn="host=$work port=$p_full user=postgres dbname=restore"
"$pgbin/pg_restore" -d "$full_dst_dsn" --no-owner --no-acl -j 2 "$work/full.dump"
"$pgbin/psql" "$full_dst_dsn" -XAtq -c "$counts_sql" > "$work/full_dst.tsv"
diff -u "$work/full_src.tsv" "$work/full_dst.tsv"
echo "  ok: full-DB restore drill exact row-count parity ($(wc -l < "$work/full_dst.tsv" | tr -d ' ') tables)"

echo "==> 4. Running 6-hourly auth:workos dump + restore drill (as oa_dump)"
"$pgbin/psql" "$dump_dsn" -XAtq -c "$auth_counts_sql" > "$work/auth_src.tsv"
"$pgbin/pg_dump" "$dump_dsn" -Fc -Z 6 --schema=auth --schema=workos -f "$work/auth_workos.dump"
start_pg "$work/dst_auth" "$p_auth"
"$pgbin/createdb" -h "$work" -p "$p_auth" -U postgres restore_auth
auth_dst_dsn="host=$work port=$p_auth user=postgres dbname=restore_auth"
"$pgbin/pg_restore" -d "$auth_dst_dsn" --no-owner --no-acl -j 2 "$work/auth_workos.dump"
"$pgbin/psql" "$auth_dst_dsn" -XAtq -c "$auth_counts_sql" > "$work/auth_dst.tsv"
diff -u "$work/auth_src.tsv" "$work/auth_dst.tsv"

# Verify every required WorkOS table & live view restored with exact row counts and generated columns intact!
python3 - "$work/auth_dst.tsv" <<'PY'
import sys
rows = dict(line.strip().split("\t") for line in open(sys.argv[1]) if "\t" in line)
required = [
    "auth.workos_identities",
    "auth.workos_pending",
    "workos.users",
    "workos.organizations",
    "workos.organization_memberships",
    "workos.sessions",
    "workos.events",
    "workos.sync_cursor",
    "workos.pending_deletes",
]
for t in required:
    assert t in rows, f"missing table in auth:workos dump: {t}"
    want = 0 if t == "auth.workos_pending" else 1
    assert int(rows[t]) == want, f"{t}: expected {want} rows, got {rows[t]}"
print(f"  ok: auth:workos restore drill verified all {len(required)} WorkOS tables with exact row counts")
PY

live_email="$("$pgbin/psql" "$auth_dst_dsn" -XAtq -c "SELECT email FROM workos.users_live WHERE id = 'user_01DRILL_ALICE'")"
[[ "$live_email" == "alice@drill.test" ]] || { echo "generated column / live view check failed: $live_email" >&2; exit 1; }
echo "  ok: workos.users_live generated column restored cleanly ($live_email)"
echo "==> workos_restore_drill_test.sh PASSED"
