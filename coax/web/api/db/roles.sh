#!/usr/bin/env bash
# Splits the API's database roles (docs/auth/data-and-backups.md, #13194):
#
#   coax/web/api/db/roles.sh staging      # oa_analytics_staging: oa_api_staging
#   coax/web/api/db/roles.sh production   # oa_analytics: oa_api, oa_dump, oa_readonly
#
# Roles on the instance openagents-production-pg:
#   oa_analytics   owns every schema; runs migrations. Only deploy.sh's job
#                  oa-api-migrate[-staging] uses it (secret oa-analytics[-staging]-database-url).
#   oa_api[_staging]  the API: SELECT, INSERT, UPDATE, DELETE; no DDL, no TRUNCATE, owns nothing
#                  (secret oa-api[-staging]-database-url, read by the API's runtime account).
#   oa_dump        SELECT on everything, for the nightly dump (secret oa-db-dump-database-url).
#   oa_readonly    SELECT on everything, for agents and debugging (secret oa-analytics-readonly-database-url).
#   oa_admin       a cloudsqlsuperuser that makes the roles and the guard (secret oa-pg-admin-database-url).
# Plus the guard (guard.sql): DROP and TRUNCATE fail outside a migration.
#
# Idempotent. A role that exists keeps its password. Passwords are made here,
# go only into Secret Manager, and reach Postgres as SCRAM verifiers; nothing
# is printed. It ends by checking, from the catalog and inside rolled-back
# transactions, that the runtime role can't run DDL or TRUNCATE and that the
# guard refuses DROP and TRUNCATE. It never touches a table's rows.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
project=openagentsgemini region=us-central1 inst=openagents-production-pg
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
runtime_sa="157437760789-compute@developer.gserviceaccount.com"
case "${1:-}" in
  staging) db=oa_analytics_staging owner_secret=oa-analytics-staging-database-url
           roles="oa_api_staging:oa-api-staging-database-url" runtime=oa_api_staging readers="" ;;
  production) db=oa_analytics owner_secret=oa-analytics-database-url
           roles="oa_api:oa-api-database-url oa_dump:oa-db-dump-database-url oa_readonly:oa-analytics-readonly-database-url"
           runtime=oa_api readers="oa_dump,oa_readonly" ;;
  *) echo 'usage: roles.sh staging|production' >&2; exit 1 ;;
esac
work="$(mktemp -d)"; chmod 700 "$work"
port=$((20000 + RANDOM % 20000))
cloud-sql-proxy "$project:$region:$inst" --port "$port" --token "$(gcloud auth print-access-token)" > "$work/proxy.log" 2>&1 & proxy=$!
trap 'kill $proxy 2>/dev/null || true; rm -rf "$work"' EXIT
for _ in $(seq 50); do nc -z 127.0.0.1 "$port" 2>/dev/null && break; sleep 0.2; done

# as SECRET [DB] -- psql through the proxy as the user in that DSN secret
as() {
  local secret="$1" dbname="${2:-}"; shift 2
  gcloud secrets versions access latest --secret "$secret" --project "$project" < /dev/null > "$work/dsn"
  local env; env="$(python3 - "$work/dsn" <<'PY'
import shlex, sys
d = dict(kv.split('=', 1) for kv in open(sys.argv[1]).read().split())
print('PGUSER=%s PGPASSWORD=%s PGDATABASE=%s' % (shlex.quote(d['user']), shlex.quote(d['password']), shlex.quote(d.get('dbname', 'postgres'))))
PY
)"; rm -f "$work/dsn"
  eval "local $env"
  PGUSER="$PGUSER" PGPASSWORD="$PGPASSWORD" PGDATABASE="${dbname:-$PGDATABASE}" \
    psql -h 127.0.0.1 -p "$port" -X -q -v ON_ERROR_STOP=1 "$@"
}

for pair in $roles; do
  role="${pair%%:*}" secret="${pair#*:}"
  exists="$(as oa-pg-admin-database-url postgres -Atc "SELECT count(*) FROM pg_roles WHERE rolname = '$role'")"
  if [[ "$exists" == 0 ]]; then
    python3 - "$work" "$role" "$db" "$project:$region:$inst" <<'PY'
