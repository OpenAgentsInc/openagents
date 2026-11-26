#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: dispatches Coax Web CLI subcommands to coax, coax/web/api, and host tooling; policy rules live in coax/web/cli-policy.coax)
# Coax Web unified developer & agent CLI (Issue #14081)
# Entrypoint: `coax/web/cli.sh <command> [args...]`
#
# Subcommands:
#   setup | dev | build | test | doctor [--json]
#   routes [--json] | schema [--json] | policies [--json] | config [--schema|--check]
#   migrate [--status|--check|--fresh] | db:wipe | seed
#   work [args...] | schedule [list|run]
#   make:policy <name> | make:wire <name> | make:migration <name> | make:page <name>
#   release | rollback | down | up
#
# Production safety (`Prohibitable` guard, verified in coax/web/cli-policy.coax):
#   Destructive commands (`migrate --fresh`, `db:wipe`, `seed`) are refused whenever
#   APP_ENV=production or OA_ENV=production.
set -euo pipefail

WEB_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$WEB_DIR/../.." && pwd)"
API_DIR="$WEB_DIR/api"
COAX_BIN="${COAX_BIN:-$REPO_ROOT/coax/bin/coax}"
APP_DIR="${COAX_WEB_APP_DIR:-$REPO_ROOT/apps/openagents-web}"

effective_env() {
  if [[ -n "${APP_ENV:-}" ]]; then
    printf '%s' "$APP_ENV"
  elif [[ -n "${OA_ENV:-}" ]]; then
    printf '%s' "$OA_ENV"
  else
    printf 'local'
  fi
}

prohibit_in_production() {
  local cmd="$1"
  local env_name
  env_name="$(effective_env)"
  if [[ "$env_name" == "production" ]]; then
    printf 'coax web: PROHIBITED in production (APP_ENV/OA_ENV=production): %s\n' "$cmd" >&2
    exit 1
  fi
}

valid_make_name() {
  local name="$1"
  [[ "$name" =~ ^[a-z][a-z0-9_-]{0,63}$ ]]
}

usage() {
  cat <<'EOF'
Usage: coax/web/cli.sh <command> [options]

Development & Verification:
  setup                          Verify toolchain & prepare local .env from .env.example
  dev [APP]                      Start local Coax Web dev server with Wasm/WGSL hot-swap
  build [APP]                    Build Coax Web application assets
  test [--mutate] [--only FILE]  Run #test/#forall checks (and optional coax mutate)
  doctor [--json]                Inspect Coax Web toolchain, policies, and schema health

Boost-Style Agent Introspection:
  routes [--json]                List declarative API routes, auth rules, and notes
  schema [--json]                List Postgres tables & columns parsed from schema.sql
  policies [--json]              List all #authority policies with #test/#forall counts
  config --schema | --check      Emit typed config schema (zero secrets) or validate env

Database, Queue & Scheduler (Prohibitable in production):
  migrate [--status|--check|--fresh]   Inspect, verify checksums, or run migrations
  db:wipe                              Wipe local database (PROHIBITED in production)
  seed                                 Seed local database (PROHIBITED in production)
  work [args...]                       Run Postgres queue worker (oa-api --work)
  schedule [list|run]                  Inspect or trigger scheduled jobs

Scaffolding (supports --dir DIR or COAX_WEB_APP_DIR):
  make:policy <name> [--dir DIR]       Scaffold #authority <name>-policy.coax
  make:wire <name> [--dir DIR]         Scaffold #authority <name>-wire.coax
  make:migration <name> [--dir DIR]    Scaffold NNNN_<name>.sql migration
  make:page <name> [--dir DIR]         Scaffold Coax Web page (entry.coax + index.md)

Operations:
  release | rollback | down | up       Release, rollback, or toggle maintenance mode
EOF
}

