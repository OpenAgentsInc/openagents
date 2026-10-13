#!/usr/bin/env bash
# Install a local Coder build into the owner's `coder` or `coderrc`, only
# after the harness acceptance gate passed on that exact build (#11549,
# docs/coder/harness-acceptance.md).
#
#   scripts/coder/install-local.sh --to coder|coderrc --from DIR
#                                  [--skip-acceptance REASON]
#
# --from DIR   a directory holding coder-new (or coder), openagents and
#              microcoder, such as $CARGO_TARGET_DIR/release
# --to coder   ~/.openagents/bin (coder, openagents, microcoder); the
#              replaced files are kept as <name>.before-local-install
# --to coderrc ~/.openagents/coderrc (coder-new, openagents, microcoder)
#
# A passing report for the build (~/work/harness-acceptance/<version>-<commit>/
# report.json, written by scripts/coder/harness-acceptance.sh) is used when it
# is there; otherwise the gate runs now. A failing gate refuses the install.
# --skip-acceptance REASON installs anyway and records REASON next to it.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
to="" from="" skip=""
while [ $# -gt 0 ]; do
  case "$1" in
    --to) to="${2:-}"; shift 2 ;;
    --from) from="${2:-}"; shift 2 ;;
    --skip-acceptance) skip="${2:-}"; shift 2 ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "install-local: unknown option $1" >&2; exit 2 ;;
  esac
done
die() { echo "install-local: $*" >&2; exit 2; }
case "$to" in coder) dest="$HOME/.openagents/bin"; product=coder ;; coderrc) dest="$HOME/.openagents/coderrc"; product=coder-new ;; *) die "--to coder or --to coderrc" ;; esac
[ -n "$from" ] && [ -d "$from" ] || die "--from DIR is required"
src_coder="$from/coder-new"; [ -x "$src_coder" ] || src_coder="$from/coder"
for f in "$src_coder" "$from/openagents" "$from/microcoder"; do [ -x "$f" ] || die "missing $f"; done

out="${CODER_ACCEPTANCE_OUT:-$HOME/work/harness-acceptance}"
if [ -n "$skip" ]; then
  [ "$(printf '%s' "$skip" | tr -d '[:space:]' | wc -c)" -ge 10 ] || die "--skip-acceptance needs a written reason"
  echo "install-local: skipping the harness acceptance gate: $skip" >&2
else
  build="$("$src_coder" --version 2>/dev/null | sed -n 's/^coder \([^ ]*\) (\([0-9a-f]\{10\}\)[0-9a-f]* \([a-z]*\)).*/\1-\2-\3/p' | sed 's/-clean$//')"
  report="$out/$build/report.json"
  if [ -n "$build" ] && [ -f "$report" ] && python3 -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1])).get("passed") and json.load(open(sys.argv[1])).get("coder") == sys.argv[2] else 1)' "$report" "$(cd "$(dirname "$src_coder")" && pwd -P)/$(basename "$src_coder")"; then
    echo "install-local: the gate passed on $build ($report)" >&2
  else
    echo "install-local: running the harness acceptance gate on $build" >&2
    "$here/harness-acceptance.sh" --coder "$src_coder" --out "$out" || die "the harness acceptance gate failed on $build; not installing (report under $out/$build)"
  fi
fi

mkdir -p "$dest"
install_one() {
  local src="$1" name="$2"
  if [ -e "$dest/$name" ] && [ "$to" = coder ]; then cp -p "$dest/$name" "$dest/$name.before-local-install"; fi
  cp "$src" "$dest/$name.new" && codesign -s - -f "$dest/$name.new" 2>/dev/null || true
  mv -f "$dest/$name.new" "$dest/$name"
}
install_one "$src_coder" "$product"
install_one "$from/openagents" openagents
install_one "$from/microcoder" microcoder
[ -z "$skip" ] || printf '%s %s\n' "$(date -u +%FT%TZ)" "$skip" >>"$dest/acceptance-skipped.log"
echo "install-local: installed $("$dest/$product" --version 2>/dev/null) into $dest" >&2
