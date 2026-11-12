#!/usr/bin/env bash
# Firm reason: 5. The allowlist counts of the green-main pipeline
# (docs/verse/green-main-pipeline.md, "The allowlist counts", #13266): git show
# and awk over five files at a commit, so after.sh reads them with no host and
# no build.
#
#   coax/remote/counts.sh show [REV]        NAME VALUE for each count at REV (default HEAD)
#   coax/remote/counts.sh check BASE [REV]  the counts a commit in BASE..REV raised and REV
#                                           still holds raised: one line each,
#                                           "count NAME CULPRIT BASELINE NOW"; status 1 when any
#
# The counts: parity-known and parity-gaps (everglade/features.coax EF_KNOWN and
# EF_GAPS), components-to-move (coax/web/components-check.coax CC_TO_MOVE),
# wire-allowlist (the lines of proto/wire-allowlist.txt), tools-port (the lines of
# coax/tools-port.txt), layer-RULE (aethyr/layer-allowlist.txt's lines per rule)
# and sql-rules-routes (coax/web/api/*.c calling plpgsql functions in schema.sql).
#
# The rule. Walking BASE..REV (first parent), the baseline of a count starts at
# its value at BASE and falls with it: the lowest value since BASE. REV is red
# for a count when its value is above that baseline, and the culprit is the
# first commit after the lowest point that went above it. A commit whose
# message has a trailer "Rebaseline: NAME: why" (the why at least ten
# characters) sets the baseline to its own value: a count that grows because a
# check covers more (tools-port: 755 to 1,243 on 2026-11-02, when tools-check
# began to list every untracked tool, 45bdff5358) says so in the commit, and
# the growth is not red. A count that falls is the new baseline.
set -uo pipefail
export LC_ALL=C
CN_FILES=(everglade/features.coax coax/web/components-check.coax proto/wire-allowlist.txt coax/tools-port.txt aethyr/layer-allowlist.txt coax/web/api/schema.sql ':(glob)coax/web/api/*.c')
CN_NAMES="parity-known parity-gaps components-to-move wire-allowlist tools-port layer-aethyr-up layer-everglade-up layer-tests sql-rules-routes"
cn_show() {   # rev: NAME VALUE lines (a count whose file is missing at REV is left out)
  local r="$1"
  git show "$r:everglade/features.coax" 2> /dev/null | sed -nE 's/^EF_KNOWN :: \[([0-9]+)\].*/parity-known \1/p; s/^EF_GAPS :: \[([0-9]+)\].*/parity-gaps \1/p'
  git show "$r:coax/web/components-check.coax" 2> /dev/null | sed -nE 's/^CC_TO_MOVE :: \[([0-9]+)\].*/components-to-move \1/p'
  local f n
  for f in wire-allowlist:proto/wire-allowlist.txt tools-port:coax/tools-port.txt; do
    git cat-file -e "$r:${f#*:}" 2> /dev/null || continue
    echo "${f%%:*} $(git show "$r:${f#*:}" | grep -cvE '^[[:space:]]*(#|$)')"
  done
  if git cat-file -e "$r:aethyr/layer-allowlist.txt" 2> /dev/null; then
    git show "$r:aethyr/layer-allowlist.txt" |
      awk '!/^[[:space:]]*(#|$)/ { n[$1]++ } END { split("aethyr-up everglade-up tests", k, " "); for (i = 1; i <= 3; i++) print "layer-" k[i], n[k[i]] + 0 }'
  fi
  if git cat-file -e "$r:coax/web/api/schema.sql" 2> /dev/null; then
    local fns
    fns="$(git show "$r:coax/web/api/schema.sql" | awk '
      /^CREATE (OR REPLACE )?FUNCTION / { fn = $0; sub(/^CREATE (OR REPLACE )?FUNCTION /, "", fn); sub(/\(.*/, "", fn) }
      /LANGUAGE / && fn != "" { if ($0 ~ /LANGUAGE plpgsql([;[:space:]]|$)/ && fn ~ /\./) print fn "("; fn = "" }
    ' | sort -u)"
    n=0
    [[ -z "$fns" ]] || n="$(git grep -Fl -f <(printf '%s\n' "$fns") "$r" -- ':(glob)coax/web/api/*.c' 2> /dev/null | wc -l | tr -d ' ')"
    echo "sql-rules-routes $n"
  fi
}
cn_rebaselined() {   # commit: the names its "Rebaseline: NAME: why" trailers name
  git log -1 --format='%(trailers:key=Rebaseline,valueonly,separator=%x0a)' "$1" 2> /dev/null |
    awk '{ i = index($0, ":"); if (i < 2) next; name = substr($0, 1, i - 1); why = substr($0, i + 1); gsub(/^[ \t]+|[ \t]+$/, "", name); gsub(/^[ \t]+/, "", why); if (length(why) >= 10) print name }'
}
cn_check() {   # base rev: "count NAME CULPRIT BASELINE NOW" lines; status 1 when any
  local base="$1" rev="${2:-HEAD}" c rows
  git rev-parse -q --verify "$base^{commit}" > /dev/null 2>&1 || { echo "counts: no commit $base" >&2; return 2; }
  # The commits that touch a counted file, oldest first; each one's counts and rebaselines.
  rows="$( { cn_show "$base" | sed 's/^/0 base /'
    local i=0
    while read -r c; do
      i=$((i + 1))
      cn_show "$c" | sed "s/^/$i $c /"
      cn_rebaselined "$c" | sed "s/^/$i $c rebaseline /"
    done < <(git rev-list --reverse --first-parent "$base..$rev" -- "${CN_FILES[@]}")
  } )"
  awk -v names="$CN_NAMES" '
    # rows: INDEX COMMIT NAME VALUE, or INDEX COMMIT rebaseline NAME
    $3 == "rebaseline" { rb[$1 SUBSEP $4] = 1; next }
    { v[$1 SUBSEP $3] = $4; at[$1] = $2; if ($1 > last) last = $1 }
    END {
      n = split(names, nm, " "); bad = 0
      for (k = 1; k <= n; k++) {
        name = nm[k]; have = 0
        for (i = 0; i <= last; i++) {
          if (!((i SUBSEP name) in v)) continue
          x = v[i SUBSEP name] + 0
          if (!have || x <= low || (i SUBSEP name) in rb) { low = x; lowat = i; have = 1 }
          now = x
        }
        if (!have || now <= low) continue
        for (i = lowat + 1; i <= last; i++) if ((i SUBSEP name) in v && v[i SUBSEP name] + 0 > low) break
        print "count", name, at[i], low, now; bad = 1
      }
      exit bad
    }' <<< "$rows"
}
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  case "${1:-}" in
    show) cn_show "${2:-HEAD}" ;;
    check) [[ -n "${2:-}" ]] || { echo "counts.sh check BASE [REV]" >&2; exit 2; }; cn_check "$2" "${3:-HEAD}" ;;
    *) sed -n '6,10p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
  esac
fi