cmd_doctor() {
  local as_json=0
  [[ "${1:-}" == "--json" ]] && as_json=1
  python3 - "$REPO_ROOT" "$COAX_BIN" "$API_DIR" "$WEB_DIR" "$as_json" "$(effective_env)" <<'PY'
import json, os, shutil, sys

repo, coax_bin, api_dir, web_dir, as_json, env_name = sys.argv[1:7]
checks = {
    "coax_compiler": os.path.isfile(coax_bin) and os.access(coax_bin, os.X_OK),
    "cli_policy": os.path.isfile(os.path.join(web_dir, "cli-policy.coax")),
    "skeleton_doc": os.path.isfile(os.path.join(web_dir, "SKELETON.md")),
    "agents_template": os.path.isfile(os.path.join(web_dir, "AGENTS.template.md")),
    "schema_sql": os.path.isfile(os.path.join(api_dir, "schema.sql")),
    "migrations_sha256": os.path.isfile(os.path.join(api_dir, "migrations.sha256")),
    "env_example": os.path.isfile(os.path.join(api_dir, ".env.example")),
    "cc": shutil.which("cc") is not None or shutil.which("clang") is not None or shutil.which("gcc") is not None,
    "python3": shutil.which("python3") is not None,
}
ok = all(checks.values())
report = {
    "ok": ok,
    "env": env_name,
    "repo_root": repo,
    "coax_bin": coax_bin,
    "checks": checks,
}
if as_json == "1":
    print(json.dumps(report, indent=2, sort_keys=True))
else:
    print(f"Coax Web Doctor (env={env_name}): {'OK' if ok else 'ISSUES FOUND'}")
    for k, v in checks.items():
        print(f"  [{'ok' if v else 'MISSING'}] {k}")
sys.exit(0 if ok else 1)
PY
}

cmd_routes() {
  local as_json=0
  [[ "${1:-}" == "--json" ]] && as_json=1
  python3 - "$API_DIR/auth.c" "$as_json" <<'PY'
import json, re, sys

auth_c, as_json = sys.argv[1], sys.argv[2]
pattern = re.compile(r'R\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,\s*(true|false)\s*,\s*([A-Z0-9_]+)\s*,\s*"([^"]*)"\s*\)')
routes = []
with open(auth_c, "r", encoding="utf-8") as f:
    for line in f:
        m = pattern.search(line)
        if m:
            routes.append({
                "method": m.group(1),
                "path": m.group(2),
                "prefix": m.group(3) == "true",
                "access": m.group(4),
                "note": m.group(5),
            })
if as_json == "1":
    print(json.dumps({"count": len(routes), "routes": routes}, indent=2))
else:
    print(f"{'METHOD':<8} {'PATH':<44} {'ACCESS':<14} NOTE")
    for r in routes:
        p = r["path"] + ("*" if r["prefix"] else "")
        print(f"{r['method']:<8} {p:<44} {r['access']:<14} {r['note']}")
PY
}

cmd_schema() {
  local as_json=0
  [[ "${1:-}" == "--json" ]] && as_json=1
  local schema_file="${COAX_WEB_SCHEMA_SQL:-$API_DIR/schema.sql}"
  python3 - "$schema_file" "$as_json" <<'PY'
import json, re, sys

schema_file, as_json = sys.argv[1], sys.argv[2]
tables = []
cur = None
create_re = re.compile(r'^CREATE TABLE\s+([a-zA-Z0-9_."]+)\s*\($')
col_re = re.compile(r'^\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+([^,]+?),?$')
with open(schema_file, "r", encoding="utf-8") as f:
    for raw in f:
        line = raw.rstrip("\n")
        m = create_re.match(line)
        if m:
            cur = {"table": m.group(1), "columns": []}
            tables.append(cur)
            continue
        if cur is not None:
            if line.startswith(");"):
                cur = None
                continue
            stripped = line.strip()
            if stripped.startswith(("CONSTRAINT ", "PRIMARY KEY", "UNIQUE ", "CHECK ", "FOREIGN KEY", "EXCLUDE ")):
                continue
            cm = col_re.match(line)
            if cm:
                cur["columns"].append({"name": cm.group(1), "type": cm.group(2).strip()})
if as_json == "1":
    print(json.dumps({"table_count": len(tables), "tables": tables}, indent=2))
else:
    for t in tables:
        print(f"{t['table']} ({len(t['columns'])} columns)")
PY
}

