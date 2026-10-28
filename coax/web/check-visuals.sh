#!/usr/bin/env bash
# INVARIANTS.md "Coaxial pages draw everything in Coax": refuses visible HTML,
# styled overlays and DOM-built UI in an app's pages, the host and unlock/.
#   check-visuals.sh APP_DIR
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app="$(cd "$1" && pwd)"
fail=0
bad() { echo "check-visuals: $1" >&2; fail=1; }
for page in "$app"/*.html; do
  # Visible elements: anything in <body> besides the canvas, the hidden text
  # (main with h1/p/a) and scripts.
  body="$(sed -n '/<body>/,/<\/body>/p' "$page")"
  extra="$(printf '%s\n' "$body" | grep -oE '<[a-z][a-z0-9]*' | sort -u | grep -vxE '<(body|canvas|main|h1|h2|p|a|script|ul|li|em|strong|code)' || true)"
  [[ -z "$extra" ]] || bad "$(basename "$page"): visible elements $(echo $extra)"
  # Links and text live only inside <main>, which the GPU page clips away.
  outside="$(printf '%s\n' "$body" | sed '/<main>/,/<\/main>/d' | grep -E '<a |<h1|<p>' || true)"
  [[ -z "$outside" ]] || bad "$(basename "$page"): text or links outside the hidden <main>"
done
# The host and the crypto glue build no UI: no styled or created elements
# except the keyboard field (unlock/) and the dev note (host.js, localhost).
for f in "$here/host.js" "$here"/unlock/*.ts; do
  [[ -e "$f" ]] || continue
  case "$f" in *.test.ts) continue ;; esac
  n="$(grep -cE 'innerHTML|insertAdjacentHTML|\.style\.|createElement\("(div|span|button|a|img|form|section|pre|p|h[1-6]|style|svg|canvas)"\)|className *=' "$f" || true)"
  allowed=0
  [[ "$(basename "$f")" == host.js ]] && allowed="$(grep -cE '// dev only' "$f" || true)"
  [[ "$n" -le "$allowed" ]] || bad "$(basename "$f"): builds or styles visible UI ($n place(s)); draw it in Coax"
done
exit $fail
