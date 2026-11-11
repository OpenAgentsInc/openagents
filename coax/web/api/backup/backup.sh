#!/usr/bin/env bash
# Firm reason: 5, it wraps pg_dump, pg_restore, psql, age and curl inside the postgres:18 job
# image, where no Coax binary ships yet. Its count comparison is to move into Coax (follow-up to #13218).
# The API database's logical backups (docs/auth/data-and-backups.md, #13194).
#
#   backup.sh dump    pg_dump of the whole database, encrypted to recipients.txt,
#                     into the locked bucket, with its row counts
#   backup.sh drill   the newest dump, decrypted with the drill key, restored into
#                     a throwaway Postgres in this container, its row counts checked
#                     against the dump's and against production; the result goes
#                     into the bucket under drills/
#
# Environment: OA_DUMP_DATABASE (libpq DSN of the SELECT-only role oa_dump),
# OA_BACKUP_BUCKET, OA_BACKUP_PREFIX (default oa_analytics), and for the drill
# OA_DRILL_AGE_KEY (the drill key's private half). Runs as Cloud Run jobs
# oa-db-dump (nightly) and oa-db-drill (monthly): coax/web/api/backup/deploy.sh.
set -euo pipefail
mode="${1:-}"
bucket="${OA_BACKUP_BUCKET:?}" prefix="${OA_BACKUP_PREFIX:-oa_analytics}"
dsn="${OA_DUMP_DATABASE:?}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }

token() {
  curl -sf -H 'Metadata-Flavor: Google' \
    http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token |
    sed -E 's/.*"access_token":"([^"]+)".*/\1/'
}
put() {  # put LOCAL NAME: create only, never overwrite
  curl -sf -X POST --data-binary @"$1" -H "Authorization: Bearer $(token)" -H 'Content-Type: application/octet-stream' \
    "https://storage.googleapis.com/upload/storage/v1/b/$bucket/o?uploadType=media&ifGenerationMatch=0&name=$2" > /dev/null
}
get() {  # get NAME LOCAL
  local enc; enc="$(printf '%s' "$1" | sed 's|/|%2F|g')"
  curl -sf -H "Authorization: Bearer $(token)" -o "$2" "https://storage.googleapis.com/storage/v1/b/$bucket/o/$enc?alt=media"
}

# Every table's exact row count, as "schema.table<TAB>count", sorted.
counts_sql="SELECT format('%s.%s', n.nspname, c.relname) || E'\t' ||
  (xpath('/row/c/text()', query_to_xml(format('SELECT count(*) AS c FROM %I.%I', n.nspname, c.relname), false, true, '')))[1]::text
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_toast%'
ORDER BY 1;"

dump() {
  local stamp; stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  local t0; t0="$(date +%s)"
  # One snapshot for both the dump and its counts, held open by a psql session.
  coproc PG { psql "$dsn" -X -q -A -t -v ON_ERROR_STOP=1 2>&1; }
  echo "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SELECT pg_export_snapshot();" >&"${PG[1]}"
  local snap; read -r snap <&"${PG[0]}"
  [[ "$snap" =~ ^[0-9A-F-]+$ ]] || { echo "dump: no snapshot: $snap" >&2; exit 1; }
  printf '%s\nSELECT %s;\n' "$counts_sql" "'--end--'" >&"${PG[1]}"
  : > "$work/counts.tsv"
  local line; while read -r line <&"${PG[0]}"; do [[ "$line" == --end-- ]] && break; printf '%s\n' "$line" >> "$work/counts.tsv"; done
  pg_dump "$dsn" --snapshot="$snap" -Fc -Z 6 -f "$work/db.dump"
  local pid="$PG_PID"; echo "COMMIT;" >&"${PG[1]}"; eval "exec ${PG[1]}>&-"; wait "$pid" || true
  age -R /backup/recipients.txt -o "$work/db.dump.age" "$work/db.dump"
  rm -f "$work/db.dump"
  put "$work/db.dump.age" "$prefix/$stamp.dump.age"
  put "$work/counts.tsv" "$prefix/$stamp.counts.tsv"
  log "dump: $prefix/$stamp.dump.age, $(wc -c < "$work/db.dump.age") bytes, $(wc -l < "$work/counts.tsv") tables, $(( $(date +%s) - t0 ))s"
}