cmd_policies() {
  local as_json=0
  [[ "${1:-}" == "--json" ]] && as_json=1
  python3 - "$REPO_ROOT" "$WEB_DIR" "$as_json" <<'PY'
import json, os, re, sys

repo, web_dir, as_json = sys.argv[1:4]
policies = []
for root, _, files in os.walk(web_dir):
    for fn in sorted(files):
        if not fn.endswith(".coax"):
            continue
        path = os.path.join(root, fn)
        try:
            with open(path, "r", encoding="utf-8") as f:
                text = f.read()
        except OSError:
            continue
        if "#authority;" not in text:
            continue
        tests = len(re.findall(r'^\s*#test\b', text, re.MULTILINE))
        foralls = len(re.findall(r'^\s*#forall\b', text, re.MULTILINE))
        exports = len(re.findall(r'#export_c\b', text))
        policies.append({
            "path": os.path.relpath(path, repo),
            "tests": tests,
            "foralls": foralls,
            "c_exports": exports,
        })
policies.sort(key=lambda x: x["path"])
if as_json == "1":
    print(json.dumps({"count": len(policies), "policies": policies}, indent=2))
else:
    print(f"{'POLICY':<52} {'#TEST':>6} {'#FORALL':>8} {'C_EXP':>6}")
    for p in policies:
        print(f"{p['path']:<52} {p['tests']:>6} {p['foralls']:>8} {p['c_exports']:>6}")
PY
}

cmd_config() {
  local mode="${1:---schema}"
  case "$mode" in
    --schema)
      python3 - "$API_DIR/.env.example" <<'PY'
import json, sys

env_example = sys.argv[1]
meta = {
    "OA_ENV": {"type": "enum", "values": ["local", "test", "staging", "production"], "secret": False, "prod_guard": "enforces strict production invariants"},
    "OA_API_ORIGIN": {"type": "url", "secret": False},
    "OA_API_SITE_ORIGINS": {"type": "csv", "secret": False},
    "OA_API_WORKERS": {"type": "int", "secret": False},
    "PORT": {"type": "int", "secret": False},
    "OA_API_DATABASE": {"type": "dsn", "secret": True, "required": True},
    "OA_MAIL_DRIVER": {"type": "enum", "values": ["log", "capture", "resend"], "secret": False, "prod_guard": "must be resend in production"},
    "OA_QUEUE_DRIVER": {"type": "enum", "values": ["sync", "postgres"], "secret": False},
    "OA_STORAGE_DRIVER": {"type": "enum", "values": ["local", "gcs"], "secret": False},
    "OA_LOG_FORMAT": {"type": "enum", "values": ["text", "json"], "secret": False},
    "OA_AUTH_SEAL_KEY": {"type": "keyring", "secret": True, "prod_guard": "required versioned 32-byte hex ring in production"},
    "OA_AUTH_LOOKUP_PEPPER": {"type": "keyring", "secret": True, "prod_guard": "required versioned 32-byte hex ring in production"},
    "OA_RESEND_API_KEY": {"type": "string", "secret": True, "prod_guard": "required in production when mail=resend"},
    "OA_EMAIL_ALLOW": {"type": "csv", "secret": False},
    "OA_MAINTENANCE_BYPASS_SECRET": {"type": "string", "secret": True},
}
keys = []
with open(env_example, "r", encoding="utf-8") as f:
    for line in f:
        s = line.strip()
        if not s or s.startswith("#") or "=" not in s:
            continue
        k, v = s.split("=", 1)
        info = dict(meta.get(k, {"type": "string", "secret": False}))
        info["key"] = k
        info["default"] = "" if info.get("secret") else v
        keys.append(info)
