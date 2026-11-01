#!/usr/bin/env bash
# Firm reason:4. A bootstrap gate runs before the final Coax compiler exists.
# Replay complete supported mutation shards. This never asserts a pass itself:
# the actual compiler --merge must validate all records and return its verdict.
set -euo pipefail
export LC_ALL=C
fail(){ echo "coax shard replay: $*" >&2;exit 2; }
sha(){ if command -v shasum >/dev/null 2>&1;then shasum -a256 | awk '{print $1}';else sha256sum | awk '{print $1}';fi; }
hash(){ [[ "$1" =~ ^[0-9a-f]{64}$ ]]; }
[[ $# == 4 ]] || fail 'usage: bootstrap-mutation-shards.sh MANIFEST COMPILER CLOSURE ENTRY'
manifest="$1" compiler="$2" closure="$3" entry="$4"
[[ -f "$manifest" && ! -L "$manifest" ]] || fail 'missing or linked manifest'
[[ -x "$compiler" && -f "$compiler" && ! -L "$compiler" ]] || fail 'missing or linked compiler'
hash "$closure" || fail 'invalid current closure hash'
args=() records=() digests=()
{
 IFS= read -r schema || fail 'missing schema'
 [[ "$schema" == coax-bootstrap-shards-v1 ]] || fail 'unsupported schema'
 IFS=$'\t' read -r tag saved_compiler extra || fail 'missing compiler binding'
 [[ "$tag" == compiler && -z "$extra" ]] && hash "$saved_compiler" || fail 'invalid compiler binding'
 [[ "$(sha < "$compiler")" == "$saved_compiler" ]] || fail 'compiler binary differs from shard proof'
 IFS=$'\t' read -r tag saved_closure extra || fail 'missing closure binding'
 [[ "$tag" == closure && -z "$extra" && "$saved_closure" == "$closure" ]] || fail 'source closure differs from shard proof'
 IFS=$'\t' read -r tag saved_entry extra || fail 'missing entry binding'
 [[ "$tag" == entry && -z "$extra" && "$saved_entry" == "$entry" ]] || fail 'entry differs from shard proof'
 while IFS=$'\t' read -r tag digest path extra;do
  [[ "$tag" == record && -z "$extra" && -n "$path" ]] && hash "$digest" || fail 'invalid record row'
  [[ -f "$path" && ! -L "$path" ]] || fail 'missing or linked shard record'
  [[ "$(sha < "$path")" == "$digest" ]] || fail 'shard record hash differs'
  [[ ${#records[@]} -lt 65536 ]] || fail 'too many shard records'
  args+=(--merge "$path");records+=("$path");digests+=("$digest")
 done
} < "$manifest"
[[ ${#records[@]} -gt 0 ]] || fail 'no shard records; full mutation fallback is forbidden'
status=0
"$compiler" mutate "$entry" "${args[@]}" || status=$?
[[ "$(sha < "$compiler")" == "$saved_compiler" ]] || fail 'compiler changed during merge'
for ((i=0;i<${#records[@]};i++));do
 [[ "$(sha < "${records[i]}")" == "${digests[i]}" ]] || fail 'shard record changed during merge'
done
exit "$status"