drill() {
  local t0; t0="$(date +%s)"
  local list newest
  curl -sf -H "Authorization: Bearer $(token)" \
    "https://storage.googleapis.com/storage/v1/b/$bucket/o?prefix=$prefix/&fields=items(name),nextPageToken&maxResults=1000" > "$work/list.json"
  newest="$(grep -oE "\"$prefix/[0-9TZ]+\.dump\.age\"" "$work/list.json" | tr -d '"' | sort | tail -1)"
  [[ -n "$newest" ]] || { echo "drill: no dump in gs://$bucket/$prefix/" >&2; exit 1; }
  local stamp="${newest#"$prefix/"}"; stamp="${stamp%.dump.age}"
  get "$newest" "$work/db.dump.age"
  get "$prefix/$stamp.counts.tsv" "$work/dumped.tsv"
  ( umask 077; printf '%s\n' "${OA_DRILL_AGE_KEY:?}" > "$work/key" )
  age -d -i "$work/key" -o "$work/db.dump" "$work/db.dump.age"; rm -f "$work/key" "$work/db.dump.age"
  # A throwaway Postgres in this container: the restore target, gone when the job ends.
  mkdir -p "$work/pg" "$work/sock"; chown -R postgres "$work/pg" "$work/sock"; chmod 755 "$work"
  su-exec postgres initdb -D "$work/pg" -U postgres -A trust > /dev/null
  su-exec postgres pg_ctl -D "$work/pg" -o "-k $work/sock -c listen_addresses=''" -l "$work/sock/pg.log" -w start > /dev/null
  local local_dsn="host=$work/sock user=postgres dbname=restore"
  su-exec postgres createdb -h "$work/sock" -U postgres restore
  chmod 644 "$work/db.dump"
  local r0; r0="$(date +%s)"
  local restore_status=0
  pg_restore -d "$local_dsn" --no-owner --no-acl -j 2 "$work/db.dump" 2> "$work/restore.err" || restore_status=$?
  local restore_s=$(( $(date +%s) - r0 ))
  psql "$local_dsn" -X -q -A -t -c "$counts_sql" > "$work/restored.tsv"
  psql "$dsn" -X -q -A -t -c "$counts_sql" > "$work/live.tsv"
  su-exec postgres pg_ctl -D "$work/pg" -m fast stop > /dev/null
  # The verdict: every dumped table restored with exactly its dumped row count.
  local mismatch tables rows drift
  mismatch="$(join -t $'\t' -a 1 -e MISSING -o 0,1.2,2.2 "$work/dumped.tsv" "$work/restored.tsv" | awk -F'\t' '$2 != $3' | head -20)"
  tables="$(wc -l < "$work/dumped.tsv")"; rows="$(awk -F'\t' '{s += $2} END {print s + 0}' "$work/restored.tsv")"
  drift="$(join -t $'\t' -o 0,1.2,2.2 "$work/restored.tsv" "$work/live.tsv" | awk -F'\t' '{d = $3 - $2; if (d < 0) d = -d; s += d} END {print s + 0}')"
  local now; now="$(date -u +%s)"
  local dumped_at; dumped_at="$(date -u -d "$(echo "$stamp" | sed -E 's/(....)(..)(..)T(..)(..)(..)Z/\1-\2-\3 \4:\5:\6/')" +%s)"
  local ok=true; [[ -z "$mismatch" ]] || ok=false
  local errors; errors="$(grep -c '^pg_restore: error' "$work/restore.err" || true)"
  cat > "$work/result.json" <<JSON
{"ok":$ok,"dump":"$newest","tables":$tables,"rows":$rows,"restore_exit":$restore_status,"restore_errors":$errors,"dump_age_s":$(( now - dumped_at )),"restore_s":$restore_s,"drill_s":$(( now - t0 )),"rows_changed_since_dump":$drift}
JSON
  put "$work/result.json" "drills/$(date -u +%Y%m%dT%H%M%SZ).json"
  cat "$work/result.json"
  [[ "$errors" == 0 ]] || { echo "drill: pg_restore errors:"; grep '^pg_restore: error' "$work/restore.err" | head -10; }
  $ok || { echo "drill: FAILED, tables that differ (table, dumped, restored):"; echo "$mismatch"; exit 1; }
}

case "$mode" in
  dump) dump ;;
  drill) drill ;;
  *) echo 'usage: backup.sh dump|drill' >&2; exit 2 ;;
esac