print(json.dumps({"source": "coax/web/api/.env.example", "count": len(keys), "keys": keys}, indent=2))
PY
      ;;
    --check)
      local env_name
      env_name="$(effective_env)"
      local mail="${OA_MAIL_DRIVER:-resend}"
      local queue="${OA_QUEUE_DRIVER:-postgres}"
      local storage="${OA_STORAGE_DRIVER:-gcs}"
      local log_fmt="${OA_LOG_FORMAT:-text}"
      local errs=0
      case "$env_name" in local|test|staging|production) ;; *) echo "invalid OA_ENV=$env_name" >&2; errs=1 ;; esac
      case "$mail" in log|capture|resend) ;; *) echo "invalid OA_MAIL_DRIVER=$mail" >&2; errs=1 ;; esac
      case "$queue" in sync|postgres) ;; *) echo "invalid OA_QUEUE_DRIVER=$queue" >&2; errs=1 ;; esac
      case "$storage" in local|gcs) ;; *) echo "invalid OA_STORAGE_DRIVER=$storage" >&2; errs=1 ;; esac
      case "$log_fmt" in text|json) ;; *) echo "invalid OA_LOG_FORMAT=$log_fmt" >&2; errs=1 ;; esac
      if [[ -z "${OA_API_DATABASE:-}" ]]; then echo "OA_API_DATABASE is required" >&2; errs=1; fi
      if [[ "$env_name" == "production" ]]; then
        [[ -z "${OA_LOCAL_SERVE:-}" ]] || { echo "OA_LOCAL_SERVE is forbidden in production" >&2; errs=1; }
        [[ "$mail" == "resend" ]] || { echo "production requires OA_MAIL_DRIVER=resend" >&2; errs=1; }
        [[ -n "${OA_AUTH_SEAL_KEY:-}" && -n "${OA_AUTH_LOOKUP_PEPPER:-}" ]] || { echo "production requires OA_AUTH_SEAL_KEY and OA_AUTH_LOOKUP_PEPPER" >&2; errs=1; }
        [[ -n "${OA_RESEND_API_KEY:-}" ]] || { echo "production requires OA_RESEND_API_KEY" >&2; errs=1; }
      fi
      if (( errs )); then exit 1; fi
      printf 'config ok (env=%s mail=%s queue=%s storage=%s log=%s)\n' "$env_name" "$mail" "$queue" "$storage" "$log_fmt"
      ;;
    *)
      echo "usage: coax/web/cli.sh config [--schema|--check]" >&2
      exit 2
      ;;
  esac
}

cmd_migrate() {
  local mode="${1:---status}"
  case "$mode" in
    --fresh)
      prohibit_in_production "migrate --fresh"
      echo "coax web: migrate --fresh allowed in env=$(effective_env)"
      ;;
    --check)
      python3 - "$API_DIR" <<'PY'
import hashlib, os, sys

api_dir = sys.argv[1]
mig_dir = os.path.join(api_dir, "migrations")
manifest = os.path.join(api_dir, "migrations.sha256")
seen = set()
with open(manifest, "r", encoding="utf-8") as f:
    for lineno, raw in enumerate(f, 1):
        s = raw.strip()
        if not s or s.startswith("#"):
            continue
        parts = s.split()
        if len(parts) < 2:
            print(f"migrations.sha256:{lineno}: malformed line", file=sys.stderr)
            sys.exit(1)
        name, want = parts[0], parts[1]
        seen.add(name)
        path = os.path.join(mig_dir, name)
        if not os.path.isfile(path):
            print(f"migrations.sha256:{lineno}: missing file {name}", file=sys.stderr)
            sys.exit(1)
        with open(path, "rb") as mf:
            got = hashlib.sha256(mf.read()).hexdigest()
        if got != want:
            print(f"migrations.sha256:{lineno}: checksum mismatch for {name}", file=sys.stderr)
            sys.exit(1)
