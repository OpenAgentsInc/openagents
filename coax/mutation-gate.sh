#!/usr/bin/env bash
# Firm reason: 4. Part of the compiler bootstrap, which exists before Coax.
#
# Which surviving mutants stop a build. A mutation gate's survivors stop it
# only when the change since a base commit caused them: when they lie on a
# changed line, or when the change removed or edited a check (#assert, #test,
# #forall, #authority) in the survivor's file or in the file the gate mutates.
# Other survivors are reported, not gating. The full mutation check
# (COAX_MUTATION_FULL=1 coax/build.sh) gates on every survivor.
#
#   mutation-gate.sh changed ROOT BASE [REF]   lines changed from BASE to REF
#                                              (default: the working tree, with
#                                              untracked .coax files), as
#                                              coax-relative PATH:LINE, and
#                                              PATH:checks for a file that lost
#                                              or changed a check line
#   mutation-gate.sh survivors < REPORT        a failed `coax mutate` report's
#                                              survivors as PATH:LINE; exit 2
#                                              when it failed for another reason
#   mutation-gate.sh verdict CHANGED SURVIVORS exit 0 when no survivor gates,
#                                              else print those that do as
#                                              PATH:LINE and exit 1. A survivor
#                                              line is PATH:LINE, or
#                                              "LABEL: PATH:LINE" where LABEL
#                                              names the gate ("... mutate FILE
#                                              ..."). CHANGED "strict" gates on
#                                              every survivor.
set -euo pipefail
export LC_ALL=C
mode="${1:-}"
case "$mode" in
  changed)
    [[ $# == 3 || $# == 4 ]] || { echo 'usage: mutation-gate.sh changed ROOT BASE [REF]' >&2; exit 2; }
    root="$2" base="$3" ref="${4:-}"
    git -C "$root" rev-parse --verify -q "$base^{commit}" > /dev/null || { echo "mutation-gate: unknown base commit $base" >&2; exit 2; }
    # New-side lines of every hunk. A pure deletion marks the line after it,
    # so removing a check's line still counts as a change at that place. The
    # prefixes are explicit so a user's diff.noprefix or diff.mnemonicPrefix
    # cannot hide every path. A file whose removed lines include a check line
    # that it does not add back unchanged is marked PATH:checks: a mutant on
    # one of its unchanged lines may survive because that check went.
    {
      git -C "$root" -c core.quotePath=false diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ -U0 "$base" ${ref:+"$ref"} -- coax | awk '
        function flush(   l) { if (path != "") for (l in removed) if (!(l in added)) { print path ":checks"; break }; split("", removed); split("", added) }
        function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t\r]+$/, "", s); return s }
        /^diff --git / { flush(); path = ""; header = 1; next }
        header && /^\+\+\+ / { path = (substr($0, 5) == "/dev/null") ? "" : substr($0, 7); sub(/^coax\//, "", path); next }
        header && /^(--- |index |new file|deleted file|similarity|rename |old mode|new mode|copy )/ { next }
        /^@@ / && path != "" {
          header = 0
          split($3, new, ","); start = substr(new[1], 2) + 0; count = (2 in new) ? new[2] + 0 : 1
          if (count == 0) { print path ":" start; print path ":" (start + 1); next }
          for (i = 0; i < count; i++) print path ":" (start + i)
          next
        }
        !header && /^-/ { l = trim(substr($0, 2)); if (l ~ /(^|[{;][ \t]*)#(assert|test|forall|authority)([^A-Za-z0-9_]|$)/) removed[l] = 1; next }
        !header && /^\+/ { added[trim(substr($0, 2))] = 1; next }
        END { flush() }'
      # A file the working tree adds without committing is changed throughout.
      if [[ -z "$ref" ]]; then
        git -C "$root" -c core.quotePath=false ls-files --others --exclude-standard -- coax | while IFS= read -r file; do
          [[ "$file" == *.coax && -f "$root/$file" ]] || continue
          awk -v p="${file#coax/}" '{ print p ":" NR } END { if (NR == 0) print p ":1" }' "$root/$file"
        done
      fi
    } | sort -u
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
    [[ "$changed" == strict || -f "$changed" ]] || { echo "mutation-gate: no changed-lines file $changed" >&2; exit 2; }
    # Each survivor gates when its line changed, or its file or the gate's
    # mutated file lost a check. Strict: every survivor gates.
    gating="$(awk -v changed_file="$changed" '
      BEGIN { strict = changed_file == "strict"; if (!strict) while ((getline l < changed_file) > 0) changed[l] = 1 }
      {
        line = $0; root = ""
        if (match(line, /: [^ :]+:[0-9]+$/)) {
          label = substr(line, 1, RSTART - 1); line = substr(line, RSTART + 2)
          n = split(label, w, " "); for (i = 1; i < n; i++) if (w[i] == "mutate") { root = w[i + 1]; break }
        }
        path = line; sub(/:[0-9]+$/, "", path)
        if (strict || (line in changed) || ((path ":checks") in changed) || (root != "" && ((root ":checks") in changed))) print line
      }' "$survivors" | sort -u)"
    [[ -z "$gating" ]] || { printf '%s\n' "$gating"; exit 1; }
    ;;
  *) echo 'usage: mutation-gate.sh changed|survivors|verdict ...' >&2; exit 2 ;;
esac