import base64, hashlib, hmac, os, secrets, sys
work, role, db, inst = sys.argv[1:]
pw = secrets.token_urlsafe(32)
salt, it = os.urandom(16), 4096
salted = hashlib.pbkdf2_hmac('sha256', pw.encode(), salt, it)
ck = hmac.new(salted, b'Client Key', hashlib.sha256).digest()
sk = hmac.new(salted, b'Server Key', hashlib.sha256).digest()
b = lambda x: base64.b64encode(x).decode()
os.umask(0o077)
open(work + '/verifier', 'w').write('SCRAM-SHA-256$%d:%s$%s:%s' % (it, b(salt), b(hashlib.sha256(ck).digest()), b(sk)))
open(work + '/newdsn', 'w').write('host=/cloudsql/%s user=%s dbname=%s password=%s' % (inst, role, db, pw))
PY
    as oa-pg-admin-database-url postgres -v role="$role" -v verifier="$(cat "$work/verifier")" \
      <<< "CREATE ROLE :\"role\" LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD :'verifier'"
    if gcloud secrets describe "$secret" --project "$project" >/dev/null 2>&1; then
      gcloud secrets versions add "$secret" --project "$project" --data-file "$work/newdsn" >/dev/null
    else
      gcloud secrets create "$secret" --project "$project" --replication-policy automatic --data-file "$work/newdsn" >/dev/null
    fi
    rm -f "$work/verifier" "$work/newdsn"
    echo "roles: made $role; its DSN is in $secret"
  else
    gcloud secrets describe "$secret" --project "$project" >/dev/null || { echo "roles: $role exists but $secret doesn't" >&2; exit 1; }
    echo "roles: $role already there"
  fi
done
# The API's runtime account reads its own DSN (and the dump's); oa_readonly's is for people and agents.
for pair in $roles; do
  role="${pair%%:*}" secret="${pair#*:}"
  [[ "$role" == oa_readonly ]] && continue
  gcloud secrets add-iam-policy-binding "$secret" --project "$project" --member "serviceAccount:$runtime_sa" \
    --role roles/secretmanager.secretAccessor --condition=None >/dev/null
done

# The guard's schema belongs to oa_admin, so the owner role can't rewrite it; the owner lets it in.
as "$owner_secret" "$db" <<< "GRANT CREATE ON DATABASE $db TO oa_admin"
as oa-pg-admin-database-url "$db" -f "$here/guard.sql"
as "$owner_secret" "$db" -v runtime="$runtime" -v readers="$readers" -f "$here/grants.sql" > /dev/null
echo "roles: guard and grants applied in $db"

# Checks: catalog only, and DDL tried inside transactions that roll back.
bad="$(as oa-pg-admin-database-url "$db" -v runtime="$runtime" -Atf - <<'SQL'
SELECT 'runtime may ' || p || ' on ' || c.oid::regclass
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace, unnest(ARRAY['TRUNCATE', 'TRIGGER', 'REFERENCES']) p
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT LIKE 'pg_%' AND n.nspname <> 'information_schema'
  AND has_table_privilege(:'runtime', c.oid, p)
UNION ALL
SELECT 'runtime may CREATE in schema ' || nspname FROM pg_namespace
WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema' AND has_schema_privilege(:'runtime', oid, 'CREATE')
UNION ALL
SELECT 'runtime can''t ' || p || ' ' || c.oid::regclass
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace, unnest(ARRAY['SELECT', 'INSERT', 'UPDATE', 'DELETE']) p
WHERE c.relkind IN ('r', 'p') AND c.relowner = 'oa_analytics'::regrole AND NOT has_table_privilege(:'runtime', c.oid, p)
UNION ALL
SELECT 'runtime owns ' || c.oid::regclass FROM pg_class c WHERE c.relowner = :'runtime'::regrole
UNION ALL
SELECT 'no TRUNCATE guard on ' || c.oid::regclass
FROM pg_class c WHERE c.relkind IN ('r', 'p') AND c.relowner = 'oa_analytics'::regrole
  AND NOT EXISTS (SELECT 1 FROM pg_trigger t WHERE t.tgrelid = c.oid AND t.tgname = 'oa_guard_no_truncate')
UNION ALL
SELECT 'event trigger missing: ' || e FROM unnest(ARRAY['oa_guard_refuse_drop', 'oa_guard_new_tables']) e
WHERE NOT EXISTS (SELECT 1 FROM pg_event_trigger WHERE evtname = e AND evtenabled <> 'D');
SQL
)"
[[ -z "$bad" ]] || { echo "$bad" >&2; exit 1; }
probe="$(as "$owner_secret" "$db" -At <<'SQL' 2>&1 || true
BEGIN;
CREATE TABLE public.oa_guard_probe (x int);
SAVEPOINT a;
DO $$ BEGIN TRUNCATE public.oa_guard_probe; RAISE NOTICE 'TRUNCATE allowed'; EXCEPTION WHEN insufficient_privilege THEN RAISE NOTICE 'truncate refused'; END $$;
DO $$ BEGIN DROP TABLE public.oa_guard_probe; RAISE NOTICE 'DROP allowed'; EXCEPTION WHEN insufficient_privilege THEN RAISE NOTICE 'drop refused'; END $$;
ROLLBACK;
SQL
)"
[[ "$probe" == *"truncate refused"* && "$probe" == *"drop refused"* ]] || { echo "roles: the guard let the owner through: $probe" >&2; exit 1; }
echo "roles: checked: $runtime has no DDL or TRUNCATE and owns nothing; the owner's DROP and TRUNCATE are refused outside a migration"