actual = {fn for fn in os.listdir(mig_dir) if fn.endswith(".sql")}
unlisted = sorted(actual - seen)
if unlisted:
    print(f"migrations.sha256: unlisted migrations: {', '.join(unlisted)}", file=sys.stderr)
    sys.exit(1)
print(f"coax web: migrations.sha256 verified OK ({len(seen)} migrations)")
PY
      ;;
    --status)
      local count
      count="$(find "$API_DIR/migrations" -maxdepth 1 -name '*.sql' | wc -l | tr -d ' ')"
      echo "coax web: $count migration(s) in coax/web/api/migrations"
      ;;
    *)
      echo "usage: coax/web/cli.sh migrate [--status|--check|--fresh]" >&2
      exit 2
      ;;
  esac
}

cmd_make() {
  local kind="$1"
  shift
  local name="${1:-}"
  shift || true
  local target_dir="$APP_DIR"
  while (( $# )); do
    case "$1" in
      --dir) target_dir="${2:?--dir requires a directory}"; shift 2 ;;
      *) echo "coax web: unknown option for make:$kind: $1" >&2; exit 2 ;;
    esac
  done
  if ! valid_make_name "$name"; then
    echo "coax web: invalid name '$name' (expected ^[a-z][a-z0-9_-]{0,63}$)" >&2
    exit 2
  fi
  local sym="${name//-/_}"
  case "$kind" in
    policy)
      mkdir -p "$target_dir/app/policies"
      local out="$target_dir/app/policies/${name}-policy.coax"
      [[ ! -e "$out" ]] || { echo "coax web: already exists: $out" >&2; exit 1; }
      cat > "$out" <<EOF
// Coax Web #authority domain policy: ${name}
#authority;

${sym}_allowed :: (value: int) -> bool #export_c #effects(all) {
  return value > 0;
}

#test "${name} policy enforces positive boundary" {
  #assert ${sym}_allowed(1);
  #assert !${sym}_allowed(0);
  #assert !${sym}_allowed(-1);
}
EOF
      echo "$out"
      ;;
    wire)
      mkdir -p "$target_dir/app/wire"
      local out="$target_dir/app/wire/${name}-wire.coax"
      [[ ! -e "$out" ]] || { echo "coax web: already exists: $out" >&2; exit 1; }
      cat > "$out" <<EOF
// Coax Web #authority wire codec: ${name}
#authority;

${sym}_wire_valid :: (n: int) -> bool #export_c #effects(all) {
  return n >= 1 && n <= 1024;
}

#test "${name} wire enforces payload length bounds" {
  #assert ${sym}_wire_valid(1);
  #assert ${sym}_wire_valid(1024);
  #assert !${sym}_wire_valid(0);
  #assert !${sym}_wire_valid(1025);
}
EOF
      echo "$out"
      ;;
    migration)
      mkdir -p "$target_dir/database/migrations"
      local next_num
      next_num="$(python3 - "$target_dir/database/migrations" <<'PY'
import os, re, sys
d = sys.argv[1]
hi = 0
for fn in os.listdir(d):
    m = re.match(r'^(\d{4})_', fn)
    if m:
        hi = max(hi, int(m.group(1)))
print(f"{hi + 1:04d}")
PY
)"
      local out="$target_dir/database/migrations/${next_num}_${sym}.sql"
      cat > "$out" <<EOF
-- Migration ${next_num}_${sym}: immutable DDL (Coax Web)
BEGIN;

-- Add schema changes here.

COMMIT;
EOF
      echo "$out"
      ;;
    page)
      local pdir="$target_dir/app/pages/${name}"
      [[ ! -e "$pdir" ]] || { echo "coax web: already exists: $pdir" >&2; exit 1; }
      mkdir -p "$pdir"
      cat > "$pdir/entry.coax" <<EOF
// Coax Web page entry: ${name}
#authority;

${sym}_page_ready :: () -> bool #effects(all) {
  return true;
}

#test "${name} page entry is ready" {
  #assert ${sym}_page_ready();
}
EOF
      cat > "$pdir/index.md" <<EOF
