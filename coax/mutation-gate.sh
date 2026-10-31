#!/usr/bin/env bash
# Firm reason: 4. Part of the compiler bootstrap, which exists before Coax.
#
# Which surviving mutants stop a build. A mutation gate's survivors stop it
# only when they lie on lines changed since a base commit; survivors on other
# lines are reported, not gating. The full mutation check (COAX_MUTATION_FULL=1
# coax/build.sh) gates on every survivor.
#
#   mutation-gate.sh changed ROOT BASE [REF]   lines changed from BASE to REF
#                                              (default: the working tree), as
#                                              coax-relative PATH:LINE
#   mutation-gate.sh survivors < REPORT        a failed `coax mutate` report's
#                                              survivors as PATH:LINE; exit 2
#                                              when it failed for another reason
#   mutation-gate.sh verdict CHANGED SURVIVORS exit 0 when no survivor is on a
#                                              changed line, else print those
#                                              that are and exit 1. CHANGED
#                                              "strict" gates on every survivor.
set -euo pipefail
export LC_ALL=C
mode="${1:-}"
case "$mode" in
  changed)
    [[ $# == 3 || $# == 4 ]] || { echo 'usage: mutation-gate.sh changed ROOT BASE [REF]' >&2; exit 2; }
    root="$2" base="$3" ref="${4:-}"
    git -C "$root" rev-parse --verify -q "$base^{commit}" > /dev/null || { echo "mutation-gate: unknown base commit $base" >&2; exit 2; }
    # New-side lines of every hunk. A pure deletion marks the line after it,
    # so removing a check's line still counts as a change at that place.
    git -C "$root" diff --no-color --no-ext-diff -U0 "$base" ${ref:+"$ref"} -- coax | awk '
      /^\+\+\+ / { path = ($2 == "/dev/null") ? "" : substr($2, 3); sub(/^coax\//, "", path); next }
      /^@@ / && path != "" {
        split($3, new, ","); start = substr(new[1], 2) + 0; count = (2 in new) ? new[2] + 0 : 1
        if (count == 0) { print path ":" start; print path ":" (start + 1); next }
        for (i = 0; i < count; i++) print path ":" (start + i)
      }' | sort -u
    ;;
  survivors)
    [[ $# == 1 ]] || { echo 'usage: mutation-gate.sh survivors < REPORT' >&2; exit 2; }
    awk '
      function clean(p,   n, i, out, parts, k, lead) {
        n = split(p, parts, "/"); k = 0
        for (i = 1; i <= n; i++) {
          if (parts[i] == "." || parts[i] == "") continue
          if (parts[i] == ".." && k > 0 && out[k] != "..") { k--; continue }
          out[++k] = parts[i]
        }
        lead = substr(p, 1, 1) == "/" ? "/" : ""
        p = ""; for (i = 1; i <= k; i++) p = p (i > 1 ? "/" : "") out[i]
        return lead p
      }
      /^[^ ].*:[0-9]+:[0-9]+: mutant survived: / {
        split($0, f, ":"); print clean(f[1]) ":" f[2]; survived++; next
      }
      /^not reached by any check: / { other = 1 }
      /^[0-9]+ mutants?, [0-9]+ killed, [0-9]+ survived/ { summary = 1 }
      /^coax: / { other = 1 }
      END { if (!summary || other || !survived) exit 2 }' | sort -u
    ;;
  verdict)
    [[ $# == 3 ]] || { echo 'usage: mutation-gate.sh verdict CHANGED SURVIVORS' >&2; exit 2; }
    changed="$2" survivors="$3"
    [[ -s "$survivors" ]] || exit 0
    if [[ "$changed" == strict ]]; then cat "$survivors"; exit 1; fi
    [[ -f "$changed" ]] || { echo "mutation-gate: no changed-lines file $changed" >&2; exit 2; }
    gating="$(comm -12 <(sort -u "$changed") <(sort -u "$survivors"))"
    [[ -z "$gating" ]] || { printf '%s\n' "$gating"; exit 1; }
    ;;
  *) echo 'usage: mutation-gate.sh changed|survivors|verdict ...' >&2; exit 2 ;;
esac