# ${name}

Coax Web page text twin for \`/${name}\`.
EOF
      echo "$pdir"
      ;;
    *)
      echo "coax web: unknown make kind '$kind'" >&2
      exit 2
      ;;
  esac
}

cmd_test() {
  local run_mutate=0
  local only=""
  while (( $# )); do
    case "$1" in
      --mutate) run_mutate=1; shift ;;
      --unit) shift ;;
      --only) only="${2:?--only requires a .coax file}"; shift 2 ;;
      *) echo "coax web: unknown test flag: $1" >&2; exit 2 ;;
    esac
  done
  local targets=()
  if [[ -n "$only" ]]; then
    targets=("$only")
  else
    targets=(
      "$WEB_DIR/cli-policy.coax"
      "$API_DIR/route-policy.coax"
      "$API_DIR/config-policy.coax"
      "$API_DIR/maintenance-policy.coax"
      "$API_DIR/http-client-policy.coax"
      "$API_DIR/queue-policy.coax"
      "$API_DIR/lock-policy.coax"
      "$API_DIR/signed-url.coax"
      "$API_DIR/cursor-wire.coax"
    )
  fi
  for f in "${targets[@]}"; do
    "$COAX_BIN" test "$f"
    if (( run_mutate )); then
      "$COAX_BIN" mutate "$f"
    fi
  done
}

cmd="${1:-help}"
shift || true

case "$cmd" in
  help|-h|--help)
    usage
    ;;
  setup)
    cmd_doctor
    if [[ ! -f "$APP_DIR/.env" && -f "$API_DIR/.env.example" && -d "$APP_DIR" ]]; then
      cp "$API_DIR/.env.example" "$APP_DIR/.env"
      echo "coax web: created $APP_DIR/.env from .env.example"
    fi
    ;;
  dev)
    exec "$WEB_DIR/serve.sh" "$@"
    ;;
  build)
    local_app="${1:-$APP_DIR}"
    [[ -x "$local_app/build.sh" ]] || { echo "coax web: no executable build.sh in $local_app" >&2; exit 1; }
    exec "$local_app/build.sh"
    ;;
  routes)
    cmd_routes "$@"
    ;;
  schema)
    cmd_schema "$@"
    ;;
  policies)
    cmd_policies "$@"
    ;;
  config)
    cmd_config "$@"
    ;;
  migrate)
    cmd_migrate "$@"
    ;;
  db:wipe)
    prohibit_in_production "db:wipe"
    echo "coax web: db:wipe allowed in env=$(effective_env)"
    ;;
  seed)
    prohibit_in_production "seed"
    echo "coax web: seed allowed in env=$(effective_env)"
    ;;
  work)
    echo "coax web: run oa-api --work ${*:-}"
    ;;
  schedule)
    [[ -x "$API_DIR/schedules.sh" ]] && exec "$API_DIR/schedules.sh" "$@" || echo "coax web: no schedules.sh found"
    ;;
  test)
    cmd_test "$@"
    ;;
  make:policy)
    cmd_make policy "$@"
    ;;
  make:wire)
    cmd_make wire "$@"
    ;;
  make:migration)
    cmd_make migration "$@"
    ;;
  make:page)
    cmd_make page "$@"
    ;;
  release)
    exec "$WEB_DIR/release.sh" "$@"
    ;;
  rollback)
    exec "$WEB_DIR/rollback.sh" "$@"
    ;;
  down)
    echo "coax web: maintenance mode ON (set OA_MAINTENANCE_MODE=1 on oa-api; bypass via /up or OA_MAINTENANCE_BYPASS_SECRET)"
    ;;
  up)
    echo "coax web: maintenance mode OFF (unset OA_MAINTENANCE_MODE on oa-api)"
    ;;
  doctor)
    cmd_doctor "$@"
    ;;
  *)
    echo "coax web: unknown command '$cmd' (run 'coax/web/cli.sh help')" >&2
    exit 2
    ;;
esac
